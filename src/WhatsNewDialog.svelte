<script lang="ts">
  import {onMount,tick} from "svelte";
  import {X} from "@lucide/svelte";
  import type {UiLocale} from "./i18n/shared";
  import {releaseNotes,previewReleaseVersion} from "./lib/release-notes";
  import {prepareReleaseNotice,dismissReleaseNotice} from "./lib/release-notice";
  export let version:string;
  export let locale:UiLocale="vi";
  export let existingInstall=false;
  export let ready=false;
  let pending=false,previewing=false;
  let dialog:HTMLDialogElement;
  $: displayedVersion=previewing?previewReleaseVersion:version;
  $: content=releaseNotes[displayedVersion]?.[locale];
  $: shouldOpen=!!content && (previewing || (pending && ready));
  $: if(shouldOpen)void tick().then(()=>{if(shouldOpen&&dialog&&!dialog.open)dialog.showModal();});
  $: if(!shouldOpen && dialog?.open)dialog.close();
  function dismiss(){
    dialog?.close();
    if(previewing){previewing=false;return;}
    pending=false;
    try{dismissReleaseNotice(localStorage,version);}catch{/* Closed for this run even if browser storage is unavailable. */}
  }
  function trapFocus(event:KeyboardEvent){
    if(event.key!=="Tab")return;
    const buttons=dialog.querySelectorAll<HTMLButtonElement>('button');
    const first=buttons[0],last=buttons[buttons.length-1];
    if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}
    else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
  }
  onMount(()=>{
    pending=prepareReleaseNotice(localStorage,version,existingInstall,!!releaseNotes[version]);
    if(import.meta.env.DEV){
      const preview=()=>{previewing=true;};
      window.addEventListener("innote:preview-whats-new",preview);
      return ()=>{window.removeEventListener("innote:preview-whats-new",preview);dialog?.close();};
    }
    return ()=>dialog?.close();
  });
</script>

{#if content}
  <dialog class="whats-new" bind:this={dialog} aria-labelledby="whats-new-title" aria-describedby="whats-new-intro" onkeydown={trapFocus} oncancel={e=>{e.preventDefault();dismiss();}}>
    <header>
      <div class="heading"><h2 id="whats-new-title">{content.title}</h2><span class="release-version">v{displayedVersion}</span></div>
      <button class="icon-button" aria-label={content.close} onclick={dismiss}><X size={20}/></button>
    </header>
    <p class="intro" id="whats-new-intro">{content.subtitle}</p>
    <ul class="release-items">
      {#each content.items as item}<li><h3>{item.title}</h3><p>{item.detail}</p></li>{/each}
    </ul>
    <footer><p>{content.once}</p><button class="button primary" onclick={dismiss}>{content.done}</button></footer>
  </dialog>
{/if}

<style>
  .whats-new{width:min(640px,calc(100vw - 48px));max-height:calc(100vh - 48px);margin:auto;padding:28px;border:0;border-radius:14px;background:var(--paper,#fff);color:var(--ink,#172d29);box-shadow:0 20px 64px #10251b40;overflow:hidden}
  .whats-new[open]{display:flex;flex-direction:column}
  .whats-new::backdrop{background:#10251b70}
  header{display:flex;align-items:flex-start;justify-content:space-between;gap:16px;flex-shrink:0}
  .heading{display:flex;align-items:center;gap:12px;flex-wrap:wrap;min-width:0}
  h2{font-size:22px;line-height:1.4;margin:2px 0;font-weight:650;letter-spacing:-.4px}
  .release-version{padding:4px 9px;border-radius:6px;background:#edf5f0;color:#245c49;font-size:12px;font-weight:600;white-space:nowrap}
  header button{min-width:36px;min-height:36px;flex-shrink:0}
  .intro{font-size:14px;line-height:1.65;color:var(--muted,#52645f);margin:12px 0 24px;flex-shrink:0}
  .release-items{overflow:auto;min-height:0;margin:0;padding:0 8px 0 20px;list-style:disc}
  li{padding-left:4px;margin-bottom:20px}
  li:last-child{margin-bottom:0}
  li::marker{color:#146b56}
  h3{font-size:14px;line-height:1.6;font-weight:650;margin:0 0 5px}
  li p{font-size:13px;line-height:1.75;color:var(--muted,#52645f);margin:0;overflow-wrap:anywhere}
  footer{display:flex;align-items:center;gap:20px;justify-content:space-between;border-top:1px solid var(--line,#e1e8e4);padding-top:20px;margin-top:24px;flex-shrink:0}
  footer p{font-size:12px;line-height:1.65;color:var(--muted,#52645f);margin:0;max-width:34ch}
  footer button{min-height:42px;flex-shrink:0}
  @media(max-height:650px){.whats-new{padding:24px}.intro{margin-bottom:20px}li{margin-bottom:16px}footer{margin-top:20px;padding-top:16px}}
</style>
