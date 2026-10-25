// Shapes shared by the character modules (docs/character/avatar-contract.md). The words themselves come from
// the server: `GET /api/v1/avatar/states` (vocabulary) and `GET /api/v1/avatar/characters/{id}`.
// Only types live here, so the pure modules can import them without pulling in code.

export type Emotion =
  | "neutral"
  | "gentle"
  | "happy"
  | "excited"
  | "sincere"
  | "apologetic"
  | "troubled"
  | "surprised"
  | "serious"
  | "playful";

/** A semantic gesture, not a file name: each motion set maps it to its own clips. */
export type Gesture =
  | "greet"
  | "bow"
  | "wave"
  | "present"
  | "guide"
  | "explain"
  | "hand_chest"
  | "think"
  | "hope"
  | "raise_hand"
  | "happy"
  | "clap";

/** Expression name → weight (0–1). Names are the VRM's own expressions. */
export type FaceWeights = Record<string, number>;

/** One clip of a motion set. Loops are graph nodes (`from` = `to` = `id`); other clips are edges. */
export interface MotionClip {
  id: string;
  url: string;
  loop: boolean;
  duration: number;
  from: string;
  to: string;
  /** Gestures this loop can show; `emotion:<name>` marks a loop that suits an emotion. */
  gestures: string[];
}

export interface MotionSet {
  id: string;
  /** The loop the character returns to between gestures. */
  rest: string;
  clips: MotionClip[];
}

/** assets/characters/<id>/behavior.json, without its motion sets. Every part is optional. */
export interface CharacterBehavior {
  emotions?: Record<string, FaceWeights>;
  state_expressions?: Record<string, FaceWeights>;
  gesture_fallbacks?: Record<string, string[]>;
  /** One-shot gestures and how many loops of them to hold before moving on. */
  oneshot?: Record<string, number>;
  /** Talking postures to use after a one-shot gesture, by emotion. */
  postures?: Record<string, string[]>;
}

export interface CharacterManifest {
  id: string;
  display_name: Record<string, string>;
  assets_available: boolean;
  vrm_url: string | null;
  height_m?: number;
  motion_sets?: MotionSet[];
  behavior?: CharacterBehavior;
  look_at?: { rangeDegrees?: { horizontal?: number; up?: number; down?: number } };
  /** A background scene behind her, only where the server has one locally (`AVATAR_SCENE_DIR`). */
  scene?: SceneSpec | null;
}

/** Where the background scene stands: metres in three.js axes, the character at the origin facing +Z. */
export interface SceneSpec {
  id: string;
  url: string;
  position?: [number, number, number];
  rotation_y_deg?: number;
  /** Show the painted textures as they are (game backgrounds are baked), not lit by the scene lights. */
  unlit?: boolean;
  fog?: { color: string; near: number; far: number } | null;
}
