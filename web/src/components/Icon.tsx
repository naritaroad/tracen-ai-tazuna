// Decorative line icons. They carry no words: the button's text or aria-label names the action.

const PATHS = {
  plus: "M12 5v14M5 12h14",
  mic: "M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3zM5 11a7 7 0 0 0 14 0M12 18v3",
  stop: "M7 7h10v10H7z",
  newChat: "M12 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-6M17 3l4 4-8 8H9v-4z",
  menu: "M4 7h16M4 12h16M4 17h10",
  sidebar: "M5 4h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM9 4v16",
  close: "M6 6l12 12M18 6L6 18",
  search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
  chat: "M5 5h14a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H10l-5 4V6a1 1 0 0 1 1-1z",
  info: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v6M12 7.5v.5",
  calendar: "M5 6h14a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zM4 10h16M8 3v4M16 3v4",
  track: "M3 12c0-3 4-5 9-5s9 2 9 5-4 5-9 5-9-2-9-5zM8 12c0-1 1.8-1.6 4-1.6s4 .6 4 1.6-1.8 1.6-4 1.6-4-.6-4-1.6z",
  clock: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 7v5l3 2",
  director: "M3 21h18M5 21V10M19 21V10M9 21v-5h6v5M2 10l10-6 10 6",
  user: "M12 4a4 4 0 1 0 0 8 4 4 0 0 0 0-8zM4 21a8 8 0 0 1 16 0",
  shield: "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z",
  inbox: "M4 13l2-8h12l2 8v6H4zM4 13h5l1 2h4l1-2h5",
  database: "M12 4c4.4 0 8 1.3 8 3s-3.6 3-8 3-8-1.3-8-3 3.6-3 8-3zM4 7v10c0 1.7 3.6 3 8 3s8-1.3 8-3V7M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3",
  activity: "M3 12h4l3-8 4 16 3-8h4",
  users: "M9 4a4 4 0 1 0 0 8 4 4 0 0 0 0-8zM2 21a7 7 0 0 1 14 0M16 4a4 4 0 0 1 0 8M22 21a7 7 0 0 0-4-6.3",
  sliders: "M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1M15 4v4M9 10v4M17 16v4",
  back: "M19 12H5M11 6l-6 6 6 6",
  arrowUp: "M12 19V5M6 11l6-6 6 6",
  arrowDown: "M12 5v14M6 13l6 6 6-6",
  copy: "M9 9h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V10a1 1 0 0 1 1-1zM5 15H4V4h11v1",
  check: "M5 12l5 5 9-10",
  logout: "M15 4h4v16h-4M10 8l-4 4 4 4M6 12h10",
  character: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM9 10v1M15 10v1M8.5 14.5c1 1.2 2.2 1.8 3.5 1.8s2.5-.6 3.5-1.8",
  grid: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
  chevronUp: "M6 15l6-6 6 6",
  pin: "M9 3h6M10 3v6l-3 4h10l-3-4V3M12 13v8",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg className="icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d={PATHS[name]} />
    </svg>
  );
}
