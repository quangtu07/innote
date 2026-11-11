export interface Device {
  id: string;
  name: string;
  kind: string;
  default: boolean;
}
export interface Segment {
  id: string;
  start: number;
  end: number;
  text: string;
  edited: boolean;
}
export interface SmoothedTranscript {
  text: string;
  provider: string;
  model: string;
  prompt: string;
  created: string;
}
export interface Summary {
  text: string;
  provider: string;
  model: string;
  source: string;
  prompt: string;
  created: string;
  source_hash?: string;
}
export interface CorrectedTranscript {
  text:string; segments:Segment[]; provider:string; profile_id:string; model:string; created:string; audio_sha256:string;
}
export interface Meeting {
  speech_quality?: "fast" | "balanced" | "accurate";
  id: string;
  title: string;
  created: string;
  duration: number;
  status: string;
  language: string;
  model: string;
  provider?: "local" | "soniox" | "groq" | "mixed";
  processed: number;
  segments: Segment[];
  live?: Segment[];
  notes: string;
  error: string | null;
  warnings: string[];
  smoothed?: SmoothedTranscript | null;
  corrected?:CorrectedTranscript|null;
  summary?: Summary | null;
  translations?: { id: string; source_id: string; source_ids?: string[]; start: number; end: number; original: string; text: string; language: string; provider: string }[];
  speech?: { model: string; profile_id: string | null; translation_language: string | null; connections: number; uploaded_samples: number; local_fallback: boolean } | null;
}
export interface Model {
  id: string;
  name: string;
  bytes: number;
  installed: boolean;
}
export interface Snapshot {
  corrections?:{id:string;stage:string;processed:number;total:number;model:string;cancelling:boolean}[];
  translation_preview?: { id: string; language: string; text: string; committed: number } | null;
  sessions: Meeting[];
  devices: Device[];
  models: Model[];
  data_dir: string;
  active_id: string | null;
  has_microphone?: boolean;
  mic_muted?: boolean;
  soniox_configured?: boolean;
  llm_configured?: boolean;
}
