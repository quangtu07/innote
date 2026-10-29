export const locales = ["vi", "en", "ko", "zh"] as const;
export type UiLocale = (typeof locales)[number];

export const localeNames: Record<UiLocale, string> = {
  vi: "Tiếng Việt",
  en: "English",
  ko: "한국어",
  zh: "中文（简体）",
};

export const localeEnglishNames: Record<UiLocale, string> = {
  vi: "Vietnamese",
  en: "English",
  ko: "Korean",
  zh: "Chinese (Simplified)",
};

export const localeTags: Record<UiLocale, string> = {
  vi: "vi-VN",
  en: "en-US",
  ko: "ko-KR",
  zh: "zh-CN",
};

export function isUiLocale(value: unknown): value is UiLocale {
  return typeof value === "string" && locales.includes(value as UiLocale);
}

export function fill(template: string, values: Record<string, string | number>) {
  return template.replace(/\{(\w+)\}/g, (_, key: string) =>
    String(values[key] ?? `{${key}}`),
  );
}