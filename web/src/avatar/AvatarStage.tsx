// The character: one instance for the whole site (concierge/ConciergeProvider.tsx moves it between
// the chat page and the floating concierge). With local character files and WebGL it shows the 3D
// player (`VrmStage`, loaded on demand); otherwise, or if loading fails, the 2D state card. Both
// read the same snapshot, so the chat works either way.

import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api";
import type { AvatarSnapshot } from "./states";
import { pickTap, type TapZone } from "./taps.ts";
import type { CharacterManifest } from "./vocab";

const VrmStage = lazy(() => import("./VrmStage"));

export interface AvatarStageProps {
  snapshot: AvatarSnapshot;
  characterId: string;
  /** The chat's greeting line, said once when the character first appears on an empty chat. */
  greeting?: string;
  /** Somewhere on screen shows her; she loads the first time this is true. */
  active: boolean;
  /** The page she is on, for a tap line about it (taps.ts `pageOf`). */
  page: string | null;
  /** The slot shows her in front of the background scene (the chat page). */
  backdrop: boolean;
}

/** Character manifest from GET /api/v1/avatar/characters/{id}; null while loading or if absent. */
export function useCharacter(id: string): CharacterManifest | null {
  const [manifest, setManifest] = useState<CharacterManifest | null>(null);
  useEffect(() => {
    api.get<CharacterManifest>(`/avatar/characters/${id}`).then(setManifest).catch(() => setManifest(null));
  }, [id]);
  return manifest;
}

// The manifest keys its names by `ja` / `zhTW` / `en` (assets/characters/*/character.json).
const MANIFEST_LANG: Record<string, string> = { "zh-TW": "zhTW", ja: "ja", "en-GB": "en" };

/** The character's name comes with its manifest from the API; the language file is only the fallback. */
function characterName(manifest: CharacterManifest | null, locale: string): string | undefined {
  return manifest?.display_name[MANIFEST_LANG[locale] ?? locale];
}

function webglAvailable(): boolean {
  try {
    const gl = document.createElement("canvas").getContext("webgl2") ?? document.createElement("canvas").getContext("webgl");
    gl?.getExtension("WEBGL_lose_context")?.loseContext();
    return !!gl;
  } catch {
    return false;
  }
}

type View = "loading" | "ready" | "failed";

interface PlaceholderProps {
  snapshot: AvatarSnapshot;
  manifest: CharacterManifest | null;
  note: string;
  onTap: () => void;
  /** Changes on each tap reaction, replaying the little hop. */
  hop: number;
}

export function PlaceholderAvatar({ snapshot, manifest, note, onTap, hop }: PlaceholderProps) {
  const { t, i18n } = useTranslation();
  const state = snapshot.intent.state;
  return (
    <div className="card avatar-card" data-state={state} aria-live="polite">
      <button key={hop} type="button" className={`avatar-figure ${hop ? "hop" : ""}`} onClick={onTap} aria-label={t("avatar.tapLabel")} title={t("avatar.tapLabel")}>
        {t("avatar.monogram")}
      </button>
      <div>
        <div className="avatar-name">{characterName(manifest, i18n.language) ?? t("chat.tazuna")}</div>
        <div className="avatar-state">{t(`avatar.state.${state}`)}</div>
        <div className="muted small">{note}</div>
      </div>
    </div>
  );
}

/** How long a tap line stays up. */
const SAY_MS = 4500;

export function AvatarStage({ snapshot, characterId, greeting, active, page, backdrop }: AvatarStageProps) {
  const { t, i18n } = useTranslation();
  const [view, setView] = useState<View>("loading");
  const manifest = useCharacter(characterId);
  const [webgl] = useState(webglAvailable);
  // Loaded on first showing, then kept while hidden or moved: no reload, and the player pauses off screen.
  const [shown, setShown] = useState(active);
  useEffect(() => {
    if (active) setShown(true);
  }, [active]);

  const [say, setSay] = useState<{ text: string; id: number } | null>(null);
  const lastLine = useRef<string | null>(null);
  const taps2d = useRef<number[]>([]);
  useEffect(() => {
    if (!say) return;
    const id = window.setTimeout(() => setSay(null), SAY_MS);
    return () => window.clearTimeout(id);
  }, [say]);

  const set = manifest?.motion_sets?.[0];
  const can3d = !!(manifest?.assets_available && manifest.vrm_url && set && webgl);
  const show3d = can3d && view !== "failed";
  const state = snapshot.intent.state;

  const ready = useCallback(() => setView("ready"), []);
  const fail = useCallback((err: unknown) => {
    console.warn("[avatar] 3D player unavailable, using the 2D card", err);
    setView("failed");
  }, []);

  /** A tap: pick her line (never the same one twice running) and show it above her. */
  const tap = useCallback(
    (zone: TapZone, streak: number) => {
      const reaction = pickTap(zone, streak, page, Math.random, lastLine.current);
      lastLine.current = reaction.key;
      const line = t(reaction.key);
      setSay((s) => ({ text: line, id: (s?.id ?? 0) + 1 }));
      return { ...reaction, line };
    },
    [page, t],
  );
  // The 2D card answers with words and a hop; like the 3D one, not while she is explaining.
  const tap2d = () => {
    if (state !== "idle" && state !== "listening") return;
    const now = performance.now();
    taps2d.current = [...taps2d.current.filter((at) => now - at < 4000), now];
    tap("body", taps2d.current.length);
  };

  // Why there is no 3D: no WebGL, no model on this server, or a model without motion data (an API
  // older than its character files — restarting it fixes that).
  const hasModel = !!(manifest?.assets_available && manifest.vrm_url);
  const missing = !manifest || can3d ? null : !webgl ? "noWebgl" : !hasModel ? "noAssets" : "noMotion";
  useEffect(() => {
    if (missing) console.warn(`[avatar] 2D card: ${missing}`, { assets: manifest?.assets_available, model: manifest?.vrm_url, motionSets: manifest?.motion_sets?.length ?? 0 });
  }, [missing, manifest]);
  const note = !manifest
    ? t("avatar.placeholder")
    : missing
      ? t(`avatar.${missing}`)
      : view === "failed"
        ? t("avatar.failed")
        : t("avatar.loading");

  if (!shown) return null;
  return (
    <div className="avatar-frame">
      {show3d ? (
        <div className={`card avatar-card avatar-card-3d ${view === "ready" ? "ready" : ""} ${backdrop && manifest?.scene ? "with-scene" : ""}`} data-state={state}>
          <Suspense fallback={null}>
            <VrmStage character={manifest!} set={set!} snapshot={snapshot} greeting={greeting} backdrop={backdrop} onReady={ready} onFail={fail} onTap={tap} />
          </Suspense>
          {view !== "ready" && <div className="avatar-loading muted small">{note}</div>}
          <div className="avatar-caption" aria-live="polite">
            <span className="avatar-name">{characterName(manifest, i18n.language) ?? t("chat.tazuna")}</span>
            <span className="avatar-state">{t(`avatar.state.${state}`)}</span>
          </div>
        </div>
      ) : (
        <PlaceholderAvatar snapshot={snapshot} manifest={manifest} note={note} onTap={tap2d} hop={say?.id ?? 0} />
      )}
      {say && (
        <div key={say.id} className="avatar-say" role="status">
          {say.text}
        </div>
      )}
    </div>
  );
}
