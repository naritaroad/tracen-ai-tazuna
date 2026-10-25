// Tapping the character (docs/character/avatar-contract.md §5): a short line and a gesture from the fixed
// vocabulary. Pure, so it can be tested without a browser; the words are language-file keys.

import type { Emotion, Gesture } from "./vocab";

export type TapZone = "head" | "body";

export interface TapReaction {
  /** Language-file key of the line she says. */
  key: string;
  gesture: Gesture;
  emotion: Emotion;
  intensity: number;
}

const BODY: TapReaction[] = [
  { key: "avatar.tap.body1", gesture: "wave", emotion: "happy", intensity: 0.6 },
  { key: "avatar.tap.body2", gesture: "happy", emotion: "excited", intensity: 0.55 },
  { key: "avatar.tap.body3", gesture: "clap", emotion: "happy", intensity: 0.7 },
  { key: "avatar.tap.body4", gesture: "present", emotion: "gentle", intensity: 0.6 },
];

const HEAD: TapReaction[] = [
  { key: "avatar.tap.head1", gesture: "hand_chest", emotion: "surprised", intensity: 0.45 },
  { key: "avatar.tap.head2", gesture: "happy", emotion: "playful", intensity: 0.6 },
];

const SHY: TapReaction = { key: "avatar.tap.shy", gesture: "hand_chest", emotion: "troubled", intensity: 0.5 };

/** Pages with a line of their own, by the first part of the path ("" and "c" are the chat). */
const PAGE_LINES: Record<string, string> = {
  chat: "avatar.tap.page.chat",
  info: "avatar.tap.page.info",
  calendar: "avatar.tap.page.calendar",
  services: "avatar.tap.page.services",
  planner: "avatar.tap.page.planner",
  director: "avatar.tap.page.director",
  memory: "avatar.tap.page.memory",
};

export function pageOf(pathname: string): string | null {
  const first = pathname.split("/")[1] ?? "";
  const page = first === "" || first === "c" ? "chat" : first;
  return page in PAGE_LINES ? page : null;
}

/** Three taps on the head within a few seconds make her shy. */
export const SHY_STREAK = 3;

/**
 * Picks her reaction. `streak` counts recent taps (this one included); `last` is the previous
 * line's key, never repeated straight away. A body tap sometimes answers about the current page.
 */
export function pickTap(zone: TapZone, streak: number, page: string | null, random: () => number, last: string | null): TapReaction {
  if (zone === "head" && streak >= SHY_STREAK) return SHY;
  const pool = zone === "head" ? [...HEAD] : [...BODY];
  if (zone === "body" && page && PAGE_LINES[page]) pool.push({ key: PAGE_LINES[page], gesture: "guide", emotion: "gentle", intensity: 0.6 });
  const fresh = pool.filter((r) => r.key !== last);
  // The page line is offered more often: it is the one that helps.
  const weights = fresh.map((r) => (r.key.startsWith("avatar.tap.page.") ? 2 : 1));
  let r = random() * weights.reduce((a, b) => a + b, 0);
  for (let i = 0; i < fresh.length; i++) {
    r -= weights[i];
    if (r <= 0) return fresh[i];
  }
  return fresh[fresh.length - 1];
}
