//! Local speaker analysis: raw intervals are immutable; user edits are
//! annotation only. Nothing here rewrites ASR text or audio.
use crate::types::{Segment, Session};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeakerInterval {
    pub start: f64,
    pub end: f64,
    pub cluster: i32,
    #[serde(default)]
    pub confidence: Option<f32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeakerParticipant {
    pub id: String,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub segments: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SegmentSpeaker {
    pub state: String,
    pub participant_ids: Vec<String>,
    #[serde(default)]
    pub source: Option<String>,
    pub label: String,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub manual: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeakerSource {
    pub source: String,
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub intervals: Vec<SpeakerInterval>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeakerAnalysis {
    pub session_id: String,
    pub transcript_version: String,
    pub transcript_hash: String,
    pub revision: u64,
    pub status: String,
    #[serde(default)]
    pub sources: Vec<SpeakerSource>,
    #[serde(default)]
    pub participants: Vec<SpeakerParticipant>,
    #[serde(default)]
    pub segments: BTreeMap<String, SegmentSpeaker>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Clone)]
pub struct SourceInput {
    pub source: String,
    pub status: String,
    pub error: Option<String>,
    pub intervals: Vec<SpeakerInterval>,
    pub audio_hash: String,
    pub model: String,
}
pub struct AnalysisInput {
    pub analysis_id: String,
    pub transcript_version: String,
    pub transcript_hash: String,
    pub sources: Vec<SourceInput>,
}

#[derive(Clone, Serialize, Deserialize)]
struct HeadRow {
    analysis_id: String,
    transcript_version: String,
    transcript_hash: String,
    revision: i64,
    updated_at: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct ParticipantRow {
    id: String,
    session_id: String,
    source: String,
    display_name: String,
    created_at: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct ClusterRow {
    run_id: String,
    cluster_index: i32,
    participant_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct OverrideRow {
    segment_id: String,
    state: String,
    participant_ids_json: String,
    note: Option<String>,
    updated_at: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct SpeakerState {
    head: HeadRow,
    participants: Vec<ParticipantRow>,
    clusters: Vec<ClusterRow>,
    overrides: Vec<OverrideRow>,
}
pub fn ensure_schema(c: &Connection) -> Result<()> {
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS speaker_runs(
            id TEXT PRIMARY KEY, analysis_id TEXT NOT NULL, session_id TEXT NOT NULL,
            transcript_version TEXT NOT NULL, transcript_hash TEXT NOT NULL,
            audio_hash TEXT NOT NULL DEFAULT '', source TEXT NOT NULL,
            status TEXT NOT NULL, model TEXT NOT NULL DEFAULT '', error TEXT,
            created_at TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS speaker_intervals(
            run_id TEXT NOT NULL, start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL,
            cluster_index INTEGER NOT NULL, confidence REAL);
         CREATE TABLE IF NOT EXISTS speaker_participants(
            id TEXT PRIMARY KEY, session_id TEXT NOT NULL, source TEXT NOT NULL,
            display_name TEXT NOT NULL, created_at TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS speaker_cluster_map(
            run_id TEXT NOT NULL, cluster_index INTEGER NOT NULL, participant_id TEXT NOT NULL,
            PRIMARY KEY(run_id, cluster_index));
         CREATE TABLE IF NOT EXISTS speaker_overrides(
            session_id TEXT NOT NULL, transcript_hash TEXT NOT NULL, segment_id TEXT NOT NULL,
            state TEXT NOT NULL, participant_ids_json TEXT NOT NULL, note TEXT,
            updated_at TEXT NOT NULL, PRIMARY KEY(session_id, transcript_hash, segment_id));
         CREATE TABLE IF NOT EXISTS speaker_heads(
            session_id TEXT PRIMARY KEY, analysis_id TEXT NOT NULL,
            transcript_version TEXT NOT NULL, transcript_hash TEXT NOT NULL,
            annotation_revision INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS speaker_history(
            id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL,
            revision INTEGER NOT NULL, body_json TEXT NOT NULL, created_at TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS speaker_runs_analysis ON speaker_runs(analysis_id, session_id);
         CREATE INDEX IF NOT EXISTS speaker_intervals_run ON speaker_intervals(run_id);
         CREATE INDEX IF NOT EXISTS speaker_history_session ON speaker_history(session_id, id);"
    )?;
    Ok(())
}

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}
fn db(root: &Path, id: &str) -> Result<Connection> {
    let location = crate::meeting_store::locate(root, id)?;
    let c = crate::meeting_store::connection(&location.database)?;
    ensure_schema(&c)?;
    Ok(c)
}
pub fn selected_segments<'a>(s: &'a Session, version: &str) -> Result<&'a [Segment]> {
    match version {
        "original" => Ok(&s.segments),
        "corrected" => s
            .corrected
            .as_ref()
            .map(|c| c.segments.as_slice())
            .context("Chưa có bản hiệu chỉnh để phân biệt người nói"),
        _ => anyhow::bail!("Phiên bản bản chép không hợp lệ"),
    }
}
/// Stable SHA-256 of IDs, times and UTF-8 text for one transcript version.
pub fn transcript_hash(s: &Session, version: &str) -> Result<String> {
    let segments = selected_segments(s, version)?;
    anyhow::ensure!(!segments.is_empty(), "Chưa có bản chép lời để phân tích");
    let mut h = Sha256::new();
    h.update(version.as_bytes());
    h.update((segments.len() as u64).to_le_bytes());
    for segment in segments {
        h.update((segment.id.len() as u64).to_le_bytes());
        h.update(segment.id.as_bytes());
        h.update(segment.start.to_bits().to_le_bytes());
        h.update(segment.end.to_bits().to_le_bytes());
        h.update((segment.text.len() as u64).to_le_bytes());
        h.update(segment.text.as_bytes());
    }
    Ok(hex::encode(h.finalize()))
}
fn merged_duration(mut ranges: Vec<(f64, f64)>) -> f64 {
    ranges.retain(|(a, b)| b.is_finite() && a.is_finite() && *b > *a);
    ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut total = 0.;
    let mut current: Option<(f64, f64)> = None;
    for (start, end) in ranges {
        match current {
            Some((s, e)) if start <= e => current = Some((s, e.max(end))),
            Some((s, e)) => {
                total += e - s;
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    if let Some((s, e)) = current {
        total += e - s;
    }
    total
}
fn state_for(total: f64, mut ranked: Vec<(String, f64)>) -> (String, Vec<String>, f64) {
    ranked.retain(|(_, c)| c.is_finite() && *c > 0.);
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let Some(_) = ranked.first() else {
        return ("unassigned".into(), vec![], 0.);
    };
    let top = ranked[0].1;
    let ratio = if total > 0. { top / total } else { 0. };
    let second = ranked.get(1).map(|x| x.1).unwrap_or(0.);
    let strong: Vec<_> = ranked
        .iter()
        .filter(|(_, c)| *c / total.max(0.001) >= 0.10 || *c >= 0.75)
        .map(|(id, _)| id.clone())
        .collect();
    if strong.len() > 1 && top < second * 1.8 {
        return ("multiple".into(), strong, ratio);
    }
    if ratio >= 0.12 || top >= 0.75 {
        ("single".into(), vec![ranked[0].0.clone()], ratio)
    } else {
        ("unassigned".into(), vec![], ratio)
    }
}

/// Pure attribution: map each ASR segment to participants using immutable
/// intervals. Manual overrides win; thresholds are deliberately conservative.
fn attribute(
    segments: &[Segment],
    runs: &[RunRow],
    cluster_to_participant: &HashMap<(String, i32), String>,
    participant_source: &HashMap<String, String>,
    participant_name: &HashMap<String, String>,
    overrides: &HashMap<String, (String, Vec<String>, bool)>,
) -> (BTreeMap<String, SegmentSpeaker>, HashMap<String, usize>) {
    let mut out = BTreeMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for segment in segments {
        let total = (segment.end - segment.start).max(0.);
        if let Some((state, ids, manual)) = overrides.get(&segment.id) {
            let source = source_for(ids, participant_source);
            let label = label_for(state, ids, participant_name);
            for id in ids {
                *counts.entry(id.clone()).or_default() += 1;
            }
            out.insert(
                segment.id.clone(),
                SegmentSpeaker {
                    state: state.clone(),
                    participant_ids: ids.clone(),
                    source,
                    label,
                    confidence: None,
                    manual: *manual,
                },
            );
            continue;
        }
        let mut ranges: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        for run in runs {
            for interval in &run.intervals {
                let Some(pid) = cluster_to_participant.get(&(run.id.clone(), interval.cluster))
                else {
                    continue;
                };
                let start = interval.start.max(segment.start);
                let end = interval.end.min(segment.end);
                if end > start {
                    ranges.entry(pid.clone()).or_default().push((start, end));
                }
            }
        }
        let ranked = ranges
            .into_iter()
            .map(|(id, ranges)| (id, merged_duration(ranges)))
            .collect();
        let (state, ids, confidence) = state_for(total, ranked);
        let source = source_for(&ids, participant_source);
        let label = label_for(&state, &ids, participant_name);
        for id in &ids {
            *counts.entry(id.clone()).or_default() += 1;
        }
        out.insert(
            segment.id.clone(),
            SegmentSpeaker {
                state,
                participant_ids: ids,
                source,
                label,
                confidence: Some(confidence as f32),
                manual: false,
            },
        );
    }
    (out, counts)
}
fn source_for(ids: &[String], sources: &HashMap<String, String>) -> Option<String> {
    let unique: std::collections::BTreeSet<_> = ids
        .iter()
        .filter_map(|id| sources.get(id).cloned())
        .collect();
    if unique.is_empty() {
        None
    } else if unique.len() == 1 {
        unique.into_iter().next()
    } else {
        Some("system+microphone".into())
    }
}
fn label_for(state: &str, ids: &[String], names: &HashMap<String, String>) -> String {
    match state {
        "multiple" => "Nhiều người".into(),
        "unassigned" => "Chưa xác định".into(),
        _ => ids
            .iter()
            .filter_map(|id| names.get(id).cloned())
            .collect::<Vec<_>>()
            .join(" & "),
    }
}
#[derive(Clone)]
struct RunRow {
    id: String,
    source: String,
    status: String,
    error: Option<String>,
    intervals: Vec<SpeakerInterval>,
}
pub fn save_analysis(root: &Path, id: &str, input: &AnalysisInput) -> Result<()> {
    let mut c = db(root, id)?;
    let tx = c.transaction()?;
    tx.execute(
        "DELETE FROM speaker_cluster_map WHERE run_id IN (SELECT id FROM speaker_runs WHERE session_id=?1)",
        [id],
    )?;
    tx.execute(
        "DELETE FROM speaker_intervals WHERE run_id IN (SELECT id FROM speaker_runs WHERE session_id=?1)",
        [id],
    )?;
    tx.execute("DELETE FROM speaker_runs WHERE session_id=?1", [id])?;
    tx.execute("DELETE FROM speaker_participants WHERE session_id=?1", [id])?;
    tx.execute("DELETE FROM speaker_overrides WHERE session_id=?1", [id])?;
    tx.execute("DELETE FROM speaker_history WHERE session_id=?1", [id])?;
    for source in &input.sources {
        let run_id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO speaker_runs(id,analysis_id,session_id,transcript_version,transcript_hash,audio_hash,source,status,model,error,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![run_id, input.analysis_id, id, input.transcript_version, input.transcript_hash, source.audio_hash, source.source, source.status, source.model, source.error, now()],
        )?;
        let mut clusters: Vec<i32> = source.intervals.iter().map(|i| i.cluster).collect();
        clusters.sort_unstable();
        clusters.dedup();
        for (position, cluster) in clusters.into_iter().enumerate() {
            let participant_id = uuid::Uuid::new_v4().to_string();
            let name = if source.source == "microphone" {
                if position == 0 { "Bạn?".to_string() } else { format!("Bạn? {}", position + 1) }
            } else {
                format!("Người nói {}", position + 1)
            };
            tx.execute(
                "INSERT INTO speaker_participants(id,session_id,source,display_name,created_at) VALUES(?1,?2,?3,?4,?5)",
                params![participant_id, id, source.source, name, now()],
            )?;
            tx.execute(
                "INSERT INTO speaker_cluster_map(run_id,cluster_index,participant_id) VALUES(?1,?2,?3)",
                params![run_id, cluster, participant_id],
            )?;
            for interval in source.intervals.iter().filter(|i| i.cluster == cluster) {
                tx.execute(
                    "INSERT INTO speaker_intervals(run_id,start_ms,end_ms,cluster_index,confidence) VALUES(?1,?2,?3,?4,?5)",
                    params![run_id, (interval.start * 1000.).round() as i64, (interval.end * 1000.).round() as i64, cluster, interval.confidence],
                )?;
            }
        }
    }
    let updated = now();
    tx.execute(
        "INSERT INTO speaker_heads(session_id,analysis_id,transcript_version,transcript_hash,annotation_revision,updated_at)
         VALUES(?1,?2,?3,?4,1,?5)
         ON CONFLICT(session_id) DO UPDATE SET analysis_id=excluded.analysis_id,
            transcript_version=excluded.transcript_version,transcript_hash=excluded.transcript_hash,
            annotation_revision=1,updated_at=excluded.updated_at",
        params![id, input.analysis_id, input.transcript_version, input.transcript_hash, updated],
    )?;
    tx.commit()?;
    crate::meeting_bundle::changed(root, id, true);
    Ok(())
}
fn load_head(c: &Connection, id: &str) -> Result<Option<HeadRow>> {
    Ok(c.query_row(
        "SELECT analysis_id,transcript_version,transcript_hash,annotation_revision,updated_at FROM speaker_heads WHERE session_id=?1",
        [id],
        |r| Ok(HeadRow { analysis_id: r.get(0)?, transcript_version: r.get(1)?, transcript_hash: r.get(2)?, revision: r.get(3)?, updated_at: r.get(4)? }),
    ).optional()?)
}
pub fn export_labels(root: &Path, id: &str, version: &str) -> Result<HashMap<String, String>> {
    let analysis = load_analysis(root, id, version)?;
    if analysis.status != "ready" {
        return Ok(HashMap::new());
    }
    let mut labels = HashMap::new();
    for (segment_id, segment) in &analysis.segments {
        let label = match segment.state.as_str() {
            "multiple" => "Nhiều người".to_string(),
            "unassigned" => "Chưa xác định".to_string(),
            _ => segment.label.clone(),
        };
        let label = if segment.source.as_deref() == Some("microphone") {
            format!("Mic · {label}")
        } else if segment.source.as_deref() == Some("soniox") {
            format!("Cuộc họp · {label}")
        } else {
            label
        };
        if !label.is_empty() {
            labels.insert(segment_id.clone(), label);
        }
    }
    Ok(labels)
}
pub fn load_analysis(root: &Path, id: &str, version: &str) -> Result<SpeakerAnalysis> {
    let session = crate::storage::load(root, id)?;
    let current_hash = transcript_hash(&session, version)?;
    let c = db(root, id)?;
    let Some(head) = load_head(&c, id)? else {
        return Ok(SpeakerAnalysis { session_id: id.into(), transcript_version: version.into(), transcript_hash: current_hash, revision: 0, status: "none".into(), sources: vec![], participants: vec![], segments: BTreeMap::new(), updated_at: None });
    };
    if head.transcript_version != version || head.transcript_hash != current_hash {
        return Ok(SpeakerAnalysis { session_id: id.into(), transcript_version: version.into(), transcript_hash: current_hash, revision: head.revision as u64, status: "stale".into(), sources: vec![], participants: vec![], segments: BTreeMap::new(), updated_at: Some(head.updated_at) });
    }
    let mut runs = Vec::new();
    {
        let mut q = c.prepare("SELECT id,source,status,error FROM speaker_runs WHERE analysis_id=?1 AND session_id=?2")?;
        let mut rows = q.query(params![head.analysis_id, id])?;
        while let Some(row) = rows.next()? {
            let run_id: String = row.get(0)?;
            let mut iq = c.prepare("SELECT start_ms,end_ms,cluster_index,confidence FROM speaker_intervals WHERE run_id=?1 ORDER BY start_ms")?;
            let intervals = iq.query_map([&run_id], |r| Ok(SpeakerInterval {
                start: r.get::<_, i64>(0)? as f64 / 1000.,
                end: r.get::<_, i64>(1)? as f64 / 1000.,
                cluster: r.get(2)?,
                confidence: r.get::<_, Option<f64>>(3)?.map(|x| x as f32),
            }))?.collect::<rusqlite::Result<Vec<_>>>()?;
            runs.push(RunRow { id: run_id, source: row.get(1)?, status: row.get(2)?, error: row.get(3)?, intervals });
        }
    }
    let mut participant_source = HashMap::new();
    let mut participant_name = HashMap::new();
    {
        let mut q = c.prepare("SELECT id,source,display_name FROM speaker_participants WHERE session_id=?1")?;
        let mut rows = q.query([id])?;
        while let Some(row) = rows.next()? {
            let pid: String = row.get(0)?;
            participant_source.insert(pid.clone(), row.get(1)?);
            participant_name.insert(pid, row.get(2)?);
        }
    }
    let mut cluster_to_participant = HashMap::new();
    {
        let mut q = c.prepare("SELECT run_id,cluster_index,participant_id FROM speaker_cluster_map WHERE run_id IN (SELECT id FROM speaker_runs WHERE analysis_id=?1 AND session_id=?2)")?;
        let mut rows = q.query(params![head.analysis_id, id])?;
        while let Some(row) = rows.next()? {
            cluster_to_participant.insert((row.get(0)?, row.get(1)?), row.get(2)?);
        }
    }
    let mut overrides = HashMap::new();
    {
        let mut q = c.prepare("SELECT segment_id,state,participant_ids_json FROM speaker_overrides WHERE session_id=?1 AND transcript_hash=?2")?;
        let mut rows = q.query(params![id, current_hash])?;
        while let Some(row) = rows.next()? {
            let ids: Vec<String> = serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default();
            overrides.insert(row.get::<_, String>(0)?, (row.get::<_, String>(1)?, ids, true));
        }
    }
    let segments = selected_segments(&session, version)?;
    let (segments, counts) = attribute(segments, &runs, &cluster_to_participant, &participant_source, &participant_name, &overrides);
    let mut used_ids: HashSet<String> = cluster_to_participant.values().cloned().collect();
    for (_, ids, _) in overrides.values() {
        used_ids.extend(ids.iter().cloned());
    }
    let mut participants: Vec<SpeakerParticipant> = participant_name
        .iter()
        .filter(|(pid, _)| used_ids.contains(*pid))
        .map(|(pid, name)| SpeakerParticipant {
            id: pid.clone(),
            name: name.clone(),
            source: participant_source.get(pid).cloned().unwrap_or_default(),
            segments: *counts.get(pid).unwrap_or(&0),
        })
        .collect();
    participants.sort_by(|a, b| a.source.cmp(&b.source).then(a.name.cmp(&b.name)));
    let sources = runs.iter().map(|run| SpeakerSource {
        source: run.source.clone(),
        status: run.status.clone(),
        error: run.error.clone(),
        intervals: run.intervals.clone(),
    }).collect();
    let status = if participants.is_empty() { "empty" } else { "ready" };
    Ok(SpeakerAnalysis { session_id: id.into(), transcript_version: version.into(), transcript_hash: current_hash, revision: head.revision as u64, status: status.into(), sources, participants, segments, updated_at: Some(head.updated_at) })
}
fn load_state(c: &Connection, id: &str) -> Result<SpeakerState> {
    let head = load_head(c, id)?.context("Chưa có kết quả phân biệt người nói")?;
    let mut participants = Vec::new();
    {
        let mut q = c.prepare("SELECT id,session_id,source,display_name,created_at FROM speaker_participants WHERE session_id=?1")?;
        let mut rows = q.query([id])?;
        while let Some(row) = rows.next()? {
            participants.push(ParticipantRow { id: row.get(0)?, session_id: row.get(1)?, source: row.get(2)?, display_name: row.get(3)?, created_at: row.get(4)? });
        }
    }
    let mut clusters = Vec::new();
    {
        let mut q = c.prepare("SELECT run_id,cluster_index,participant_id FROM speaker_cluster_map WHERE run_id IN (SELECT id FROM speaker_runs WHERE analysis_id=?1 AND session_id=?2)")?;
        let mut rows = q.query(params![head.analysis_id, id])?;
        while let Some(row) = rows.next()? {
            clusters.push(ClusterRow { run_id: row.get(0)?, cluster_index: row.get(1)?, participant_id: row.get(2)? });
        }
    }
    let mut overrides = Vec::new();
    {
        let mut q = c.prepare("SELECT segment_id,state,participant_ids_json,note,updated_at FROM speaker_overrides WHERE session_id=?1 AND transcript_hash=?2")?;
        let mut rows = q.query(params![id, head.transcript_hash])?;
        while let Some(row) = rows.next()? {
            overrides.push(OverrideRow { segment_id: row.get(0)?, state: row.get(1)?, participant_ids_json: row.get(2)?, note: row.get(3)?, updated_at: row.get(4)? });
        }
    }
    Ok(SpeakerState { head, participants, clusters, overrides })
}
fn save_state(tx: &rusqlite::Transaction<'_>, id: &str, state: &SpeakerState) -> Result<()> {
    tx.execute("DELETE FROM speaker_cluster_map WHERE run_id IN (SELECT id FROM speaker_runs WHERE analysis_id=?1 AND session_id=?2)", params![state.head.analysis_id, id])?;
    tx.execute("DELETE FROM speaker_participants WHERE session_id=?1", [id])?;
    tx.execute("DELETE FROM speaker_overrides WHERE session_id=?1 AND transcript_hash=?2", params![id, state.head.transcript_hash])?;
    for p in &state.participants {
        tx.execute("INSERT INTO speaker_participants(id,session_id,source,display_name,created_at) VALUES(?1,?2,?3,?4,?5)", params![p.id, p.session_id, p.source, p.display_name, p.created_at])?;
    }
    for c in &state.clusters {
        tx.execute("INSERT INTO speaker_cluster_map(run_id,cluster_index,participant_id) VALUES(?1,?2,?3)", params![c.run_id, c.cluster_index, c.participant_id])?;
    }
    for o in &state.overrides {
        tx.execute("INSERT INTO speaker_overrides(session_id,transcript_hash,segment_id,state,participant_ids_json,note,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![id, state.head.transcript_hash, o.segment_id, o.state, o.participant_ids_json, o.note, o.updated_at])?;
    }
    tx.execute(
        "INSERT INTO speaker_heads(session_id,analysis_id,transcript_version,transcript_hash,annotation_revision,updated_at) VALUES(?1,?2,?3,?4,?5,?6)
         ON CONFLICT(session_id) DO UPDATE SET analysis_id=excluded.analysis_id,transcript_version=excluded.transcript_version,
            transcript_hash=excluded.transcript_hash,annotation_revision=excluded.annotation_revision,updated_at=excluded.updated_at",
        params![id, state.head.analysis_id, state.head.transcript_version, state.head.transcript_hash, state.head.revision, state.head.updated_at],
    )?;
    Ok(())
}
fn with_state(root: &Path, id: &str, expected_revision: u64, mutate: impl FnOnce(&mut SpeakerState) -> Result<()>) -> Result<SpeakerAnalysis> {
    let mut c = db(root, id)?;
    let tx = c.transaction()?;
    let mut state = load_state(&tx, id)?;
    anyhow::ensure!(state.head.revision as u64 == expected_revision, "Danh sách người nói đã thay đổi; hãy tải lại");
    tx.execute(
        "INSERT INTO speaker_history(session_id,revision,body_json,created_at) VALUES(?1,?2,?3,?4)",
        params![id, state.head.revision, serde_json::to_string(&state)?, now()],
    )?;
    mutate(&mut state)?;
    state.head.revision += 1;
    state.head.updated_at = now();
    save_state(&tx, id, &state)?;
    tx.commit()?;
    crate::meeting_bundle::changed(root, id, true);
    load_analysis(root, id, &state.head.transcript_version)
}
pub fn rename(root: &Path, id: &str, participant_id: &str, name: &str, expected_revision: u64) -> Result<SpeakerAnalysis> {
    let name = name.trim();
    anyhow::ensure!(!name.is_empty() && name.chars().count() <= 60, "Tên người nói phải từ 1 đến 60 ký tự");
    with_state(root, id, expected_revision, |state| {
        let p = state.participants.iter_mut().find(|p| p.id == participant_id).context("Không tìm thấy người nói")?;
        p.display_name = name.to_string();
        Ok(())
    })
}
pub fn merge(root: &Path, id: &str, from_ids: &[String], into_id: &str, expected_revision: u64) -> Result<SpeakerAnalysis> {
    anyhow::ensure!(!from_ids.is_empty(), "Chọn ít nhất một người nói để gộp");
    with_state(root, id, expected_revision, |state| {
        let into = state.participants.iter().find(|p| p.id == into_id).cloned().context("Không tìm thấy người nói đích")?;
        for from in from_ids {
            anyhow::ensure!(from != into_id, "Không thể gộp một người vào chính nó");
            let source = state.participants.iter().find(|p| &p.id == from).map(|p| p.source.clone()).context("Không tìm thấy người nói nguồn")?;
            anyhow::ensure!(source == into.source, "Chỉ gộp các cụm trong cùng một nguồn");
        }
        for c in &mut state.clusters {
            if from_ids.contains(&c.participant_id) {
                c.participant_id = into_id.to_string();
            }
        }
        for o in &mut state.overrides {
            let mut ids: Vec<String> = serde_json::from_str(&o.participant_ids_json).unwrap_or_default();
            let before = ids.len();
            ids.retain(|x| !from_ids.contains(x));
            if ids.len() != before {
                if !ids.iter().any(|x| x == into_id) {
                    ids.insert(0, into_id.to_string());
                }
                o.participant_ids_json = serde_json::to_string(&ids)?;
                o.updated_at = now();
            }
        }
        state.participants.retain(|p| !from_ids.contains(&p.id));
        Ok(())
    })
}
pub fn reassign(root: &Path, id: &str, segment_id: &str, state_name: &str, participant_ids: &[String], expected_revision: u64) -> Result<SpeakerAnalysis> {
    anyhow::ensure!(["single", "multiple", "unassigned"].contains(&state_name), "Trạng thái nhãn không hợp lệ");
    let session = crate::storage::load(root, id)?;
    let head = load_head(&db(root, id)?, id)?.context("Chưa có kết quả phân biệt người nói")?;
    let segments = selected_segments(&session, &head.transcript_version)?;
    anyhow::ensure!(segments.iter().any(|s| s.id == segment_id), "Đoạn không thuộc bản chép hiện tại");
    with_state(root, id, expected_revision, |state| {
        for pid in participant_ids {
            anyhow::ensure!(state.participants.iter().any(|p| &p.id == pid), "Người nói không hợp lệ");
        }
        if state_name == "unassigned" {
            anyhow::ensure!(participant_ids.is_empty(), "Đoạn chưa xác định không có người nói");
        } else {
            anyhow::ensure!(!participant_ids.is_empty(), "Chọn ít nhất một người nói");
        }
        if state_name == "single" {
            anyhow::ensure!(participant_ids.len() == 1, "Một đoạn chỉ có một người nói");
        }
        state.overrides.retain(|o| o.segment_id != segment_id);
        state.overrides.push(OverrideRow {
            segment_id: segment_id.into(),
            state: state_name.into(),
            participant_ids_json: serde_json::to_string(participant_ids)?,
            note: None,
            updated_at: now(),
        });
        Ok(())
    })
}
pub fn undo(root: &Path, id: &str, expected_revision: u64) -> Result<SpeakerAnalysis> {
    let mut c = db(root, id)?;
    let tx = c.transaction()?;
    let head = load_head(&tx, id)?.context("Chưa có kết quả phân biệt người nói")?;
    anyhow::ensure!(head.revision as u64 == expected_revision, "Danh sách người nói đã thay đổi; hãy tải lại");
    let (row_id, body): (i64, String) = tx.query_row(
        "SELECT id,body_json FROM speaker_history WHERE session_id=?1 ORDER BY id DESC LIMIT 1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional()?.context("Không còn thao tác để hoàn tác")?;
    let state: SpeakerState = serde_json::from_str(&body)?;
    save_state(&tx, id, &state)?;
    tx.execute("DELETE FROM speaker_history WHERE id=?1", [row_id])?;
    tx.commit()?;
    crate::meeting_bundle::changed(root, id, true);
    load_analysis(root, id, &state.head.transcript_version)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn seg(id: &str, start: f64, end: f64) -> Segment {
        Segment { id: id.into(), start, end, text: id.into(), edited: false }
    }
    fn run(id: &str, intervals: Vec<(f64, f64, i32)>) -> RunRow {
        RunRow {
            id: id.into(),
            source: "system".into(),
            status: "ready".into(),
            error: None,
            intervals: intervals
                .into_iter()
                .map(|(start, end, cluster)| SpeakerInterval { start, end, cluster, confidence: None })
                .collect(),
        }
    }
    fn names(ids: &[(&str, &str)]) -> HashMap<String, String> {
        ids.iter().map(|(id, name)| (id.to_string(), name.to_string())).collect()
    }
    fn sources(ids: &[(&str, &str)]) -> HashMap<String, String> {
        ids.iter().map(|(id, source)| (id.to_string(), source.to_string())).collect()
    }
    #[test]
    fn same_speaker_returning_keeps_participant() {
        let segments = vec![seg("one", 0., 2.), seg("two", 2., 4.), seg("three", 4., 6.), seg("four", 6., 8.)];
        let runs = vec![run("r1", vec![(0., 2., 0), (2., 4., 1), (4., 6., 2), (6., 8., 0)])];
        let mut map = HashMap::new();
        for (cluster, pid) in [(0, "p1"), (1, "p2"), (2, "p3")] {
            map.insert(("r1".to_string(), cluster), pid.to_string());
        }
        let (out, _) = attribute(
            &segments,
            &runs,
            &map,
            &sources(&[("p1", "system"), ("p2", "system"), ("p3", "system")]),
            &names(&[("p1", "Một"), ("p2", "Hai"), ("p3", "Ba")]),
            &HashMap::new(),
        );
        assert_eq!(out["one"].participant_ids, vec!["p1"]);
        assert_eq!(out["two"].participant_ids, vec!["p2"]);
        assert_eq!(out["three"].participant_ids, vec!["p3"]);
        assert_eq!(out["four"].participant_ids, vec!["p1"]);
    }
    #[test]
    fn sources_do_not_merge_and_weak_evidence_is_unassigned() {
        let segments = vec![seg("a", 0., 4.), seg("b", 4., 8.)];
        let mut system = run("sys", vec![(0., 4., 0)]);
        system.source = "system".into();
        let mut mic = run("mic", vec![(4., 8., 0)]);
        mic.source = "microphone".into();
        let map = HashMap::from([
            (("sys".to_string(), 0), "p1".to_string()),
            (("mic".to_string(), 0), "p2".to_string()),
        ]);
        let (out, _) = attribute(
            &segments,
            &[system, mic],
            &map,
            &sources(&[("p1", "system"), ("p2", "microphone")]),
            &names(&[("p1", "Loa"), ("p2", "Bạn")]),
            &HashMap::new(),
        );
        assert_eq!(out["a"].participant_ids, vec!["p1"]);
        assert_eq!(out["a"].source.as_deref(), Some("system"));
        assert_eq!(out["b"].participant_ids, vec!["p2"]);
        assert_eq!(out["b"].source.as_deref(), Some("microphone"));
        let (out, _) = attribute(
            &segments,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(out["a"].state, "unassigned");
    }
    #[test]
    fn hash_is_stable_and_changes_only_with_content() {
        let dir = std::env::temp_dir().join(format!("innote-speaker-hash-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Session {
            speech_quality: Default::default(),
            id: uuid::Uuid::new_v4().to_string(),
            title: "QA".into(),
            created: "now".into(),
            duration: 2.,
            status: "completed".into(),
            language: "vi".into(),
            model: "moonshine-vi".into(),
            provider: "local".into(),
            processed: 2.,
            segments: vec![seg("one", 0., 2.)],
            live: vec![],
            notes: String::new(),
            error: None,
            warnings: vec![],
            smoothed: None,
            corrected: None,
            summary: None,
            translations: vec![],
            speech: None,
        };
        let first = transcript_hash(&s, "original").unwrap();
        assert_eq!(first, transcript_hash(&s, "original").unwrap());
        s.segments[0].text = "đã sửa".into();
        assert_ne!(first, transcript_hash(&s, "original").unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
#[test]
    fn save_load_rename_reassign_merge_and_undo_roundtrip() {
        let root = std::env::temp_dir().join(format!("innote-speaker-db-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut s = Session {
            speech_quality: Default::default(),
            id: uuid::Uuid::new_v4().to_string(),
            title: "QA".into(),
            created: "now".into(),
            duration: 4.,
            status: "completed".into(),
            language: "vi".into(),
            model: "moonshine-vi".into(),
            provider: "local".into(),
            processed: 4.,
            segments: vec![seg("one", 0., 2.), seg("two", 2., 4.)],
            live: vec![],
            notes: String::new(),
            error: None,
            warnings: vec![],
            smoothed: None,
            corrected: None,
            summary: None,
            translations: vec![],
            speech: None,
        };
        crate::storage::save(&root, &s).unwrap();
        let hash = transcript_hash(&s, "original").unwrap();
        save_analysis(&root, &s.id, &AnalysisInput {
            analysis_id: "analysis-1".into(),
            transcript_version: "original".into(),
            transcript_hash: hash.clone(),
            sources: vec![SourceInput {
                source: "system".into(),
                status: "ready".into(),
                error: None,
                intervals: vec![
                    SpeakerInterval { start: 0., end: 2., cluster: 0, confidence: None },
                    SpeakerInterval { start: 2., end: 4., cluster: 1, confidence: None },
                ],
                audio_hash: "audio".into(),
                model: "qa".into(),
            }],
        }).unwrap();
        let a = load_analysis(&root, &s.id, "original").unwrap();
        assert_eq!(a.status, "ready");
        assert_eq!(a.participants.len(), 2);
        let p1 = a.participants.iter().find(|p| p.name == "Người nói 1").unwrap().id.clone();
        let p2 = a.participants.iter().find(|p| p.name == "Người nói 2").unwrap().id.clone();
        assert_eq!(a.segments["one"].participant_ids, vec![p1.clone()]);
        let renamed = rename(&root, &s.id, &p1, "Một", 1).unwrap();
        assert_eq!(renamed.participants.iter().find(|p| p.id == p1).unwrap().name, "Một");
        assert_eq!(renamed.segments["one"].label, "Một");
        let changed = reassign(&root, &s.id, "one", "single", &[p2.clone()], 2).unwrap();
        assert_eq!(changed.segments["one"].participant_ids, vec![p2.clone()]);
        let undone = undo(&root, &s.id, 3).unwrap();
        assert_eq!(undone.segments["one"].participant_ids, vec![p1.clone()]);
        let merged = merge(&root, &s.id, &[p2.clone()], &p1, 2).unwrap();
        assert_eq!(merged.participants.len(), 1);
        assert_eq!(merged.segments["two"].participant_ids, vec![p1.clone()]);
        let restored = undo(&root, &s.id, 3).unwrap();
        assert_eq!(restored.participants.len(), 2);
        let labels = export_labels(&root, &s.id, "original").unwrap();
        assert!(!labels.is_empty());
        let exported = crate::storage::export_version_with_labels(&s, "txt", "original", &labels);
        assert!(exported.contains("[Một] one"));
        assert_eq!(s.segments[0].text, "one");
        s.segments[0].text = "changed".into();
        crate::storage::save(&root, &s).unwrap();
        let stale = load_analysis(&root, &s.id, "original").unwrap();
        assert_eq!(stale.status, "stale");
        s.segments[0].text = "one".into();
        assert_eq!(hash, transcript_hash(&s, "original").unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
}