// Behaviour planning (docs/character/avatar-behavior-design.md §3.2): an `avatar.intent` becomes a
// performance — target face, a short list of gesture steps, gaze, head tilt and accents.
// Pure and deterministic for a given `random`, so it can be tested without a browser.

import { loopsTagged, type MotionGraph } from "./motionGraph.ts";
import type { AvatarIntent } from "./states";
import type { CharacterBehavior, Emotion, FaceWeights, MotionClip } from "./vocab";

export type GazeMode = "user" | "think" | "down" | "chat";

export type Hold =
  | { kind: "loops"; n: number }
  /** Until the reply has been spoken (or a short pause when there is no speech). */
  | { kind: "speech" }
  /** Until the next performance. */
  | { kind: "state" };

export interface MotionStep {
  loop: string;
  gesture: string;
  hold: Hold;
  delayMs: number;
}

export interface Accent {
  kind: "nod" | "bow" | "tilt" | "wink";
  atMs: number;
  amount: number;
}

export interface Performance {
  state: AvatarIntent["state"];
  emotion: string;
  intensity: number;
  face: FaceWeights;
  steps: MotionStep[];
  gaze: GazeMode;
  /** Head roll in radians; positive leans toward the viewer's left. */
  headTilt: number;
  accents: Accent[];
}

export interface PlanInput {
  intent: AvatarIntent;
  behavior: CharacterBehavior;
  graph: MotionGraph | null;
  random: () => number;
  /** Loops used most recently, newest last; they are picked less often. */
  recent: readonly string[];
  reducedMotion: boolean;
}

/** For models without a behavior.json: VRM preset expressions only. */
const PRESET_EMOTIONS: Record<string, FaceWeights> = {
  neutral: {},
  gentle: { relaxed: 0.35 },
  happy: { happy: 0.6 },
  excited: { happy: 0.8, surprised: 0.15 },
  sincere: { relaxed: 0.3 },
  apologetic: { sad: 0.5 },
  troubled: { sad: 0.6 },
  surprised: { surprised: 0.7 },
  serious: { angry: 0.15 },
  playful: { happy: 0.45 },
};

const STATE_EMOTION: Record<string, Emotion> = {
  idle: "neutral",
  listening: "gentle",
  thinking: "neutral",
  working: "serious",
  explaining: "gentle",
  confirming: "happy",
  handoff: "sincere",
  apologizing: "apologetic",
  error: "troubled",
};

/** States in which the character talks, so a gesture is held while speaking. */
const SPEAKING = new Set(["explaining", "confirming", "handoff", "apologizing"]);
const BUSY = new Set(["thinking", "working"]);
/** Waiting this long before the thinking pose avoids a gesture for replies that come at once. */
export const THINK_DELAY_MS = 900;

const clamp01 = (x: number) => Math.min(1, Math.max(0, x));

function addFace(into: FaceWeights, weights: FaceWeights | undefined, scale: number) {
  for (const [name, w] of Object.entries(weights ?? {})) into[name] = clamp01(Math.max(into[name] ?? 0, w * scale));
}

export function faceFor(intent: AvatarIntent, behavior: CharacterBehavior): FaceWeights {
  const emotion = intent.emotion ?? STATE_EMOTION[intent.state] ?? "neutral";
  const intensity = clamp01(intent.intensity ?? 0.5);
  const table = behavior.emotions ?? PRESET_EMOTIONS;
  const face: FaceWeights = {};
  // While busy, the state's face leads and the emotion only colours it.
  const busy = BUSY.has(intent.state);
  addFace(face, behavior.state_expressions?.[intent.state], 1);
  addFace(face, table[emotion], (busy ? 0.4 : 1) * (0.55 + 0.45 * intensity));
  if (!Object.keys(face).length && intent.expression) face[intent.expression] = 0.6;
  return face;
}

/** Weighted pick that avoids the most recent loops. */
function pick(loops: MotionClip[], recent: readonly string[], random: () => number): MotionClip {
  const weights = loops.map((c) => {
    const age = recent.length - 1 - recent.lastIndexOf(c.id);
    return recent.includes(c.id) ? (age === 0 ? 0.25 : 0.6) : 1;
  });
  let r = random() * weights.reduce((a, b) => a + b, 0);
  for (let i = 0; i < loops.length; i++) {
    r -= weights[i];
    if (r <= 0) return loops[i];
  }
  return loops[loops.length - 1];
}

/** The gesture, or the first fallback this motion set can show, with its loops. */
export function resolveGesture(graph: MotionGraph, behavior: CharacterBehavior, gesture: string): { gesture: string; loops: MotionClip[] } | null {
  for (const g of [gesture, ...(behavior.gesture_fallbacks?.[gesture] ?? [])]) {
    const loops = loopsTagged(graph, g).filter((c) => c.id !== graph.rest);
    if (loops.length) return { gesture: g, loops };
  }
  return null;
}

function planSteps(input: PlanInput, emotion: string): MotionStep[] {
  const { intent, behavior, graph, random, recent } = input;
  if (!graph || input.reducedMotion || !intent.gesture) return [];
  // A greeting or a reaction to a tap: one gesture while the line is said, then back to rest.
  const greeting = intent.reason === "greeting" || intent.reason === "poke";
  if (!greeting && !SPEAKING.has(intent.state) && !BUSY.has(intent.state)) return [];
  const first = resolveGesture(graph, behavior, intent.gesture);

  if (BUSY.has(intent.state)) {
    if (!first) return [];
    const loop = first.loops.find((c) => recent[recent.length - 1] === c.id) ?? pick(first.loops, recent, random);
    return [{ loop: loop.id, gesture: first.gesture, hold: { kind: "state" }, delayMs: intent.state === "thinking" ? THINK_DELAY_MS : 0 }];
  }

  const steps: MotionStep[] = [];
  const used: string[] = [...recent];
  // A loop that suits the emotion itself (e.g. the playful chin pose) is used about half the time.
  const mood = loopsTagged(graph, `emotion:${emotion}`).filter((c) => c.id !== graph.rest);
  const moodStep = (): MotionStep | null =>
    mood.length && random() < 0.5 ? { loop: pick(mood, used, random).id, gesture: `emotion:${emotion}`, hold: { kind: "speech" }, delayMs: 0 } : null;

  if (first) {
    const oneshot = behavior.oneshot?.[first.gesture];
    if (oneshot === undefined && !greeting) {
      const m = moodStep();
      if (m) return [m];
    }
    const loop = pick(first.loops, used, random);
    used.push(loop.id);
    // A loop's own gesture decides its length: a greeting as a wave waves longer than a greeting bow holds.
    const n = behavior.oneshot?.[loop.gestures[0]] ?? oneshot;
    steps.push({ loop: loop.id, gesture: first.gesture, hold: n !== undefined ? { kind: "loops", n } : { kind: "speech" }, delayMs: 0 });
    if (oneshot === undefined || greeting) return steps;
  }
  if (greeting) return steps;

  // After a one-shot (a bow, a wave), talk with a posture that suits the emotion.
  const m = moodStep();
  if (m) return [...steps, m];
  for (const g of behavior.postures?.[emotion] ?? ["hand_chest"]) {
    const posture = resolveGesture(graph, behavior, g);
    if (!posture) continue;
    const loops = posture.loops.filter((c) => c.id !== steps[0]?.loop);
    if (!loops.length) continue;
    steps.push({ loop: pick(loops, used, random).id, gesture: posture.gesture, hold: { kind: "speech" }, delayMs: 0 });
    break;
  }
  return steps;
}

export function planPerformance(input: PlanInput): Performance {
  const { intent, graph, random, reducedMotion } = input;
  const emotion = intent.emotion ?? STATE_EMOTION[intent.state] ?? "neutral";
  const intensity = clamp01(intent.intensity ?? 0.5);
  const steps = planSteps(input, emotion);
  const accents: Accent[] = [];

  if (!reducedMotion) {
    // Acknowledge the trainer's message before starting to think.
    if (intent.reason === "user.sent") accents.push({ kind: "nod", atMs: 0, amount: 0.55 });
    // A bow the motion set cannot show is done by the procedural layer instead.
    const bowing = intent.gesture === "bow" || intent.gesture === "greet";
    const canBow = graph && steps.some((s) => s.gesture === "bow" || s.gesture === "greet");
    if (bowing && !canBow && (SPEAKING.has(intent.state) || intent.reason === "greeting"))
      accents.push({ kind: "bow", atMs: 150, amount: intent.state === "handoff" ? 0.8 : 0.55 });
    if (emotion === "playful" && random() < 0.4) accents.push({ kind: "wink", atMs: 450, amount: 1 });
  }

  const gaze: GazeMode = intent.state === "thinking" ? "think" : intent.state === "working" ? "down" : intent.state === "listening" ? "chat" : "user";
  const tilt =
    intent.state === "thinking" ? 0.07 : intent.state === "listening" ? 0.05 : emotion === "playful" ? 0.08 : emotion === "apologetic" || emotion === "troubled" ? -0.04 : 0;
  return { state: intent.state, emotion, intensity, face: faceFor(intent, input.behavior), steps, gaze, headTilt: reducedMotion ? 0 : tilt, accents };
}
