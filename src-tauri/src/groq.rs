//! Groq Whisper is request/response ASR, not a streaming WebSocket.
use crate::caption::{CaptionOptions, CaptionText};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const RATE: usize = 16000;
#[cfg(test)]
const CHUNK: usize = 7 * RATE / 2;
// Five seconds of context lets Whisper recover words at a request boundary.
// This is still batched ASR; requests advance by 3.5 seconds of new audio.
const OVERLAP: usize = 5 * RATE;
/// Unconfirmed audio survives a failed HTTP request and the adapter's collector.
/// Bounded independently of the capture channel, and never written to disk.
#[derive(Default)]
pub(crate) struct RetryAudio {
    samples: std::collections::VecDeque<f32>,
    offset: usize,
}
impl RetryAudio {
    pub(crate) fn restart(&mut self) -> Vec<f32> {
        self.offset = 0;
        self.samples.iter().copied().collect()
    }
    pub(crate) fn append(&mut self, samples: &[f32]) -> Result<()> {
        ensure!(
            self.samples.len() + samples.len() <= RATE * 30,
            "Audio chưa xác nhận tồn đọng hơn 30 giây; hãy kiểm tra mạng/hạn mức"
        );
        self.samples.extend(samples);
        Ok(())
    }
    pub(crate) fn acknowledge(&mut self, through: usize) {
        let count = through.saturating_sub(self.offset).min(self.samples.len());
        self.samples.drain(..count);
        self.offset += count;
    }
    pub(crate) fn take(&mut self) -> Vec<f32> {
        self.offset = 0;
        self.samples.drain(..).collect()
    }
}
pub fn is_speech_model(model: &str) -> bool {
    matches!(model, "whisper-large-v3" | "whisper-large-v3-turbo")
}

fn endpoint(base: &str) -> Result<String> {
    let base = if base.trim().is_empty() {
        "https://api.groq.com/openai/v1"
    } else {
        base.trim().trim_end_matches('/')
    };
    let url = reqwest::Url::parse(base).context("Groq Base URL không hợp lệ")?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Groq Base URL không hợp lệ"
    );
    Ok(format!("{base}/audio/transcriptions"))
}
fn status_error(code: u16) -> anyhow::Error {
    let reason=match code{401=>"API key không đúng hoặc hết hạn",403=>"key không có quyền dùng model",429=>"đã chạm hạn mức Groq; chỉ chuyển sang dự phòng đã chọn, nếu không có hãy chờ hạn mức hồi lại rồi bắt đầu lại",400|404=>"model hoặc cấu hình audio không hợp lệ",413=>"đoạn audio quá lớn",_=>"dịch vụ chưa xử lý được yêu cầu"};
    anyhow::anyhow!("Groq {code}: {reason}")
}
fn multipart(samples: &[i16], model: &str, language: Option<&str>) -> (String, Vec<u8>) {
    let boundary = format!("innote-{}", uuid::Uuid::new_v4());
    let mut body = Vec::new();
    for (name, value) in [
        ("model", model),
        ("response_format", "verbose_json"),
        ("timestamp_granularities[]", "word"),
        ("temperature", "0"),
    ] {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    }
    if let Some(language) = language.filter(|l| !matches!(*l, "" | "auto" | "multi")) {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"language\"\r\n\r\n{language}\r\n").as_bytes());
    }
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"caption.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes());
    let bytes = (samples.len() * 2) as u32;
    body.extend_from_slice(b"RIFF");
    body.extend_from_slice(&(36 + bytes).to_le_bytes());
    body.extend_from_slice(b"WAVEfmt ");
    body.extend_from_slice(&16u32.to_le_bytes());
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&16000u32.to_le_bytes());
    body.extend_from_slice(&32000u32.to_le_bytes());
    body.extend_from_slice(&2u16.to_le_bytes());
    body.extend_from_slice(&16u16.to_le_bytes());
    body.extend_from_slice(b"data");
    body.extend_from_slice(&bytes.to_le_bytes());
    body.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}
#[derive(Debug)]
pub(crate) struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}
fn parse(raw: &[u8], duration: f64) -> Result<Vec<Word>> {
    ensure!(raw.len() < 2_000_000, "Groq trả dữ liệu quá lớn");
    let v: Value = serde_json::from_slice(raw).context("Groq trả JSON không hợp lệ")?;
    let words = v["words"].as_array();
    if words.is_none() && v["text"].as_str().is_some_and(|s| s.trim().is_empty()) {
        return Ok(vec![]);
    }
    let words = words.context("Groq chưa trả word timestamps; không thể ghép đoạn an toàn")?;
    let mut result = Vec::new();
    for word in words {
        let text = word["word"].as_str().context("Groq thiếu nội dung từ")?;
        let start = word["start"].as_f64().context("Groq thiếu mốc đầu")?;
        let end = word["end"].as_f64().context("Groq thiếu mốc cuối")?;
        ensure!(
            start.is_finite()
                && end.is_finite()
                && start >= 0.
                && end >= start
                && end <= duration + 1.,
            "Groq trả timestamp không hợp lệ"
        );
        result.push(Word {
            text: text.into(),
            start: start.min(duration),
            end: end.min(duration),
        });
    }
    Ok(result)
}
/// Shared HTTP adapter. Credentials and provider response bodies never enter errors.
pub(crate) async fn recognize(
    client: &reqwest::Client,
    key: &str,
    base: &str,
    model: &str,
    language: Option<&str>,
    samples: &[i16],
) -> Result<Vec<Word>> {
    ensure!(is_speech_model(model), "Model Groq không hỗ trợ chép lời");
    let (content, body) = multipart(samples, model, language);
    let response = client
        .post(endpoint(base)?)
        .bearer_auth(key)
        .header("Content-Type", content)
        .body(body)
        .send()
        .await
        .map_err(|_| {
            anyhow::anyhow!("Groq phản hồi quá lâu hoặc mất mạng; hãy kiểm tra rồi bắt đầu lại")
        })?;
    ensure!(
        response.status().is_success(),
        "{}",
        status_error(response.status().as_u16())
    );
    let data = response
        .bytes()
        .await
        .map_err(|_| anyhow::anyhow!("Không đọc được kết quả Groq; bản thu vẫn được giữ"))?;
    parse(&data, samples.len() as f64 / RATE as f64)
}
pub fn test_speech(key: &str, base: &str, model: &str) -> Result<()> {
    ensure!(is_speech_model(model), "Model Groq không hỗ trợ chép lời");
    let (content, body) = multipart(&vec![0; RATE], model, Some("en"));
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()?
        .post(endpoint(base)?)
        .bearer_auth(key)
        .header("Content-Type", content)
        .body(body)
        .send()
        .map_err(|_| anyhow::anyhow!("Groq: không kết nối được; kiểm tra mạng hoặc Base URL"))?;
    ensure!(
        response.status().is_success(),
        "{}",
        status_error(response.status().as_u16())
    );
    parse(&response.bytes()?, 1.)?;
    Ok(())
}

pub(crate) struct AudioChunk {
    pub samples: Vec<i16>,
    pub base: usize,
    pub final_: bool,
}
impl AudioChunk {
    pub(crate) fn voiced(&self) -> bool {
        self.samples.iter().any(|s| s.unsigned_abs() > 24)
    }
}
#[derive(Default)]
struct Assembler {
    overlap_limit: Option<usize>,
    pending: Vec<i16>,
    context: Vec<i16>,
    cursor: usize,
}
impl Assembler {
    fn take(&mut self, len: usize, final_: bool) -> AudioChunk {
        let remainder = self.pending.split_off(len);
        let audio = std::mem::replace(&mut self.pending, remainder);
        let base = self.cursor.saturating_sub(self.context.len());
        let mut samples = std::mem::take(&mut self.context);
        samples.extend(audio);
        self.cursor += len;
        self.context = samples[samples
            .len()
            .saturating_sub(self.overlap_limit.unwrap_or(OVERLAP))..]
            .to_vec();
        AudioChunk {
            samples,
            base,
            final_,
        }
    }
}
pub(crate) fn confirmed_words(
    words: Vec<Word>,
    chunk: &AudioChunk,
    watermark: &mut f64,
    deferred: &mut Vec<Word>,
) -> Vec<Word> {
    let base = chunk.base as f64 / RATE as f64;
    let until =
        base + chunk.samples.len() as f64 / RATE as f64 - if chunk.final_ { 0. } else { 0.4 };
    let mut shown = Vec::new();
    let mut current: Vec<Word> = words
        .into_iter()
        .map(|word| Word {
            text: word.text,
            start: base + word.start,
            end: base + word.end,
        })
        .collect();
    // A word held at the edge is still a real recognition result. Prefer a
    // later overlapping recognition, but don't erase it when the next request
    // omits that part of the sentence (observed with both Whisper variants).
    let retained: Vec<Word> = deferred
        .drain(..)
        .filter(|old| {
            !current
                .iter()
                .any(|new| new.end > old.start + 0.015 && new.start < old.end - 0.015)
        })
        .collect();
    current.extend(retained);
    current.sort_by(|a, b| a.start.total_cmp(&b.start));
    for word in current {
        let start = word.start;
        let end = word.end;
        if end > *watermark + 0.015 && end <= until && start >= *watermark - 0.15 {
            *watermark = end;
            shown.push(word);
        } else if end > until && end > *watermark + 0.015 {
            deferred.push(word);
        }
    }
    shown
}
fn visible_words(
    words: Vec<Word>,
    chunk: &AudioChunk,
    watermark: &mut f64,
    deferred: &mut Vec<Word>,
) -> String {
    confirmed_words(words, chunk, watermark, deferred)
        .into_iter()
        .map(|w| w.text)
        .collect::<Vec<_>>()
        .join(" ")
}

pub async fn run_captions<F: FnMut(CaptionText)>(
    app: &tauri::AppHandle,
    opts: &CaptionOptions,
    stop: Arc<AtomicBool>,
    mut publish: F,
) -> Result<()> {
    ensure!(!opts.translate&&opts.mode=="standalone","Groq Whisper hiện dùng cho phụ đề lời gốc độc lập; chưa có dịch streaming/cuộc họp dùng chung");
    let chunk_samples = opts.speech_quality.groq_chunk_samples();
    let overlap_samples = opts.speech_quality.groq_overlap_samples();
    let advance = chunk_samples as f64 / RATE as f64;
    let source = crate::caption::audio_input(opts, stop.clone())?;
    let retry = opts.retry_audio.clone();
    let initial = retry.lock().unwrap().restart();
    let (queue_tx, mut queue) = tokio::sync::mpsc::channel::<AudioChunk>(3);
    let (failure_tx, mut failure) = tokio::sync::mpsc::unbounded_channel();
    let producer = tokio::spawn(async move {
        let result:Result<()>=async{
            let mut raw_rx=source.lock().await;
            let mut audio=Assembler { overlap_limit: Some(overlap_samples), ..Default::default() };
            audio.pending=initial.iter().map(|s|(s.clamp(-1.,1.)*32767.).round() as i16).collect();
            while audio.pending.len()>=chunk_samples {
                queue_tx.send(audio.take(chunk_samples,false)).await.map_err(|_|anyhow::anyhow!("Đã đóng hàng đợi phụ đề"))?;
            }
            while let Some(samples)=raw_rx.recv().await {
                let samples=samples?;
                retry.lock().unwrap().append(&samples)?;
                audio.pending.extend(samples.iter().map(|s|(s.clamp(-1.,1.)*32767.).round() as i16));
                while audio.pending.len()>=chunk_samples {
                    queue_tx.try_send(audio.take(chunk_samples,false)).map_err(|_|anyhow::anyhow!("Groq không xử lý kịp: hàng đợi audio đã đầy. Hãy dừng và kiểm tra mạng/hạn mức."))?;
                }
            }
            if !audio.pending.is_empty()||!audio.context.is_empty(){let len=audio.pending.len();queue_tx.send(audio.take(len,true)).await.map_err(|_|anyhow::anyhow!("Đã đóng hàng đợi phụ đề"))?;}
            Ok(())
        }.await;
        if let Err(e) = result {
            let _ = failure_tx.send(e);
        }
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let consume = async {
        let mut watermark = -1.;
        let mut deferred = Vec::new();
        let mut last_request: Option<tokio::time::Instant> = None;
        crate::caption::emit_status(app,"groq","active",false,&format!("Đang gom audio loa: Groq Whisper cập nhật khoảng mỗi {advance:.1} giây, không phải streaming từng từ."));
        while let Some(chunk) = queue.recv().await {
            if !chunk.voiced() && deferred.is_empty() {
                opts.retry_audio
                    .lock()
                    .unwrap()
                    .acknowledge(chunk.base + chunk.samples.len());
                continue;
            }
            if let Some(last) = last_request {
                tokio::time::sleep_until(last + Duration::from_millis(3200)).await;
            }
            crate::caption::emit_status(
                app,
                "groq",
                if stop.load(Ordering::Acquire) {
                    "stopping"
                } else {
                    "sending"
                },
                true,
                "Đang gửi một đoạn audio lên Groq để chép lời; không yêu cầu dịch.",
            );
            last_request = Some(tokio::time::Instant::now());
            let words = recognize(
                &client,
                &opts.api_key,
                &opts.base_url,
                &opts.model,
                opts.source_language.as_deref(),
                &chunk.samples,
            )
            .await?;
            if std::env::var("INNOTE_GROQ_AUDIT").as_deref() == Ok("1") {
                use tauri::Emitter;
                let _=app.emit("groq-caption-audit",serde_json::json!({"model":opts.model,"base":chunk.base,"samples":chunk.samples.len(),"final":chunk.final_,"watermark":watermark,"words":words.iter().map(|w|serde_json::json!({"text":w.text,"start":w.start,"end":w.end})).collect::<Vec<_>>()}));
            }
            let text = visible_words(words, &chunk, &mut watermark, &mut deferred);
            // Retain the unconfirmed tail (including words deferred at the edge)
            // until a later response or an explicitly configured fallback handles it.
            let confirmed = if text.trim().is_empty() && watermark < chunk.base as f64 / RATE as f64
            {
                chunk.base
            } else {
                (watermark.max(0.) * RATE as f64) as usize
            };
            opts.retry_audio.lock().unwrap().acknowledge(confirmed);
            if !text.trim().is_empty() {
                publish(CaptionText {
                    text,
                    original: String::new(),
                    final_: true,
                });
            }
            if !stop.load(Ordering::Acquire) {
                crate::caption::emit_status(
                    app,
                    "groq",
                    "active",
                    false,
                    &format!("Đang nghe đoạn tiếp theo; Groq Whisper cập nhật khoảng mỗi {advance:.1} giây."),
                );
            }
        }
        ensure!(
            stop.load(Ordering::Acquire),
            "Nguồn âm thanh đã dừng; chọn lại loa/tai nghe rồi bật phụ đề"
        );
        Ok(())
    };
    let deadline = async {
        while !stop.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        tokio::time::sleep(Duration::from_secs(12)).await;
    };
    let result = tokio::select! {result=consume=>result,Some(error)=failure.recv()=>Err(error),_=deadline=>Err(anyhow::anyhow!("Đã dừng Groq; chưa xử lý xong phần audio cuối trong 12 giây"))};
    stop.store(true, Ordering::Release);
    producer.abort();
    let _ = producer.await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_async_recognizer_preserves_times_and_sanitizes_http_errors() {
        use std::io::{Read, Write};
        for code in [200, 401, 403, 429, 503] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut input = Vec::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    input.extend_from_slice(&buffer[..count]);
                    if let Some(end) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&input[..end]).to_lowercase();
                        let length: usize = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        if input.len() >= end + 4 + length {
                            assert!(headers.starts_with("post /audio/transcriptions "));
                            assert!(headers.contains("bearer private-test-key"));
                            let body = String::from_utf8_lossy(&input[end + 4..]);
                            assert!(body.contains("whisper-large-v3-turbo"));
                            assert!(body.contains("\r\nen\r\n"));
                            break;
                        }
                    }
                }
                let body = if code == 200 {
                    r#"{"words":[{"word":"hello","start":0.2,"end":0.7}]}"#
                } else {
                    r#"{"error":{"message":"private-test-key"}}"#
                };
                write!(socket,"HTTP/1.1 {code} Response\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            });
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let result = runtime.block_on(async {
                recognize(
                    &reqwest::Client::new(),
                    "private-test-key",
                    &format!("http://{address}"),
                    "whisper-large-v3-turbo",
                    Some("en"),
                    &vec![100; RATE],
                )
                .await
            });
            if code == 200 {
                let words = result.unwrap();
                assert_eq!(words.len(), 1);
                assert_eq!(words[0].text, "hello");
                assert_eq!((words[0].start, words[0].end), (0.2, 0.7));
            } else {
                let error = format!("{:#}", result.unwrap_err());
                assert!(error.contains(&code.to_string()));
                assert!(!error.contains("private-test-key"));
            }
            server.join().unwrap();
        }
    }
    #[test]
    fn processing_levels_keep_every_new_sample_and_bound_overlap() {
        for level in [
            crate::speech_quality::SpeechQuality::Fast,
            crate::speech_quality::SpeechQuality::Balanced,
            crate::speech_quality::SpeechQuality::Accurate,
        ] {
            let n = level.groq_chunk_samples();
            let overlap = level.groq_overlap_samples();
            let mut audio = Assembler {
                overlap_limit: Some(overlap),
                ..Default::default()
            };
            for turn in 0..4 {
                audio.pending = vec![turn as i16 + 1; n];
                let chunk = audio.take(n, false);
                let context = (turn * n).min(overlap);
                assert_eq!(chunk.base, turn * n - context);
                assert_eq!(chunk.samples.len(), context + n);
                assert_eq!(
                    &chunk.samples[context..],
                    vec![turn as i16 + 1; n].as_slice()
                );
            }
        }
    }
    #[test]
    fn speech_probe_posts_multipart_and_classifies_auth_and_quota_without_secrets() {
        use std::io::{Read, Write};
        for code in [200, 401, 429] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut input = Vec::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    input.extend_from_slice(&buffer[..count]);
                    if let Some(header) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers =
                            String::from_utf8_lossy(&input[..header]).to_ascii_lowercase();
                        let length: usize = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        if input.len() >= header + 4 + length {
                            assert!(headers.starts_with("post /audio/transcriptions "));
                            assert!(headers.contains("bearer private-test-key"));
                            assert!(input.windows(4).any(|w| w == b"RIFF"));
                            break;
                        }
                    }
                }
                let body = if code == 200 {
                    r#"{"text":"","words":[]}"#
                } else {
                    r#"{"error":{"message":"private-test-key"}}"#
                };
                write!(socket,"HTTP/1.1 {code} Response\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            });
            let result = test_speech(
                "private-test-key",
                &format!("http://{address}"),
                "whisper-large-v3",
            );
            if code == 200 {
                result.unwrap();
            } else {
                let message = result.unwrap_err().to_string();
                assert!(message.contains(&code.to_string()));
                assert!(!message.contains("private-test-key"));
            }
            server.join().unwrap();
        }
    }
    #[test]
    fn multipart_audio_has_wav_language_and_no_secret() {
        let (content, body) = multipart(&[123, -456], "whisper-large-v3-turbo", Some("en"));
        assert!(content.starts_with("multipart/form-data; boundary="));
        assert!(body.windows(4).any(|b| b == b"RIFF"));
        assert!(body.windows(4).any(|b| b == b"WAVE"));
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("whisper-large-v3-turbo") && text.contains("\r\nen\r\n"));
    }
    #[test]
    fn overlap_keeps_samples_and_drops_previously_shown_words() {
        let mut a = Assembler::default();
        a.pending = vec![100; CHUNK];
        let first = a.take(CHUNK, false);
        assert_eq!(first.base, 0);
        a.pending = vec![200; CHUNK];
        let second = a.take(CHUNK, false);
        assert_eq!(second.base, CHUNK.saturating_sub(OVERLAP));
        assert_eq!(second.samples.len(), CHUNK + OVERLAP.min(CHUNK));
        let mut watermark = second.base as f64 / RATE as f64 + 0.5;
        let words = vec![
            Word {
                text: "old".into(),
                start: 0.,
                end: 0.3,
            },
            Word {
                text: "new".into(),
                start: 0.6,
                end: 1.2,
            },
        ];
        assert_eq!(
            visible_words(words, &second, &mut watermark, &mut Vec::new()),
            "new"
        );
    }
    #[test]
    fn errors_never_echo_raw_provider_messages() {
        assert!(status_error(429).to_string().contains("429"));
        assert!(status_error(401).to_string().contains("API key"));
        assert!(parse(br#"{"words":[{"word":"bad","start":5,"end":2}]}"#, 6.).is_err());
    }
    #[test]
    fn retry_keeps_failed_and_collector_pending_audio_after_acknowledged_prefix() {
        let mut retry = RetryAudio::default();
        retry.append(&vec![0.1; RATE * 5]).unwrap();
        retry.acknowledge(RATE * 4);
        retry.append(&vec![0.2; RATE * 5]).unwrap();
        let replay = retry.restart();
        assert_eq!(replay.len(), RATE * 6);
        assert_eq!(replay[0], 0.1);
        assert_eq!(replay[RATE], 0.2);
        retry.acknowledge(RATE * 2);
        assert_eq!(retry.take(), vec![0.2; RATE * 4]);
        assert!(retry.append(&vec![0.; RATE * 31]).is_err());
    }
    #[test]
    fn deferred_last_word_survives_an_omission_in_the_final_response() {
        let chunk = AudioChunk {
            samples: vec![100; RATE * 10],
            base: 0,
            final_: false,
        };
        let mut watermark = 9.3;
        let mut deferred = vec![];
        assert_eq!(
            visible_words(
                vec![Word {
                    text: "tomorrow.".into(),
                    start: 9.3,
                    end: 9.64
                }],
                &chunk,
                &mut watermark,
                &mut deferred
            ),
            ""
        );
        let last = AudioChunk {
            samples: vec![100; RATE * 8],
            base: RATE * 5,
            final_: true,
        };
        assert_eq!(
            visible_words(
                vec![Word {
                    text: "dong.".into(),
                    start: 0.56,
                    end: 1.76
                }],
                &last,
                &mut watermark,
                &mut deferred
            ),
            "tomorrow."
        );
        assert!(deferred.is_empty());
    }
    #[test]
    fn newer_overlap_replaces_a_deferred_partial_word() {
        let mut deferred = vec![Word {
            text: "is $20.".into(),
            start: 4.38,
            end: 5.,
        }];
        let mut watermark = 4.38;
        let next = AudioChunk {
            samples: vec![100; RATE * 10],
            base: 0,
            final_: false,
        };
        let words = vec![
            Word {
                text: "is".into(),
                start: 4.38,
                end: 4.68,
            },
            Word {
                text: "20".into(),
                start: 4.68,
                end: 5.04,
            },
        ];
        assert_eq!(
            visible_words(words, &next, &mut watermark, &mut deferred),
            "is 20"
        );
        assert!(deferred.is_empty());
    }
}
