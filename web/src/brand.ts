import { useSyncExternalStore } from "react";

// The official crest is a file the API serves at /brand/crest.svg (404 without it).
// When it loads, the mark and favicon switch to it; otherwise the horseshoe mark stays.

let crestUrl: string | null = null;
const listeners = new Set<() => void>();

/** The favicon follows the browser's light/dark tabs: violet ink on light, wisteria on dark. */
function faviconSvg(svg: string) {
  const style = "<style>@media (prefers-color-scheme: dark){svg{color:#cfbff9 !important}}</style>";
  return svg.replace(/<svg\b[^>]*>/, (open) => open + style);
}

async function loadCrest() {
  try {
    const res = await fetch("/brand/crest.svg", { credentials: "same-origin" });
    if (!res.ok || !res.headers.get("content-type")?.startsWith("image/svg+xml")) return;
    const svg = await res.text();
    crestUrl = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
    const icon = document.querySelector<HTMLLinkElement>("link[rel='icon']");
    if (icon) icon.href = URL.createObjectURL(new Blob([faviconSvg(svg)], { type: "image/svg+xml" }));
    listeners.forEach((l) => l());
  } catch {
    // No crest: keep the horseshoe.
  }
}

void loadCrest();

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Object URL of the crest, or null while it loads or when this machine has none. */
export function useCrest() {
  return useSyncExternalStore(subscribe, () => crestUrl);
}
