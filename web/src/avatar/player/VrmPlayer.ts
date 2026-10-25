// The 3D character player (docs/character/avatar-contract.md §1 AvatarStage). It owns its canvas, loads the
// VRM and the motion set, and turns each `Performance` into clips, expressions, gaze and accents.
// It never decides business state: performances come from the planner, speech from the reply text.

import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { VRMLoaderPlugin, VRMUtils, type VRM } from "@pixiv/three-vrm";
import { VRMAnimationLoaderPlugin, VRMLookAtQuaternionProxy, createVRMAnimationClip, type VRMAnimation } from "@pixiv/three-vrm-animation";

import type { Performance } from "../behavior.ts";
import { buildGraph, type MotionGraph } from "../motionGraph.ts";
import type { SpeechPlan } from "../speech.ts";
import type { TapZone } from "../taps.ts";
import type { CharacterManifest, MotionClip, MotionSet, SceneSpec } from "../vocab";
import { BodyLayer } from "./body.ts";
import { FaceDirector } from "./face.ts";
import { MotionDirector } from "./motion.ts";

export type Quality = "high" | "standard";

export interface PlayerOptions {
  host: HTMLElement;
  character: CharacterManifest;
  set: MotionSet;
  quality: Quality;
  reducedMotion: boolean;
  chatSide: "left" | "below";
  signal?: AbortSignal;
  /** A tap while she is free (idle or listening); `streak` counts taps in the last few seconds. */
  onPoke?: (zone: TapZone, streak: number) => void;
}

/** A gesture is held at least this long, even for a one-word reply. */
const MIN_DWELL_S = 1.4;
/** How long to wait for the reply text before treating the reply as unspoken. */
const SPEECH_WAIT_MS = 1500;
const UNSPOKEN_HOLD_S = 2.6;
/** Longest frame the clock follows (a background tab may skip far more); physics step length. */
const MAX_FRAME_S = 0.25;
const PHYSICS_STEP_S = 1 / 30;
/** She follows a mouse anywhere within this many canvas half-sizes of her centre. */
const POINTER_REACH = 8;
/** After this long without the pointer moving, she looks back at the trainer. */
const POINTER_IDLE_MS = 6000;

async function fetchBuffer(url: string, signal?: AbortSignal): Promise<ArrayBuffer> {
  const res = await fetch(url, { credentials: "same-origin", signal });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return res.arrayBuffer();
}

export class VrmPlayer {
  readonly canvas: HTMLCanvasElement;
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  // Far enough for the whole background corridor (about 35 m).
  private camera = new THREE.PerspectiveCamera(22, 3 / 4, 0.1, 80);
  private vrm!: VRM;
  private graph: MotionGraph;
  private motion!: MotionDirector;
  private face!: FaceDirector;
  private body!: BodyLayer;
  private animLoader = new GLTFLoader();
  private raf = 0;
  private last = 0;
  private time = 0;
  private visible = true;
  private onScreen = true;
  private disposed = false;
  private fps: number;
  private headY = 1.45;
  private topY = 1.65;
  private cleanups: (() => void)[] = [];
  private backdrop: THREE.Object3D | null = null;
  private backdropLoading: Promise<void> | null = null;
  private backdropWanted = false;
  private pointerAt = 0;

  private perf: Performance | null = null;
  private perfAt = 0;
  private step = -1;
  private stepStarted = false;
  private speechEnd: number | null = null;
  /** Scheduled work: gesture steps belong to one performance, speech nods to one utterance. */
  private pending: { at: number; kind: "step" | "accent" | "speech"; run: () => void }[] = [];
  private pokes: number[] = [];
  /** Loops chosen most recently (for the planner's variety). */
  readonly recent: string[] = [];

  private constructor(private opts: PlayerOptions) {
    this.canvas = document.createElement("canvas");
    this.canvas.className = "avatar-canvas";
    this.canvas.setAttribute("aria-hidden", "true");
    this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, alpha: true, antialias: true, powerPreference: opts.quality === "high" ? "high-performance" : "low-power" });
    this.renderer.setClearColor(0x000000, 0);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, opts.quality === "high" ? 2 : 1.5));
    this.fps = opts.quality === "high" ? 60 : 30;
    this.graph = buildGraph(opts.set);
    this.animLoader.register((parser) => new VRMAnimationLoaderPlugin(parser));
    this.light();
  }

  /** Cel lighting in the manner of the game: a warm key from high on her right, far enough to the
   *  side that the shade texture shows on her other side; a cool sky fill so the shade keeps its
   *  colour; and a back light that MToon turns into a rim. */
  private light() {
    const key = new THREE.DirectionalLight(0xfff3e4, 2.1);
    key.position.set(-2.0, 2.4, 0.9);
    const back = new THREE.DirectionalLight(0xe6ecff, 1.3);
    back.position.set(1.2, 1.6, -2.2);
    const sky = new THREE.HemisphereLight(0xf2efff, 0x6b5f78, 0.3);
    this.scene.add(key, back, sky);
  }

  /** MToon settings for the game look: the game's shade texture with a softer edge, a thin warm
   *  rim on the lit silhouette, and outlines a little stronger than the exported hairline. */
  private dressMaterials() {
    this.vrm.scene.traverse((o) => {
      const mesh = o as THREE.Mesh;
      if (!mesh.isMesh) return;
      for (const m of ([] as THREE.Material[]).concat(mesh.material)) {
        const mt = m as unknown as Record<string, unknown>;
        if (!mt.isMToonMaterial) continue;
        const set = (k: string, v: number) => {
          if (typeof mt[k] === "number") mt[k] = v;
        };
        set("shadingToonyFactor", 0.88);
        set("shadingShiftFactor", 0.05);
        set("giEqualizationFactor", 0.75);
        (mt.parametricRimColorFactor as THREE.Color | undefined)?.set(0x3a3242);
        set("parametricRimFresnelPowerFactor", 4.5);
        set("parametricRimLiftFactor", 0.06);
        set("rimLightingMixFactor", 0.9);
        if (typeof mt.outlineWidthFactor === "number" && mt.outlineWidthFactor > 0) set("outlineWidthFactor", mt.outlineWidthFactor * 1.35);
      }
    });
  }

  /** Loads everything; throws if WebGL or the model is unavailable (the stage then shows the 2D card). */
  static async create(opts: PlayerOptions): Promise<VrmPlayer> {
    const player = new VrmPlayer(opts);
    try {
      await player.load();
      return player;
    } catch (err) {
      player.dispose();
      throw err;
    }
  }

  private async load() {
    const { character, signal } = this.opts;
    if (!character.vrm_url) throw new Error("no model");
    const loader = new GLTFLoader();
    loader.register((parser) => new VRMLoaderPlugin(parser));
    const gltf = await loader.parseAsync(await fetchBuffer(character.vrm_url, signal), "");
    signal?.throwIfAborted();
    const vrm = gltf.userData.vrm as VRM | undefined;
    if (!vrm) throw new Error("not a VRM");
    VRMUtils.removeUnnecessaryVertices(gltf.scene);
    VRMUtils.combineSkeletons(gltf.scene);
    VRMUtils.rotateVRM0(vrm);
    vrm.scene.traverse((o) => (o.frustumCulled = false));
    // Clips may carry gaze tracks; the proxy receives them.
    const proxy = new VRMLookAtQuaternionProxy(vrm.lookAt!);
    proxy.name = "VRMLookAtQuaternionProxy";
    vrm.scene.add(proxy);
    this.vrm = vrm;
    this.scene.add(vrm.scene);
    this.dressMaterials();

    const lively = !this.opts.reducedMotion;
    this.motion = new MotionDirector(vrm.scene, this.graph, (clip) => this.loadClip(clip));
    this.motion.onArrive = (loop) => this.arrived(loop);
    this.face = new FaceDirector(vrm, Math.random, lively);
    this.body = new BodyLayer(vrm, this.camera, Math.random, lively);
    this.body.chatSide = this.opts.chatSide;
    this.body.onGazeShift = () => Math.random() < 0.45 && this.face.blinkSoon();

    const rest = this.graph.rest;
    await this.motion.ensure([rest]);
    signal?.throwIfAborted();
    this.motion.start(rest);
    this.settle();
    this.measure();
    void this.motion.preloadAll();

    this.opts.host.appendChild(this.canvas);
    this.listen();
    this.resize();
    this.raf = requestAnimationFrame(this.frame);
  }

  private async loadClip(clip: MotionClip): Promise<THREE.AnimationClip> {
    const gltf = await this.animLoader.parseAsync(await fetchBuffer(clip.url, this.opts.signal), "");
    const anim = (gltf.userData.vrmAnimations as VRMAnimation[] | undefined)?.[0];
    if (!anim) throw new Error(`${clip.id}: no VRM animation`);
    const out = createVRMAnimationClip(anim, this.vrm);
    out.name = clip.id;
    return out;
  }

  /** Pose the first frame and let the spring bones (hair, ribbon) come to rest before showing. */
  private settle() {
    this.motion.update(0);
    this.vrm.humanoid.update();
    this.vrm.scene.updateMatrixWorld(true);
    this.vrm.springBoneManager?.reset();
    for (let i = 0; i < 45; i++) {
      this.motion.update(1 / 30);
      this.vrm.update(1 / 30);
    }
  }

  private measure() {
    const head = this.vrm.humanoid.getRawBoneNode("head");
    if (head) this.headY = head.getWorldPosition(new THREE.Vector3()).y;
    // The top of the hat or hair, so the frame never cuts it.
    this.topY = Math.max(this.headY + 0.12, new THREE.Box3().setFromObject(this.vrm.scene).max.y);
  }

  /** Upper body in a tall frame, head and shoulders in a short one. The slot may ask, through CSS
   *  custom properties on the host, for room above her head (`--avatar-headroom`, a share of the
   *  height, for speech bubbles) and for more of her below the head (`--avatar-body`, metres). */
  private frameCamera(width: number, height: number) {
    const aspect = width / Math.max(1, height);
    const bust = height < 260;
    const css = getComputedStyle(this.opts.host);
    const headroom = THREE.MathUtils.clamp(parseFloat(css.getPropertyValue("--avatar-headroom")) || 0, 0, 0.5);
    const body = parseFloat(css.getPropertyValue("--avatar-body")) || (bust ? 0.42 : 0.86);
    const bottom = this.headY - body;
    const top = bottom + (this.topY + 0.03 - bottom) / (1 - headroom);
    const needH = top - bottom;
    const needW = bust ? 0.46 : Math.max(0.66, body * 0.62);
    const tan = Math.tan(THREE.MathUtils.degToRad(this.camera.fov / 2));
    const dist = Math.max(needH / 2 / tan, needW / 2 / (tan * aspect));
    const cy = (top + bottom) / 2;
    this.camera.aspect = aspect;
    this.camera.position.set(0, cy + 0.04, dist);
    this.camera.lookAt(0, cy, 0);
    this.camera.updateProjectionMatrix();
  }

  resize() {
    const r = this.opts.host.getBoundingClientRect();
    const w = Math.max(1, Math.round(r.width));
    const h = Math.max(1, Math.round(r.height));
    this.renderer.setSize(w, h, false);
    this.frameCamera(w, h);
  }

  private listen() {
    const host = this.opts.host;
    const ro = new ResizeObserver(() => this.resize());
    ro.observe(host);
    const io = new IntersectionObserver((entries) => (this.onScreen = entries.some((e) => e.isIntersecting)));
    io.observe(host);
    const vis = () => (this.visible = document.visibilityState === "visible");
    document.addEventListener("visibilitychange", vis);
    const ndc = (e: PointerEvent) => {
      const r = this.canvas.getBoundingClientRect();
      return new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    };
    // She follows a mouse anywhere near her, not only over her canvas; a finger only while on her.
    const move = (e: PointerEvent) => {
      if (e.pointerType === "touch" && e.target !== this.canvas) return;
      const p = ndc(e);
      const near = Math.abs(p.x) < POINTER_REACH && Math.abs(p.y) < POINTER_REACH;
      this.body.setPointer(near ? p : null);
      this.pointerAt = near ? performance.now() : 0;
    };
    const leave = (e: PointerEvent) => {
      if (!e.relatedTarget) this.forgetPointer();
    };
    const down = (e: PointerEvent) => this.poke(ndc(e), e.clientX, e.clientY);
    window.addEventListener("pointermove", move, { passive: true });
    document.documentElement.addEventListener("pointerleave", leave);
    window.addEventListener("blur", this.forgetPointer);
    this.canvas.addEventListener("pointerdown", down);
    this.cleanups.push(
      () => ro.disconnect(),
      () => io.disconnect(),
      () => document.removeEventListener("visibilitychange", vis),
      () => window.removeEventListener("pointermove", move),
      () => document.documentElement.removeEventListener("pointerleave", leave),
      () => window.removeEventListener("blur", this.forgetPointer),
      () => this.canvas.removeEventListener("pointerdown", down),
    );
  }

  private forgetPointer = () => {
    this.pointerAt = 0;
    this.body?.setPointer(null);
  };

  setChatSide(side: "left" | "below") {
    if (this.body) this.body.chatSide = side;
  }

  // ---------- background scene ----------

  /** Shows the background scene behind her (loaded on first use), or leaves the canvas transparent. */
  setBackdrop(spec: SceneSpec | null | undefined, show: boolean) {
    this.backdropWanted = show && !!spec;
    if (this.backdrop) this.backdrop.visible = this.backdropWanted;
    this.scene.fog = this.backdropWanted && spec?.fog ? new THREE.Fog(spec.fog.color, spec.fog.near, spec.fog.far) : null;
    if (this.backdropWanted && spec && !this.backdrop && !this.backdropLoading) {
      this.backdropLoading = this.loadBackdrop(spec).catch((err) => console.warn("[avatar] background scene unavailable", err));
    }
    this.resize();
  }

  private async loadBackdrop(spec: SceneSpec) {
    const gltf = await new GLTFLoader().parseAsync(await fetchBuffer(spec.url, this.opts.signal), "");
    if (this.disposed) return;
    const root = gltf.scene;
    root.traverse((o) => {
      const mesh = o as THREE.Mesh;
      if (!mesh.isMesh || !spec.unlit) return;
      // Game backgrounds are painted with their light; shown unlit they keep the painted colours.
      const src = mesh.material as THREE.MeshStandardMaterial;
      // A little darker and cooler than the texture, so she stands out in front of it.
      mesh.material = new THREE.MeshBasicMaterial({ map: src.map, color: 0xd9d5e0, alphaTest: src.alphaTest || (src.transparent ? 0.5 : 0), side: THREE.DoubleSide });
      src.dispose();
    });
    const [x, y, z] = spec.position ?? [0, 0, 0];
    root.position.set(x, y, z);
    root.rotation.y = THREE.MathUtils.degToRad(spec.rotation_y_deg ?? 0);
    root.visible = this.backdropWanted;
    this.backdrop = root;
    this.scene.add(root);
  }

  // ---------- performances ----------

  perform(p: Performance) {
    const now = performance.now();
    this.perf = p;
    this.perfAt = now;
    this.step = -1;
    this.stepStarted = false;
    const silence = p.state === "thinking" || p.state === "idle" || p.state === "listening";
    this.pending = this.pending.filter((j) => j.kind === "accent" || (j.kind === "speech" && !silence));
    // This performance's own speech (if any) arrives after it; an earlier reply's end time does not count.
    this.speechEnd = null;
    if (silence) this.face.speak(null, now); // A new turn or the end of one: stop talking.
    this.face.setBase(p.face);
    this.body.setGaze(p.gaze);
    this.body.setTilt(p.headTilt);
    for (const a of p.accents) this.later(a.atMs, "accent", () => (a.kind === "wink" ? this.face.flash({ wink: 1 }, 380, performance.now()) : this.body.accent(a.kind, a.amount, this.time)));
    if (!p.steps.length) this.motion.goTo(this.graph.rest);
    else this.nextStep();
  }

  /** The reply text arrived: move the mouth and nod on sentence beats. */
  speak(plan: SpeechPlan) {
    const now = performance.now();
    this.face.speak(plan, now);
    this.speechEnd = now + plan.durationMs;
    if (this.opts.reducedMotion) return;
    this.pending = this.pending.filter((j) => j.kind !== "speech");
    this.later(220, "speech", () => this.body.accent("nod", 0.35, this.time));
    for (const b of plan.beats) this.later(b, "speech", () => this.body.accent("nod", 0.5 + Math.random() * 0.35, this.time));
  }

  private later(ms: number, kind: "step" | "accent" | "speech", run: () => void) {
    this.pending.push({ at: performance.now() + ms, kind, run });
  }

  private nextStep() {
    const p = this.perf;
    if (!p) return;
    this.step += 1;
    this.stepStarted = false;
    const s = p.steps[this.step];
    if (!s) {
      this.motion.goTo(this.graph.rest);
      return;
    }
    const go = () => {
      if (this.perf !== p) return;
      this.stepStarted = true;
      this.motion.goTo(s.loop);
    };
    if (s.delayMs > 0) this.later(s.delayMs, "step", go);
    else go();
  }

  private arrived(loop: string) {
    if (loop === this.graph.rest) return;
    this.recent.push(loop);
    if (this.recent.length > 4) this.recent.shift();
  }

  /** Decides when the current gesture step is done. */
  private runSteps(now: number) {
    const p = this.perf;
    const s = p?.steps[this.step];
    if (!p || !s || !this.stepStarted || this.motion.node !== s.loop) return;
    const held = this.motion.loopElapsed;
    const clip = this.graph.clips.get(s.loop);
    if (s.hold.kind === "loops") {
      if (held >= Math.max(0.2, s.hold.n * (clip?.duration ?? 1))) this.nextStep();
    } else if (s.hold.kind === "speech") {
      if (held < MIN_DWELL_S) return;
      const noSpeech = this.speechEnd === null && now - this.perfAt > SPEECH_WAIT_MS;
      if (noSpeech ? held >= UNSPOKEN_HOLD_S : this.speechEnd !== null && now > this.speechEnd + 400) this.nextStep();
    }
  }

  /** Tapping the character: a pleased look and a nod, a pat on the head makes her a little shy; the
   *  stage then answers with a line and a gesture (onPoke). She does not react while explaining. */
  private poke(ndc: THREE.Vector2, x: number, y: number) {
    const now = performance.now();
    this.body.setPointer(ndc);
    this.face.blinkSoon();
    const busy = this.perf && this.perf.state !== "idle" && this.perf.state !== "listening";
    if (busy) return;
    this.pokes = [...this.pokes.filter((t) => now - t < 4000), now];
    const streak = this.pokes.length;
    const onHead = this.onHead(x, y);
    if (!this.opts.reducedMotion) {
      if (onHead && streak >= 3) {
        this.face.flash({ shy: 0.75, troubled: 0.25 }, 2200, now);
        this.body.accent("tilt", -1, this.time);
      } else if (onHead) {
        this.face.flash({ happy: 0.7, shy: 0.35 }, 1800, now);
        this.body.accent("tilt", 1, this.time);
      } else {
        this.face.flash({ smile: 0.55, happy: 0.3 }, 1500, now);
        this.body.accent("nod", 0.6, this.time);
      }
    }
    this.opts.onPoke?.(onHead ? "head" : "body", streak);
  }

  /** Whether a screen point is on her head. The head bone sits at the base of the skull, so the
   *  zone is a circle about 0.13 m across centred a little above it, measured on screen, which
   *  holds however closely the camera frames her (the chat column or the concierge). */
  private onHead(x: number, y: number): boolean {
    const bone = this.vrm.humanoid.getRawBoneNode("head");
    if (!bone) return false;
    const r = this.canvas.getBoundingClientRect();
    const toScreen = (up: number) => {
      const v = bone.getWorldPosition(new THREE.Vector3()).add(new THREE.Vector3(0, up, 0)).project(this.camera);
      return { x: ((v.x + 1) / 2) * r.width + r.left, y: ((1 - v.y) / 2) * r.height + r.top };
    };
    const centre = toScreen(0.09);
    const radius = Math.max(14, centre.y - toScreen(0.22).y);
    return Math.hypot(centre.x - x, centre.y - y) < radius;
  }

  // ---------- loop ----------

  private frame = (now: number) => {
    if (this.disposed) return;
    this.raf = requestAnimationFrame(this.frame);
    if (!this.visible || !this.onScreen) {
      this.last = now;
      return;
    }
    if (this.last && now - this.last < 1000 / this.fps - 2) return;
    // Clips and the face keep wall-clock time (speech is timed by the clock); slow frames are
    // split into short physics steps so hair and ribbons stay stable.
    const dt = this.last ? Math.min((now - this.last) / 1000, MAX_FRAME_S) : 1 / this.fps;
    this.last = now;
    this.time += dt;
    if (this.pointerAt && now - this.pointerAt > POINTER_IDLE_MS) this.forgetPointer();

    for (const job of this.pending.filter((j) => j.at <= now)) job.run();
    this.pending = this.pending.filter((j) => j.at > now);

    this.motion.update(dt);
    this.runSteps(now);
    this.body.update(dt, this.time);
    this.face.update(dt, now, this.time);
    const steps = Math.min(8, Math.ceil(dt / PHYSICS_STEP_S - 1e-6));
    for (let i = 0; i < steps; i++) this.vrm.update(dt / steps);
    this.renderer.render(this.scene, this.camera);
    this.body.restoreBones();
  };

  dispose() {
    this.disposed = true;
    cancelAnimationFrame(this.raf);
    for (const c of this.cleanups) c();
    this.motion?.dispose();
    this.backdrop?.traverse((o) => {
      const mesh = o as THREE.Mesh;
      if (!mesh.isMesh) return;
      mesh.geometry.dispose();
      const m = mesh.material as THREE.MeshBasicMaterial;
      m.map?.dispose();
      m.dispose();
    });
    if (this.vrm) {
      this.scene.remove(this.vrm.scene);
      VRMUtils.deepDispose(this.vrm.scene);
    }
    this.renderer.dispose();
    this.renderer.forceContextLoss();
    this.canvas.remove();
  }
}
