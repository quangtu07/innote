import { vi, type UiStrings } from "./vi";
import { en } from "./en";
import { ko } from "./ko";
import { zh } from "./zh";

export { locales, localeNames, localeEnglishNames, localeTags, isUiLocale, fill } from "./shared";
export type { UiLocale } from "./shared";
import type { UiLocale } from "./shared";

export const uiStrings: Record<UiLocale, UiStrings> = {
  vi,
  en: { ...vi, ...en },
  ko: { ...vi, ...en, ...ko },
  zh: { ...vi, ...en, ...zh },
};

export function stringsFor(locale: UiLocale): UiStrings {
  return uiStrings[locale];
}