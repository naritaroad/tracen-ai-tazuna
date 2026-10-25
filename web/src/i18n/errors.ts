// The API answers with a stable error code and an English developer message; people see the code's
// translation, with the message kept only as detail for the codes that carry one.

import { ApiError } from "../api";
import i18n from "./index";

export function errorMessage(e: unknown): string {
  if (e instanceof ApiError) return i18n.t(`apiError.${e.code}`, { detail: e.message, defaultValue: e.message });
  // fetch() rejects with a TypeError when the server cannot be reached at all.
  if (e instanceof TypeError) return i18n.t("apiError.network");
  return e instanceof Error ? e.message : String(e);
}
