export type SpeechQuality = "fast" | "balanced" | "accurate";
export const SPEECH_LEVELS_KEY = "may.speech.processing-levels";
export function isSpeechQuality(value: unknown): value is SpeechQuality { return ["fast","balanced","accurate"].includes(value as string); }
export function qualityKey(mode: string, profile: string, model: string, language: string, translate: boolean) { return JSON.stringify([mode,profile,model,language,translate]); }
export function parseProcessingLevels(raw: string | null): Record<string,SpeechQuality> {
  try {
    const parsed=JSON.parse(raw??"{}");if(!parsed||typeof parsed!=="object"||Array.isArray(parsed))return {};
    return Object.fromEntries(Object.entries(parsed).filter(([key,value])=>key.length<512&&key.startsWith('["')&&isSpeechQuality(value)).slice(0,128)) as Record<string,SpeechQuality>;
  } catch {return {};}
}
export function qualitySupported(provider: string) { return ["local","soniox","groq"].includes(provider); }
export function processingDetails(provider: string, model: string, level: SpeechQuality, locale="vi") {
  const index=["fast","balanced","accurate"].indexOf(level), en=locale!=="vi";
  if(!provider||!model)return en?"Choose a profile and model to see its processing controls.":"Chọn cấu hình và model để xem các mức xử lý phù hợp.";
  if(!qualitySupported(provider))return en?"This API currently supports its default processing only.":"API này hiện chỉ hỗ trợ mức mặc định; chưa có tham số ba mức được xác minh.";
  if(provider==="groq")return en?`Same Whisper model: ${[3.2,3.2,4][index]} s of new audio per request, ${[3,5,6][index]} s overlap context. Still chunked recognition; API time is additional.`:`Giữ nguyên model Whisper: mỗi ${[3.2,3.2,4][index]} giây audio mới/yêu cầu, thêm ${[3,5,6][index]} giây ngữ cảnh chồng lấn. Vẫn chép theo đoạn; còn thời gian chờ API.`;
  if(provider==="soniox")return en?`Same Soniox model: endpoint delay up to ${[0.5,0.75,1.2][index]} s. This is finalization timing, not total caption latency.`:`Giữ nguyên model Soniox: thời gian chờ chốt câu tối đa ${[0.5,0.75,1.2][index]} giây. Đây là thời gian chốt, không phải độ trễ tổng.`;
  if(model==="nemotron-3.5-q8")return en?`Same Nemotron weights: finalize after ${[0.25,0.3,0.55][index]} s of silence. Recognition speed still depends on CPU.`:`Giữ nguyên model Nemotron: chờ ${[0.25,0.3,0.55][index]} giây im lặng để chốt đoạn. Tốc độ nhận dạng vẫn phụ thuộc CPU.`;
  return en?`Same Moonshine model: ${[0.25,0.5,0.5][index]} s voice-detection window; maximum phrase ${[10,15,25][index]} s. Partial text can still change.`:`Giữ nguyên model Moonshine: cửa sổ phát hiện giọng ${[0.25,0.5,0.5][index]} giây, đoạn nói tối đa ${[10,15,25][index]} giây. Chữ tạm vẫn có thể thay đổi.`;
}
