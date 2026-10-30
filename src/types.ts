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
export interface Meeting {
  id: string;
  title: string;
  created: string;
  duration: number;
  status: string;
  language: string;
  model: string;
  provider?: "local" | "soniox" | "mixed";
  processed: number;
  segments: Segment[];
  live?: Segment[];
  notes: string;
  error: string | null;
  warnings: string[];
  smoothed?: SmoothedTranscript | null;
}
export interface Model {
  id: string;
  name: string;
  bytes: number;
  installed: boolean;
}
export interface Snapshot {
  sessions: Meeting[];
  devices: Device[];
  models: Model[];
  data_dir: string;
  active_id: string | null;
  soniox_configured?: boolean;
  llm_configured?: boolean;
}
