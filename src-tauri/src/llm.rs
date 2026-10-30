use anyhow::{bail, ensure, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

const SYSTEM_PROMPT: &str = "Bạn là bộ hiệu đính bản chép lời cuộc họp. Nhiệm vụ duy nhất là làm mịn câu chữ: sửa lỗi chính tả, dấu câu, viết hoa, tách câu và sửa những từ bị nhận dạng sai rõ ràng do âm thanh. Tuyệt đối KHÔNG thêm ý mới, không bỏ ý, không tóm tắt, không diễn giải, không đổi nghĩa, không đổi số liệu, tên riêng, mốc thời gian hay thứ tự nội dung. Không dịch sang ngôn ngữ khác. Giữ nguyên ngôn ngữ gốc của từng đoạn. Nếu không chắc một từ có bị sai hay không, giữ nguyên từ đó. Chỉ trả về văn bản đã làm mịn, không thêm lời giải thích, tiêu đề hay code fence.";

pub fn default_model(provider: &str) -> Result<&'static str> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "openai" => Ok("gpt-4o-mini"),
        "deepseek" => Ok("deepseek-chat"),
        "gemini" => Ok("gemini-2.5-flash"),
        "openai-compatible" => Ok(""),
        other => bail!("Nhà cung cấp AI không hợp lệ: {other}"),
    }
}

fn resolved_base(provider: &str, base_url: &str) -> Result<String> {
    let base = if base_url.trim().is_empty() {
        match provider {
            "openai" => "https://api.openai.com/v1".into(),
            "deepseek" => "https://api.deepseek.com".into(),
            "openai-compatible" => bail!("Cần nhập Base URL cho OpenAI-compatible"),
            _ => bail!("Nhà cung cấp AI không hợp lệ"),
        }
    } else {
        base_url.trim().trim_end_matches('/').to_string()
    };
    let parsed = reqwest::Url::parse(&base).context("Base URL không hợp lệ")?;
    ensure!(
        matches!(parsed.scheme(), "http" | "https"),
        "Base URL phải là http hoặc https"
    );
    Ok(base)
}

fn chat_endpoint(provider: &str, base_url: &str) -> Result<String> {
    let base = resolved_base(provider, base_url)?;
    Ok(if base.ends_with("/chat/completions") {
        base
    } else {
        format!("{base}/chat/completions")
    })
}

fn models_endpoint(provider: &str, base_url: &str) -> Result<String> {
    let base = resolved_base(provider, base_url)?;
    Ok(if base.ends_with("/models") {
        base
    } else {
        format!("{base}/models")
    })
}

fn gemini_endpoint(model: &str, key: &str) -> Result<String> {
    ensure!(!model.trim().is_empty(), "Cần nhập model Gemini");
    ensure!(!key.trim().is_empty(), "Cần nhập API key Gemini");
    let model = model.trim();
    let parsed = reqwest::Url::parse(&format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
    ))
    .context("Model Gemini không hợp lệ")?;
    Ok(format!("{parsed}?key={}", key.trim()))
}

fn user_prompt(original: &str, additional: &str) -> String {
    let additional = additional.trim();
    let mut prompt = format!("BẢN CHÉP GỐC CẦN LÀM MỊN:\n{original}");
    if !additional.is_empty() {
        prompt.push_str(
            "\n\nYÊU CẦU BỔ SUNG CỦA NGƯỜI DÙNG (chỉ áp dụng nếu không thay đổi nội dung):\n",
        );
        prompt.push_str(additional);
    }
    prompt.push_str(
        "\n\nHãy trả lại toàn bộ bản đã làm mịn. Không thêm bình luận, không tóm tắt, không bỏ đoạn nào.",
    );
    prompt
}

fn openai_text(value: &Value) -> Option<String> {
    let choice = &value["choices"][0];
    let content = &choice["message"]["content"];
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    if let Some(parts) = content.as_array() {
        let joined = parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Some(joined);
        }
    }
    if let Some(text) = choice["text"].as_str() {
        return Some(text.to_string());
    }
    if let Some(delta) = choice["delta"]["content"].as_str() {
        return Some(delta.to_string());
    }
    None
}

fn parse_ai_response(raw: &str, provider: &str) -> Result<String> {
    let trimmed = raw.trim().trim_start_matches('\u{feff}');

    // Support streaming/SSE chunks if an OpenAI-compatible proxy forces text/event-stream
    if trimmed.starts_with("data:") || trimmed.contains("\ndata:") || trimmed.contains("\r\ndata:")
    {
        let mut full_text = String::new();
        let mut has_sse_content = false;
        for line in trimmed.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("data:") {
                let rest = rest.trim();
                if rest.is_empty() || rest == "[DONE]" {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(rest) {
                    if let Some(err) = v.get("error") {
                        let msg = err
                            .get("message")
                            .and_then(Value::as_str)
                            .or_else(|| err.as_str())
                            .unwrap_or("Lỗi từ AI");
                        bail!("AI báo lỗi: {msg}");
                    }
                    if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                        full_text.push_str(t);
                        has_sse_content = true;
                    } else if let Some(t) = v["choices"][0]["message"]["content"].as_str() {
                        full_text.push_str(t);
                        has_sse_content = true;
                    } else if let Some(t) = v["choices"][0]["text"].as_str() {
                        full_text.push_str(t);
                        has_sse_content = true;
                    }
                }
            }
        }
        if has_sse_content && !full_text.trim().is_empty() {
            return Ok(clean_response(&full_text));
        }
    }

    let json_candidate = clean_response(trimmed);
    let value: Value = serde_json::from_str(&json_candidate)
        .or_else(|_| serde_json::from_str(trimmed))
        .map_err(|e| {
            let snippet: String = trimmed.chars().take(200).collect();
            anyhow::anyhow!("AI trả về JSON không hợp lệ: {e}. Nội dung phản hồi: {snippet}")
        })?;

    if let Some(err) = value.get("error") {
        let msg = err
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| err.as_str())
            .unwrap_or("Lỗi từ AI");
        bail!("AI báo lỗi: {msg}");
    }

    let text = if provider == "gemini" {
        gemini_text(&value)
    } else {
        openai_text(&value)
    }
    .context("Không tìm thấy nội dung trong phản hồi AI")?;

    Ok(clean_response(&text))
}

fn gemini_text(value: &Value) -> Option<String> {
    value["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
}

fn clean_response(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines();
    let first = lines.next().unwrap_or_default();
    if !first.starts_with("```") {
        return trimmed.to_string();
    }
    let mut body = lines.collect::<Vec<_>>();
    if body.last().is_some_and(|line| line.trim() == "```") {
        body.pop();
    }
    body.join("\n").trim().to_string()
}
pub fn smooth(
    provider: &str,
    model: &str,
    base_url: &str,
    api_key: &str,
    original: &str,
    additional: &str,
) -> Result<String> {
    let provider = provider.trim().to_ascii_lowercase();
    let model = if model.trim().is_empty() {
        default_model(&provider)?
    } else {
        model.trim()
    };
    ensure!(!model.trim().is_empty(), "Cần nhập model AI");
    ensure!(!original.trim().is_empty(), "Bản chép lời đang trống");
    ensure!(
        original.chars().count() <= 160_000,
        "Bản chép quá dài để gửi cho AI. Hãy chia nhỏ cuộc họp."
    );
    ensure!(
        additional.chars().count() <= 4_000,
        "Yêu cầu thêm quá dài (tối đa 4.000 ký tự)"
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .context("Không tạo được HTTP client")?;
    let prompt = user_prompt(original, additional);

    let (url, body, key) = match provider.as_str() {
        "openai" | "deepseek" | "openai-compatible" => {
            ensure!(
                !api_key.trim().is_empty(),
                "Cần nhập API key cho nhà cung cấp này"
            );
            let url = chat_endpoint(&provider, base_url)?;
            let body = json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": prompt}
                ],
                "temperature": 0.1,
                "stream": false
            });
            (url, body, api_key.trim().to_string())
        }
        "gemini" => {
            ensure!(!api_key.trim().is_empty(), "Cần nhập API key Gemini");
            let url = gemini_endpoint(model, api_key)?;
            let body = json!({
                "system_instruction": {"parts": [{"text": SYSTEM_PROMPT}]},
                "contents": [{"role": "user", "parts": [{"text": prompt}]}],
                "generationConfig": {"temperature": 0.1}
            });
            (url, body, String::new())
        }
        _ => bail!("Nhà cung cấp AI không hợp lệ"),
    };

    let mut request = client
        .post(&url)
        .header("Accept", "application/json")
        .json(&body);
    if !key.is_empty() {
        request = request.bearer_auth(key);
    }
    let response = request.send().context("Không gửi được yêu cầu tới AI")?;
    let status = response.status();
    let raw = response.text().context("Không đọc được phản hồi AI")?;
    if !status.is_success() {
        let detail: String = raw.chars().take(500).collect();
        bail!("AI trả về lỗi {status}: {detail}");
    }
    let text = parse_ai_response(&raw, &provider)?;
    ensure!(!text.is_empty(), "AI trả về bản làm mịn rỗng");
    Ok(text)
}

const SUMMARY_SYSTEM: &str = "Bạn là trợ lý tóm tắt cuộc họp. Chỉ dùng thông tin có trong bản chép được cung cấp. Tuyệt đối không thêm ý ngoài, không suy đoán, không bịa nội dung. Nếu bản chép không đề cập điều gì thì ghi \"Không đề cập\". Giữ nguyên tên riêng, con số và mốc thời gian. Trả về bằng đúng ngôn ngữ gốc của bản chép, không dịch. Kết quả trình bày dạng Markdown với các đề mục.";

fn summary_prompt(transcript: &str, style: &str, additional: &str) -> String {
    let focus = match style.trim() {
        "decisions" => "Tập trung vào QUYẾT ĐỊNH và VIỆC CẦN LÀM. Khi có việc cần làm, chỉ ghi người phụ trách/thời hạn nếu bản chép nói rõ.",
        "full" => "Cho bản tóm tắt đầy đủ hơn nhưng vẫn ngắn gọn và có cấu trúc.",
        _ => "Cho bản tóm tắt ngắn gọn 3–6 gạch đầu dòng.",
    };
    let mut prompt = format!(
        "BẢN CHÉP CUỘC HỌP:\n{transcript}\n\nYÊU CẦU: {focus}"
    );
    let additional = additional.trim();
    if !additional.is_empty() {
        prompt.push_str(&format!("\n\nLƯU Ý THÊM CỦA NGƯỜI DÙNG: {additional}"));
    }
    prompt.push_str(
        "\n\nTrả về Markdown:\n## Tóm tắt ngắn\n...\n## Quyết định\n...\n## Việc cần làm\n...\n## Rủi ro / câu hỏi mở\n...\nKhông thêm ghi chú ngoài bản chép.",
    );
    prompt
}

pub fn summarize(
    provider: &str,
    model: &str,
    base_url: &str,
    api_key: &str,
    transcript: &str,
    style: &str,
    additional: &str,
) -> Result<String> {
    let provider = provider.trim().to_ascii_lowercase();
    let model = if model.trim().is_empty() {
        default_model(&provider)?
    } else {
        model.trim()
    };
    ensure!(!model.trim().is_empty(), "Cần nhập model AI");
    ensure!(!transcript.trim().is_empty(), "Bản chép lời đang trống");
    ensure!(
        transcript.chars().count() <= 160_000,
        "Bản chép quá dài để tóm tắt. Hãy chia nhỏ cuộc họp."
    );
    ensure!(
        additional.chars().count() <= 4_000,
        "Yêu cầu thêm quá dài (tối đa 4.000 ký tự)"
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .context("Không tạo được HTTP client")?;
    let prompt = summary_prompt(transcript, style, additional);

    let (url, body, key) = match provider.as_str() {
        "openai" | "deepseek" | "openai-compatible" => {
            ensure!(
                !api_key.trim().is_empty(),
                "Cần nhập API key cho nhà cung cấp này"
            );
            let url = chat_endpoint(&provider, base_url)?;
            let body = json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": SUMMARY_SYSTEM},
                    {"role": "user", "content": prompt}
                ],
                "temperature": 0.2,
                "max_tokens": 1600,
                "stream": false
            });
            (url, body, api_key.trim().to_string())
        }
        "gemini" => {
            ensure!(!api_key.trim().is_empty(), "Cần nhập API key Gemini");
            let url = gemini_endpoint(model, api_key)?;
            let body = json!({
                "system_instruction": {"parts": [{"text": SUMMARY_SYSTEM}]},
                "contents": [{"role": "user", "parts": [{"text": prompt}]}],
                "generationConfig": {"temperature": 0.2, "maxOutputTokens": 1600}
            });
            (url, body, String::new())
        }
        _ => bail!("Nhà cung cấp AI không hợp lệ"),
    };

    let mut request = client
        .post(&url)
        .header("Accept", "application/json")
        .json(&body);
    if !key.is_empty() {
        request = request.bearer_auth(key);
    }
    let response = request.send().context("Không gửi được yêu cầu tới AI")?;
    let status = response.status();
    let raw = response.text().context("Không đọc được phản hồi AI")?;
    if !status.is_success() {
        let detail: String = raw.chars().take(500).collect();
        bail!("AI trả về lỗi {status}: {detail}");
    }
    let text = parse_ai_response(&raw, &provider)?;
    ensure!(!text.is_empty(), "AI trả về bản tóm tắt rỗng");
    Ok(text)
}

pub fn test_connection(
    provider: &str,
    model: &str,
    base_url: &str,
    api_key: &str,
) -> Result<()> {
    let reply = smooth(
        provider,
        model,
        base_url,
        api_key,
        "Kiểm tra kết nối.",
        "Chỉ trả lời một từ: OK",
    )?;
    ensure!(!reply.trim().is_empty(), "AI không trả về nội dung kiểm tra");
    Ok(())
}

pub fn fetch_models(provider: &str, base_url: &str, api_key: &str) -> Result<Vec<String>> {
    let provider = provider.trim().to_ascii_lowercase();
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .context("Không tạo được HTTP client")?;
    let (url, key) = match provider.as_str() {
        "openai" | "deepseek" | "openai-compatible" => {
            ensure!(
                !api_key.trim().is_empty(),
                "Cần nhập API key cho nhà cung cấp này"
            );
            (models_endpoint(&provider, base_url)?, api_key.trim())
        }
        "gemini" => {
            ensure!(!api_key.trim().is_empty(), "Cần nhập API key Gemini");
            (
                format!(
                    "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                    api_key.trim()
                ),
                "",
            )
        }
        _ => bail!("Nhà cung cấp AI không hợp lệ"),
    };
    let mut request = client.get(&url);
    if !key.is_empty() {
        request = request.bearer_auth(key);
    }
    let response = request.send().context("Không gửi được yêu cầu lấy model")?;
    let status = response.status();
    let raw = response.text().context("Không đọc được danh sách model")?;
    if !status.is_success() {
        let detail: String = raw.chars().take(300).collect();
        bail!("API trả về lỗi {status}: {detail}");
    }
    let value: Value = serde_json::from_str(&raw).context("API trả về JSON không hợp lệ")?;
    let mut models = Vec::new();
    if provider == "gemini" {
        if let Some(items) = value["models"].as_array() {
            for item in items {
                if let Some(name) = item["name"].as_str() {
                    let supported = item["supportedGenerationMethods"]
                        .as_array()
                        .map(|methods| methods.iter().any(|m| m.as_str() == Some("generateContent")))
                        .unwrap_or(true);
                    if supported {
                        models.push(name.strip_prefix("models/").unwrap_or(name).to_string());
                    }
                }
            }
        }
    } else if let Some(items) = value["data"].as_array() {
        models.extend(
            items
                .iter()
                .filter_map(|item| item["id"].as_str().map(str::to_string)),
        );
    }
    models.sort();
    models.dedup();
    ensure!(!models.is_empty(), "API không trả về model nào");
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_defaults_and_endpoints_are_safe() {
        assert_eq!(default_model("openai").unwrap(), "gpt-4o-mini");
        assert_eq!(default_model("deepseek").unwrap(), "deepseek-chat");
        assert_eq!(default_model("gemini").unwrap(), "gemini-2.5-flash");
        assert!(default_model("unknown").is_err());
        assert_eq!(
            chat_endpoint("openai", "").unwrap(),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_endpoint("deepseek", "https://api.deepseek.com/").unwrap(),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            chat_endpoint("openai-compatible", "http://127.0.0.1:11434/v1").unwrap(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert!(chat_endpoint("openai-compatible", "").is_err());
        assert!(chat_endpoint("openai", "file:///etc/passwd").is_err());
    }

    #[test]
    fn prompt_keeps_original_and_adds_optional_request() {
        let normal = user_prompt("Nội dung gốc", "");
        assert!(normal.contains("Nội dung gốc"));
        assert!(!normal.contains("YÊU CẦU BỔ SUNG"));
        let custom = user_prompt("Nội dung gốc", "Thêm dấu câu");
        assert!(custom.contains("YÊU CẦU BỔ SUNG"));
        assert!(custom.contains("Thêm dấu câu"));
    }

    #[test]
    fn response_cleanup_removes_code_fence_but_keeps_text() {
        assert_eq!(clean_response("```text\nXin chào\n```"), "Xin chào");
        assert_eq!(clean_response("  Xin chào  "), "Xin chào");
    }

    #[test]
    fn parse_ai_response_handles_sse_and_bom_and_json() {
        let json_ok = r#"{"choices":[{"message":{"content":"Bản làm mịn."}}]}"#;
        assert_eq!(
            parse_ai_response(json_ok, "openai").unwrap(),
            "Bản làm mịn."
        );

        let sse_stream = "data: {\"choices\":[{\"delta\":{\"content\":\"Hôm \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"nay.\"}}]}\n\ndata: [DONE]\n";
        assert_eq!(
            parse_ai_response(sse_stream, "openai-compatible").unwrap(),
            "Hôm nay."
        );

        let bom_json = format!("\u{feff}{json_ok}");
        assert_eq!(
            parse_ai_response(&bom_json, "openai").unwrap(),
            "Bản làm mịn."
        );

        let err_json = r#"{"error":{"message":"API key không hợp lệ"}}"#;
        assert!(parse_ai_response(err_json, "openai").is_err());
    }

    #[test]
    fn openai_compatible_roundtrip_returns_text() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 8192];
            let _ = stream.read(&mut request);
            let body = r#"{"choices":[{"message":{"content":"Xin chào"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        let result = smooth(
            "openai-compatible",
            "test-model",
            &format!("http://{address}/v1"),
            "test-key",
            "Nội dung gốc",
            "",
        )
        .unwrap();
        assert_eq!(result, "Xin chào");
        server.join().unwrap();
    }

    #[test]
    fn openai_compatible_model_list_roundtrip() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 8192];
            let _ = stream.read(&mut request);
            let body = r#"{"data":[{"id":"model-b"},{"id":"model-a"},{"id":"model-a"}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        let models = fetch_models(
            "openai-compatible",
            &format!("http://{address}/v1"),
            "test-key",
        )
        .unwrap();
        assert_eq!(models, vec!["model-a", "model-b"]);
        server.join().unwrap();
    }
}