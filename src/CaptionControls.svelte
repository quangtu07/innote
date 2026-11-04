<script lang="ts">
  import { onMount } from "svelte";
  import { emit, listen } from "@tauri-apps/api/event";
  import { readCaptionStyle, CAPTION_STYLE_KEY,type CaptionStyle } from "./lib/caption-style";
  import { captionWindowVisible, refreshCaptionWindow } from "./lib/caption-window";
  import CaptionWindowToggle from "./CaptionWindowToggle.svelte";
  export let visibilityControl = true;
  export let persist = true;
  export let expanded = false;
  export let collapsible = true;
  export let style:CaptionStyle = readCaptionStyle();
  export let onstylechange:(style:CaptionStyle)=>void=()=>{};
  let error = "";
  onMount(() => {
    if(!persist)return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const track = (promise: Promise<() => void>) => void promise.then(fn => { if(disposed)fn();else unlisteners.push(fn); }).catch(()=>{});
    track(listen<boolean>("caption-window-visibility", ({payload}) => captionWindowVisible.set(payload)));
    void refreshCaptionWindow().catch(() => {});
    const refresh = () => { void refreshCaptionWindow().catch(() => {}); };
    track(listen("caption-style",()=>{style=readCaptionStyle();}));
    const syncStyle=(event:StorageEvent)=>{if(event.key===CAPTION_STYLE_KEY)style=readCaptionStyle();};
    window.addEventListener("storage",syncStyle);
    window.addEventListener("focus", refresh);
    track(listen("caption-layout-error", () => {
      error = "Không bật được thao tác kéo phụ đề. Hãy mở lại Innote bản mới.";
    }));
    return () => { disposed = true; unlisteners.forEach(fn=>fn()); window.removeEventListener("focus", refresh);window.removeEventListener("storage",syncStyle); };
  });
  async function save() {
    if(!persist)return;
    localStorage.setItem(CAPTION_STYLE_KEY, JSON.stringify(style));
    onstylechange({...style});
    try { await emit("caption-style", style); } catch (e) { error = String(e); }
  }
</script>

{#if visibilityControl}<div class="caption-visibility">
  <CaptionWindowToggle />
  <span>Hiện hộp trước để chỉnh vị trí. Ẩn hộp không dừng phụ đề; bắt đầu phụ đề sẽ tự hiện hộp.</span>
</div>{/if}
{#if error}<p class="test-error" role="alert">{error}</p>{/if}
{#if persist}<p class="caption-style-summary" aria-label="Kiểu phụ đề hiện tại">{style.fontSize}px · {style.align === "left" ? "Căn trái" : "Căn giữa"} · {style.showOriginal ? "Song ngữ" : "Một dòng ngôn ngữ"} · {style.transparent ? "Nền trong suốt" : `Nền ${style.backgroundColor}`} · Màu chữ {style.color}</p>{/if}
{#snippet appearanceFields()}
  <p>Chọn màu nền để dễ thấy hộp phụ đề, hoặc bật Trong suốt để chỉ hiện chữ. Mọi thao tác được thực hiện tại đây.</p>
  <div class="caption-customize-fields">
    <label><span>Cỡ chữ <output>{style.fontSize}px</output></span>
      <input aria-label="Cỡ chữ phụ đề" type="range" min="12" max="64" step="2" value={style.fontSize} oninput={(e) => { style={...style,fontSize:Number(e.currentTarget.value)}; void save(); }} />
    </label>
    <label>Căn chữ
      <select aria-label="Căn chữ phụ đề" value={style.align} onchange={(e) => { style={...style,align:e.currentTarget.value as "left"|"center"}; void save(); }}><option value="left">Căn trái</option><option value="center">Căn giữa</option></select>
    </label>
    <label>Màu chữ
      <input aria-label="Màu chữ phụ đề" type="color" value={style.color} oninput={(e) => { style={...style,color:e.currentTarget.value}; void save(); }} />
    </label>
    <label class="caption-bilingual-control"><input type="checkbox" aria-label="Song ngữ" checked={style.showOriginal} onchange={(e) => { style={...style,showOriginal:e.currentTarget.checked}; void save(); }} /> Song ngữ</label>
  </div>
  {#if style.showOriginal}<p>Khi bật Dịch sang, song ngữ chờ đủ một cặp gốc/dịch đã hoàn tất rồi hiện cả hai dòng cùng lúc. Có thể trễ hơn chế độ chỉ hiện bản dịch. Chỉ chép lời gốc vẫn cập nhật ngay.</p>{/if}
  <div class="caption-background-fields">
    <label>Màu nền hộp phụ đề
      <input aria-label="Màu nền hộp phụ đề" type="color" disabled={style.transparent} value={style.backgroundColor} oninput={(e) => { style={...style,backgroundColor:e.currentTarget.value}; void save(); }} />
    </label>
    <label class="caption-bilingual-control"><input type="checkbox" aria-label="Trong suốt" checked={style.transparent} onchange={(e) => { style={...style,transparent:e.currentTarget.checked}; void save(); }} /> Trong suốt</label>
  </div>
  <div class="caption-position-actions">
    <span>Giữ chuột trong hộp để di chuyển. Kéo bốn góc để phóng to hoặc thu nhỏ, kể cả khi chọn nền trong suốt.</span>
  </div>
{/snippet}
{#if collapsible}
  <details class="caption-customize" bind:open={expanded}>
    <summary>Tùy chỉnh chữ phụ đề</summary>
    {@render appearanceFields()}
  </details>
{:else}
  <div class="caption-customize">{@render appearanceFields()}</div>
{/if}

<style>
  .caption-customize { margin: 24px 0; border-top: 1px solid var(--line); padding-top: 20px; }
  .caption-visibility { display: flex; align-items: center; flex-wrap: wrap; gap: 16px; margin: 24px 0; }
  .caption-visibility span { flex: 1 1 240px; font-size: 12px; line-height: 1.7; color: var(--muted); }
  summary { cursor: pointer; font-size: 14px; font-weight: 600; color: var(--green); }
  p { margin: 16px 0; font-size: 13px; line-height: 1.7; color: var(--muted); }
  .caption-customize-fields { display: grid; grid-template-columns: minmax(140px, 2fr) minmax(120px, 1fr) 80px minmax(100px, 1fr); gap: 24px; align-items: start; }
  label { display: flex; flex-direction: column; gap: 10px; min-width: 0; font-size: 13px; color: var(--muted); }
  label span { display: flex; justify-content: space-between; width: 100%; }
  output { font-variant-numeric: tabular-nums; }
  select { width: 100%; height: 38px; padding: 0 10px; border: 1px solid #cad8cf; border-radius: 8px; background: var(--paper); color: #172d29; }
  input[type="range"] { width: 100%; height: 38px; margin: 0; accent-color: var(--green); }
  input[type="color"] { width: 52px; height: 38px; padding: 3px; border: 1px solid #cad8cf; border-radius: 8px; background: var(--paper); }
  .caption-bilingual-control { flex-direction: row; align-items: center; align-self: end; min-height: 38px; gap: 8px; cursor: pointer; color: #172d29; }
  input[type="checkbox"] { accent-color: var(--green); width: 17px; height: 17px; }
  .caption-position-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 16px; margin-top: 24px; }
  .caption-background-fields { display: flex; gap: 24px; align-items: end; flex-wrap: wrap; margin-top: 24px; }
  .caption-position-actions span { flex: 1 1 240px; color: var(--muted); font-size: 12px; line-height: 1.7; }
  @media(max-width:1050px) { .caption-customize-fields { grid-template-columns: repeat(2,minmax(0,1fr)); gap: 20px; } }
</style>
