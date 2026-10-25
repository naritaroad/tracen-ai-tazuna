// Return to the page the trainer was on after the page reloads.
// The Android app (Capacitor) reloads its start URL "/" when the WebView is recreated,
// for example around an Android permission prompt; without this the open conversation is lost.

import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router";

const KEY = "tracen.lastPath";

// Once per page load, not on every visit to "/": choosing "new conversation" must stay on "/".
let restoredThisLoad = false;

function read(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

function write(path: string): void {
  try {
    localStorage.setItem(KEY, path);
  } catch {
    /* storage blocked: nothing to restore later */
  }
}

/** Forget the page, e.g. on sign-out, so the next account does not open the previous one's conversation. */
export function forgetLastPath(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* storage blocked */
  }
}

export function RouteMemory() {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();

  useEffect(() => {
    if (!restoredThisLoad) {
      restoredThisLoad = true;
      const last = read();
      if (pathname === "/" && last && last !== "/" && !last.startsWith("/login")) {
        navigate(last, { replace: true });
        return;
      }
    }
    if (!pathname.startsWith("/login")) write(pathname + search);
  }, [pathname, search, navigate]);

  return null;
}
