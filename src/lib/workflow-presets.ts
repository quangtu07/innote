import {normalizeCaptionStyle,type CaptionStyle} from "./caption-style";
export const PRESETS_KEY="innote.workflow-presets.v1";
export type PresetMode="meeting"|"captionsonly";
export interface ModelChoice {profileId:string;model:string}
export interface TaskAssignments {
 transcription?:ModelChoice&{fallback?:ModelChoice};
 caption?:ModelChoice&{targetLang:string;translate?:boolean;fallback?:ModelChoice};
 smooth?:ModelChoice&{fallback?:ModelChoice};summary?:ModelChoice&{fallback?:ModelChoice};
 correction?:ModelChoice&{fallback?:ModelChoice};
}
export interface WorkflowPreset {
 id:string;name:string;mode:PresetMode;tasks:TaskAssignments;
 source:string;quality:"fast"|"balanced"|"accurate";
 captions:boolean;translate:boolean;target:string;transcribe:boolean;localFallback:boolean;
 captionStyle?:CaptionStyle;
}
export function copyPreset<T>(value:T):T{return JSON.parse(JSON.stringify(value));}
/** Session launch templates configure only work that runs during capture. */
export function realtimeAssignments(tasks:TaskAssignments):TaskAssignments {
 return copyPreset({...(tasks.transcription?{transcription:tasks.transcription}:{}),...(tasks.caption?{caption:tasks.caption}:{})});
}
function choice(v:any){return v&&typeof v.profileId==="string"&&typeof v.model==="string"?{profileId:v.profileId,model:v.model}:undefined;}
export function parsePresets(raw:string|null):WorkflowPreset[] {
 if(!raw)return [];
 try{const root=JSON.parse(raw);if(root?.version!==1||!Array.isArray(root.presets))return [];
 const ids=new Set<string>();
 return root.presets.flatMap((v:any)=>{
  if(!v||typeof v.id!=="string"||ids.has(v.id)||typeof v.name!=="string"||!v.name.trim()||!["meeting","captionsonly"].includes(v.mode))return [];
  ids.add(v.id);const tasks:TaskAssignments={};
  for(const key of ["transcription","caption","smooth","correction","summary"] as const){const c=choice(v.tasks?.[key]);if(c)(tasks as any)[key]={...c,...(choice(v.tasks[key].fallback)?{fallback:choice(v.tasks[key].fallback)}:{}),...(key==="caption"?{targetLang:typeof v.tasks[key].targetLang==="string"?v.tasks[key].targetLang:"vi",translate:v.tasks[key].translate===true}:{})};}
  return [{id:v.id,name:v.name.trim().slice(0,80),mode:v.mode,tasks,source:typeof v.source==="string"?v.source:"auto",quality:["fast","balanced","accurate"].includes(v.quality)?v.quality:"balanced",captions:v.captions===true,translate:v.translate===true,target:typeof v.target==="string"?v.target:"vi",transcribe:v.transcribe!==false,localFallback:v.localFallback===true,...(v.captionStyle&&typeof v.captionStyle==="object"?{captionStyle:normalizeCaptionStyle(v.captionStyle)}:{})} as WorkflowPreset];
 });}catch{return []}
}
export function serializePresets(presets:WorkflowPreset[]){return JSON.stringify({version:1,presets:parsePresets(JSON.stringify({version:1,presets}))});}
/** Write the launch copy before changing UI state; undo partial writes on failure. */
export function writePresetPreferences(storage:Pick<Storage,"getItem"|"setItem"|"removeItem">,entries:Record<string,string>){
 const previous=Object.fromEntries(Object.keys(entries).map(k=>[k,storage.getItem(k)]));
 try{for(const[k,v]of Object.entries(entries))storage.setItem(k,v);}
 catch(error){for(const[k,v]of Object.entries(previous)){try{if(v===null)storage.removeItem(k);else storage.setItem(k,v);}catch{}}throw error;}
}
