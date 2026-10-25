// The 3D view of the character stage. Loaded on demand (three.js is a separate chunk), so the
// chat page does not wait for it; any failure is reported and the stage falls back to the 2D card.

import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { planPerformance } from "./behavior.ts";
import { buildGraph } from "./motionGraph.ts";
import { VrmPlayer, type Quality } from "./player/VrmPlayer.ts";
import { planSpeech } from "./speech.ts";
import type { AvatarIntent, AvatarSnapshot } from "./states";
import { SHY_STREAK, type TapReaction, type TapZone } from "./taps.ts";
import type { CharacterManifest, MotionSet } from "./vocab";

export interface VrmStageProps {
  character: CharacterManifest;
  set: MotionSet;
  snapshot: AvatarSnapshot;
  /** Said once when the stage is first ready (the chat's greeting line). */
  greeting?: string;
  /** Show the background scene behind her (the chat page), when the server has one. */
  backdrop: boolean;
  onReady: () => void;
  onFail: (err: unknown) => void;
  /** A tap while she is free: the line to say and the gesture, or null to stay quiet. */
  onTap?: (zone: TapZone, streak: number) => (TapReaction & { line: string }) | null;
}

/** Phones and small machines get the standard quality (Q11): lower resolution, 30 FPS. */
function detectQuality(): Quality {
  const nav = navigator as Navigator & { deviceMemory?: number; connection?: { saveData?: boolean } };
  const coarse = window.matchMedia("(pointer: coarse)").matches;
  const small = (nav.deviceMemory ?? 8) <= 4 || (nav.hardwareConcurrency ?? 8) <= 4;
  return coarse || small || nav.connection?.saveData ? "standard" : "high";
}

const NARROW = "(max-width: 760px)";
/** Taps closer together than this only count toward the shy streak; the gesture plays out. */
const TAP_GAP_MS = 1500;

export default function VrmStage({ character, set, snapshot, greeting, backdrop, onReady, onFail, onTap }: VrmStageProps) {
  const { i18n } = useTranslation();
  const host = useRef<HTMLDivElement>(null);
  const [player, setPlayer] = useState<VrmPlayer | null>(null);
  /** True until the character has greeted on this empty chat; leaving the empty chat re-arms it. */
  const greetPending = useRef(true);
  const graph = useRef(buildGraph(set));
  const callbacks = useRef({ onReady, onFail });
  callbacks.current = { onReady, onFail };
  // The player is made once; taps reach the current props through this.
  const poked = useRef<(zone: TapZone, streak: number) => void>(() => undefined);
  const lastTap = useRef(0);
  const restore = useRef<number | undefined>(undefined);

  useEffect(() => {
    if (!host.current) return;
    const abort = new AbortController();
    let made: VrmPlayer | null = null;
    graph.current = buildGraph(set);
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const narrow = window.matchMedia(NARROW);
    VrmPlayer.create({
      host: host.current,
      character,
      set,
      quality: detectQuality(),
      reducedMotion,
      chatSide: narrow.matches ? "below" : "left",
      signal: abort.signal,
      onPoke: (zone, streak) => poked.current(zone, streak),
    })
      .then((p) => {
        if (abort.signal.aborted) return p.dispose();
        made = p;
        // Development only: lets the browser console inspect the player.
        if (import.meta.env.DEV) Object.assign(window, { __tracenAvatar: p });
        setPlayer(p);
        callbacks.current.onReady();
      })
      .catch((err) => {
        if (!abort.signal.aborted) callbacks.current.onFail(err);
      });
    const side = () => made?.setChatSide(narrow.matches ? "below" : "left");
    narrow.addEventListener("change", side);
    return () => {
      abort.abort();
      narrow.removeEventListener("change", side);
      made?.dispose();
      window.clearTimeout(restore.current);
      setPlayer(null);
    };
  }, [character, set]);

  // Moving to another slot also changes the framing (its headroom), so this re-frames as well.
  useEffect(() => {
    player?.setBackdrop(character.scene, backdrop);
  }, [player, character, backdrop]);

  const intent = snapshot.intent;
  const latest = useRef({ intent, lang: i18n.language });
  latest.current = { intent, lang: i18n.language };
  const plan = useCallback(
    (p: VrmPlayer, i: AvatarIntent) =>
      planPerformance({
        intent: i,
        behavior: character.behavior ?? {},
        graph: graph.current,
        random: Math.random,
        recent: p.recent,
        reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
      }),
    [character],
  );

  useEffect(() => {
    if (player) player.perform(plan(player, intent));
  }, [player, intent, plan]);

  // On an empty chat the character greets once, with the chat's greeting line.
  useEffect(() => {
    if (!greeting) {
      greetPending.current = true;
      return;
    }
    const { intent: now, lang } = latest.current;
    if (!player || !greetPending.current || now.state !== "idle") return;
    greetPending.current = false;
    player.perform(plan(player, { ...now, reason: "greeting", emotion: "happy", intensity: 0.55, gesture: "greet" }));
    player.speak(planSpeech(greeting, lang));
  }, [player, greeting, plan]);

  // A tap: a one-shot gesture and a short line, then back to whatever she was doing.
  poked.current = (zone, streak) => {
    const now = performance.now();
    if (!player || (now - lastTap.current < TAP_GAP_MS && !(zone === "head" && streak === SHY_STREAK))) return;
    const said = onTap?.(zone, streak);
    if (!said) return;
    lastTap.current = now;
    const { intent: base, lang } = latest.current;
    player.perform(plan(player, { ...base, reason: "poke", emotion: said.emotion, intensity: said.intensity, gesture: said.gesture }));
    const speech = planSpeech(said.line, lang);
    player.speak(speech);
    window.clearTimeout(restore.current);
    restore.current = window.setTimeout(() => {
      if (latest.current.intent === base) player.perform(plan(player, base));
    }, Math.max(2600, speech.durationMs + 900));
  };

  const utterance = snapshot.utterance;
  useEffect(() => {
    if (player && utterance) player.speak(planSpeech(utterance.text, utterance.lang));
  }, [player, utterance]);

  return <div ref={host} className={`avatar-3d ${player ? "ready" : ""}`} />;
}
