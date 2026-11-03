export interface CaptionStyle {
  fontSize: number;
  align: "left" | "center";
  color: string;
  showOriginal: boolean;
  backgroundColor: string;
  transparent: boolean;
}
export const CAPTION_STYLE_KEY = "innote.caption.style";
export function readCaptionStyle(): CaptionStyle {
  let raw: any = null;
  try { raw = JSON.parse(localStorage.getItem(CAPTION_STYLE_KEY) ?? "null"); } catch { /* defaults */ }
  return {
    fontSize: Number.isFinite(raw?.fontSize) ? Math.min(64, Math.max(12, raw.fontSize)) : 22,
    align: raw?.align === "left" ? "left" : "center",
    color: /^#[0-9a-f]{6}$/i.test(raw?.color) ? raw.color : "#ffffff",
    showOriginal: raw?.showOriginal === true,
    backgroundColor: /^#[0-9a-f]{6}$/i.test(raw?.backgroundColor) ? raw.backgroundColor : "#172d29",
    transparent: raw?.transparent === true,
  };
}
