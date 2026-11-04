import { writable } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import type { Snapshot } from "../types";

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
    if(command === "open_caption_window" && !await invoke<boolean>("caption_running")) {
      const snapshot=await invoke<Snapshot>("snapshot");
      const active=snapshot.sessions.find(session=>session.id===snapshot.active_id);
      // Opening the overlay during Soniox recording attaches its existing stream.
      // Never start a standalone capture or rotate translation configuration here.
      if(active?.speech && !active.speech.local_fallback && ["recording","paused"].includes(active.status)) {
        await invoke("start_caption",{options:{
          mode:"meeting",profileId:active.speech.profile_id,model:active.speech.model,
          provider:"",apiKey:"",baseUrl:"",sourceLanguage:active.language,
          targetLanguage:active.speech.translation_language ?? "vi",translate:!!active.speech.translation_language,
          speechQuality:active.speech_quality ?? "balanced",fallback:null,device:null,sessionDir:null,
        }});
      }
    }
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
