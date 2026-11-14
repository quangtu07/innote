<script lang="ts">
  import {Merge, Pencil, Play, RotateCcw, Users, X} from "@lucide/svelte";
  import type {SpeakerAnalysis, SpeakerParticipant} from "./types";
  import { speakerColorFor } from "./speaker-colors";
  export let analysis: SpeakerAnalysis | null = null;
  export let busy = false;
  export let error = "";
  export let progress = "";
  export let exportSpeakers = true;
  export let onToggleExportSpeakers: (value: boolean) => void = () => {};
  export let onCancel: () => void = () => {};
  export let onRename: (id: string, name: string) => void = () => {};
  export let onMerge: (from: string, into: string) => void = () => {};
  export let onUndo: () => void = () => {};
  export let onPreview: (id: string) => void = () => {};
  export let onRetry: () => void = () => {};
  export let onClose: () => void = () => {};
  let renaming = "";
  let draft = "";
  function startRename(p: SpeakerParticipant) {
    renaming = p.id;
    draft = p.name;
  }
  function finishRename() {
    if (!renaming) return;
    const name = draft.trim();
    if (name && name !== analysis?.participants.find((p) => p.id === renaming)?.name) {
      onRename(renaming, name);
    }
    renaming = "";
  }
  function sourceName(source: string) {
    return source === "microphone" ? "Mic" : source === "soniox" ? "Soniox" : "Loa";
  }
  function otherParticipants(p: SpeakerParticipant) {
    return (analysis?.participants ?? []).filter((x) => x.id !== p.id && x.source === p.source);
  }
  function participantColor(p: SpeakerParticipant) {
    return speakerColorFor(analysis?.participants ?? [], p.id);
  }
  $: needsReview = analysis
    ? Object.values(analysis.segments).filter((s) => s.state !== "single").length
    : 0;
</script>

<section class="speaker-panel" aria-labelledby="speaker-panel-title">
  <header>
    <div>
      <h2 id="speaker-panel-title"><Users size={17} /> Người nói</h2>
      <p>{analysis?.status === "ready" ? `${analysis.participants.length} cụm · ${needsReview} đoạn cần rà` : "Chưa có kết quả"}</p>
    </div>
    <div class="panel-head-actions">
      <button class="text-button" title="Chạy lại với nguồn khác" disabled={busy} onclick={onRetry}><RotateCcw size={14} /> Chạy lại</button>
      <button class="icon-button" aria-label="Đóng danh sách người nói" title="Đóng" onclick={onClose}><X size={18} /></button>
    </div>
  </header>
  {#if analysis?.status === "stale"}
    <div class="panel-note" role="status">Bản chép đã thay đổi. Hãy chạy lại phân tích.</div>
    <button class="button secondary" disabled={busy} onclick={onRetry}>Chạy lại</button>
  {:else if analysis?.status === "empty"}
    <div class="panel-note" role="status">Không tìm thấy giọng nói trong nguồn đã chọn.</div>
    <button class="button secondary" disabled={busy} onclick={onRetry}>Thử lại</button>
  {:else if analysis?.status === "ready"}
    {#if analysis.sources.some((s) => s.status !== "ready")}<p class="panel-note" role="status">Đã bỏ qua nguồn không có tiếng: {analysis.sources.filter((s) => s.status !== "ready").map((s) => `${s.source === "microphone" ? "Mic" : s.source === "soniox" ? "Soniox" : "Loa"} (${s.status})`).join(", ")}.</p>{/if}
    <div class="speaker-list">
      {#each analysis.participants as p (p.id)}
        <article class="speaker-row">
          <div class="speaker-main">
            <span class="speaker-dot" data-source={p.source} style={`background:${participantColor(p).fg}`}></span>
            {#if renaming === p.id}
              <input
                aria-label="Tên người nói"
                bind:value={draft}
                maxlength="60"
                disabled={busy}
                onkeydown={(e) => { if (e.key === "Enter") finishRename(); if (e.key === "Escape") renaming = ""; }}
                onblur={finishRename}
              />
            {:else}
              <button class="speaker-name" title="Đổi tên người nói" disabled={busy} style={`color:${participantColor(p).fg}`} onclick={() => startRename(p)}>
                {p.name}{#if p.name === "Bạn?"}<small> (chưa xác nhận)</small>{/if}
              </button>
            {/if}
            <small>{sourceName(p.source)} · {p.segments} đoạn</small>
          </div>
          <div class="speaker-actions">
            <button class="text-button" title="Nghe đoạn đầu của người này" disabled={busy || !p.segments} onclick={() => onPreview(p.id)}><Play size={14} /> Nghe</button>
            <button class="text-button" title="Đổi tên" disabled={busy} onclick={() => startRename(p)}><Pencil size={14} /> Đổi tên</button>
            {#if otherParticipants(p).length}
              <label class="merge-select">
                <Merge size={13} />
                <select aria-label={`Gộp ${p.name} vào người khác`} disabled={busy} onchange={(e) => { const into = e.currentTarget.value; e.currentTarget.value = ""; if (into) onMerge(p.id, into); }}>
                  <option value="">Gộp vào…</option>
                  {#each otherParticipants(p) as other}<option value={other.id}>{other.name}</option>{/each}
                </select>
              </label>
            {/if}
          </div>
        </article>
      {/each}
    </div>
    <footer>
      <p>{needsReview ? `${needsReview} đoạn chưa chắc hoặc nhiều người — bấm nhãn trong bản chép để sửa.` : "Các đoạn đã có nhãn rõ ràng. Bạn vẫn có thể sửa từng đoạn."}</p>
      <button class="button secondary small" disabled={busy || analysis.revision <= 1} onclick={onUndo}><RotateCcw size={14} /> Hoàn tác</button>
      <label class="checkbox export-speakers"><input type="checkbox" checked={exportSpeakers} disabled={busy} onchange={(e) => onToggleExportSpeakers(e.currentTarget.checked)} /> Kèm tên khi xuất/tóm tắt</label>
    </footer>
  {:else if analysis?.status === "none"}
    <div class="panel-note" role="status">Chưa phân tích cuộc họp này.</div>
  {:else}
    <div class="panel-note" role="status">Chọn nguồn rồi bắt đầu phân tích.</div>
  {/if}
  {#if error}<p class="panel-error" role="alert">{error}</p>{/if}
  {#if busy}<div class="panel-busy" role="status"><span>{progress || "Đang phân tích…"}</span><button class="text-button" onclick={onCancel}>Hủy</button></div>{/if}
</section>
<style>
  .speaker-panel{background:#fff;border:1px solid #e1e8e4;border-radius:12px;padding:16px;display:flex;flex-direction:column;gap:12px;min-width:0}
  header{display:flex;align-items:flex-start;justify-content:space-between;gap:8px}
  .panel-head-actions{display:flex;align-items:center;gap:6px;flex-shrink:0}
  h2{display:flex;align-items:center;gap:7px;font-size:15px;margin:0;color:#172d29}
  header p{margin:4px 0 0;font-size:11px;color:#52645f}
  .speaker-list{display:flex;flex-direction:column;gap:8px;max-height:400px;min-height:76px;flex:1 1 auto;overflow:auto}
  .speaker-row{border-top:1px solid #edf1ed;padding-top:8px}
  .speaker-row:first-child{border-top:0;padding-top:0}
  .speaker-main{display:flex;align-items:center;gap:7px;flex-wrap:wrap;min-width:0}
  .speaker-dot{width:9px;height:9px;border-radius:50%;background:#2f5d8a;flex-shrink:0}
  .speaker-dot[data-source="microphone"]{background:#146b56}
  .speaker-dot[data-source="system"]{background:#2f5d8a}
  .speaker-dot[data-source="soniox"]{background:#8a5a2f}
  .speaker-name{border:0;background:transparent;color:#172d29;font-size:13px;font-weight:650;padding:0;cursor:pointer;text-align:left}
  .speaker-name:hover{color:#146b56;text-decoration:underline}
  .speaker-main small{font-size:10px;color:#52645f;flex-basis:100%;padding-left:16px}
  .speaker-main input{width:100%;max-width:180px;min-height:30px;font-size:13px}
  .speaker-actions{display:flex;gap:8px;align-items:center;flex-wrap:wrap;padding-left:16px;margin-top:4px}
  .speaker-actions .text-button{font-size:11px;display:inline-flex;align-items:center;gap:4px;padding:0;min-height:26px}
  .merge-select{display:inline-flex;align-items:center;gap:3px;font-size:11px;color:#52645f}
  .merge-select select{min-height:26px;font-size:11px;max-width:120px;border:1px solid #d5e0d8;border-radius:6px;background:#fff;color:#385649}
  footer{display:flex;align-items:flex-start;justify-content:space-between;gap:10px;border-top:1px solid #e1e8e4;padding-top:10px;flex-wrap:wrap}
  footer p{font-size:11px;line-height:1.6;color:#52645f;margin:0;flex:1;min-width:150px}
  .export-speakers{font-size:11px;flex-basis:100%;min-height:26px;color:#52645f}
  .panel-note{font-size:12px;line-height:1.7;color:#52645f;margin:0}
  .panel-error{font-size:12px;line-height:1.6;color:#9e3e36;margin:0;overflow-wrap:anywhere}
  .panel-busy{display:flex;align-items:center;justify-content:space-between;gap:8px;font-size:11px;color:#385649;background:#edf3ef;border-radius:8px;padding:8px 10px}
  .panel-busy button{flex-shrink:0}
  @media(max-width:1000px){.speaker-list{max-height:260px}}
</style>