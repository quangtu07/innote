<script lang="ts">
  import { onMount, tick } from "svelte";
  import { emit, listen } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
  import { invoke } from "@tauri-apps/api/core";
  import { CaptionPresentation } from "./lib/caption-presentation";

  interface CaptionTextEvent {
    text: string;
    original: string;
    final_: boolean;
    translating?: boolean;
  }
  interface CaptionStatusEvent {
    state: string;
    provider: string;
    sending: boolean;
    message: string;
  }
  import { readCaptionStyle, CAPTION_STYLE_KEY } from "./lib/caption-style";
  const BOUNDS_KEY = "innote.caption.bounds";
  function saveBounds() {
    try {
      localStorage.setItem(BOUNDS_KEY, JSON.stringify(bounds));
    } catch {
      /* best-effort */
    }
  }

  let style = readCaptionStyle();
  let bounds: { x: number; y: number; w: number; h: number } | null = null;
  let text = "";
  let translatedLine: HTMLDivElement;
  let originalLine: HTMLDivElement;
  let isTranslating=false;
  const presentation = new CaptionPresentation();
  let disposed = false;
  async function updateCaption(payload: CaptionTextEvent) {
    presentation.push({...payload,original:payload.original??""});
    await applyPresentation();
  }
  async function applyPresentation(force=false) {
    const cue=presentation.view(style.showOriginal);
    const nextMode=presentation.isTranslating();
    if(!force&&cue.text===text&&cue.original===original&&nextMode===isTranslating)return;
    isTranslating=nextMode;
    text=cue.text;original=cue.original;
    await tick();
    if (translatedLine) translatedLine.scrollTop = translatedLine.scrollHeight;
    if (originalLine) originalLine.scrollTop = originalLine.scrollHeight;
  }
  function syncStyle(){style=readCaptionStyle();void applyPresentation(true);}
  let original = "";
  let status: CaptionStatusEvent = {
    state: "idle",
    provider: "",
    sending: false,
    message: "",
  };
  let passthrough = true;
  const appWindow = getCurrentWebviewWindow();

  // Keep pointer capture while resizing a transparent WebView. Applying the
  // latest position after the async size lookup also handles fast drags.
  function startGesture(event: PointerEvent, direction: string) {
    if (event.button !== 0) return;
    event.preventDefault();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const start = { x: event.screenX, y: event.screenY };
    let pointer = { ...start };
    let origin: { x: number; y: number; w: number; h: number } | null = null;
    let dirty = false,
      applying = false;
    async function apply() {
      if (!origin || applying) return;
      applying = true;
      try {
        while (dirty) {
          dirty = false;
          const dx = Math.round((pointer.x - start.x) * devicePixelRatio);
          const dy = Math.round((pointer.y - start.y) * devicePixelRatio);
          if (direction === "move") {
            await appWindow.setPosition(
              new PhysicalPosition(origin.x + dx, origin.y + dy),
            );
          } else {
            const west = direction.includes("West"),
              north = direction.includes("North");
            const w = Math.max(
              Math.round(180 * devicePixelRatio),
              origin.w + (west ? -dx : dx),
            );
            const h = Math.max(
              Math.round(90 * devicePixelRatio),
              origin.h + (north ? -dy : dy),
            );
            await appWindow.setSize(new PhysicalSize(w, h));
            if (west || north) {
              await appWindow.setPosition(
                new PhysicalPosition(
                  origin.x + (west ? origin.w - w : 0),
                  origin.y + (north ? origin.h - h : 0),
                ),
              );
            }
          }
        }
      } catch (e) {
        status = { ...status, message: String(e) };
      } finally {
        applying = false;
      }
    }
    const move = (e: PointerEvent) => {
      if (e.pointerId !== event.pointerId) return;
      pointer = { x: e.screenX, y: e.screenY };
      dirty = true;
      void apply();
    };
    const end = (e: PointerEvent) => {
      move(e);
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", end);
      target.removeEventListener("pointercancel", end);
      if (target.hasPointerCapture(event.pointerId))
        target.releasePointerCapture(event.pointerId);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", end);
    target.addEventListener("pointercancel", end);
    void Promise.all([appWindow.outerPosition(), appWindow.innerSize()]).then(
      ([position, size]) => {
        origin = {
          x: position.x,
          y: position.y,
          w: size.width,
          h: size.height,
        };
        void apply();
      },
    );
  }
  async function enableDirectDragging() {
    try { await appWindow.setIgnoreCursorEvents(false); passthrough = false; }
    catch { void emit("caption-layout-error", {}).catch(() => {}); }
  }

  onMount(() => {
    void enableDirectDragging();
    const unlisteners: Array<() => void> = [];
    const track = (p: Promise<() => void>) =>
      p
        .then((fn) => {
          if (disposed) fn();
          else unlisteners.push(fn);
        })
        .catch(() => {});
    track(
      listen<CaptionTextEvent>("caption-text", ({ payload }) => {
        void updateCaption(payload);
      }),
    );
    track(
      listen<CaptionStatusEvent>("caption-status", ({ payload }) => {
        status = payload;
        if (["idle","error","connecting"].includes(payload.state)) { presentation.clear();isTranslating=false;text = ""; original = ""; }
      }),
    );
    track(listen<boolean>("caption-mode",({payload})=>{presentation.setMode(payload);void applyPresentation();}));
    track(listen("caption-style", syncStyle));
    const syncStorage = (event: StorageEvent) => {
      if (event.key === CAPTION_STYLE_KEY) syncStyle();
    };
    window.addEventListener("storage", syncStorage);
    void invoke<{ status: CaptionStatusEvent; caption: CaptionTextEvent; translating?:boolean; paired?:CaptionTextEvent|null }>(
      "caption_snapshot",
    )
      .then((s) => {
        if (!disposed) {
          status = s.status;
          if (s.status.sending || s.status.state === "stopping" || s.status.state === "active") {presentation.restore(s);void applyPresentation();}
        }
      })
      .catch(() => {});
    // Rust owns close handling and stops the worker; avoid a second WebView
    // close listener that requires an additional native destroy permission.
    const saved = (() => {
      try {
        return JSON.parse(localStorage.getItem(BOUNDS_KEY) ?? "null");
      } catch {
        return null;
      }
    })();
    if (saved && saved.x !== undefined && saved.y !== undefined) {
      void appWindow.setPosition(new PhysicalPosition(saved.x, saved.y));
      if (saved.w && saved.h)
        void appWindow.setSize(new PhysicalSize(saved.w, saved.h));
    }
    track(
      appWindow.onMoved((event) => {
        const pos = event.payload;
        bounds = { x: pos.x, y: pos.y, w: bounds?.w ?? 0, h: bounds?.h ?? 0 };
        saveBounds();
      }),
    );
    track(
      appWindow.onResized((event) => {
        const size = event.payload;
        bounds = {
          x: bounds?.x ?? 0,
          y: bounds?.y ?? 0,
          w: size.width,
          h: size.height,
        };
        saveBounds();
        requestAnimationFrame(() => {
          if (translatedLine) translatedLine.scrollTop = translatedLine.scrollHeight;
          if (originalLine) originalLine.scrollTop = originalLine.scrollHeight;
        });
      }),
    );
    return () => {
      disposed = true;
      window.removeEventListener("storage", syncStorage);
      unlisteners.forEach((fn) => fn());
    };
  });

</script>
<svelte:head><title>Innote — Phụ đề</title></svelte:head>
<div class="caption-root" data-passthrough={passthrough} style={`--font-size:${style.fontSize}px; --text-color:${style.color}; text-align:${style.align}; background-color:${style.transparent ? "transparent" : style.backgroundColor};`} role="presentation" onpointerdown={(e) => startGesture(e, "move")}>
  <div class="caption-lines">
    {#if style.showOriginal && original.trim()}<div class="caption-original" bind:this={originalLine}>{original}</div>{/if}
    {#if text}<div class="caption-text" bind:this={translatedLine}>{text}</div>{/if}
    {#if !text && !original && style.showOriginal && isTranslating}<div class="caption-placeholder">Đang chờ cặp bản gốc và bản dịch…</div>
    {:else if !text && !original && !status.sending}<div class="caption-placeholder">Kéo dòng chữ này để đặt vị trí phụ đề</div>{/if}
  </div>
  {#each ["NorthWest", "NorthEast", "SouthWest", "SouthEast"] as direction}
      <span class="resize" class:west={direction.includes("West")} class:north={direction.includes("North")} role="presentation" onpointerdown={(e) => { e.stopPropagation(); startGesture(e,direction); }}></span>
  {/each}
</div>
<style>
  .caption-root { width: 100vw; height: 100vh; padding: 12px 20px; border-radius: 12px; cursor: grab; touch-action: none; box-sizing: border-box; overflow: hidden; user-select: none; --display-font-size: min(var(--font-size), max(12px, calc((100vh - 20px) / 2.5))); }
  .caption-lines { height: 100%; min-height: 0; display: flex; flex-direction: column; justify-content: flex-end; gap: 8px; pointer-events: none; overflow: hidden; }
  .caption-root:active { cursor: grabbing; }
  .caption-text, .caption-placeholder { min-height: 0; color: var(--text-color); font-size: var(--display-font-size); line-height: 1.45; font-weight: 600; overflow: auto; overflow-wrap: anywhere; text-shadow: -1px -1px 1px #101c17, 1px -1px 1px #101c17, -1px 1px 1px #101c17, 1px 1px 1px #101c17, 0 2px 4px #101c17; scrollbar-width: none; }
  .caption-original { min-height: 0; max-height: 30%; flex-shrink: 0; font-size: max(12px, calc(var(--display-font-size) * .75)); line-height: 1.4; color: var(--text-color); overflow: auto; scrollbar-width: none; overflow-wrap: anywhere; text-shadow: 0 1px 2px #101c17, 1px 0 2px #101c17, -1px 0 2px #101c17; }
  .resize { position: absolute; width: 20px; height: 20px; z-index: 1; right: 0; bottom: 0; touch-action: none; cursor: nwse-resize; }
  .resize.west { left: 0; right: auto; cursor: nesw-resize; }
  .resize.north { top: 0; bottom: auto; cursor: nesw-resize; }
  .resize.north.west { cursor: nwse-resize; }
  @media(max-height:120px) { .caption-root { padding: 6px 12px; } .caption-lines { gap: 4px; } }
</style>
