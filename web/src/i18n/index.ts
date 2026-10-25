// UI copy lives in ./locales/<code>.json, one file per language; components only hold keys.
// Academy content (information, facilities, calendar, character names) comes from the API in the
// requested locale instead, so staff can edit it without a release. See docs/frontend/frontend-i18n.md.

import i18n, { type BackendModule } from "i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import { initReactI18next } from "react-i18next";

/** Shape of every language file; zh-TW.json is the reference (scripts/check-i18n.mjs keeps the others in step). */
export type Messages = typeof import("./locales/zh-TW.json");

/**
 * Languages offered in the menu. `label` is the language's own name, shown the same in every UI
 * language, so it is configuration rather than translatable copy. Adding a language: add
 * locales/<code>.json and one entry here.
 */
export const LOCALES = [
  { code: "zh-TW", label: "繁體中文" },
  { code: "ja", label: "日本語" },
  // en-GB internally; the menu just says English.
  { code: "en-GB", label: "English" },
] as const;

export type LocaleCode = (typeof LOCALES)[number]["code"];

export const DEFAULT_LOCALE: LocaleCode = "zh-TW";

/** Map a browser language to one of ours: zh-* -> zh-TW, ja-* -> ja, everything else -> en-GB. */
export function toLocale(lang: string | undefined): LocaleCode {
  const l = (lang ?? "").toLowerCase();
  if (l.startsWith("zh")) return "zh-TW";
  if (l.startsWith("ja")) return "ja";
  return "en-GB";
}

// Each language is its own chunk, fetched only when that language (or the fallback) is in use.
const bundles = import.meta.glob<{ default: Messages }>("./locales/*.json");

const lazyBundles: BackendModule = {
  type: "backend",
  init() {},
  read(language, _namespace, callback) {
    const load = bundles[`./locales/${language}.json`];
    if (!load) return callback(new Error(`no UI copy for ${language}`), false);
    load().then(
      (m) => callback(null, m.default),
      (e: Error) => callback(e, false),
    );
  },
};

function applyDocumentLanguage(lng: string) {
  document.documentElement.lang = lng;
  document.title = i18n.t("app.short");
}

/** Resolves once the current language and the fallback are loaded; render the app after this. */
export const i18nReady = i18n
  .use(lazyBundles)
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    supportedLngs: LOCALES.map((l) => l.code),
    fallbackLng: DEFAULT_LOCALE,
    load: "currentOnly",
    // The system language decides only the first visit; a choice the user makes is kept in localStorage.
    detection: {
      order: ["localStorage", "navigator"],
      lookupLocalStorage: "tracen.locale",
      caches: ["localStorage"],
      convertDetectedLanguage: toLocale,
    },
    interpolation: { escapeValue: false },
    // changeLanguage() loads the next bundle before switching, so the page never shows bare keys.
    react: { useSuspense: false },
  })
  .then(() => applyDocumentLanguage(i18n.resolvedLanguage ?? DEFAULT_LOCALE));

i18n.on("languageChanged", applyDocumentLanguage);

export default i18n;
