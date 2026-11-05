//! Groq reads the meeting's existing mixed archives. The overlay consumes the
//! same committed transcript; it never starts a second capture/upload worker.
use crate::{
    groq::{self, AudioChunk, Word},
    live::PcmTail,
    types::*,
    May, Run,
};
use anyhow::{ensure, Result};
use std::{
    path::Path,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tauri::Emitter;
const RATE: usize = 16000;

fn segments(words: Vec<Word>) -> Vec<Segment> {
    let mut result = Vec::new();
    let mut group: Vec<Word> = Vec::new();
    for word in words {
        let sentence = word
            .text
            .trim_end()
            .ends_with(['.', '?', '!', '。', '？', '！']);
        group.push(word);
        if group.len() >= 7 || sentence {
            result.push(segment(&group));
            group.clear();
        }
    }
    if !group.is_empty() {
        result.push(segment(&group));
    }
    result
}
fn segment(words: &[Word]) -> Segment {
    Segment {
        id: uuid::Uuid::new_v4().to_string(),
        start: words[0].start,
        end: words.last().unwrap().end,
        text: words
            .iter()
            .map(|w| w.text.trim())
            .collect::<Vec<_>>()
            .join(" "),
        edited: false,
    }
}

pub(crate) fn run_from(
    may: &May,
    run: &Run,
    app: &tauri::AppHandle,
    dir: &Path,
    profile: &crate::config::AiProfile,
    model: &str,
    language: &str,
    start: usize,
) -> Result<()> {
    ensure!(
        profile.provider == "groq" && groq::is_speech_model(model),
        "Chép lời Groq cần model Whisper đã hỗ trợ"
    );
    let quality = crate::storage::load(&may.root, &run.id)?.speech_quality;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let client=reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?;
        let mut pcm=PcmTail::new(dir);let mut cursor=start;let mut floor=start;
        let mut watermark=start as f64/RATE as f64-0.001;let mut deferred=Vec::new();let mut last_request=None;
        let mut finishing:Option<Instant>=None;let mut tail_flushed=false;
        may.edit(&run.id,|s|{if let Some(info)=s.speech.as_mut(){info.connections+=1;}})?;
        loop {
            if run.cancel.load(Ordering::Acquire){break;}
            let available=pcm.available()?;let done=run.capture_done.load(Ordering::Acquire);
            let paused=run.paused.load(Ordering::Acquire)&&!done;
            if done && finishing.is_none(){finishing=Some(Instant::now());}
            ensure!(!finishing.is_some_and(|t|t.elapsed()>Duration::from_secs(30)),"Groq chưa xử lý hết phần cuối trong 30 giây; bản thu vẫn được giữ");
            let tail=done&&cursor>=available&&!deferred.is_empty()&&!tail_flushed;
            if done&&cursor>=available&&!tail{break;}
            ensure!(available.saturating_sub(cursor)<=30*RATE,"Groq chậm hơn nguồn thu trên 30 giây; bản thu vẫn được giữ, hãy kiểm tra mạng/hạn mức");
            if !tail && (available<=cursor || (available-cursor<quality.groq_chunk_samples()&&!done&&!paused)) {
                if paused&&available<=cursor {
                    // Once the pre-pause tail is flushed, omit the archive's pause
                    // hole and reset overlap. Result timestamps remain absolute.
                    cursor=cursor.max((crate::audio::clock_hns().saturating_sub(run.epoch)/625) as usize);floor=cursor;
                }
                tokio::time::sleep(Duration::from_millis(60)).await;continue;
            }
            let end=if tail{available}else{(cursor+quality.groq_chunk_samples()).min(available)};
            let base=cursor.saturating_sub(quality.groq_overlap_samples()).max(floor).min(end);
            let samples=pcm.read(base,end)?.into_iter().map(|s|(s.clamp(-1.,1.)*32767.).round() as i16).collect();
            let chunk=AudioChunk{samples,base,final_:paused||done&&end==available};
            if chunk.voiced()||!deferred.is_empty(){
                if let Some(last)=last_request{tokio::time::sleep_until(last+Duration::from_millis(3200)).await;}
                if run.cancel.load(Ordering::Acquire){break;}
                if run.speech.enabled.load(Ordering::Acquire){crate::caption::emit_status(app,"groq","sending",true,"Đang gửi một đoạn audio cuộc họp lên Groq; bản chép và phụ đề dùng chung kết quả.");}
                last_request=Some(tokio::time::Instant::now());
                let words=groq::recognize(&client,&profile.key,&profile.base_url,model,Some(language),&chunk.samples).await?;
                let confirmed=groq::confirmed_words(words,&chunk,&mut watermark,&mut deferred);
                let added=segments(confirmed);let text=added.iter().map(|s|s.text.as_str()).collect::<Vec<_>>().join(" ");
                may.edit(&run.id,|s|{
                    s.segments.extend(added);s.processed=s.processed.max(watermark.max(0.));
                    if let Some(info)=s.speech.as_mut(){info.uploaded_samples+=chunk.samples.len() as u64;}
                })?;
                let _=app.emit("session-changed",run.id.clone());
                if run.speech.enabled.load(Ordering::Acquire){
                    crate::caption::set_translation_mode(app,false);
                    if !text.trim().is_empty(){crate::caption::emit_text(app,crate::caption::CaptionText{text,original:String::new(),final_:true});}
                    crate::caption::emit_status(app,"groq","active",false,"Đang chép lời cuộc họp bằng Groq Whisper theo đoạn; phụ đề không gửi audio lần hai.");
                }
            }else{may.edit(&run.id,|s|{s.processed=s.processed.max(end as f64/RATE as f64);})?;}
            cursor=end;if tail{tail_flushed=true;}
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmed_segments_keep_word_timing_and_boundaries() {
        let words = (0..16)
            .map(|i| Word {
                text: format!("word{i}"),
                start: 4. + i as f64 / 4.,
                end: 4.2 + i as f64 / 4.,
            })
            .collect();
        let s = segments(words);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].start, 4.);
        assert_eq!(s[0].text.split_whitespace().count(), 7);
        assert_eq!(s[2].text, "word14 word15");
        assert!(s.windows(2).all(|p| p[0].end <= p[1].start));
    }
}
