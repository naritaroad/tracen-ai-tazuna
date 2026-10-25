// Procedural layer on top of the clips: where the character looks (eyes, then head), nods on
// sentence beats, a bow when the motion set has none, head tilt, breathing and a slight sway.
// It writes small rotations onto the normalized humanoid bones after the mixer has posed them.

import * as THREE from "three";
import type { VRM, VRMHumanBoneName } from "@pixiv/three-vrm";

import type { Accent, GazeMode } from "../behavior.ts";

const BONES = ["spine", "chest", "neck", "head"] as const satisfies readonly VRMHumanBoneName[];
type Bone = (typeof BONES)[number];

interface ActiveAccent {
  kind: Accent["kind"];
  start: number;
  amount: number;
  dur: number;
}

const DURATION: Record<Accent["kind"], number> = { nod: 0.45, bow: 1.9, tilt: 1.3, wink: 0 };

/** 0→1→0 with an ease-in, a hold and a slower ease-out (for bows). */
function holdCurve(u: number, rise = 0.25, hold = 0.25): number {
  const s = (x: number) => x * x * (3 - 2 * x);
  if (u < rise) return s(u / rise);
  if (u < rise + hold) return 1;
  return 1 - s(Math.min(1, (u - rise - hold) / (1 - rise - hold)));
}

/** Critically damped spring toward a target (Daniel Holden, "Spring-It-On"). */
function spring(x: number, v: number, target: number, halflife: number, dt: number): [number, number] {
  const y = (4 * Math.LN2) / (halflife + 1e-5) / 2;
  const j0 = x - target;
  const j1 = v + j0 * y;
  const e = Math.exp(-y * dt);
  return [e * (j0 + j1 * dt) + target, e * (v - j1 * y * dt)];
}

export class BodyLayer {
  readonly gazeTarget = new THREE.Object3D();
  private nodes = new Map<Bone, THREE.Object3D>();
  /** Each bone's pose before this frame's offsets (what the mixer left), put back after rendering. */
  private base = new Map<Bone, THREE.Quaternion>();
  private accents: ActiveAccent[] = [];
  private mode: GazeMode = "user";
  private pointer: THREE.Vector2 | null = null;
  private saccade = new THREE.Vector2();
  private nextSaccade = 0.5;
  private thinkSide = 1;
  private tiltGoal = 0;
  private tilt = 0;
  private tiltV = 0;
  private aim = new THREE.Vector3();
  private aimV = new THREE.Vector3();
  private eye = new THREE.Vector3();
  private headPos = new THREE.Vector3();
  private tmp = new THREE.Vector3();
  private euler = new THREE.Euler();
  private q = new THREE.Quaternion();
  /** Called on large gaze shifts (the face blinks with them). */
  onGazeShift: (() => void) | null = null;
  chatSide: "left" | "below" = "left";

  constructor(
    vrm: VRM,
    private camera: THREE.PerspectiveCamera,
    private random: () => number = Math.random,
    private lively = true,
  ) {
    for (const b of BONES) {
      const n = vrm.humanoid.getNormalizedBoneNode(b);
      if (n) {
        this.nodes.set(b, n);
        this.base.set(b, n.quaternion.clone());
      }
    }
    if (vrm.lookAt) vrm.lookAt.target = this.gazeTarget;
    this.aim.copy(camera.position);
    this.eye.copy(camera.position);
  }

  setGaze(mode: GazeMode) {
    if (mode === this.mode) return;
    if (mode === "think") this.thinkSide = this.random() < 0.5 ? -1 : 1;
    this.mode = mode;
    this.onGazeShift?.();
  }

  setPointer(ndc: THREE.Vector2 | null) {
    if (!ndc) this.pointer = null;
    else (this.pointer ??= new THREE.Vector2()).copy(ndc);
  }

  setTilt(rad: number) {
    this.tiltGoal = rad;
  }

  accent(kind: Accent["kind"], amount: number, t: number) {
    if (kind === "wink" || !this.lively) return;
    this.accents.push({ kind, start: t, amount, dur: DURATION[kind] * (kind === "nod" ? 0.85 + 0.3 * this.random() : 1) });
  }

  /** After rendering: take this frame's offsets off again, so they never pile up.
   *  The bones must not be reset to rest before the mixer instead: three.js writes a bone only
   *  when the clip's value changed since the last write, so on a held key the bone would stay at
   *  rest for a frame — the head (or the whole upper body) snapped 11–14° every few seconds. */
  restoreBones() {
    for (const [b, n] of this.nodes) n.quaternion.copy(this.base.get(b)!);
  }

  /** Where the eyes look, in camera space (metres at the camera's distance). */
  private gazeOffset(t: number): [number, number] {
    if (this.mode === "think") return [0.32 * this.thinkSide, 0.26 + 0.03 * Math.sin(t * 0.9)];
    if (this.mode === "down") return [0.12 + 0.05 * Math.sin(t * 0.6), -0.42];
    if (this.mode === "chat") return this.chatSide === "left" ? [-0.55, -0.12] : [0.05, -0.5];
    return [0, 0];
  }

  update(dt: number, t: number) {
    const cam = this.camera;
    // Eye target: the trainer (camera) or the pointer, plus small saccades.
    this.nextSaccade -= dt;
    if (this.nextSaccade <= 0) {
      const r = this.mode === "user" ? 0.03 : 0.05;
      this.saccade.set((this.random() * 2 - 1) * r, (this.random() * 2 - 1) * r * 0.6);
      this.nextSaccade = 0.6 + this.random() * 2.2;
    }
    const head = this.nodes.get("head");
    head?.getWorldPosition(this.headPos);
    const dist = cam.position.distanceTo(this.headPos);
    const right = this.tmp.set(1, 0, 0).applyQuaternion(cam.quaternion);
    const up = new THREE.Vector3(0, 1, 0).applyQuaternion(cam.quaternion);
    const goal = new THREE.Vector3();
    if (this.pointer && this.mode !== "think" && this.mode !== "down") {
      goal.set(this.pointer.x, this.pointer.y, 0.5).unproject(cam).sub(cam.position).normalize().multiplyScalar(dist).add(cam.position);
    } else {
      const [ox, oy] = this.gazeOffset(t);
      goal.copy(cam.position).addScaledVector(right, ox * dist).addScaledVector(up, oy * dist);
    }
    goal.addScaledVector(right, this.saccade.x * dist).addScaledVector(up, this.saccade.y * dist);
    // Eyes jump quickly; the head follows slowly, critically damped.
    this.eye.lerp(goal, 1 - Math.exp(-28 * dt));
    this.gazeTarget.position.copy(this.eye);
    for (const k of ["x", "y", "z"] as const) {
      const [p, v] = spring(this.aim[k], this.aimV[k], goal[k], 0.35, dt);
      this.aim[k] = p;
      this.aimV[k] = v;
    }

    // Head turn toward the aim point beyond what the eyes cover (model faces +Z).
    const d = this.aim.clone().sub(this.headPos);
    const yaw = THREE.MathUtils.clamp(Math.atan2(d.x, d.z), -0.6, 0.6);
    const pitch = THREE.MathUtils.clamp(-Math.atan2(d.y, Math.hypot(d.x, d.z)), -0.4, 0.4);

    [this.tilt, this.tiltV] = spring(this.tilt, this.tiltV, this.lively ? this.tiltGoal : 0, 0.5, dt);

    const off: Record<Bone, [number, number, number]> = { spine: [0, 0, 0], chest: [0, 0, 0], neck: [0, 0, 0], head: [0, 0, 0] };
    const add = (b: Bone, x: number, y: number, z: number) => {
      off[b][0] += x;
      off[b][1] += y;
      off[b][2] += z;
    };
    add("head", pitch * 0.35, yaw * 0.4, this.tilt);
    add("neck", pitch * 0.15, yaw * 0.2, this.tilt * 0.35);

    if (this.lively) {
      // Breathing and a slow sway; both far smaller than the clips' own movement.
      const breath = Math.sin((t * Math.PI * 2) / 4.2);
      add("chest", breath * 0.006, 0, 0);
      add("spine", breath * 0.003, 0, 0);
      add("head", 0, Math.sin(t * 0.37) * 0.012 + Math.sin(t * 0.91) * 0.006, Math.sin(t * 0.53 + 1) * 0.008);
    }

    this.accents = this.accents.filter((a) => t - a.start < a.dur);
    for (const a of this.accents) {
      const u = (t - a.start) / a.dur;
      if (a.kind === "nod") {
        // Down, back up with a small overshoot.
        const k = Math.sin(Math.PI * u) * (1 - 0.35 * u) - 0.12 * Math.sin(Math.PI * Math.min(1, Math.max(0, u - 0.6) / 0.4));
        add("head", 0.11 * a.amount * k, 0, 0);
        add("neck", 0.05 * a.amount * k, 0, 0);
      } else if (a.kind === "bow") {
        const k = holdCurve(u);
        add("spine", 0.24 * a.amount * k, 0, 0);
        add("chest", 0.12 * a.amount * k, 0, 0);
        add("neck", 0.1 * a.amount * k, 0, 0);
        add("head", 0.08 * a.amount * k, 0, 0);
      } else if (a.kind === "tilt") {
        add("head", 0, 0, -0.12 * a.amount * holdCurve(u, 0.3, 0.3));
      }
    }

    for (const [b, n] of this.nodes) {
      this.base.get(b)!.copy(n.quaternion);
      const [x, y, z] = off[b];
      if (x === 0 && y === 0 && z === 0) continue;
      n.quaternion.multiply(this.q.setFromEuler(this.euler.set(x, y, z, "YXZ")));
    }
  }
}
