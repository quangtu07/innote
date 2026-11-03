import { writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";

export const captionWindowVisible = writable(false);
export const captionWindowPending = writable(false);
let queue = Promise.resolve();
let pending = 0;
let version = 0;
export async function refreshCaptionWindow() {
  const current = version;
  const visible = await invoke<boolean>("caption_window_visible");
  if (current === version) captionWindowVisible.set(visible);
}
function change(command: string) {
  version++;
  pending++;
  captionWindowPending.set(true);
  const job = queue.then(async () => {
    await invoke(command);
    captionWindowVisible.set(await invoke<boolean>("caption_window_visible"));
  });
  queue = job.catch(() => {});
  return job.finally(() => {
    pending--;
    if (!pending) captionWindowPending.set(false);
  });
}
export const showCaptionWindow = () => change("open_caption_window");
export const hideCaptionWindow = () => change("hide_caption_window");
export const closeCaptionWindow = () => change("close_caption_window");
