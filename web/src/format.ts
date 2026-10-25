// Locale-aware formatting. Words and their order come from the language files; numbers, money and
// lists use Intl so each language gets its own separators.

import i18n from "./i18n";

// Dates from the API are RFC 3339; show them in academy time (Japan, UTC+9) for everyone.
export function formatDateTime(iso: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    timeZone: "Asia/Tokyo",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(iso));
}

export function hoursMinutes(total: number): string {
  return i18n.t("common.hoursMinutes", { h: Math.floor(total / 60), m: total % 60 });
}

export function formatNumber(n: number, locale: string): string {
  return new Intl.NumberFormat(locale).format(n);
}

export function formatYen(n: number, locale: string): string {
  return new Intl.NumberFormat(locale, { style: "currency", currency: "JPY" }).format(n);
}

/** "A, B and C" (conjunction) or "A, B or C" (disjunction), with each language's own separators. */
export function formatList(items: string[], locale: string, type: "conjunction" | "disjunction" = "conjunction"): string {
  return new Intl.ListFormat(locale, { style: "long", type }).format(items);
}

export function todayInAcademy(offsetDays = 0): string {
  const d = new Date(Date.now() + offsetDays * 86400000);
  return new Intl.DateTimeFormat("en-CA", { timeZone: "Asia/Tokyo" }).format(d); // YYYY-MM-DD
}

export function newId(): string {
  return crypto.randomUUID();
}
