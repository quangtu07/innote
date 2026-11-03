//! Only user-configured alternatives are eligible; no provider is inferred.
use anyhow::Result;
pub fn eligible(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}").to_lowercase();
    if [
        "thiết bị",
        "loa/tai nghe",
        "nguồn âm thanh",
        "tồn đọng",
        "storage",
        "thư mục",
        "bản chép đang trống",
    ]
    .iter()
    .any(|s| text.contains(s))
    {
        return false;
    }
    [
        "401",
        "402",
        "403",
        "404",
        "429",
        "500",
        "502",
        "503",
        "504",
        "hạn mức",
        "số dư",
        "quota",
        "kết nối",
        "mất mạng",
        "quá lâu",
        "gián đoạn",
        "đóng phiên",
        "kết thúc phiên",
        "không gửi được",
        "yêu cầu kết nối lại",
        "model",
        "chưa sẵn sàng",
    ]
    .iter()
    .any(|s| text.contains(s))
}
pub fn once<T>(
    primary: impl FnOnce() -> Result<T>,
    alternative: Option<impl FnOnce() -> Result<T>>,
) -> Result<(T, bool)> {
    match primary() {
        Ok(value) => Ok((value, false)),
        Err(error) => {
            if !eligible(&error) {
                return Err(error);
            }
            match alternative {
                Some(run) => run().map(|value| (value, true)).map_err(|next| {
                    anyhow::anyhow!("Model chính lỗi: {error}. Model dự phòng cũng lỗi: {next}")
                }),
                None => Err(error),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_uses_only_explicit_alternative_once() {
        let mut calls = 0;
        let result = once(
            || anyhow::bail!("Groq 429"),
            Some(|| {
                calls += 1;
                Ok("fallback")
            }),
        )
        .unwrap();
        assert_eq!(result, ("fallback", true));
        assert_eq!(calls, 1);
    }
    #[test]
    fn device_error_and_success_do_not_switch() {
        assert!(!eligible(&anyhow::anyhow!("Không mở được loa/tai nghe")));
        let result = once(|| Ok("primary"), Some(|| panic!("must not switch"))).unwrap();
        assert_eq!(result, ("primary", false));
    }
    #[test]
    fn http_quota_failure_uses_the_explicit_second_endpoint() {
        use std::io::{Read, Write};
        fn server(code: u16, body: &'static str) -> (String, std::thread::JoinHandle<()>) {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let thread = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut data = Vec::new();
                let mut buffer = [0; 8192];
                loop {
                    let n = socket.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&buffer[..n]);
                    if let Some(end) = data.windows(4).position(|b| b == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&data[..end]).to_lowercase();
                        let len: usize = headers
                            .lines()
                            .find_map(|s| s.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        if data.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                write!(socket,"HTTP/1.1 {code} Result\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            });
            (format!("http://{address}"), thread)
        }
        let (first, a) = server(429, r#"{"error":{"message":"quota secret-key"}}"#);
        let (second, b) = server(
            200,
            r#"{"choices":[{"message":{"content":"preserved negation and 15 million"}}]}"#,
        );
        let (result, switched) = once(
            || {
                crate::llm::smooth(
                    "openai-compatible",
                    "primary",
                    &first,
                    "secret-key",
                    "not 50 million",
                    "",
                )
            },
            Some(|| {
                crate::llm::smooth(
                    "openai-compatible",
                    "backup",
                    &second,
                    "other-key",
                    "not 50 million",
                    "",
                )
            }),
        )
        .unwrap();
        assert!(switched);
        assert_eq!(result, "preserved negation and 15 million");
        a.join().unwrap();
        b.join().unwrap();
    }
}
