<script lang="ts">
  import { captionWindowVisible, captionWindowPending, showCaptionWindow, hideCaptionWindow } from "./lib/caption-window";
  let error = "";
  async function toggle() {
    error = "";
    try { if ($captionWindowVisible) await hideCaptionWindow(); else await showCaptionWindow(); }
    catch (e) { error = String(e); }
  }
</script>

<button class="button secondary" type="button" aria-pressed={$captionWindowVisible} disabled={$captionWindowPending} onclick={toggle}>{$captionWindowVisible ? "Ẩn hộp phụ đề" : "Hiện hộp phụ đề"}</button>
{#if error}<span class="test-error" role="alert">{error}</span>{/if}
