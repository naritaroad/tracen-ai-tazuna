// Body clips: walks the motion graph with an AnimationMixer. Loops are held; to reach another
// loop the director plays the transition clips between them (game-native start → loop → end),
// cross-fading briefly where two clips meet. Clips load on first use; a request waits for them.

import * as THREE from "three";

import { findPath, isLoop, type MotionGraph } from "../motionGraph.ts";
import type { MotionClip } from "../vocab";

/** Fade lengths (s): leaving a loop mid-way needs more than joining two clips that meet. */
const FADE_FROM_LOOP = 0.28;
const FADE_JOIN = 0.12;
const FADE_DIRECT = 0.5;

export class MotionDirector {
  readonly mixer: THREE.AnimationMixer;
  private actions = new Map<string, THREE.AnimationAction>();
  private loading = new Map<string, Promise<void>>();
  private failed = new Set<string>();
  private current: { clip: MotionClip; action: THREE.AnimationAction } | null = null;
  private path: string[] = [];
  private pathGoal: string | null = null;
  private goal: string;
  private arrived = 0;
  private clock = 0;
  private disposed = false;
  /** Called when a loop starts playing. */
  onArrive: ((loop: string) => void) | null = null;

  constructor(
    root: THREE.Object3D,
    private graph: MotionGraph,
    private load: (clip: MotionClip) => Promise<THREE.AnimationClip>,
    private random: () => number = Math.random,
  ) {
    this.mixer = new THREE.AnimationMixer(root);
    this.goal = graph.rest;
    this.mixer.addEventListener("finished", (e) => {
      if (this.current && e.action === this.current.action) this.onTransitionEnd();
    });
  }

  /** Loads clips (once each); failures are logged and the clip stays unavailable. */
  ensure(ids: string[]): Promise<void> {
    return Promise.all(
      ids.map((id) => {
        if (this.actions.has(id)) return Promise.resolve();
        let p = this.loading.get(id);
        if (!p) {
          const clip = this.graph.clips.get(id);
          if (!clip) return Promise.resolve();
          p = this.load(clip)
            .then((anim) => {
              if (this.disposed) return;
              const action = this.mixer.clipAction(anim);
              action.setLoop(clip.loop ? THREE.LoopRepeat : THREE.LoopOnce, Infinity);
              action.clampWhenFinished = !clip.loop;
              this.actions.set(id, action);
            })
            .catch((err) => {
              this.failed.add(id);
              console.warn(`[avatar] clip ${id} failed to load`, err);
            });
          this.loading.set(id, p);
        }
        return p;
      }),
    ).then(() => undefined);
  }

  /** Loads every clip of the set one after another, so the first gestures do not wait. */
  async preloadAll(): Promise<void> {
    for (const id of this.graph.clips.keys()) {
      if (this.disposed) return;
      await this.ensure([id]);
    }
  }

  /** Starts in a loop with no fade (first frame). */
  start(loop: string) {
    this.goal = loop;
    this.play(loop, 0);
  }

  get node(): string | null {
    return this.current?.clip.loop ? this.current.clip.id : null;
  }

  /** Seconds since the current loop started; 0 while in a transition. */
  get loopElapsed(): number {
    return this.node ? this.clock - this.arrived : 0;
  }

  get target(): string {
    return this.goal;
  }

  /** Go to a loop. In a transition the request waits for it to finish. */
  goTo(loop: string) {
    if (!isLoop(this.graph, loop) || this.failed.has(loop)) return;
    this.goal = loop;
    const cur = this.current;
    if (!cur || !cur.clip.loop || cur.clip.id === loop) return;
    const path = findPath(this.graph, cur.clip.id, loop);
    const needed = [...(path ?? []), loop];
    if (needed.some((id) => this.failed.has(id))) return;
    if (needed.some((id) => !this.actions.has(id))) {
      void this.ensure(needed).then(() => {
        if (this.goal === loop && this.current?.clip.loop) this.goTo(loop);
      });
      return;
    }
    // Unreachable through clips: fade straight to the loop.
    this.path = path ?? [loop];
    this.pathGoal = loop;
    const first = this.path.shift()!;
    this.play(first, path ? FADE_FROM_LOOP : FADE_DIRECT);
  }

  private onTransitionEnd() {
    const node = this.current!.clip.to;
    if (this.pathGoal !== this.goal) {
      // The goal changed during the transition: plan again from where it ends.
      this.path = findPath(this.graph, node, this.goal) ?? [this.goal];
      this.pathGoal = this.goal;
    }
    let next = this.path[0] ?? node;
    if (this.failed.has(next)) {
      this.path = [];
      next = this.graph.rest;
    }
    if (!this.actions.has(next)) {
      // Hold the last (clamped) frame until the clip has loaded.
      if (this.failed.has(next)) return;
      void this.ensure([next]).then(() => {
        if (this.current && !this.current.clip.loop && this.current.clip.to === node) this.onTransitionEnd();
      });
      return;
    }
    if (this.path[0] === next) this.path.shift();
    const joins = this.graph.clips.get(next)?.from === node;
    this.play(next, joins ? FADE_JOIN : FADE_DIRECT);
  }

  private play(id: string, fade: number) {
    const action = this.actions.get(id);
    const clip = this.graph.clips.get(id);
    if (!action || !clip) return;
    const prev = this.current?.action;
    action.reset();
    action.enabled = true;
    action.setEffectiveWeight(1);
    // Loops run at slightly different speeds each time, so repeats do not look mechanical.
    action.setEffectiveTimeScale(clip.loop ? 0.94 + this.random() * 0.12 : 1);
    action.play();
    if (prev && prev !== action) {
      if (fade > 0) prev.crossFadeTo(action, fade, false);
      else prev.stop();
    }
    this.current = { clip, action };
    if (clip.loop) {
      this.arrived = this.clock;
      this.onArrive?.(id);
    }
  }

  update(dt: number) {
    this.clock += dt;
    this.mixer.update(dt);
  }

  dispose() {
    this.disposed = true;
    this.mixer.stopAllAction();
    this.mixer.uncacheRoot(this.mixer.getRoot());
  }
}
