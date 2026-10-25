// Character state machine (docs/character/avatar-contract.md). The server sends `avatar.intent`;
// this controller applies priority, minimum hold time and interruption rules, and remembers
// the reply being "spoken". The 3D player and the 2D placeholder read the same `AvatarSnapshot`.

import { useCallback, useEffect, useReducer, useRef } from "react";

import { estimateSpeechMs } from "./speech.ts";

export type AvatarStateName =
  | "idle"
  | "listening"
  | "thinking"
  | "working"
  | "explaining"
  | "confirming"
  | "handoff"
  | "apologizing"
  | "error";

export interface AvatarIntent {
  state: AvatarStateName;
  expression: string | null;
  priority: number;
  min_hold_ms: number;
  interruptible: boolean;
  reason: string;
  character_id: string;
  /** The cue: always present from the server; older servers send none. */
  emotion?: string;
  intensity?: number;
  gesture?: string | null;
  cue_source?: "model" | "rules" | "state";
  emotion_from_model?: { emotion: string; intensity: number; gesture: string | null };
  // Reserved; the server does not send these yet.
  look_at?: string;
  viseme?: string;
}

/** The reply text the character is saying (lip movement and nods). */
export interface Utterance {
  id: number;
  text: string;
  lang: string;
  durationMs: number;
}

export interface AvatarSnapshot {
  intent: AvatarIntent;
  since: number;
  utterance: Utterance | null;
}

const IDLE: AvatarIntent = {
  state: "idle",
  expression: null,
  priority: 0,
  min_hold_ms: 0,
  interruptible: true,
  reason: "initial",
  character_id: "9001_hayakawa_tazuna",
  emotion: "neutral",
  intensity: 0.5,
  gesture: null,
};

type Action =
  | { type: "intent"; intent: AvatarIntent; now: number; force?: boolean }
  | { type: "speak"; utterance: Utterance };

function reduce(current: AvatarSnapshot, action: Action): AvatarSnapshot {
  if (action.type === "speak") return { ...current, utterance: action.utterance };
  const { intent, now, force } = action;
  const held = now - current.since;
  const locked = !current.intent.interruptible || held < current.intent.min_hold_ms;
  if (!force && locked && intent.priority < current.intent.priority) return current;
  // A forced change (a new message, settling) ends whatever was being said.
  return { intent, since: now, utterance: force ? null : current.utterance };
}

/** After a reply settles, drift back to idle so the character does not freeze mid-gesture. */
const SETTLE_MS = 6000;
/** Time left to read the reply after the character has finished "saying" it. */
const AFTER_SPEECH_MS = 2500;
const SETTLE_MAX_MS = 20000;

export function useAvatarController() {
  const [snapshot, dispatch] = useReducer(reduce, { intent: IDLE, since: 0, utterance: null });
  const timer = useRef<number | undefined>(undefined);
  const utteranceId = useRef(0);

  const push = useCallback((intent: AvatarIntent, force = false) => {
    dispatch({ type: "intent", intent, now: performance.now(), force });
  }, []);

  /** A new user message always interrupts and goes back to thinking. */
  const userSent = useCallback(() => {
    push({ ...IDLE, state: "thinking", expression: "thinking", priority: 1, reason: "user.sent", gesture: "think" }, true);
  }, [push]);

  const listening = useCallback(
    (typing: boolean) => {
      if (snapshot.intent.state === "idle" || snapshot.intent.state === "listening") {
        push({ ...IDLE, state: typing ? "listening" : "idle", expression: typing ? "relaxed" : null, emotion: typing ? "gentle" : "neutral", reason: "composer" });
      }
    },
    [push, snapshot.intent.state],
  );

  /** The reply text arrived: the character says it. */
  const speak = useCallback((text: string, lang: string) => {
    utteranceId.current += 1;
    dispatch({ type: "speak", utterance: { id: utteranceId.current, text, lang, durationMs: estimateSpeechMs(text, lang) } });
  }, []);

  useEffect(() => {
    window.clearTimeout(timer.current);
    const s = snapshot.intent.state;
    if (s === "explaining" || s === "apologizing" || s === "handoff" || s === "error" || s === "confirming") {
      const speech = snapshot.utterance ? snapshot.utterance.durationMs + AFTER_SPEECH_MS : 0;
      const wait = Math.min(SETTLE_MAX_MS, Math.max(SETTLE_MS, snapshot.intent.min_hold_ms, speech));
      timer.current = window.setTimeout(() => push({ ...IDLE, reason: "settle" }, true), wait);
    }
    return () => window.clearTimeout(timer.current);
  }, [snapshot, push]);

  return { snapshot, push, userSent, listening, speak };
}
