//! Optional remote jobs. No key, PCM or provider response is logged/persisted here.
//! A job is atomic: only a finished stream can advance the local watermark.
use crate::types::{Segment, WorkerJob, WorkerResult};
use anyhow::{bail, ensure, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};

const ENDPOINT: &str = "wss://stt-rt.soniox.com/transcribe-websocket";

pub fn validate_key(key: &str) -> Result<String> {
    let key = key.trim();
    ensure!(
        key.len() <= 4096 && key.bytes().all(|c| c.is_ascii_graphic()),
        "Key không hợp lệ. Hãy nhập lại, không thêm xuống dòng."
    );
    Ok(key.into())
}

fn provider_error(code: u64) -> &'static str {
    match code {
        401 => "Soniox: key không đúng hoặc đã hết hạn",
        402 => "Soniox: đã hết số dư hoặc chạm hạn mức chi tiêu",
        403 => "Soniox: key không có quyền hoặc phiên đã hết hạn",
        429 => "Soniox: vượt giới hạn yêu cầu",
        _ => "Soniox không xử lý được yêu cầu",
    }
}

#[derive(Default)]
struct Transcript {
    segments: Vec<Segment>,
    current: Option<Segment>,
    tokens: usize,
}
impl Transcript {
    fn flush(&mut self) {
        if let Some(mut s) = self.current.take() {
            s.text = s.text.trim().into();
            if !s.text.is_empty() && s.end > s.start {
                self.segments.push(s);
            }
        }
    }
    fn feed(&mut self, v: &Value, offset: f64, duration: f64) -> Result<bool> {
        if v.get("error_code").is_some() || v.get("error_type").is_some() {
            // Do not echo untrusted provider errors: they may contain secrets.
            bail!(provider_error(v["error_code"].as_u64().unwrap_or(500)));
        }
        ensure!(
            v.is_object() && v["tokens"].is_array(),
            "Soniox trả dữ liệu không hợp lệ"
        );
        for t in v["tokens"].as_array().unwrap() {
            if t["is_final"].as_bool() != Some(true) || t["translation_status"] == "translation" {
                continue;
            }
            let text = t["text"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Soniox thiếu nội dung token"))?;
            if matches!(text, "<end>" | "<fin>") {
                self.flush();
                continue;
            }
            let start = t["start_ms"]
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("Soniox thiếu mốc thời gian"))?
                / 1000.;
            let end = t["end_ms"]
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("Soniox thiếu mốc thời gian"))?
                / 1000.;
            ensure!(
                start.is_finite()
                    && end.is_finite()
                    && start >= 0.
                    && end >= start
                    && end <= duration + 1.,
                "Soniox trả mốc thời gian không hợp lệ"
            );
            self.tokens += 1;
            ensure!(
                self.tokens <= 10000 && text.len() <= 10000,
                "Soniox trả quá nhiều dữ liệu"
            );
            if self
                .current
                .as_ref()
                .is_some_and(|s| offset + start > s.end + 1.5 || s.text.len() + text.len() > 500)
            {
                self.flush();
            }
            let s = self.current.get_or_insert_with(|| Segment {
                id: uuid::Uuid::new_v4().to_string(),
                start: offset + start.min(duration),
                end: offset + end.min(duration),
                text: String::new(),
                edited: false,
            });
            s.end = s.end.max(offset + end.min(duration));
            s.text.push_str(text);
        }
        let finished = v["finished"].as_bool() == Some(true);
        if finished {
            self.flush();
        }
        Ok(finished)
    }
}

pub async fn transcribe(key: &str, job: &WorkerJob, cancel: &AtomicBool) -> Result<WorkerResult> {
    transcribe_at(ENDPOINT, key, job, cancel, Duration::from_secs(35)).await
}

pub async fn test_connection(key: &str) -> Result<()> {
    test_connection_at(ENDPOINT, key, Duration::from_secs(8)).await
}

pub async fn test_connection_at(endpoint: &str, key: &str, timeout: Duration) -> Result<()> {
    let key = validate_key(key)?;
    ensure!(!key.is_empty(), "Cần nhập API key Soniox");
    let mut request = endpoint
        .into_client_request()
        .map_err(|_| anyhow::anyhow!("Địa chỉ Soniox không hợp lệ"))?;
    let mut authorization = HeaderValue::from_str(&format!("Bearer {key}"))
        .map_err(|_| anyhow::anyhow!("Key không hợp lệ"))?;
    authorization.set_sensitive(true);
    request.headers_mut().insert("Authorization", authorization);
    let operation = async {
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(6), connect_async(request))
            .await
            .map_err(|_| anyhow::anyhow!("Kết nối Soniox quá lâu"))?
            .map_err(|_| {
                anyhow::anyhow!("Không kết nối được Soniox; kiểm tra mạng hoặc dịch vụ")
            })?;
        socket
            .send(Message::Text(
                json!({
                    "model": "stt-rt-v5",
                    "audio_format": "s16le",
                    "sample_rate": 16000,
                    "num_channels": 1,
                    "language_hints": ["vi"],
                    "enable_speaker_diarization": false
                })
                .to_string(),
            ))
            .await
            .map_err(|_| anyhow::anyhow!("Không gửi được cấu hình Soniox"))?;
        socket
            .send(Message::Text(String::new()))
            .await
            .map_err(|_| anyhow::anyhow!("Không gửi được tín hiệu tới Soniox"))?;
        while let Some(msg) = socket.next().await {
            match msg.map_err(|_| anyhow::anyhow!("Kết nối Soniox bị gián đoạn"))? {
                Message::Text(text) => {
                    let v: Value = serde_json::from_str(&text)
                        .map_err(|_| anyhow::anyhow!("Soniox trả JSON không hợp lệ"))?;
                    if let Some(code) = v.get("error_code").and_then(|c| c.as_u64()) {
                        bail!(provider_error(code));
                    }
                    if let Some(err_type) = v.get("error_type").and_then(|t| t.as_str()) {
                        if err_type == "unauthenticated" || err_type == "permission_denied" {
                            bail!("Soniox: key không đúng hoặc đã hết hạn");
                        }
                        bail!("Lỗi Soniox: {}", err_type);
                    }
                    return Ok(());
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        Ok(())
    };
    tokio::time::timeout(timeout, operation)
        .await
        .map_err(|_| anyhow::anyhow!("Soniox phản hồi quá lâu"))?
}

async fn transcribe_at(
    endpoint: &str,
    key: &str,
    job: &WorkerJob,
    cancel: &AtomicBool,
    limit: Duration,
) -> Result<WorkerResult> {
    ensure!(!cancel.load(Ordering::Acquire), "Đã dừng chép theo yêu cầu");
    let key = validate_key(key)?;
    ensure!(!key.is_empty(), "Chưa có key Soniox");
    ensure!(
        job.offset.is_finite() && job.offset >= 0. && ["vi", "en"].contains(&job.language.as_str()),
        "Job Soniox không hợp lệ"
    );
    let mut wav = hound::WavReader::open(&job.path)?;
    ensure!(
        wav.spec().sample_rate == 16000
            && wav.spec().channels == 1
            && wav.spec().bits_per_sample == 16
            && wav.duration() <= 30 * 16000,
        "Soniox cần chunk PCM16 mono 16 kHz, tối đa 30 giây"
    );
    let duration = wav.duration() as f64 / 16000.;
    let pcm: Vec<u8> = wav
        .samples::<i16>()
        .map(|s| s.map(i16::to_le_bytes))
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    let timer = Instant::now();
    let operation = async {
        let mut request = endpoint
            .into_client_request()
            .map_err(|_| anyhow::anyhow!("Địa chỉ Soniox không hợp lệ"))?;
        let mut authorization = HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| anyhow::anyhow!("Key không hợp lệ"))?;
        authorization.set_sensitive(true);
        request.headers_mut().insert("Authorization", authorization);
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(8), connect_async(request))
            .await
            .map_err(|_| anyhow::anyhow!("Kết nối Soniox quá lâu"))?
            .map_err(|_| {
                anyhow::anyhow!("Không kết nối được Soniox; kiểm tra mạng hoặc dịch vụ")
            })?;
        socket.send(Message::Text(json!({"model":"stt-rt-v5","audio_format":"s16le","sample_rate":16000,"num_channels":1,"language_hints":[job.language],"enable_speaker_diarization":false}).to_string())).await.map_err(|_| anyhow::anyhow!("Không gửi được cấu hình Soniox"))?;
        for chunk in pcm.chunks(64000) {
            socket
                .send(Message::Binary(chunk.to_vec()))
                .await
                .map_err(|_| anyhow::anyhow!("Kết nối Soniox bị gián đoạn khi gửi audio"))?;
        }
        socket
            .send(Message::Text(String::new()))
            .await
            .map_err(|_| anyhow::anyhow!("Soniox ngắt khi gửi audio"))?;
        let mut transcript = Transcript::default();
        while let Some(message) = socket.next().await {
            match message.map_err(|_| anyhow::anyhow!("Kết nối Soniox bị gián đoạn"))? {
                Message::Text(text) => {
                    let v: Value = serde_json::from_str(&text)
                        .map_err(|_| anyhow::anyhow!("Soniox trả JSON không hợp lệ"))?;
                    if transcript.feed(&v, job.offset, duration)? {
                        return Ok(WorkerResult {
                            segments: transcript.segments,
                            error: None,
                            elapsed_ms: timer.elapsed().as_millis() as u64,
                            vad_windows: 0,
                            inference_windows: 0,
                        });
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        bail!("Soniox ngắt kết nối trước khi hoàn tất; phần này sẽ được chép lại local")
    };
    let cancelled = async {
        loop {
            if cancel.load(Ordering::Acquire) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    tokio::select! {
        _ = cancelled => bail!("Đã dừng chép theo yêu cầu"),
        result = tokio::time::timeout(limit, operation) => result.map_err(|_| anyhow::anyhow!("Soniox phản hồi quá lâu"))?,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn mock_job(mode: &str, cancel: &AtomicBool, limit: Duration) -> Result<WorkerResult> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        let mode = mode.to_string();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_hdr_async(
                stream,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                    assert_eq!(request.headers()["Authorization"], "Bearer fake-test-key");
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let config = socket.next().await.unwrap().unwrap();
            let config: Value = serde_json::from_str(config.to_text().unwrap()).unwrap();
            assert!(config.get("api_key").is_none());
            assert_eq!(config["sample_rate"], 16000);
            let mut bytes = 0;
            while let Some(Ok(m)) = socket.next().await {
                match m {
                    Message::Binary(b) => bytes += b.len(),
                    Message::Text(t) if t.is_empty() => break,
                    _ => {}
                }
            }
            assert_eq!(bytes, 32000);
            let message = match mode.as_str() {
                "ok" => json!({"tokens":[{"text":"Xin chào","start_ms":100,"end_ms":800,"is_final":true}],"finished":true}).to_string(),
                "close" => { let _=socket.close(None).await; return; },
                "broken" => "not json".into(),
                "stall" => { tokio::time::sleep(Duration::from_secs(3)).await; return; },
                code => json!({"tokens":[],"error_code":code.parse::<u64>().unwrap(),"error_message":"fake-test-key must never be echoed"}).to_string(),
            };
            let _ = socket.send(Message::Text(message)).await;
        });
        let path = std::env::temp_dir().join(format!("{}.wav", uuid::Uuid::new_v4()));
        let mut wav = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 16000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..16000 {
            wav.write_sample(0i16).unwrap();
        }
        wav.finalize().unwrap();
        let result = transcribe_at(
            &endpoint,
            "fake-test-key",
            &WorkerJob {
                path: path.to_string_lossy().into(),
                offset: 10.,
                language: "vi".into(),
            },
            cancel,
            limit,
        )
        .await;
        if !server.is_finished() {
            server.abort();
        } else {
            server.await.unwrap();
        }
        std::fs::remove_file(path).unwrap();
        result
    }
    #[tokio::test]
    async fn websocket_roundtrip_auth_audio_offsets_and_error_frames() {
        let cancel = AtomicBool::new(false);
        let r = mock_job("ok", &cancel, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(r.segments[0].text, "Xin chào");
        assert_eq!(r.segments[0].start, 10.1);
        for mode in ["401", "402", "403", "429", "503", "broken", "close"] {
            let error = mock_job(mode, &cancel, Duration::from_secs(5))
                .await
                .unwrap_err()
                .to_string();
            assert!(!error.contains("fake-test-key"));
        }
    }
    #[tokio::test]
    async fn stalled_stream_times_out_and_cancel_does_not_wait_for_network() {
        let cancel = AtomicBool::new(false);
        assert!(mock_job("stall", &cancel, Duration::from_millis(250))
            .await
            .unwrap_err()
            .to_string()
            .contains("quá lâu"));
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            trigger.store(true, Ordering::Release);
        });
        let now = Instant::now();
        assert!(mock_job("stall", &cancel, Duration::from_secs(5))
            .await
            .unwrap_err()
            .to_string()
            .contains("Đã dừng"));
        assert!(now.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn final_tokens_preserve_offsets_and_ignore_interim_translation_and_control_tokens() {
        let mut t = Transcript::default();
        t.feed(&json!({"tokens":[{"text":"nháp","is_final":false},{"text":"Hello","is_final":true,"translation_status":"translation"},{"text":"Xin","is_final":true,"start_ms":500,"end_ms":900}]}),50.,10.).unwrap();
        assert!(t.feed(&json!({"tokens":[{"text":" chào.","is_final":true,"start_ms":900,"end_ms":1200},{"text":"<end>","is_final":true}],"finished":true}),50.,10.).unwrap());
        assert_eq!(t.segments.len(), 1);
        assert_eq!(t.segments[0].text, "Xin chào.");
        assert_eq!(t.segments[0].start, 50.5);
        assert_eq!(t.segments[0].end, 51.2);
    }
    #[tokio::test]
    async fn test_connection_success_and_error() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_hdr_async(
                stream,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                    assert_eq!(request.headers()["Authorization"], "Bearer valid-test-key");
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let _ = socket.next().await;
            let _ = socket
                .send(Message::Text(
                    json!({"tokens":[], "finished": true}).to_string(),
                ))
                .await;
        });
        assert!(test_connection_at(&endpoint, "valid-test-key", Duration::from_secs(3))
            .await
            .is_ok());

        let listener_err = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint_err = format!("ws://{}", listener_err.local_addr().unwrap());
        tokio::spawn(async move {
            let (stream, _) = listener_err.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_hdr_async(
                stream,
                |_: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let _ = socket.next().await;
            let _ = socket
                .send(Message::Text(
                    json!({"error_code": 401, "error_type": "unauthenticated"}).to_string(),
                ))
                .await;
        });
        let err = test_connection_at(&endpoint_err, "bad-test-key", Duration::from_secs(3))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("key không đúng"));
    }

    #[test]
    fn errors_are_classified_without_echoing_secrets_and_invalid_times_fail_closed() {
        for code in [400, 401, 402, 403, 408, 429, 500, 503] {
            let e = Transcript::default()
                .feed(
                    &json!({"error_code":code,"error_message":"secret-key"}),
                    0.,
                    10.,
                )
                .unwrap_err()
                .to_string();
            assert!(!e.contains("secret-key"));
        }
        assert!(validate_key("key\nmalformed").is_err());
        assert!(Transcript::default()
            .feed(
                &json!({"tokens":[{"text":"bad","is_final":true,"start_ms":9000,"end_ms":500}]}),
                0.,
                10.
            )
            .is_err());
    }
}
