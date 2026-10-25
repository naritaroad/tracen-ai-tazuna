// Planner, motion graph and speech timing of the character (npm test; Node runs the TypeScript directly).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { planPerformance, resolveGesture, THINK_DELAY_MS } from "../src/avatar/behavior.ts";
import { buildGraph, findPath, loopsTagged } from "../src/avatar/motionGraph.ts";
import { mouthAt, planSpeech } from "../src/avatar/speech.ts";
import { pageOf, pickTap } from "../src/avatar/taps.ts";
import type { AvatarIntent } from "../src/avatar/states";
import type { CharacterBehavior, MotionClip, MotionSet } from "../src/avatar/vocab";

const behaviorFile = JSON.parse(readFileSync(new URL("../../assets/characters/9001_hayakawa_tazuna/behavior.json", import.meta.url), "utf8"));
const behavior: CharacterBehavior = behaviorFile;

const clip = (id: string, from: string, to: string, gestures: string[] = []): MotionClip => ({ id, url: `/x/${id}`, loop: from === to && from === id, duration: 2, from, to, gestures });

/** The committed video-fitted set, as the server builds it from behavior.json. */
function videoSet(): MotionSet {
  const vf = behaviorFile.motion_sets.video_fit;
  const loops = Object.entries(vf.loops as Record<string, string[]>).map(([id, g]) => clip(id, id, id, g));
  const edges = (vf.transitions as { clip: string; from: string; to: string }[]).map((t) => clip(t.clip, t.from, t.to));
  return { id: "video_fit", rest: vf.rest, clips: [...loops, ...edges] };
}

/** A game-style set: every motion has start/loop/end through the neutral pose. */
function gameSet(): MotionSet {
  const gestures = behaviorFile.motion_sets.game.gestures as Record<string, string[]>;
  const clips: MotionClip[] = [];
  for (const m of ["idle", "bow", "wave", "present", "hand_chest", "explain", "think", "hope", "clap"]) {
    const loop = `${m}_loop`;
    clips.push(clip(`${m}_s`, m === "hope" ? "idle_loop" : "neutral", loop), clip(loop, loop, loop, gestures[m] ?? []), clip(`${m}_e`, loop, "neutral"));
  }
  return { id: "game", rest: "idle_loop", clips };
}

const intent = (over: Partial<AvatarIntent>): AvatarIntent => ({
  state: "explaining",
  expression: "smile",
  priority: 2,
  min_hold_ms: 0,
  interruptible: true,
  reason: "message.completed",
  character_id: "9001_hayakawa_tazuna",
  emotion: "gentle",
  intensity: 0.5,
  gesture: "hand_chest",
  ...over,
});

/** Deterministic random: a fixed sequence. */
const seq = (...xs: number[]) => {
  let i = 0;
  return () => xs[i++ % xs.length];
};

test("paths go through the clips between loops", () => {
  const g = buildGraph(videoSet());
  assert.deepEqual(findPath(g, "idle_polite", "chest_hold"), ["idle_to_chest"]);
  assert.deepEqual(findPath(g, "chin_hold", "chest_hold"), ["chin_to_idle", "idle_to_chest"]);
  assert.deepEqual(findPath(g, "chest_hold", "chest_hold"), []);
  const game = buildGraph(gameSet());
  assert.deepEqual(findPath(game, "think_loop", "bow_loop"), ["think_e", "bow_s"]);
  assert.deepEqual(findPath(game, "present_loop", "hope_loop"), ["present_e", "idle_s", "hope_s"]);
  assert.equal(findPath(buildGraph({ id: "x", rest: "a", clips: [clip("a", "a", "a"), clip("b", "b", "b")] }), "a", "b"), null);
});

test("every gesture of the vocabulary resolves in both sets, or is knowingly absent", () => {
  const gestures = ["greet", "bow", "wave", "present", "guide", "explain", "hand_chest", "think", "hope", "raise_hand", "happy", "clap"];
  const video = buildGraph(videoSet());
  for (const gest of gestures) {
    const r = resolveGesture(video, behavior, gest);
    assert.ok(r, `video_fit has no clip or fallback for ${gest}`);
  }
  const game = buildGraph(gameSet());
  assert.equal(resolveGesture(game, behavior, "greet")?.gesture, "greet");
  assert.equal(resolveGesture(game, behavior, "raise_hand")?.gesture, "present");
  assert.ok(loopsTagged(video, "emotion:playful").some((c) => c.id === "chin_hold"));
});

test("a reply plans a one-shot gesture, then a talking posture", () => {
  const graph = buildGraph(gameSet());
  const p = planPerformance({ intent: intent({ gesture: "bow", emotion: "sincere" }), behavior, graph, random: seq(0.1, 0.9), recent: [], reducedMotion: false });
  assert.equal(p.steps.length, 2);
  assert.deepEqual([p.steps[0].loop, p.steps[0].hold], ["bow_loop", { kind: "loops", n: 0.3 }]);
  assert.equal(p.steps[1].hold.kind, "speech");
  assert.ok(["hand_chest_loop", "explain_loop"].includes(p.steps[1].loop));
  assert.equal(p.gaze, "user");
  assert.ok((p.face.smile ?? 0) > 0);
});

test("without a bow clip the bow becomes a procedural accent", () => {
  const graph = buildGraph(videoSet());
  const p = planPerformance({ intent: intent({ state: "handoff", gesture: "bow", emotion: "sincere" }), behavior, graph, random: seq(0.9), recent: [], reducedMotion: false });
  assert.deepEqual(p.steps.map((s) => s.loop), ["chest_hold"]);
  assert.ok(p.accents.some((a) => a.kind === "bow" && a.amount > 0.7));
});

test("thinking waits before the thinking pose and looks away", () => {
  const graph = buildGraph(gameSet());
  const p = planPerformance({ intent: intent({ state: "thinking", reason: "user.sent", gesture: "think", emotion: "neutral" }), behavior, graph, random: seq(0.5), recent: [], reducedMotion: false });
  assert.deepEqual(p.steps, [{ loop: "think_loop", gesture: "think", hold: { kind: "state" }, delayMs: THINK_DELAY_MS }]);
  assert.equal(p.gaze, "think");
  assert.ok(p.accents.some((a) => a.kind === "nod" && a.atMs === 0), "acknowledges the message with a nod");
  assert.ok((p.face.thinking ?? 0) > 0.5);
});

test("recent loops are picked less often", () => {
  const graph = buildGraph(gameSet());
  const pick = (recent: string[]) =>
    planPerformance({ intent: intent({ gesture: "greet", reason: "greeting", state: "idle" }), behavior, graph, random: seq(0.3), recent, reducedMotion: false }).steps[0].loop;
  assert.equal(pick([]), "bow_loop");
  assert.equal(pick(["bow_loop"]), "wave_loop");
});

test("reduced motion keeps expressions but drops gestures and accents", () => {
  const p = planPerformance({ intent: intent({ gesture: "clap", emotion: "excited", intensity: 1 }), behavior, graph: buildGraph(gameSet()), random: seq(0.1), recent: [], reducedMotion: true });
  assert.deepEqual([p.steps, p.accents, p.headTilt], [[], [], 0]);
  assert.ok((p.face.happy ?? 0) > 0.5);
});

test("idle and listening never gesture; a missing set still gives a face", () => {
  const idle = planPerformance({ intent: intent({ state: "idle", gesture: null, emotion: "neutral" }), behavior: {}, graph: null, random: seq(0.5), recent: [], reducedMotion: false });
  assert.deepEqual(idle.steps, []);
  const listening = planPerformance({ intent: intent({ state: "listening", gesture: "wave" }), behavior, graph: buildGraph(gameSet()), random: seq(0.5), recent: [], reducedMotion: false });
  assert.deepEqual([listening.steps, listening.gaze], [[], "chat"]);
  const legacy = planPerformance({ intent: intent({ emotion: undefined, state: "apologizing" }), behavior: {}, graph: null, random: seq(0.5), recent: [], reducedMotion: false });
  assert.ok((legacy.face.sad ?? 0) > 0, "falls back to preset expressions");
});

test("Japanese kana map to their vowels; glides and long marks change the mora", () => {
  const p = planSpeech("きょうは", "ja");
  assert.deepEqual(p.syllables.map((s) => s.v), ["oh", "ou", "aa"]);
  const long = planSpeech("ラーメン", "ja");
  assert.equal(long.syllables[0].d, 250);
  assert.equal(long.syllables.at(-1)!.v, "ee", "trailing ん is closed and trimmed");
});

test("sentence ends become beats, trailing silence is not speech", () => {
  const p = planSpeech("好的。請稍等，我查一下。", "zh-TW");
  assert.equal(p.beats.length, 1, "the final period is not a beat");
  // Nine syllables, one sentence pause and one comma pause.
  assert.equal(p.durationMs, 9 * 215 + 380 + 200);
  assert.equal(p.syllables.at(-1)!.v !== null, true);
});

test("long replies speed up a little and then stop at the limit", () => {
  const text = "訓練員您好，".repeat(60);
  const p = planSpeech(text, "zh-TW", 5000);
  assert.ok(p.truncated);
  assert.ok(p.durationMs <= 5000);
  assert.ok(p.syllables[1].t < 215, "spoken faster than normal");
});

test("English syllables follow vowel groups", () => {
  const p = planSpeech("Please make sure", "en-GB");
  assert.deepEqual(p.syllables.filter((s) => s.v).map((s) => s.v), ["ee", "aa", "ou"]);
});

test("the mouth opens and closes within a syllable", () => {
  const p = planSpeech("あ", "ja");
  const mid = mouthAt(p, 60);
  assert.equal(mid.v, "aa");
  assert.ok(mid.w > 0.4);
  assert.equal(mouthAt(p, 1000).v, null);
});

test("a tap is a one-shot gesture said once, then back to rest", () => {
  const graph = buildGraph(gameSet());
  const p = planPerformance({ intent: intent({ state: "idle", reason: "poke", gesture: "wave", emotion: "happy" }), behavior, graph, random: seq(0.1), recent: [], reducedMotion: false });
  assert.equal(p.steps.length, 1);
  assert.equal(p.steps[0].gesture, "wave");
  // Without the poke reason, an idle intent never gestures.
  assert.equal(planPerformance({ intent: intent({ state: "idle", reason: "settle", gesture: "wave" }), behavior, graph, random: seq(0.1), recent: [], reducedMotion: false }).steps.length, 0);
});

test("tap lines: shy after three pats, no repeat, page line on body taps", () => {
  assert.equal(pickTap("head", 3, "info", seq(0), null).key, "avatar.tap.shy");
  for (let i = 0; i < 10; i++) {
    const r = pickTap("body", 1, null, seq(i / 10), "avatar.tap.body1");
    assert.notEqual(r.key, "avatar.tap.body1");
  }
  const keys = new Set(Array.from({ length: 20 }, (_, i) => pickTap("body", 1, "calendar", seq(i / 20), null).key));
  assert.ok(keys.has("avatar.tap.page.calendar"));
  assert.equal(pageOf("/c/123"), "chat");
  assert.equal(pageOf("/admin/runs"), null);
});
