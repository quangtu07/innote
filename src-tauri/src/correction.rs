//! Re-recognize finalized audio; never feed or replace the existing transcript.
use crate::{config::AiProfile, groq, types::*};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

pub const SONIOX_ASYNC: &str = "stt-async-v6";
const RATE: usize = 16000;
const ADVANCE: usize = RATE * 600;
const OVERLAP: usize = RATE * 2;

#[derive(Clone, Serialize, Deserialize)]
pub struct Progress {
    pub id: String,
    pub stage: String,
    pub processed: f64,
    pub total: f64,
    pub model: String,
    pub cancelling: bool,
}
pub struct Control {
    pub cancel: AtomicBool,
    pub progress: Mutex<Progress>,
}
impl Control {
    pub fn new(id: String, total: f64, model: String) -> Self {
        Self {
            cancel: AtomicBool::new(false),
            progress: Mutex::new(Progress {
                id,
                stage: "Đang đọc bản thu".into(),
                processed: 0.,
                total,
                model,
                cancelling: false,
            }),
        }
    }
    pub fn update(&self, stage: &str, processed: f64, model: &str) {
        let mut p = self.progress.lock().unwrap();
        p.stage = stage.into();
        p.processed = processed;
        p.model = model.into();
        p.cancelling = self.cancel.load(Ordering::Acquire);
    }
}
pub fn supported(provider: &str, model: &str) -> bool {
    provider == "groq" && groq::is_speech_model(model)
        || provider == "soniox" && model == SONIOX_ASYNC
        || provider == "local"
            && (crate::models::moonshine_language(model).is_some()
                || model == crate::models::NEMO_ID)
}
#[cfg(windows)]
pub(crate) fn limit_local_cpu() -> Result<()> {
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetProcessAffinityMask, SetProcessAffinityMask,
    };
    unsafe {
        let process = GetCurrentProcess();
        let mut allowed = 0usize;
        let mut system = 0usize;
        GetProcessAffinityMask(process, &mut allowed, &mut system)?;
        let mut mask = 0usize;
        for bit in 0..usize::BITS {
            let candidate = 1usize << bit;
            if allowed & candidate != 0 {
                mask |= candidate;
                if mask.count_ones() >= 2 {
                    break;
                }
            }
        }
        ensure!(mask != 0, "Không xác định được CPU cho hiệu chỉnh local");
        SetProcessAffinityMask(process, mask)?;
    }
    Ok(())
}
fn check(cancel: &AtomicBool) -> Result<()> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Đã hủy hiệu chỉnh; bản gốc và bản hiệu chỉnh trước vẫn được giữ"
    );
    Ok(())
}
async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}
async fn cancellable<T>(
    cancel: &AtomicBool,
    future: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    tokio::select! {result=future=>result,_=cancelled(cancel)=>{check(cancel)?;unreachable!()}}
}
pub fn audio_info(path: &Path) -> Result<(usize, String)> {
    let reader = hound::WavReader::open(path).context("Không mở được file ghi âm để hiệu chỉnh")?;
    let spec = reader.spec();
    ensure!(
        spec.channels == 1
            && spec.sample_rate == 16000
            && spec.bits_per_sample == 16
            && spec.sample_format == hound::SampleFormat::Int,
        "Bản thu cần WAV PCM16 mono 16kHz"
    );
    let samples = reader.duration() as usize;
    ensure!(samples > 0, "File ghi âm đang trống");
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok((samples, hex::encode(hash.finalize())))
}
fn segments(words: Vec<groq::Word>) -> Vec<Segment> {
    let mut result = Vec::new();
    let mut group: Vec<groq::Word> = Vec::new();
    for word in words {
        let boundary = sentence_end(&word.text);
        group.push(word);
        if boundary || group.len() >= 20 {
            push_segment(&mut result, &mut group);
        }
    }
    push_segment(&mut result, &mut group);
    result
}
fn sentence_end(text: &str) -> bool {
    let text = text.trim_end();
    let last = text.split_whitespace().last().unwrap_or("");
    if [
        "mr.", "mrs.", "ms.", "dr.", "prof.", "st.", "vs.", "e.g.", "i.e.",
    ]
    .contains(&last.to_lowercase().as_str())
    {
        return false;
    }
    if last.len() == 2 && last.as_bytes()[0].is_ascii_uppercase() && last.ends_with('.') {
        return false;
    }
    text.ends_with(['.', '?', '!', '。', '？', '！'])
}
fn push_segment(out: &mut Vec<Segment>, words: &mut Vec<groq::Word>) {
    if words.is_empty() {
        return;
    }
    let text = words
        .iter()
        .map(|w| w.text.trim())
        .collect::<Vec<_>>()
        .join(" ");
    if !text.trim().is_empty() {
        out.push(Segment {
            id: uuid::Uuid::new_v4().to_string(),
            start: words[0].start,
            end: words.last().unwrap().end,
            text,
            edited: false,
        });
    }
    words.clear();
}

async fn groq_file(
    client: &reqwest::Client,
    profile: &AiProfile,
    model: &str,
    language: &str,
    path: &Path,
    total: usize,
    control: &Control,
) -> Result<Vec<Segment>> {
    let mut reader = hound::WavReader::open(path)?;
    let mut cursor = 0;
    let mut watermark = -0.001;
    let mut deferred = Vec::new();
    let mut words = Vec::new();
    while cursor < total {
        check(&control.cancel)?;
        let base = cursor.saturating_sub(OVERLAP);
        let end = (cursor + ADVANCE).min(total);
        reader.seek(base as u32)?;
        let samples = reader
            .samples::<i16>()
            .take(end - base)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            samples.len() == end - base,
            "Bản thu không đủ dữ liệu; chưa thay bản hiệu chỉnh đã có"
        );
        control.update(
            "Đang gửi bản thu lên Groq",
            cursor as f64 / RATE as f64,
            model,
        );
        let recognized = cancellable(
            &control.cancel,
            groq::recognize(
                client,
                &profile.key,
                &profile.base_url,
                model,
                Some(language),
                &samples,
            ),
        )
        .await?;
        let chunk = groq::AudioChunk {
            samples,
            base,
            final_: end == total,
        };
        words.extend(groq::confirmed_words(
            recognized,
            &chunk,
            &mut watermark,
            &mut deferred,
        ));
        cursor = end;
        control.update(
            "Đã xử lý một phần bản thu",
            cursor as f64 / RATE as f64,
            model,
        );
    }
    Ok(segments(words))
}
fn soniox_segments(value: &Value, duration: f64) -> Result<Vec<Segment>> {
    let tokens = value["tokens"]
        .as_array()
        .context("Soniox chưa trả token có mốc thời gian")?;
    let mut result = Vec::new();
    let mut current: Option<Segment> = None;
    for token in tokens {
        if token["translation_status"].as_str() == Some("translation") {
            continue;
        }
        let text = token["text"]
            .as_str()
            .context("Soniox thiếu chữ trong token")?;
        if matches!(text, "<end>" | "<fin>") {
            if let Some(s) = current.take() {
                result.push(s);
            }
            continue;
        }
        let start = token["start_ms"]
            .as_f64()
            .context("Soniox thiếu mốc bắt đầu")?
            / 1000.;
        let end = token["end_ms"]
            .as_f64()
            .context("Soniox thiếu mốc kết thúc")?
            / 1000.;
        ensure!(
            start.is_finite()
                && end.is_finite()
                && start >= 0.
                && end >= start
                && end <= duration + 1.,
            "Soniox trả mốc thời gian không hợp lệ"
        );
        if current
            .as_ref()
            .is_some_and(|s| start - s.end > 1.5 || s.text.split_whitespace().count() >= 20)
        {
            result.push(current.take().unwrap());
        }
        let s = current.get_or_insert_with(|| Segment {
            id: uuid::Uuid::new_v4().to_string(),
            start: start.min(duration),
            end: end.min(duration),
            text: String::new(),
            edited: false,
        });
        s.end = s.end.max(end.min(duration));
        s.text.push_str(text);
        if sentence_end(&s.text) {
            result.push(current.take().unwrap());
        }
    }
    if let Some(s) = current {
        result.push(s);
    }
    result.retain(|s| !s.text.trim().is_empty());
    Ok(result)
}
fn http_error(code: u16) -> anyhow::Error {
    anyhow::anyhow!(
        "Soniox {code}: {}",
        match code {
            401 => "API key không đúng hoặc hết hạn",
            403 => "key không có quyền dùng model async",
            402 | 429 => "đã hết số dư hoặc chạm hạn mức",
            400 | 404 => "model/file hoặc cấu hình không hợp lệ",
            _ => "dịch vụ chưa xử lý được yêu cầu",
        }
    )
}
async fn json_response(response: reqwest::Response) -> Result<Value> {
    ensure!(
        response.status().is_success(),
        "{}",
        http_error(response.status().as_u16())
    );
    response
        .json()
        .await
        .map_err(|_| anyhow::anyhow!("Soniox trả dữ liệu không hợp lệ"))
}
fn identifier(value: &Value) -> Result<String> {
    let id = value["id"]
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .context("Soniox trả mã công việc/file không hợp lệ")?;
    Ok(id.to_owned())
}
async fn request(
    client: &reqwest::Client,
    profile: &AiProfile,
    method: reqwest::Method,
    url: &str,
    body: Option<Value>,
) -> Result<Value> {
    let builder = client.request(method, url).bearer_auth(&profile.key);
    let builder = if let Some(body) = body {
        builder.json(&body)
    } else {
        builder
    };
    json_response(
        builder
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Soniox mất mạng hoặc phản hồi quá lâu"))?,
    )
    .await
}
async fn cleanup(
    client: reqwest::Client,
    profile: AiProfile,
    base: String,
    file: String,
    job: Option<String>,
) {
    // Soniox cannot delete a job while it is transcribing. Keep polling only
    // the job created by this operation, then remove it and its uploaded file.
    if let Some(job) = job {
        let mut removed = false;
        for _ in 0..180 {
            let result = client
                .delete(format!("{base}/transcriptions/{job}"))
                .bearer_auth(&profile.key)
                .send()
                .await;
            match result {
                Ok(r) if r.status().is_success() || r.status().as_u16() == 404 => {
                    removed = true;
                    break;
                }
                Ok(r) if r.status().as_u16() == 409 => {
                    tokio::time::sleep(Duration::from_secs(5)).await
                }
                _ => return,
            }
        }
        if !removed {
            return;
        }
    }
    let _ = client
        .delete(format!("{base}/files/{file}"))
        .bearer_auth(&profile.key)
        .send()
        .await;
}
async fn soniox_file(
    client: &reqwest::Client,
    profile: &AiProfile,
    language: &str,
    path: &Path,
    duration: f64,
    control: &Control,
    base: &str,
) -> Result<Vec<Segment>> {
    let mut file_id = None;
    let mut job_id = None;
    let operation = async {
        control.update("Đang tải bản thu lên Soniox", 0., SONIOX_ASYNC);
        let boundary = format!("innote-{}", uuid::Uuid::new_v4());
        let prefix=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"recording.wav\"\r\nContent-Type: audio/wav\r\n\r\n").into_bytes();
        let suffix = format!("\r\n--{boundary}--\r\n").into_bytes();
        let file = std::fs::File::open(path)?;
        let length = file.metadata()?.len() + prefix.len() as u64 + suffix.len() as u64;
        let stream = futures_util::stream::unfold(
            (file, Some(prefix), Some(suffix)),
            |(mut file, mut prefix, mut suffix)| async move {
                if let Some(bytes) = prefix.take() {
                    return Some((Ok::<Vec<u8>, std::io::Error>(bytes), (file, prefix, suffix)));
                }
                let mut bytes = vec![0; 65536];
                match file.read(&mut bytes) {
                    Ok(0) => suffix
                        .take()
                        .map(|bytes| (Ok(bytes), (file, prefix, suffix))),
                    Ok(n) => {
                        bytes.truncate(n);
                        Some((Ok(bytes), (file, prefix, suffix)))
                    }
                    Err(e) => Some((Err(e), (file, None, None))),
                }
            },
        );
        let response = client
            .post(format!("{base}/files"))
            .bearer_auth(&profile.key)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .header("Content-Length", length)
            .body(reqwest::Body::wrap_stream(stream))
            .send()
            .await
            .map_err(|_| {
                anyhow::anyhow!("Không tải được bản thu lên Soniox; kiểm tra mạng hoặc hạn mức")
            })?;
        let uploaded = json_response(response).await?;
        let file = identifier(&uploaded)?;
        file_id = Some(file.clone());
        let mut body = json!({"model":SONIOX_ASYNC,"file_id":file,"enable_language_identification":true,"enable_speaker_diarization":false});
        if ["vi", "en", "ko", "zh", "ja"].contains(&language) {
            body["language_hints"] = json!([language]);
        }
        let job = request(
            client,
            profile,
            reqwest::Method::POST,
            &format!("{base}/transcriptions"),
            Some(body),
        )
        .await?;
        let id = identifier(&job)?;
        job_id = Some(id.clone());
        control.update("Soniox đang nghe lại bản thu", 0., SONIOX_ASYNC);
        loop {
            let status = request(
                client,
                profile,
                reqwest::Method::GET,
                &format!("{base}/transcriptions/{id}"),
                None,
            )
            .await?;
            match status["status"].as_str() {
                Some("completed") => break,
                Some("error" | "failed") => {
                    return Err(match status["error_type"].as_str(){Some("insufficient_credits"|"budget_exceeded"|"spending_limit_exceeded")=>http_error(402),Some("rate_limit_exceeded"|"quota_exceeded")=>http_error(429),_=>anyhow::anyhow!("Soniox không hiệu chỉnh được file ghi âm; kiểm tra định dạng audio hoặc quyền dùng model")});
                }
                Some("queued" | "downloading" | "transcribing" | "processing") => {}
                _ => anyhow::bail!("Soniox trả trạng thái công việc không hợp lệ"),
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let result = request(
            client,
            profile,
            reqwest::Method::GET,
            &format!("{base}/transcriptions/{id}/transcript"),
            None,
        )
        .await?;
        soniox_segments(&result, duration)
    };
    let result = cancellable(&control.cancel, async {
        tokio::time::timeout(Duration::from_secs(1800), operation)
            .await
            .context("Hiệu chỉnh Soniox quá 30 phút; bản cũ vẫn được giữ")?
    })
    .await;
    if let Some(file) = file_id {
        tokio::spawn(cleanup(
            client.clone(),
            profile.clone(),
            base.into(),
            file,
            job_id,
        ));
    }
    result
}
pub async fn transcribe(
    profile: &AiProfile,
    model: &str,
    language: &str,
    path: &Path,
    control: &Control,
    local_model: Option<&Path>,
) -> Result<CorrectedTranscript> {
    ensure!(
        supported(&profile.provider, model),
        "Hiệu chỉnh cần model nhận âm thanh: local đã tải, Groq Whisper hoặc Soniox stt-async-v6"
    );
    ensure!(
        profile.provider == "local" || !profile.key.trim().is_empty(),
        "Cấu hình hiệu chỉnh chưa có API key"
    );
    check(&control.cancel)?;
    let (total, fingerprint) = audio_info(path)?;
    check(&control.cancel)?;
    let duration = total as f64 / RATE as f64;
    let segments = if profile.provider == "local" {
        let language = crate::local_caption::language(model, Some(language))?;
        let native = crate::local_stream_caption::transcribe_file(
            local_model.context("Thiếu model local đã tải")?,
            language,
            path,
            control,
            model,
        )
        .await?;
        let mut watermark = -0.001;
        let words = native
            .into_iter()
            .filter_map(|word| {
                if word.start.is_finite()
                    && word.end.is_finite()
                    && word.start >= 0.
                    && word.end >= word.start
                    && word.end <= duration + 1.
                    && word.end > watermark + 0.015
                {
                    watermark = word.end;
                    Some(groq::Word {
                        text: word.text,
                        start: word.start.min(duration),
                        end: word.end.min(duration),
                    })
                } else {
                    None
                }
            })
            .collect();
        segments(words)
    } else {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        if profile.provider == "groq" {
            groq_file(&client, profile, model, language, path, total, control).await?
        } else {
            soniox_file(
                &client,
                profile,
                language,
                path,
                duration,
                control,
                "https://api.soniox.com/v1",
            )
            .await?
        }
    };
    check(&control.cancel)?;
    ensure!(
        !segments.is_empty(),
        "Không nhận được lời nói từ bản thu; bản hiệu chỉnh trước vẫn được giữ"
    );
    let text = segments
        .iter()
        .map(|s| s.text.trim())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(CorrectedTranscript {
        text,
        segments,
        provider: profile.provider.clone(),
        profile_id: profile.id.clone(),
        model: model.into(),
        created: chrono::Utc::now().to_rfc3339(),
        audio_sha256: fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        sync::Arc,
    };
    fn wav(seconds: usize) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("innote-correction-{}.wav", uuid::Uuid::new_v4()));
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 16000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..seconds * RATE {
            writer.write_sample(100i16).unwrap();
        }
        writer.finalize().unwrap();
        path
    }
    fn profile(base: String) -> AiProfile {
        AiProfile {
            id: "saved-profile".into(),
            name: "Groq test".into(),
            provider: "groq".into(),
            base_url: base,
            key: "private-test-key".into(),
            created: String::new(),
            import_tag: String::new(),
        }
    }
    fn receive(socket: &mut std::net::TcpStream) -> Vec<u8> {
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut input = Vec::new();
        let mut buffer = [0; 65536];
        loop {
            let n = socket.read(&mut buffer).unwrap();
            assert!(n > 0);
            input.extend_from_slice(&buffer[..n]);
            if let Some(end) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&input[..end]).to_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|s| s.strip_prefix("content-length:"))
                    .map(|s| s.trim().parse().unwrap())
                    .unwrap_or(0);
                if input.len() >= end + 4 + length {
                    return input;
                }
            }
        }
    }
    fn respond(socket: &mut std::net::TcpStream, code: u16, body: &str) {
        write!(socket,"HTTP/1.1 {code} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
    }
    #[test]
    fn audio_models_fingerprint_and_timestamps_are_not_text_polishing() {
        assert!(!sentence_end("Ms."));
        assert!(!sentence_end("Dr."));
        assert!(sentence_end("meeting."));
        let name=soniox_segments(&json!({"tokens":[{"text":"Ms","start_ms":100,"end_ms":200},{"text":".","start_ms":200,"end_ms":220},{"text":" Nguyen approved.","start_ms":220,"end_ms":900}]}),1.).unwrap();
        assert_eq!(name.len(), 1);
        assert_eq!(name[0].text, "Ms. Nguyen approved.");
        assert!(supported("groq", "whisper-large-v3"));
        assert!(supported("soniox", SONIOX_ASYNC));
        assert!(supported("local", "moonshine-en"));
        assert!(supported("local", "moonshine-vi"));
        assert!(supported("local", crate::models::NEMO_ID));
        assert!(!supported("local", "phowhisper-small"));
        assert!(!supported("groq", "openai/gpt-oss-20b"));
        assert!(!supported("soniox", crate::soniox::RT_MODEL));
        let path = wav(1);
        let (samples, hash) = audio_info(&path).unwrap();
        assert_eq!(samples, RATE);
        assert_eq!(hash.len(), 64);
        assert_eq!(audio_info(&path).unwrap().1, hash);
        std::fs::remove_file(path).unwrap();
        let data = json!({"tokens":[{"text":"Xin","start_ms":100,"end_ms":300},{"text":" chào.","start_ms":300,"end_ms":600},{"text":"Hello","translation_status":"translation"},{"text":"<end>"}]});
        let parsed = soniox_segments(&data, 1.).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text, "Xin chào.");
        assert_eq!((parsed[0].start, parsed[0].end), (0.1, 0.6));
        assert!(soniox_segments(
            &json!({"tokens":[{"text":"bad","start_ms":900,"end_ms":100}]}),
            1.
        )
        .is_err());
    }
    #[tokio::test]
    async fn groq_file_chunks_stay_under_free_limit_and_keep_overlapping_words_once() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for turn in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                let input = receive(&mut socket);
                assert!(input.len() < 25_000_000);
                assert!(input.windows(4).any(|s| s == b"RIFF"));
                let text = String::from_utf8_lossy(&input);
                assert!(text.contains("whisper-large-v3"));
                assert!(!text.contains("old transcript"));
                respond(
                    &mut socket,
                    200,
                    if turn == 0 {
                        r#"{"words":[{"word":"hello","start":0.2,"end":0.5},{"word":"boundary","start":599.5,"end":599.8}]}"#
                    } else {
                        r#"{"words":[{"word":"boundary","start":1.5,"end":1.8},{"word":"tail.","start":2.4,"end":2.8}]}"#
                    },
                );
            }
        });
        let path = wav(602);
        let control = Control::new("meeting".into(), 602., "whisper-large-v3".into());
        let result = transcribe(
            &profile(base),
            "whisper-large-v3",
            "en",
            &path,
            &control,
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.text, "hello boundary tail.");
        assert_eq!(result.segments[0].start, 0.2);
        assert!((result.segments[0].end - 600.8).abs() < 0.001);
        assert_eq!(result.model, "whisper-large-v3");
        assert_eq!(result.profile_id, "saved-profile");
        assert_eq!(control.progress.lock().unwrap().processed, 602.);
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn cancellation_interrupts_a_waiting_http_request_without_returning_a_partial_result() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let _ = receive(&mut socket);
            std::thread::sleep(Duration::from_millis(800));
        });
        let path = wav(1);
        let control = Arc::new(Control::new(
            "meeting".into(),
            1.,
            "whisper-large-v3".into(),
        ));
        let stop = control.clone();
        let trigger = async move {
            tokio::time::sleep(Duration::from_millis(120)).await;
            stop.cancel.store(true, Ordering::Release);
        };
        let started = std::time::Instant::now();
        let p = profile(base);
        let result = tokio::join!(
            transcribe(&p, "whisper-large-v3", "en", &path, &control, None),
            trigger
        )
        .0
        .unwrap_err();
        assert!(result.to_string().contains("Đã hủy"));
        assert!(started.elapsed() < Duration::from_millis(700));
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn soniox_upload_job_poll_tokens_and_cleanup_use_only_the_created_artifacts() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for expected in [
                "POST /files ",
                "POST /transcriptions ",
                "GET /transcriptions/job-id ",
                "GET /transcriptions/job-id/transcript ",
                "DELETE /transcriptions/job-id ",
                "DELETE /files/file-id ",
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                let input = receive(&mut socket);
                let text = String::from_utf8_lossy(&input);
                assert!(text.starts_with(expected));
                assert!(text.to_lowercase().contains("bearer private-test-key"));
                let body = if expected == "POST /files " {
                    assert!(text.contains("recording.wav"));
                    assert!(input.windows(4).any(|s| s == b"RIFF"));
                    r#"{"id":"file-id"}"#
                } else if expected == "POST /transcriptions " {
                    assert!(text.contains("stt-async-v6"));
                    assert!(!text.contains("translation"));
                    r#"{"id":"job-id"}"#
                } else if expected == "GET /transcriptions/job-id " {
                    r#"{"status":"completed"}"#
                } else if expected.contains("/transcript ") {
                    r#"{"tokens":[{"text":"Hello.","start_ms":100,"end_ms":700}]}"#
                } else {
                    "{}"
                };
                respond(&mut socket, 200, body);
            }
        });
        let path = wav(1);
        let control = Control::new("meeting".into(), 1., SONIOX_ASYNC.into());
        let p = profile(String::new());
        let result = soniox_file(
            &reqwest::Client::new(),
            &p,
            "en",
            &path,
            1.,
            &control,
            &base,
        )
        .await
        .unwrap();
        assert_eq!(result[0].text, "Hello.");
        tokio::task::spawn_blocking(move || server.join().unwrap())
            .await
            .unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
