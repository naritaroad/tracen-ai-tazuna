// Light / dark / follow the system. The choice is a per-viewer convenience kept in localStorage.

export type ThemeChoice = "light" | "dark" | "system";
const KEY = "tracen.theme";

export function loadTheme(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "light" || v === "dark") return v;
  } catch {
    /* storage blocked */
  }
  return "system";
}

export function applyTheme(choice: ThemeChoice): void {
  const root = document.documentElement;
  if (choice === "system") delete root.dataset.theme;
  else root.dataset.theme = choice;
  try {
    if (choice === "system") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, choice);
  } catch {
    /* storage blocked */
  }
}
