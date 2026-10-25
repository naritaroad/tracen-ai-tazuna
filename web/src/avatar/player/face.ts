// Face: blends expressions toward the performance's target, adds short accents (a wink, a
// pleased smile), blinks on its own, and moves the mouth along the speech timeline.

import type { VRM } from "@pixiv/three-vrm";

import { mouthAt, type SpeechPlan, type Vowel } from "../speech.ts";
import type { FaceWeights } from "../vocab";

const VOWELS: Vowel[] = ["aa", "ih", "ou", "ee", "oh"];
/** Expressions that already shape the mouth; speech opens it less over them. */
const MOUTH_HEAVY = ["happy", "grin", "surprised", "aa"];
/** Expressions that close the eyes; no blinking over them. */
const EYES_CLOSED = ["happy", "blink"];

interface Flash {
  weights: FaceWeights;
  start: number;
  end: number;
}

export class FaceDirector {
  private names: Set<string>;
  private target: FaceWeights = {};
  private value = new Map<string, number>();
  private flashes: Flash[] = [];
  private speech: { plan: SpeechPlan; start: number; hint: number } | null = null;
  private mouth = new Map<Vowel, number>();
  private nextBlink = 1.5;
  private blinkT = -1;
  private doubleBlink = false;
  private phase = new Map<string, number>();

  constructor(
    private vrm: VRM,
    private random: () => number = Math.random,
    private lively = true,
  ) {
    this.names = new Set((vrm.expressionManager?.expressions ?? []).map((e) => e.expressionName));
  }

  has(name: string): boolean {
    return this.names.has(name);
  }

  setBase(weights: FaceWeights) {
    this.target = Object.fromEntries(Object.entries(weights).filter(([n]) => this.names.has(n)));
  }

  /** A short expression on top of the base, fading in and out over `ms`. */
  flash(weights: FaceWeights, ms: number, now: number) {
    this.flashes.push({ weights, start: now, end: now + ms });
  }

  speak(plan: SpeechPlan | null, now: number) {
    this.speech = plan && plan.syllables.length ? { plan, start: now, hint: 0 } : null;
  }

  get speaking(): boolean {
    return this.speech !== null;
  }

  /** Blink soon (people blink with large gaze shifts). */
  blinkSoon() {
    if (this.blinkT < 0) this.nextBlink = Math.min(this.nextBlink, 0.05 + this.random() * 0.1);
  }

  private blink(dt: number, eyesClosed: number): number {
    if (this.blinkT >= 0) {
      this.blinkT += dt;
      // Close fast, open slower: 60 ms down, 30 ms shut, 110 ms up.
      const t = this.blinkT;
      const v = t < 0.06 ? t / 0.06 : t < 0.09 ? 1 : t < 0.2 ? 1 - (t - 0.09) / 0.11 : 0;
      if (t >= 0.2) {
        this.blinkT = -1;
        if (this.doubleBlink) {
          this.doubleBlink = false;
          this.nextBlink = 0.12;
        } else this.nextBlink = 1.8 + this.random() * 3.6;
      }
      return v * (1 - eyesClosed);
    }
    this.nextBlink -= dt;
    if (this.nextBlink <= 0 && eyesClosed < 0.5) {
      this.blinkT = 0;
      this.doubleBlink = this.random() < 0.12;
    }
    return 0;
  }

  update(dt: number, now: number, t: number) {
    const em = this.vrm.expressionManager;
    if (!em) return;

    // Target = base + active flashes (each a smooth bump).
    const goal: FaceWeights = { ...this.target };
    this.flashes = this.flashes.filter((f) => f.end > now);
    for (const f of this.flashes) {
      const u = (now - f.start) / (f.end - f.start);
      const k = Math.sin(Math.PI * Math.min(1, Math.max(0, u)));
      for (const [n, w] of Object.entries(f.weights)) if (this.names.has(n)) goal[n] = Math.max(goal[n] ?? 0, w * Math.min(1, k * 1.6));
    }

    // Expressions ease in quicker than they ease out.
    for (const n of new Set([...this.value.keys(), ...Object.keys(goal)])) {
      const cur = this.value.get(n) ?? 0;
      const want = goal[n] ?? 0;
      const rate = want > cur ? 7 : 4;
      let next = cur + (want - cur) * (1 - Math.exp(-rate * dt));
      if (Math.abs(next) < 0.002 && want === 0) next = 0;
      this.value.set(n, next);
    }

    let mouthHeavy = 0;
    let eyesClosed = 0;
    for (const [n, v] of this.value) {
      if (!this.phase.has(n)) this.phase.set(n, this.random() * Math.PI * 2);
      // A slight drift keeps a held expression from looking frozen.
      const drift = this.lively && v > 0.05 ? 1 + 0.05 * Math.sin(t * 0.7 + this.phase.get(n)!) : 1;
      em.setValue(n, Math.min(1, v * drift));
      if (MOUTH_HEAVY.includes(n)) mouthHeavy = Math.max(mouthHeavy, v);
      if (EYES_CLOSED.includes(n)) eyesClosed = Math.max(eyesClosed, v);
    }

    if (this.names.has("blink")) em.setValue("blink", Math.max(this.value.get("blink") ?? 0, this.blink(dt, eyesClosed)));

    // Mouth: the current syllable's vowel, softened.
    const open = new Map<Vowel, number>();
    if (this.speech) {
      const ms = now - this.speech.start;
      if (ms > this.speech.plan.durationMs + 200) this.speech = null;
      else {
        const m = mouthAt(this.speech.plan, ms, this.speech.hint);
        this.speech.hint = m.index;
        if (m.v) open.set(m.v, m.w * (1 - 0.45 * mouthHeavy));
      }
    }
    for (const v of VOWELS) {
      if (!this.names.has(v)) continue;
      const cur = this.mouth.get(v) ?? 0;
      const want = open.get(v) ?? 0;
      const next = cur + (want - cur) * (1 - Math.exp(-(want > cur ? 22 : 16) * dt));
      this.mouth.set(v, next < 0.003 ? 0 : next);
      // Vowels the base expression set (rare) keep their base value.
      em.setValue(v, Math.max(this.mouth.get(v)!, this.value.get(v) ?? 0));
    }
  }
}
