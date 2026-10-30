use crate::types::{Segment, Session};
use std::collections::HashSet;

pub fn reconcile_visible_state(s: &mut Session, active_id: Option<&str>) {
    if ["recording", "paused", "processing"].contains(&s.status.as_str())
        && active_id != Some(s.id.as_str())
    {
        s.status = "interrupted".into();
        if s.error.is_none() {
            s.error = Some("Phiên đã dừng. Bản thu và bản chép hiện có được giữ trên máy.".into());
        }
    }
}

pub fn next_job(processed: f64, duration: f64, stopped: bool, paused: bool) -> Option<(f64, f64)> {
    if !processed.is_finite() || !duration.is_finite() || (paused && !stopped) {
        return None;
    }
    let available = if stopped {
        duration
    } else {
        (duration - 0.3).max(0.)
    };
    if available - processed >= 20. || (stopped && available > processed + 0.00001) {
        Some(((processed - 1.5).max(0.), (processed + 20.).min(available)))
    } else {
        None
    }
}

pub struct Sources(HashSet<String>);
impl Sources {
    pub fn new(kinds: impl IntoIterator<Item = String>) -> Self {
        Self(kinds.into_iter().collect())
    }
    pub fn failed(&mut self, kind: &str) -> bool {
        self.0.remove(kind);
        self.0.is_empty()
    }
}

fn han_or_kana(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30ff // Hiragana, Katakana
        | 0x3400..=0x4dbf // CJK Extension A
        | 0x4e00..=0x9fff // CJK Unified Ideographs
        | 0xf900..=0xfaff // CJK Compatibility Ideographs
    )
}

fn hangul(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x11ff | 0x3130..=0x318f | 0xac00..=0xd7af)
}

/// Whitespace splits Latin words, but Chinese writes a whole turn without
/// spaces and Korean endings can attach differently at an endpoint. Han/Kana/
/// Hangul therefore become one unit per character; Latin stays a word. Comparing
/// `split_whitespace()` alone duplicated Chinese and Korean across resume windows.
fn overlap_units(text: &str) -> Vec<(String, usize, usize)> {
    let mut units = Vec::new();
    let mut start = None;
    for (index, c) in text.char_indices() {
        if han_or_kana(c) || hangul(c) {
            if let Some(from) = start.take() {
                push_word(text, from, index, &mut units);
            }
            units.push((c.to_lowercase().to_string(), index, index + c.len_utf8()));
        } else if c.is_alphanumeric() {
            start.get_or_insert(index);
        } else if let Some(from) = start.take() {
            push_word(text, from, index, &mut units);
        }
    }
    if let Some(from) = start.take() {
        push_word(text, from, text.len(), &mut units);
    }
    units
}

fn push_word(text: &str, start: usize, end: usize, units: &mut Vec<(String, usize, usize)>) {
    let key = text[start..end].to_lowercase();
    if !key.is_empty() {
        units.push((key, start, end));
    }
}

/// Return the byte offset where genuinely new text begins. The caller must have
/// already established time evidence (touching committed audio); this only
/// compares text and keeps at least one unit, so a repeated utterance is never
/// erased whole.
fn overlap_offset(previous: &str, incoming: &str) -> Option<usize> {
    let left = overlap_units(previous);
    let right = overlap_units(incoming);
    if left.is_empty() || right.is_empty() {
        return None;
    }
    let char_lead = right[0]
        .0
        .chars()
        .any(|c| han_or_kana(c) || hangul(c));
    let min = if char_lead { 2 } else { 3 };
    let cap = if char_lead { 24 } else { 12 };
    let max = left.len().min(right.len()).min(cap);
    if max < min {
        return None;
    }
    for n in (min..=max).rev() {
        if left[left.len() - n..]
            .iter()
            .map(|u| &u.0)
            .eq(right[..n].iter().map(|u| &u.0))
        {
            // `n == right.len()` is a complete repeat, not an overlap trim.
            return (n < right.len()).then(|| right[n].1);
        }
    }
    None
}

pub fn commit(s: &mut Session, incoming: Vec<Segment>, end: f64) {
    let watermark = s.processed;
    for mut x in incoming {
        if !x.start.is_finite() || !x.end.is_finite() || x.end <= watermark + 0.00001 {
            continue;
        }
        // Only trim an exact prefix that touches already committed audio. The
        // native endpoint can split one turn into adjacent segments, or replay the
        // look-back across a window; both need the same suffix/prefix evidence.
        // A real pause (>50 ms) keeps a repeated utterance.
        let adjacent = s
            .segments
            .last()
            .is_some_and(|p| p.end >= x.start - 0.05 && p.end >= watermark - 1.5);
        if adjacent {
            if let Some(previous) = s
                .segments
                .last()
                .filter(|p| !p.edited && p.end >= watermark - 1.5)
            {
                if let Some(offset) = overlap_offset(&previous.text, &x.text) {
                    x.text = x.text[offset..]
                        .trim_start_matches(|c: char| !c.is_alphanumeric())
                        .to_string();
                }
            }
        }
        x.start = x.start.max(watermark);
        x.end = x.end.min(end);
        if x.end > x.start && !x.text.trim().is_empty() {
            s.segments.push(x);
        }
    }
    s.processed = end;
    s.live.retain(|w| w.end > end + 0.00001);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refinement_removes_only_streaming_words_inside_its_durable_watermark() {
        let mut s = session();
        s.live = vec![segment(19., 20., "cũ"), segment(20., 20.5, "mới")];
        commit(&mut s, vec![segment(20., 20.3, "Đã hiệu chỉnh")], 20.3);
        assert_eq!(s.live.len(), 1);
        assert_eq!(s.live[0].text, "mới");
        assert_eq!(s.segments.len(), 1);
        assert_eq!(s.processed, 20.3);
    }
    fn session() -> Session {
        serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4(),"title":"QA","created":"now","duration":45.,"status":"processing","language":"vi","model":"base-q5_1","processed":20.,"notes":"Giữ ghi chú","segments":[],"error":null,"warnings":[]})).unwrap()
    }
    fn segment(start: f64, end: f64, text: &str) -> Segment {
        Segment {
            id: uuid::Uuid::new_v4().to_string(),
            start,
            end,
            text: text.into(),
            edited: false,
        }
    }
    #[test]
    fn pause_stops_scheduling_but_stop_drains_even_a_short_tail() {
        assert_eq!(next_job(0., 60., false, true), None);
        assert_eq!(next_job(0., 20., false, false), None);
        assert_eq!(next_job(0., 20.3, false, false), Some((0., 20.)));
        assert_eq!(next_job(20., 20.025, true, true), Some((18.5, 20.025)));
        assert_eq!(next_job(20., 20., true, false), None);
    }
    #[test]
    fn an_orphaned_database_state_cannot_claim_recording_is_still_running() {
        let mut s = session();
        s.status = "recording".into();
        let id = s.id.clone();
        reconcile_visible_state(&mut s, Some(&id));
        assert_eq!(s.status, "recording");
        reconcile_visible_state(&mut s, None);
        assert_eq!(s.status, "interrupted");
        assert!(s.error.is_some());
        assert_eq!(s.processed, 20.);
    }
    #[test]
    fn losing_one_source_keeps_the_other_and_duplicate_errors_are_safe() {
        let mut sources = Sources::new(["system".into(), "microphone".into()]);
        assert!(!sources.failed("system"));
        assert!(!sources.failed("system"));
        assert!(sources.failed("microphone"));
    }
    #[test]
    fn overlap_is_trimmed_only_with_time_evidence_and_never_erases_repeated_utterances() {
        let mut s = session();
        s.segments.push(segment(0., 20., "Một câu xe và chỉ với"));
        commit(
            &mut s,
            vec![segment(18.5, 25., "xe và chỉ với dụng cụ bình thường")],
            40.,
        );
        assert_eq!(s.segments[1].text, "dụng cụ bình thường");
        assert_eq!(s.notes, "Giữ ghi chú");
        let mut s = session();
        s.segments.push(segment(0., 20., "Xin chào mọi người"));
        commit(&mut s, vec![segment(21., 25., "Xin chào mọi người")], 40.);
        assert_eq!(s.segments.len(), 2);
        let mut s = session();
        s.segments.push(segment(0., 20., "Xin chào mọi người"));
        commit(&mut s, vec![segment(19., 25., "Xin chào mọi người")], 40.);
        assert_eq!(s.segments.len(), 2);
    }
    #[test]
    fn chinese_overlap_is_trimmed_by_character_across_resume_windows() {
        let mut s = session();
        s.segments
            .push(segment(0., 20., "因为远离大陆哺乳动物无法长途跋涉"));
        commit(
            &mut s,
            vec![segment(
                18.5,
                25.,
                "无法长途跋涉而来使得巨龟成为科隆群岛主要的食草动物",
            )],
            40.,
        );
        assert_eq!(s.segments[1].text, "而来使得巨龟成为科隆群岛主要的食草动物");
        assert_eq!(s.segments[1].start, 20.);
        // A new utterance outside the look-back keeps a repeated Chinese phrase.
        let mut s = session();
        s.segments.push(segment(0., 20., "因为远离大陆"));
        commit(&mut s, vec![segment(21., 25., "因为远离大陆")], 40.);
        assert_eq!(s.segments.len(), 2);
        assert_eq!(s.segments[1].text, "因为远离大陆");
        // A complete repeat inside the look-back is not erased entirely.
        let mut s = session();
        s.segments.push(segment(0., 20., "谢谢大家"));
        commit(&mut s, vec![segment(19., 25., "谢谢大家")], 40.);
        assert_eq!(s.segments.len(), 2);
        assert_eq!(s.segments[1].text, "谢谢大家");
    }
    #[test]
    fn short_korean_and_mixed_script_overlap_keeps_the_new_tail() {
        let mut s = session();
        s.segments.push(segment(0., 20., "오늘 회의를 시작합니다 여러분"));
        commit(
            &mut s,
            vec![segment(18.5, 25., "시작합니다 여러분 그리고 안건을 봅니다")],
            40.,
        );
        assert_eq!(s.segments[1].text, "그리고 안건을 봅니다");
        let mut s = session();
        s.segments.push(segment(0., 20., "the team reviewed 你好世界"));
        commit(
            &mut s,
            vec![segment(19., 25., "你好世界 and shipped the fix")],
            40.,
        );
        assert_eq!(s.segments[1].text, "and shipped the fix");
    }
    #[test]
    fn adjacent_native_segments_trim_the_endpoint_word_but_a_pause_keeps_it() {
        let mut s = session();
        s.segments
            .push(segment(32.54, 40., "이제 과학적 데이터가 생물권을 보"));
        commit(
            &mut s,
            vec![segment(40., 41.22, "생물권을 보라냈음을 보여줍니다")],
            50.38,
        );
        assert_eq!(s.segments[1].text, "라냈음을 보여줍니다");
        let mut s = session();
        s.segments.push(segment(0., 20., "谢谢大家"));
        commit(&mut s, vec![segment(20.1, 25., "谢谢大家")], 40.);
        assert_eq!(s.segments.len(), 2);
        assert_eq!(s.segments[1].text, "谢谢大家");
    }
    #[test]
    fn edited_text_is_never_used_for_dedupe_and_invalid_times_are_discarded() {
        let mut s = session();
        let mut previous = segment(0., 20., "xe và chỉ với");
        previous.edited = true;
        s.segments.push(previous);
        commit(
            &mut s,
            vec![
                segment(19., 25., "xe và chỉ với dụng cụ"),
                segment(f64::NAN, 26., "invalid"),
                segment(5., 10., "old"),
            ],
            40.,
        );
        assert_eq!(s.segments.len(), 2);
        assert!(s.segments[0].edited);
        assert_eq!(s.segments[1].text, "xe và chỉ với dụng cụ");
        assert_eq!(s.processed, 40.);
    }
}
