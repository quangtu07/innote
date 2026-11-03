<script lang="ts">
  import {stringsFor,type UiLocale} from "./i18n";
  import {processingDetails,qualitySupported,type SpeechQuality} from "./lib/speech-quality";
  export let value: SpeechQuality = "balanced";
  export let provider = "";
  export let model = "";
  export let disabled = false;
  export let scope = "";
  export let locale: UiLocale = "vi";
  export let onchange: (quality: SpeechQuality)=>void = ()=>{};
  $: tr=stringsFor(locale);
  $: supported=qualitySupported(provider);
  $: selected=supported?value:"balanced";
  $: labels=[tr.processingFast,tr.processingBalanced,tr.processingAccurate];
</script>

<fieldset class="speech-quality" disabled={disabled || !model || !provider} aria-label={`${scope} · ${tr.processingLevel}`}>
  <legend>{tr.processingLevel}</legend>
  <div class="speech-quality-options">
    {#each ["fast","balanced","accurate"] as level,index}
      <label class:chosen={selected===level} class:unavailable={!supported && level!=="balanced"}>
        <input type="radio" name={`speech-quality-${scope}`} value={level} checked={selected===level} disabled={!supported && level!=="balanced"} aria-label={`${scope} · ${labels[index]}`} onchange={()=>onchange(level as SpeechQuality)} />
        <span>{labels[index]}</span>
      </label>
    {/each}
  </div>
  <p class="processing-description">{processingDetails(provider,model,selected,locale)}</p>
  <small>{tr.processingHint}</small>
</fieldset>

<style>
  .speech-quality{border:0;padding:0;margin:24px 0;min-width:0}
  legend{font-weight:650;margin-bottom:12px;color:#263f36;font-size:14px}
  .speech-quality-options{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px}
  label{display:flex;gap:9px;align-items:center;padding:14px 16px;border:1px solid #cadbd2;border-radius:10px;color:#365547;background:#fff;cursor:pointer;min-width:0}
  label.chosen{border-color:#176b56;background:#edf6f1;color:#12543f}
  label.unavailable,fieldset:disabled label{opacity:.6;cursor:default}
  input{accent-color:#176b56;flex-shrink:0;margin:0}
  span{font-size:14px;font-weight:600;overflow-wrap:anywhere}
  label:focus-within{outline:2px solid #176b56;outline-offset:3px}
  .processing-description{font-size:13px;line-height:1.7;color:#446250;margin:12px 0 4px;max-width:78ch}
  small{font-size:12px;color:#526b5d;line-height:1.6;display:block}
  @media(max-width:650px){.speech-quality-options{gap:8px}label{padding:12px 8px;gap:6px}span{font-size:13px}}
</style>
