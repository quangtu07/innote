//! Processing parameters only: never selects a provider, model, key, or paid tier.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechQuality { Fast, #[default] Balanced, Accurate }
impl SpeechQuality {
    pub fn as_str(self) -> &'static str { match self { Self::Fast=>"fast",Self::Balanced=>"balanced",Self::Accurate=>"accurate" } }
    pub fn parse(value: Option<&str>) -> anyhow::Result<Self> {
        match value.unwrap_or("balanced") { "fast"=>Ok(Self::Fast),"balanced"=>Ok(Self::Balanced),"accurate"=>Ok(Self::Accurate),_=>anyhow::bail!("Mức xử lý không hợp lệ") }
    }
    pub fn supported(provider: &str) -> bool { matches!(provider,"local"|"soniox"|"groq") }
    pub fn soniox_endpoint_ms(self) -> u32 { match self {Self::Fast=>500,Self::Balanced=>750,Self::Accurate=>1200} }
    pub fn groq_chunk_samples(self) -> usize { match self {Self::Fast=>51200,Self::Balanced=>51200,Self::Accurate=>64000} }
    pub fn groq_overlap_samples(self) -> usize { match self {Self::Fast=>48000,Self::Balanced=>80000,Self::Accurate=>96000} }
    pub fn moonshine_interval(self) -> &'static str { "0.08" }
    pub fn moonshine_floor(self) -> usize { 1280 }
    pub fn moonshine_vad_window(self) -> &'static str { match self {Self::Fast=>"0.25",Self::Balanced=>"0.5",Self::Accurate=>"0.5"} }
    pub fn moonshine_lookbehind(self) -> &'static str { if self==Self::Accurate {"16384"} else {"8192"} }
    pub fn moonshine_segment(self) -> &'static str { match self {Self::Fast=>"10",Self::Balanced=>"15",Self::Accurate=>"25"} }
    pub fn nemo_endpoint_ms(self) -> i32 { match self {Self::Fast=>250,Self::Balanced=>300,Self::Accurate=>550} }
    pub fn boundary_seconds(self,multi:bool) -> usize { match (self,multi) {(Self::Fast,false)=>10,(Self::Fast,true)=>20,(Self::Balanced,false)=>20,(Self::Balanced,true)=>30,(Self::Accurate,false)=>30,(Self::Accurate,true)=>40} }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn profiles_keep_balanced_defaults_and_respect_provider_limits() {
        assert_eq!(SpeechQuality::parse(None).unwrap(),SpeechQuality::Balanced);
        assert!(SpeechQuality::parse(Some("turbo")).is_err());
        for q in [SpeechQuality::Fast,SpeechQuality::Balanced,SpeechQuality::Accurate] {
            assert_eq!(serde_json::from_str::<SpeechQuality>(&format!("\"{}\"",q.as_str())).unwrap(),q);
            assert!((500..=3000).contains(&q.soniox_endpoint_ms()));
            assert!(q.groq_chunk_samples()>=51200); // Request pacing remains >=3.2 s.
        }
        assert_eq!(SpeechQuality::Balanced.groq_chunk_samples(),51200);
        assert_eq!(SpeechQuality::Balanced.nemo_endpoint_ms(),300);
        assert!(SpeechQuality::Fast.moonshine_vad_window()!=SpeechQuality::Accurate.moonshine_vad_window());
        assert!(!SpeechQuality::supported("gemini"));
    }
}

/// Split only before a new confirmed word, never inside a provider subword.
/// Whitespace units are intentionally language-neutral (not a lexical tokenizer).
/// Translation retains its own aligned endpoint rather than this boundary.
pub fn confirmed_chunk_boundary(confirmed: &str, next: &str) -> bool {
    confirmed.split_whitespace().take(6).count() == 6
        && next.starts_with(char::is_whitespace)
        && next.trim_start().chars().next().is_some_and(char::is_alphanumeric)
}
#[cfg(test)] mod chunk_tests {
    use super::*;
    #[test] fn only_split_between_confirmed_words() {
        assert!(confirmed_chunk_boundary("one two three four five six", " seven"));
        assert!(!confirmed_chunk_boundary("one two three four five six", "teen"));
        assert!(!confirmed_chunk_boundary("one two three four five six", ","));
        assert!(!confirmed_chunk_boundary("one two three four five six", " !"));
        assert!(!confirmed_chunk_boundary("one two three four five", " six"));
        assert!(!confirmed_chunk_boundary("你好世界", "继续"));
        assert!(confirmed_chunk_boundary("ngân sách không phải năm mươi", " triệu"));
    }
}
