//! One owner for a meeting's Soniox recognition and optional translation.
//! Reads the existing speaker/microphone archives; never starts capture or local ASR.
use crate::{caption, live::PcmTail, types::*, May, Run};
use anyhow::{ensure, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::Emitter;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};

#[derive(Default)]
pub(crate) struct Control {
    pub active: AtomicBool,
    pub enabled: AtomicBool,
    pub transcribing: AtomicBool,
    target: Mutex<Option<String>>,
    generation: AtomicU64,
    preview: Mutex<Option<TranslationPreview>>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TranslationPreview {
    pub id: String,
    pub language: String,
    pub text: String,
    pub committed: usize,
}
impl Control {
    pub fn preview(&self) -> Option<TranslationPreview> {
        self.preview.lock().unwrap().clone()
    }
    pub fn target(&self) -> Option<String> {
        self.target.lock().unwrap().clone()
    }
    pub fn set_target(&self, target: Option<String>) {
        let mut old = self.target.lock().unwrap();
        if *old != target {
            *old = target;
            *self.preview.lock().unwrap() = None;
            self.generation.fetch_add(1, Ordering::Release);
        }
    }
}

/// Wire samples omit pauses; retain an exact mapping to the archived audio clock.
#[derive(Default)]
struct Timeline {
    ranges: Vec<(usize, usize, usize)>,
    sent: usize,
}
impl Timeline {
    fn append(&mut self, archive: usize, len: usize) {
        if let Some((wire, source, count)) = self.ranges.last_mut() {
            if *source + *count == archive {
                *count += len;
                self.sent += len;
                return;
            }
            debug_assert_eq!(*wire + *count, self.sent);
        }
        self.ranges.push((self.sent, archive, len));
        self.sent += len;
    }
    fn time(&self, ms: f64, end: bool) -> f64 {
        let at = (ms.max(0.) * 16.).round() as usize;
        for &(wire, archive, len) in &self.ranges {
            if at >= wire && (at < wire + len || (end && at == wire + len)) {
                return (archive + (at - wire).min(len)) as f64 / 16000.;
            }
        }
        self.ranges
            .last()
            .map(|(_, a, n)| (a + n) as f64 / 16000.)
            .unwrap_or(0.)
    }
}

#[derive(Default)]
struct Parser {
    original: String,
    translation: String,
    start: Option<f64>,
    end: f64,
    same_language: bool,
    has_interim: bool,
    waiting: VecDeque<Segment>,
}
#[derive(Default)]
struct Batch {
    originals: Vec<Segment>,
    translations: Vec<TranslationSegment>,
    partial: String,
    live_translation: String,
    live_original: String,
    end: f64,
}
impl Parser {
    fn flush_final(&mut self, batch: &mut Batch, target: Option<&str>) {
        self.source(batch, target);
        self.translated_end(batch, target);
    }
    fn source(&mut self, batch: &mut Batch, target: Option<&str>) {
        if self.original.trim().is_empty() {
            return;
        }
        let segment = Segment {
            id: uuid::Uuid::new_v4().to_string(),
            start: self.start.take().unwrap_or(self.end),
            end: self.end,
            text: std::mem::take(&mut self.original).trim().into(),
            edited: false,
        };
        batch.originals.push(segment.clone());
        if target.is_some() && self.same_language {
            batch.translations.push(Self::translated(
                &segment,
                segment.text.clone(),
                target.unwrap(),
            ));
        } else if target.is_some() {
            self.waiting.push_back(segment);
        }
        self.same_language = false;
    }
    fn translated(source: &Segment, text: String, language: &str) -> TranslationSegment {
        TranslationSegment {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: source.id.clone(),
            source_ids: if source.id.is_empty() {
                vec![]
            } else {
                vec![source.id.clone()]
            },
            start: source.start,
            end: source.end,
            original: source.text.clone(),
            text,
            language: language.into(),
            provider: "soniox".into(),
        }
    }
    fn translated_end(&mut self, batch: &mut Batch, target: Option<&str>) {
        if self.waiting.is_empty() {
            self.source(batch, target);
        }
        if self.translation.trim().is_empty() {
            return;
        }
        if let Some(language) = target {
            let sources: Vec<_> = self.waiting.drain(..).collect();
            let source = Segment {
                id: sources.first().map(|s| s.id.clone()).unwrap_or_default(),
                start: sources.first().map(|s| s.start).unwrap_or(0.),
                end: sources.last().map(|s| s.end).unwrap_or(0.),
                text: sources
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                edited: false,
            };
            let mut translation = Self::translated(
                &source,
                std::mem::take(&mut self.translation).trim().into(),
                language,
            );
            translation.source_ids = sources.iter().map(|s| s.id.clone()).collect();
            batch.translations.push(translation);
        }
    }
    fn push(&mut self, value: &Value, timeline: &Timeline, target: Option<&str>) -> Result<Batch> {
        if value.get("error_code").is_some() || value.get("error_type").is_some() {
            anyhow::bail!(crate::soniox::describe_error(
                value["error_code"].as_u64().unwrap_or(500)
            ));
        }
        ensure!(
            value["tokens"].is_array() || value["finished"] == true,
            "Soniox trả dữ liệu không hợp lệ"
        );
        let mut batch = Batch::default();
        let mut interim = String::new();
        let mut interim_translation = String::new();
        for token in value["tokens"].as_array().into_iter().flatten() {
            let word = token["text"]
                .as_str()
                .context("Soniox thiếu nội dung token")?;
            let translated = token["translation_status"] == "translation";
            if matches!(word, "<end>" | "<fin>") {
                if token["is_final"] == true {
                    // Endpoint markers may omit translation_status. If target
                    // text is already buffered, this closes that paired chunk;
                    // otherwise originals would be stored but no live pair
                    // would be published until the stream itself is closed.
                    if translated
                        || ((token["translation_status"].is_null()
                            || token["translation_status"] == "none")
                            && !self.translation.trim().is_empty())
                    {
                        self.translated_end(&mut batch, target);
                    } else {
                        self.source(&mut batch, target);
                    }
                }
                continue;
            }
            if translated {
                if token["is_final"] == true {
                    self.translation.push_str(word);
                } else {
                    interim_translation.push_str(word);
                }
            } else {
                let start = token["start_ms"].as_f64().map(|t| timeline.time(t, false));
                let end = token["end_ms"]
                    .as_f64()
                    .map(|t| timeline.time(t, true))
                    .unwrap_or(self.end);
                ensure!(
                    end.is_finite() && start.is_none_or(|s| s.is_finite() && s <= end),
                    "Soniox trả mốc thời gian không hợp lệ"
                );
                batch.end = batch.end.max(end);
                if token["is_final"] == true {
                    // Source-only can commit confirmed short chunks without
                    // waiting for an utterance endpoint. Keep translated source
                    // intact so bilingual pairs retain the provider's alignment.
                    if target.is_none()
                        && crate::speech_quality::confirmed_chunk_boundary(&self.original, word)
                    {
                        self.source(&mut batch, target);
                    }
                    if self.start.is_none() {
                        self.start = start;
                    }
                    self.end = self.end.max(end);
                    // `none` means untranslated, not necessarily target-language
                    // speech. Only copy a complete, identified target-language span.
                    let same = token["translation_status"] == "none"
                        && target.is_some_and(|lang| token["language"] == lang);
                    self.same_language = if self.original.trim().is_empty() {
                        same
                    } else {
                        self.same_language && (word.trim().is_empty() || same)
                    };
                    self.original.push_str(word);
                } else {
                    interim.push_str(word);
                }
            }
        }
        if value["finished"] == true {
            self.flush_final(&mut batch, target);
        }
        ensure!(
            self.original.len() + self.translation.len() < 128_000 && self.waiting.len() < 256,
            "Soniox thiếu ranh giới câu quá lâu"
        );
        batch.partial = format!("{}{}", self.original, interim);
        self.has_interim = !interim.trim().is_empty();
        batch.live_translation = format!("{}{}", self.translation, interim_translation);
        batch.live_original = self
            .waiting
            .front()
            .map(|s| s.text.clone())
            .unwrap_or_else(|| batch.partial.clone());
        batch.end = batch.end.max(self.end);
        Ok(batch)
    }
}

fn commit(
    may: &May,
    run: &Run,
    app: &tauri::AppHandle,
    batch: Batch,
    render: bool,
    language: Option<&str>,
) -> Result<()> {
    let mut committed = if let Some(preview) = run.speech.preview().filter(|p| Some(p.language.as_str()) == language) {
        preview.committed
    } else {
        crate::storage::load(&may.root, &run.id)?.translations.iter().filter(|t| Some(t.language.as_str()) == language).count()
    };
    if !batch.originals.is_empty() || !batch.translations.is_empty() {
        let saved = may.edit(&run.id, |s| {
            for segment in &batch.originals {
                if !s.segments.iter().any(|x| x.id == segment.id) {
                    s.segments.push(segment.clone());
                    s.processed = s.processed.max(segment.end);
                }
            }
            for segment in &batch.translations {
                if !s.translations.iter().any(|x| x.id == segment.id) {
                    s.translations.push(segment.clone());
                }
            }
        })?;
        committed = saved.translations.iter().filter(|t| Some(t.language.as_str()) == language).count();
        let _ = app.emit("session-changed", run.id.clone());
    }
    if run.speech.target().as_deref() == language {
        let preview = language.map(|language| TranslationPreview {
            id: run.id.clone(), language: language.into(),
            text: batch.live_translation.clone(), committed,
        });
        *run.speech.preview.lock().unwrap() = preview.clone();
        let _ = app.emit("meeting-translation-preview", preview);
    }
    let _ = app.emit("live-transcript", json!({"id":run.id,"update":{"text":batch.partial,"start":0.,"end":batch.end+0.001,"final_text":false,"words":[]}}));
    if render {
        caption::set_translation_mode(app, language.is_some());
    }
    if render && language.is_none() {
        for original in &batch.originals {
            caption::emit_text(
                app,
                caption::CaptionText {
                    text: original.text.clone(),
                    original: String::new(),
                    final_: true,
                },
            );
        }
        if !batch.partial.trim().is_empty() {
            caption::emit_text(
                app,
                caption::CaptionText {
                    text: batch.partial.clone(),
                    original: String::new(),
                    final_: false,
                },
            );
        }
    }
    if render && language.is_some() {
        for translation in batch.translations {
            caption::emit_text(
                app,
                caption::CaptionText {
                    text: translation.text,
                    original: translation.original,
                    final_: true,
                },
            );
        }
        if !batch.live_translation.trim().is_empty() || !batch.live_original.trim().is_empty() {
            caption::emit_text(
                app,
                caption::CaptionText {
                    text: batch.live_translation,
                    original: batch.live_original,
                    final_: false,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn configuration(language: &str, target: Option<&str>, quality: crate::speech_quality::SpeechQuality) -> Value {
    let mut config = json!({"model":"stt-rt-v5","audio_format":"pcm_s16le","sample_rate":16000,"num_channels":1,"enable_endpoint_detection":true,"enable_language_identification":true,"max_endpoint_delay_ms":quality.soniox_endpoint_ms()});
    if ["vi", "en", "ko", "zh", "ja"].contains(&language) {
        config["language_hints"] = json!([language]);
    }
    if let Some(target) = target {
        config["translation"] = json!({"type":"one_way","target_language":target});
    }
    config
}

pub(crate) fn run(
    may: &May,
    run: &Run,
    app: &tauri::AppHandle,
    dir: &Path,
    key: &str,
    language: &str,
) -> Result<()> {
    run_from(may, run, app, dir, key, language, 0)
}
pub(crate) fn run_from(
    may: &May,
    run: &Run,
    app: &tauri::AppHandle,
    dir: &Path,
    key: &str,
    language: &str,
    start: usize,
) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let mut pcm = PcmTail::new(dir); let mut cursor = start;
        while !run.cancel.load(Ordering::Acquire) {
            let generation = run.speech.generation.load(Ordering::Acquire);
            let target = run.speech.target();
            let mut request = "wss://stt-rt.soniox.com/transcribe-websocket".into_client_request()?;
            let mut auth = HeaderValue::from_str(&format!("Bearer {key}")).context("API key không hợp lệ")?;
            auth.set_sensitive(true); request.headers_mut().insert("Authorization", auth);
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(12), connect_async_with_config(request,None,true)).await.context("Kết nối Soniox quá lâu")?.map_err(crate::soniox::websocket_error)?;
            let quality = crate::storage::load(&may.root,&run.id)?.speech_quality;
            let config=configuration(language,target.as_deref(),quality);
            tokio::time::timeout(Duration::from_secs(5),socket.send(Message::Text(config.to_string()))).await.context("Gửi cấu hình Soniox quá lâu")??;
            may.edit(&run.id, |s| { if let Some(info)=s.speech.as_mut(){info.connections+=1;info.translation_language=run.speech.target();} })?;
            let mut timeline = Timeline::default(); let mut parser = Parser::default();
            let mut ticker=tokio::time::interval(Duration::from_millis(50)); ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut finishing: Option<Instant> = None; let mut keepalive=Instant::now(); let mut announced=false; let mut pause_finalized=false; let mut switch_requested:Option<Instant>=None;
            let render = |run:&Run| run.speech.enabled.load(Ordering::Acquire) && run.speech.generation.load(Ordering::Acquire)==generation;
            let result: Result<()> = async {
            let mut result = Ok(());
            loop {
                if run.cancel.load(Ordering::Acquire) { break; }
                tokio::select! {
                    _ = ticker.tick() => {
                        let changing = run.speech.generation.load(Ordering::Acquire)!=generation;
                        if changing && switch_requested.is_none() { switch_requested=Some(Instant::now()); }
                        let checkpoint=parser.original.trim().is_empty() && !parser.has_interim && parser.waiting.is_empty() && parser.translation.trim().is_empty();
                        let available = pcm.available()?;
                        let capture_done = run.capture_done.load(Ordering::Acquire);
                        if finishing.is_none() && ((!capture_done && changing && (checkpoint || switch_requested.is_some_and(|t|t.elapsed()>Duration::from_secs(2)))) || (capture_done && cursor>=available)) {
                            tokio::time::timeout(Duration::from_secs(5),socket.send(Message::Text(String::new()))).await.context("Kết thúc Soniox quá lâu")??;
                            finishing=Some(Instant::now());
                        }
                        if let Some(since)=finishing { if since.elapsed()>Duration::from_secs(4){result=Err(anyhow::anyhow!("Soniox chưa xác nhận hoàn tất phần cuối; bản thu vẫn được giữ"));break;} continue; }
                        let paused=run.paused.load(Ordering::Acquire) && !capture_done;
                        {
                            let available=if capture_done||paused{available}else{available.saturating_sub(1600)};
                            if available>cursor {
                                let end=(cursor+3200).min(available); let samples=pcm.read(cursor,end)?;
                                let bytes:Vec<u8>=samples.iter().flat_map(|s| ((s.clamp(-1.,1.)*32767.).round() as i16).to_le_bytes()).collect();
                                tokio::time::timeout(Duration::from_secs(5),socket.send(Message::Binary(bytes))).await.context("Gửi audio Soniox quá lâu")??;
                                timeline.append(cursor,end-cursor); cursor=end; keepalive=Instant::now();
                                if render(run) && !announced { caption::emit_status(app,"soniox","sending",true,if target.is_some(){"Soniox đang chép lời và dịch trên cùng luồng âm thanh."}else{"Soniox đang chép lời và hiển thị phụ đề gốc, không yêu cầu dịch."}); announced=true; }
                            } else if paused {
                                // Flush the pre-pause tail first; only then skip the archive hole.
                                if !pause_finalized {
                                    tokio::time::timeout(Duration::from_secs(5),socket.send(Message::Text(json!({"type":"finalize"}).to_string()))).await.context("Chốt câu Soniox quá lâu")??;
                                    pause_finalized=true;
                                }
                                cursor=cursor.max((crate::audio::clock_hns().saturating_sub(run.epoch) as f64 / 625.) as usize);
                            }
                        }
                        if !paused { pause_finalized=false; }
                        if keepalive.elapsed()>Duration::from_secs(8) {
                            tokio::time::timeout(Duration::from_secs(5),socket.send(Message::Text(json!({"type":"keepalive"}).to_string()))).await.context("Giữ kết nối Soniox quá lâu")??;
                            keepalive=Instant::now();
                        }
                    }
                    message=socket.next()=> {
                        let value=match message {
                            Some(Ok(Message::Text(text)))=>serde_json::from_str::<Value>(&text).context("Soniox trả JSON không hợp lệ")?,
                            Some(Ok(Message::Ping(_)))=>{socket.flush().await?;continue;},
                            Some(Ok(Message::Pong(_)))=>continue,
                            Some(Ok(Message::Close(_)))|None=>{if finishing.is_none(){result=Err(anyhow::anyhow!("Soniox đóng phiên trước khi kết thúc audio"));}break;},
                            Some(Err(_))=>{result=Err(anyhow::anyhow!("Kết nối Soniox bị gián đoạn"));break;},
                            _=>continue,
                        };
                        let batch=match parser.push(&value,&timeline,target.as_deref()) {Ok(batch)=>batch,Err(error)=>{result=Err(error);break;}};
                        if std::env::var("INNOTE_SONIOX_AUDIT").as_deref()==Ok("1"){
                            let tokens=value["tokens"].as_array();let _=app.emit("soniox-pair-audit",json!({"render":render(run),"enabled":run.speech.enabled.load(Ordering::Acquire),"generation":run.speech.generation.load(Ordering::Acquire),"epoch":generation,"markers":tokens.into_iter().flatten().filter(|t|matches!(t["text"].as_str(),Some("<end>"|"<fin>"))).collect::<Vec<_>>(),"translated_final_tokens":tokens.into_iter().flatten().filter(|t|t["translation_status"]=="translation"&&t["is_final"]==true).count(),"translated_interim_tokens":tokens.into_iter().flatten().filter(|t|t["translation_status"]=="translation"&&t["is_final"]!=true).count(),"pending_translation_chars":parser.translation.len(),"waiting_sources":parser.waiting.len(),"batch_pairs":batch.translations.len(),"finished":value["finished"]}));
                        }
                        commit(may,run,app,batch,render(run),target.as_deref())?;
                        if value["finished"]==true { if finishing.is_none(){result=Err(anyhow::anyhow!("Soniox kết thúc phiên ngoài dự kiến"));} break; }
                    }
                }
            }
            result
            }.await;
            // Flush provider-final tokens on every exit, including send/read timeouts.
            // Interim tokens are never persisted as if they were confirmed speech.
            let mut tail=Batch::default(); parser.flush_final(&mut tail,target.as_deref());
            commit(may,run,app,tail,false,target.as_deref())?;
            may.edit(&run.id, |s| { if let Some(info)=s.speech.as_mut(){info.uploaded_samples+=timeline.sent as u64;} })?;
            let _=tokio::time::timeout(Duration::from_millis(300),socket.close(None)).await;
            result?;
            if run.cancel.load(Ordering::Acquire) || (run.capture_done.load(Ordering::Acquire) && cursor>=pcm.available()?) { break; }
            // Reconfigure only by draining then opening a new epoch; no audio is replayed.
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_confirmed_chunks_keep_timestamps_tail_and_translation_alignment() {
        let mut timeline = Timeline::default();
        timeline.append(0, 160000);
        let words = ["one", " two", " three", " four", " five", " six", "teen", ",", " seven", " eight"];
        let tokens: Vec<_> = words.iter().enumerate().map(|(i, text)| json!({
            "text":text,"is_final":true,"translation_status":"original",
            "start_ms":i*100,"end_ms":(i+1)*100
        })).collect();
        let mut parser = Parser::default();
        let batch = parser.push(&json!({"tokens":tokens}), &timeline, None).unwrap();
        assert_eq!(batch.originals.len(), 1);
        assert_eq!(batch.originals[0].text, "one two three four five sixteen,");
        assert_eq!(batch.originals[0].start, 0.);
        assert_eq!(batch.originals[0].end, 0.8);
        let tail = parser.push(&json!({"finished":true}), &timeline, None).unwrap();
        assert_eq!(tail.originals[0].text, "seven eight");
        assert_eq!(tail.originals[0].start, 0.8);
        assert_eq!(tail.originals[0].end, 1.);
        assert!(parser.push(&json!({"finished":true}), &timeline, None).unwrap().originals.is_empty());
        // The same tokens must stay one complete aligned source when translating.
        let mut translated = Parser::default();
        assert!(translated.push(&json!({"tokens":tokens}), &timeline, Some("vi")).unwrap().originals.is_empty());
        let pair = translated.push(&json!({"tokens":[
            {"text":"<end>","is_final":true,"translation_status":"original"},
            {"text":"Một bản dịch đủ câu.","is_final":true,"translation_status":"translation"},
            {"text":"<end>","is_final":true,"translation_status":"translation"}
        ]}), &timeline, Some("vi")).unwrap();
        assert_eq!(pair.translations[0].original, "one two three four five sixteen, seven eight");
        assert_eq!(pair.translations[0].source_ids, vec![pair.originals[0].id.clone()]);
    }
    #[test]
    fn timeline_retains_pause_and_cutover_offsets() {
        let mut t = Timeline::default();
        t.append(0, 16000);
        t.append(48000, 16000);
        assert_eq!(t.time(1000., false), 3.);
        assert_eq!(t.time(1000., true), 1.);
        assert_eq!(t.time(1500., false), 3.5);
    }
    #[test]
    fn original_commits_before_delayed_translation_without_losing_source_time() {
        let mut t = Timeline::default();
        t.append(32000, 32000);
        let mut p = Parser::default();
        let b=p.push(&json!({"tokens":[{"text":"Not 50 million.","is_final":true,"translation_status":"original","start_ms":100,"end_ms":900},{"text":"<end>","is_final":true,"translation_status":"original"}]}),&t,Some("vi")).unwrap();
        assert_eq!(b.originals.len(), 1);
        assert_eq!(b.originals[0].start, 2.1);
        assert!(b.translations.is_empty());
        let id = b.originals[0].id.clone();
        let b=p.push(&json!({"tokens":[{"text":"Không phải 50 triệu.","is_final":true,"translation_status":"translation"},{"text":"<end>","is_final":true,"translation_status":"translation"}]}),&t,Some("vi")).unwrap();
        assert_eq!(b.translations[0].source_id, id);
        assert_eq!(b.translations[0].start, 2.1);
    }
    #[test]
    fn untagged_endpoint_publishes_a_pair_before_stream_finishes() {
        for status in [Value::Null, json!("none")] {
            let mut timeline = Timeline::default();
            timeline.append(0, 32000);
            let mut parser = Parser::default();
            let first=parser.push(&json!({"tokens":[{"text":"Not 50 million.","is_final":true,"translation_status":"original","start_ms":0,"end_ms":900},{"text":"<end>","is_final":true}]}),&timeline,Some("vi")).unwrap();
            assert_eq!(first.originals.len(), 1);
            assert!(first.translations.is_empty());
            let second=parser.push(&json!({"tokens":[{"text":"Không phải 50 triệu.","translation_status":"translation","is_final":true},{"text":"<end>","is_final":true,"translation_status":status}]}),&timeline,Some("vi")).unwrap();
            assert_eq!(second.translations.len(), 1);
            assert_eq!(second.translations[0].source_id, first.originals[0].id);
            assert_eq!(second.translations[0].original, "Not 50 million.");
            assert_eq!(second.translations[0].text, "Không phải 50 triệu.");
            let mut tail = Batch::default();
            parser.flush_final(&mut tail, Some("vi"));
            assert!(tail.translations.is_empty());
        }
    }
    #[test]
    fn transcription_only_never_creates_translation_and_ignores_interim() {
        let mut t = Timeline::default();
        t.append(0, 32000);
        let mut p = Parser::default();
        let b = p
            .push(
                &json!({"tokens":[{"text":"wrong","is_final":false,"start_ms":0,"end_ms":500}]}),
                &t,
                None,
            )
            .unwrap();
        assert!(b.originals.is_empty());
        let b=p.push(&json!({"tokens":[{"text":"Right","is_final":true,"start_ms":0,"end_ms":500}],"finished":true}),&t,None).unwrap();
        assert_eq!(b.originals[0].text, "Right");
        assert!(b.translations.is_empty());
    }
    #[test]
    fn interrupted_epoch_keeps_final_original_and_translation_without_promoting_interim() {
        let mut t = Timeline::default();
        t.append(16000, 32000);
        let mut p = Parser::default();
        p.push(&json!({"tokens":[
            {"text":"One.","is_final":true,"translation_status":"original","start_ms":0,"end_ms":300},
            {"text":"<end>","is_final":true,"translation_status":"original"},
            {"text":" Two.","is_final":true,"translation_status":"original","start_ms":500,"end_ms":800},
            {"text":"Một. Hai.","is_final":true,"translation_status":"translation"},
            {"text":"unconfirmed","is_final":false,"start_ms":900,"end_ms":1000}
        ]}),&t,Some("vi")).unwrap();
        let mut batch = Batch::default();
        p.flush_final(&mut batch, Some("vi"));
        assert_eq!(batch.originals.len(), 1);
        assert_eq!(batch.originals[0].text, "Two.");
        assert_eq!(batch.translations.len(), 1);
        assert_eq!(batch.translations[0].source_ids.len(), 2);
        assert_eq!(batch.translations[0].original, "One. Two.");
        assert_eq!(batch.translations[0].text, "Một. Hai.");
        let mut again = Batch::default();
        p.flush_final(&mut again, Some("vi"));
        assert!(again.originals.is_empty() && again.translations.is_empty());
    }
    #[test]
    fn translation_is_explicit_and_idempotent_control_does_not_restart_stt() {
        assert!(configuration("multi", None, Default::default()).get("translation").is_none());
        assert!(configuration("multi", None, Default::default()).get("language_hints").is_none());
        assert_eq!(
            configuration("en", Some("vi"), Default::default())["translation"],
            json!({"type":"one_way","target_language":"vi"})
        );
        let c = Control::default();
        c.set_target(Some("vi".into()));
        let version = c.generation.load(Ordering::Acquire);
        c.set_target(Some("vi".into()));
        assert_eq!(c.generation.load(Ordering::Acquire), version);
        c.set_target(None);
        assert_eq!(c.target(), None);
    }
    #[test]
    fn untranslated_foreign_tokens_are_not_saved_as_target_language() {
        let mut timeline = Timeline::default();
        timeline.append(0, 32000);
        let mut parser = Parser::default();
        let source = parser.push(&json!({"tokens":[
            {"text":"你好世界。","language":"zh","translation_status":"none","is_final":true,"start_ms":0,"end_ms":1000},
            {"text":"<end>","is_final":true}
        ]}), &timeline, Some("vi")).unwrap();
        assert_eq!(source.originals.len(), 1);
        assert!(source.translations.is_empty());
        let preview = parser.push(&json!({"tokens":[
            {"text":"Xin chào","language":"vi","translation_status":"translation","is_final":false}
        ]}), &timeline, Some("vi")).unwrap();
        assert_eq!(preview.live_translation, "Xin chào");
        assert!(preview.translations.is_empty());
        let final_batch = parser.push(&json!({"tokens":[
            {"text":"Xin chào thế giới.","language":"vi","translation_status":"translation","is_final":true},
            {"text":"<end>","is_final":true}
        ]}), &timeline, Some("vi")).unwrap();
        assert_eq!(final_batch.translations[0].text, "Xin chào thế giới.");
        assert_eq!(final_batch.translations[0].source_id, source.originals[0].id);
        assert!(final_batch.live_translation.is_empty());
    }
    #[test]
    fn already_target_language_and_multiple_endpoints_are_not_lost() {
        let mut t = Timeline::default();
        t.append(0, 32000);
        let mut p = Parser::default();
        let batch=p.push(&json!({"tokens":[{"text":"Một.","language":"vi","is_final":true,"translation_status":"none","start_ms":0,"end_ms":300},{"text":"<end>","is_final":true,"translation_status":"none"},{"text":"Hai.","language":"vi","is_final":true,"translation_status":"none","start_ms":500,"end_ms":800},{"text":"<fin>","is_final":true,"translation_status":"none"}],"finished":true}),&t,Some("vi")).unwrap();
        assert_eq!(batch.originals.len(), 2);
        assert_eq!(batch.translations.len(), 2);
        assert_eq!(batch.translations[1].text, "Hai.");
        assert!(p
            .push(&json!({"finished":true}), &t, Some("vi"))
            .unwrap()
            .originals
            .is_empty());
    }
}
