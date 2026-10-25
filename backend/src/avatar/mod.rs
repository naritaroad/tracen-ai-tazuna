//! Character player interface (docs/character/avatar-contract.md).
//! The server derives the lifecycle state from the run, and the reply cue (emotion, intensity,
//! semantic gesture) from `cues`; the browser turns both into clips, expressions and gaze.
//! The model never names a clip or a bone.

pub mod cues;

use std::collections::HashMap;
use std::path::{Component, Path as FsPath, PathBuf};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{Value, json};

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use cues::{Cue, CueSource, EMOTIONS, GESTURES};

pub const DEFAULT_CHARACTER: &str = "9001_hayakawa_tazuna";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarState {
    Idle,
    Listening,
    Thinking,
    Working,
    Explaining,
    Confirming,
    Handoff,
    Apologizing,
    Error,
}

struct StateSpec {
    state: AvatarState,
    clip_enter: Option<&'static str>,
    clip_hold: &'static str,
    clip_exit: Option<&'static str>,
    expression: Option<&'static str>,
    /// Default cue when the event carries none (a reply always carries one).
    emotion: &'static str,
    gesture: Option<&'static str>,
    priority: u8,
    min_hold_ms: u32,
    interruptible: bool,
}

const STATES: [StateSpec; 9] = [
    StateSpec { state: AvatarState::Idle, clip_enter: None, clip_hold: "idle_polite", clip_exit: None, expression: None, emotion: "neutral", gesture: None, priority: 0, min_hold_ms: 0, interruptible: true },
    StateSpec { state: AvatarState::Listening, clip_enter: None, clip_hold: "idle_polite", clip_exit: None, expression: Some("relaxed"), emotion: "gentle", gesture: None, priority: 0, min_hold_ms: 0, interruptible: true },
    StateSpec { state: AvatarState::Thinking, clip_enter: None, clip_hold: "idle_polite", clip_exit: None, expression: Some("thinking"), emotion: "neutral", gesture: Some("think"), priority: 1, min_hold_ms: 0, interruptible: true },
    StateSpec { state: AvatarState::Working, clip_enter: None, clip_hold: "idle_polite", clip_exit: None, expression: Some("serious"), emotion: "serious", gesture: Some("think"), priority: 1, min_hold_ms: 0, interruptible: true },
    StateSpec { state: AvatarState::Explaining, clip_enter: Some("idle_to_chest"), clip_hold: "chest_hold", clip_exit: Some("chest_to_idle"), expression: Some("smile"), emotion: "gentle", gesture: Some("hand_chest"), priority: 2, min_hold_ms: 0, interruptible: true },
    StateSpec { state: AvatarState::Confirming, clip_enter: Some("idle_to_chest"), clip_hold: "chest_hold", clip_exit: Some("chest_to_idle"), expression: Some("happy"), emotion: "happy", gesture: Some("hope"), priority: 2, min_hold_ms: 0, interruptible: true },
    StateSpec { state: AvatarState::Handoff, clip_enter: Some("idle_to_chest"), clip_hold: "chest_hold", clip_exit: Some("chest_to_idle"), expression: Some("troubled"), emotion: "sincere", gesture: Some("bow"), priority: 3, min_hold_ms: 1500, interruptible: false },
    StateSpec { state: AvatarState::Apologizing, clip_enter: Some("idle_to_chest"), clip_hold: "chest_hold", clip_exit: Some("chest_to_idle"), expression: Some("troubled"), emotion: "apologetic", gesture: Some("hand_chest"), priority: 2, min_hold_ms: 0, interruptible: true },
    StateSpec { state: AvatarState::Error, clip_enter: None, clip_hold: "idle_polite", clip_exit: None, expression: Some("troubled"), emotion: "troubled", gesture: None, priority: 3, min_hold_ms: 0, interruptible: true },
];

fn spec(state: AvatarState) -> &'static StateSpec {
    STATES.iter().find(|s| s.state == state).expect("every state has a spec")
}

fn round2(x: f32) -> f64 {
    (f64::from(x) * 100.0).round() / 100.0
}

/// The payload of an `avatar.intent` event, with the state's default cue.
pub fn intent(state: AvatarState, reason: &str) -> Value {
    let s = spec(state);
    json!({
        "state": s.state,
        "expression": s.expression,
        "priority": s.priority,
        "min_hold_ms": s.min_hold_ms,
        "interruptible": s.interruptible,
        "reason": reason,
        "character_id": DEFAULT_CHARACTER,
        "emotion": s.emotion,
        "intensity": 0.5,
        "gesture": s.gesture,
        "cue_source": CueSource::State,
    })
}

/// The intent sent with a reply: the state plus the cue chosen for that reply.
pub fn reply_intent(state: AvatarState, reason: &str, cue: &Cue) -> Value {
    let mut v = intent(state, reason);
    v["emotion"] = json!(cue.emotion);
    v["intensity"] = json!(round2(cue.intensity));
    v["gesture"] = json!(cue.gesture);
    v["cue_source"] = json!(cue.source);
    if let Some(m) = cue.from_model {
        v["emotion_from_model"] = json!({ "emotion": m.emotion, "intensity": round2(m.intensity), "gesture": m.gesture });
    }
    v
}

async fn states() -> Json<Value> {
    let list: Vec<Value> = STATES
        .iter()
        .map(|s| {
            json!({
                "state": s.state, "clip_enter": s.clip_enter, "clip_hold": s.clip_hold, "clip_exit": s.clip_exit,
                "expression": s.expression, "emotion": s.emotion, "gesture": s.gesture,
                "priority": s.priority, "min_hold_ms": s.min_hold_ms, "interruptible": s.interruptible,
            })
        })
        .collect();
    Json(json!({ "version": 2, "states": list, "vocabulary": { "emotions": EMOTIONS, "gestures": GESTURES } }))
}

fn character_dirs(root: &FsPath) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.join("character.json").is_file()).collect();
    dirs.sort();
    dirs
}

fn read_json(path: &FsPath) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

async fn characters(State(state): State<AppState>) -> Json<Value> {
    let list: Vec<Value> = character_dirs(&state.config.characters_dir)
        .iter()
        .filter_map(|dir| {
            let c = read_json(&dir.join("character.json"))?;
            Some(json!({
                "id": c["id"],
                "display_name": c["displayName"],
                "assets_available": state.config.avatar_asset_dir.is_some(),
            }))
        })
        .collect();
    Json(json!({ "characters": list, "default": DEFAULT_CHARACTER }))
}

// ---------- motion sets ----------
// The player walks a graph: every loop clip is a node (`from` = `to` = its id), every other clip is
// an edge between two nodes. `neutral` is a pose node with no loop of its own (game clips meet there).

fn set_clip(id: &str, url: String, looped: bool, duration: &Value, from: &str, to: &str, gestures: &Value) -> Value {
    json!({ "id": id, "url": url, "loop": looped, "duration": duration, "from": from, "to": to, "gestures": gestures })
}

/// The committed, video-fitted clips (animations.json); the graph comes from behavior.json.
fn video_fit_set(id: &str, anim: &Value, behavior: &Value) -> Option<Value> {
    let spec = &behavior["motion_sets"]["video_fit"];
    let rest = spec["rest"].as_str()?;
    let by_id: HashMap<&str, &Value> = anim["clips"].as_array()?.iter().filter_map(|c| Some((c["id"].as_str()?, c))).collect();
    let url = |c: &Value| format!("/avatar-assets/{id}/animation/{}", c["file"].as_str().unwrap_or_default());
    let mut clips = Vec::new();
    for (loop_id, gestures) in spec["loops"].as_object()? {
        let Some(c) = by_id.get(loop_id.as_str()) else { continue };
        clips.push(set_clip(loop_id, url(c), true, &c["duration"], loop_id, loop_id, gestures));
    }
    for t in spec["transitions"].as_array().into_iter().flatten() {
        let (Some(clip), Some(from), Some(to)) = (t["clip"].as_str(), t["from"].as_str(), t["to"].as_str()) else { continue };
        let Some(c) = by_id.get(clip) else { continue };
        clips.push(set_clip(clip, url(c), false, &c["duration"], from, to, &json!([])));
    }
    by_id.contains_key(rest).then(|| json!({ "id": "video_fit", "rest": rest, "clips": clips }))
}

/// The character a game motion library belongs to: its `character` field, else the folder in `model`.
fn game_character(manifest: &Value) -> Option<String> {
    if let Some(c) = manifest["character"].as_str() {
        return Some(c.to_string());
    }
    let model = manifest["model"].as_str()?.replace('\\', "/");
    let rest = &model[model.find("characters/")? + "characters/".len()..];
    rest.split('/').next().filter(|s| valid_id(s)).map(str::to_string)
}

fn game_manifest(config: &Config, id: &str) -> Option<(PathBuf, Value)> {
    let dir = config.avatar_game_motion_dir.as_ref()?;
    let manifest = read_json(&dir.join("game_motions.json"))?;
    (game_character(&manifest).as_deref() == Some(id)).then(|| (dir.clone(), manifest))
}

/// Game-derived clips: start `_s` (neutral -> loop), `_loop`, end `_e` (loop -> neutral).
/// A motion with `enter_from: idle` starts from the idle loop instead of the neutral pose.
fn game_set(id: &str, manifest: &Value, behavior: &Value) -> Option<Value> {
    let spec = &behavior["motion_sets"]["game"];
    let rest_motion = spec["rest"].as_str().or(manifest["default"].as_str()).unwrap_or("idle");
    let rest = format!("{rest_motion}_loop");
    let node = |side: &Value| if side.as_str() == Some("idle") { rest.clone() } else { "neutral".to_string() };
    let mut clips = Vec::new();
    for m in manifest["motions"].as_array()? {
        let Some(mid) = m["id"].as_str() else { continue };
        let parts = &m["parts"];
        let loop_id = format!("{mid}_loop");
        let gestures = spec["gestures"].get(mid).cloned().unwrap_or(json!([]));
        let edges = [("s", node(&m["enter_from"]), loop_id.clone()), ("loop", loop_id.clone(), loop_id.clone()), ("e", loop_id.clone(), node(&m["exit_to"]))];
        for (part, from, to) in edges {
            let Some(file) = parts[part]["file"].as_str() else { continue };
            let looped = part == "loop";
            let g = if looped { gestures.clone() } else { json!([]) };
            clips.push(set_clip(&format!("{mid}_{part}"), format!("/avatar-assets/{id}/game/{file}"), looped, &parts[part]["duration"], &from, &to, &g));
        }
    }
    clips.iter().any(|c| c["id"] == rest.as_str()).then(|| json!({ "id": "game", "rest": rest, "clips": clips }))
}

/// The local background scene (`scene.json` in `AVATAR_SCENE_DIR`): its model file and where it
/// stands relative to the character (metres, three.js axes; the character faces +Z at the origin).
fn scene(config: &Config, id: &str) -> Option<Value> {
    let dir = config.avatar_scene_dir.as_ref()?;
    let manifest = read_json(&dir.join("scene.json"))?;
    let file = manifest["file"].as_str().filter(|f| f.ends_with(".glb") && !f.contains(['/', '\\']))?;
    dir.join(file).is_file().then(|| {
        json!({
            "id": manifest["id"], "url": format!("/avatar-assets/{id}/scene/{file}"),
            "position": manifest["position"], "rotation_y_deg": manifest["rotation_y_deg"],
            "unlit": manifest["unlit"], "fog": manifest["fog"],
        })
    })
}

async fn character(State(state): State<AppState>, Path(id): Path<String>) -> AppResult<Json<Value>> {
    if !valid_id(&id) {
        return Err(AppError::NotFound("character"));
    }
    let dir = state.config.characters_dir.join(&id);
    let c = read_json(&dir.join("character.json")).ok_or(AppError::NotFound("character"))?;
    let anim = read_json(&dir.join("animation").join("animations.json")).unwrap_or(json!({ "clips": [] }));
    let mut behavior = read_json(&dir.join("behavior.json")).unwrap_or(json!({}));
    let assets = state.config.avatar_asset_dir.is_some();
    let clips: Vec<Value> = anim["clips"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|clip| {
            let file = clip["file"].as_str().unwrap_or_default();
            json!({
                "id": clip["id"], "label": clip["label"], "loop": clip["loop"], "next": clip.get("next"),
                "duration": clip["duration"], "usage": clip["usage"], "interrupt": clip["interrupt"],
                "url": assets.then(|| format!("/avatar-assets/{id}/animation/{file}")),
            })
        })
        .collect();
    // Preferred set first: the richer game library when it is configured locally.
    let mut motion_sets = Vec::new();
    if assets {
        if let Some(set) = game_manifest(&state.config, &id).and_then(|(_, m)| game_set(&id, &m, &behavior)) {
            motion_sets.push(set);
        }
        if let Some(set) = video_fit_set(&id, &anim, &behavior) {
            motion_sets.push(set);
        }
    }
    if let Some(b) = behavior.as_object_mut() {
        b.remove("motion_sets");
    }
    Ok(Json(json!({
        "id": c["id"],
        "display_name": c["displayName"],
        "height_m": c["vrm"]["heightMeters"],
        "vrm_url": assets.then(|| format!("/avatar-assets/{id}/vrm/{}", FsPath::new(c["vrm"]["path"].as_str().unwrap_or("")).file_name().and_then(|f| f.to_str()).unwrap_or(""))),
        "expressions": c["expressions"],
        "look_at": c["lookAt"],
        "clips": clips,
        "fps": anim["fps"],
        "motion_sets": motion_sets,
        "scene": if assets { scene(&state.config, &id) } else { None },
        "behavior": behavior,
        "license": c["license"],
        "assets_available": assets,
    })))
}

/// Local runtime files only: `.vrm` under vrm/, `.vrma` under animation/vrma/, `.vrma` of the
/// local game motion library under game/vrma/, and the `.glb` background scene under scene/.
async fn asset(State(state): State<AppState>, Path((id, rest)): Path<(String, String)>) -> AppResult<Response> {
    let root = state.config.avatar_asset_dir.as_ref().ok_or(AppError::NotFound("asset"))?;
    let rel = FsPath::new(&rest);
    let safe = valid_id(&id) && rel.components().all(|c| matches!(c, Component::Normal(_)));
    let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or_default();
    let vrma_dir = rest.starts_with("animation/vrma/") || rest.starts_with("game/vrma/");
    let allowed = (rest.starts_with("vrm/") && ext == "vrm") || (vrma_dir && ext == "vrma") || (rest.starts_with("scene/") && ext == "glb");
    if !safe || !allowed {
        return Err(AppError::NotFound("asset"));
    }
    let path = if let Some(game_rel) = rest.strip_prefix("game/") {
        game_manifest(&state.config, &id).ok_or(AppError::NotFound("asset"))?.0.join(game_rel)
    } else if let Some(scene_rel) = rest.strip_prefix("scene/") {
        state.config.avatar_scene_dir.as_ref().ok_or(AppError::NotFound("asset"))?.join(scene_rel)
    } else {
        root.join(&id).join(rel)
    };
    let bytes = tokio::fs::read(path).await.map_err(|_| AppError::NotFound("asset"))?;
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("model/gltf-binary"));
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=3600"));
    Ok(response)
}

/// Reserved: the player reports clip ends and user interruptions here.
async fn player_events() -> AppError {
    AppError::NotImplemented
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/states", get(states))
        .route("/characters", get(characters))
        .route("/characters/{id}", get(character))
        .route("/sessions/{id}/events", post(player_events))
}

pub fn asset_routes() -> Router<AppState> {
    Router::new().route("/avatar-assets/{id}/{*rest}", get(asset))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn behavior() -> Value {
        json!({ "motion_sets": {
            "video_fit": { "rest": "idle_polite", "loops": { "idle_polite": [], "chest_hold": ["hand_chest"], "missing": ["x"] },
                "transitions": [{ "clip": "idle_to_chest", "from": "idle_polite", "to": "chest_hold" }] },
            "game": { "rest": "idle", "gestures": { "bow": ["bow", "greet"] } } } })
    }

    #[test]
    fn video_fit_set_keeps_known_clips_only() {
        let anim = json!({ "clips": [
            { "id": "idle_polite", "file": "vrma/idle_polite.vrma", "duration": 7.0 },
            { "id": "chest_hold", "file": "vrma/chest_hold.vrma", "duration": 5.2 },
            { "id": "idle_to_chest", "file": "vrma/idle_to_chest.vrma", "duration": 1.3 } ] });
        let set = video_fit_set("c1", &anim, &behavior()).unwrap();
        let clips = set["clips"].as_array().unwrap();
        assert_eq!(set["rest"], "idle_polite");
        assert_eq!(clips.len(), 3);
        let edge = clips.iter().find(|c| c["id"] == "idle_to_chest").unwrap();
        assert_eq!((edge["from"].as_str(), edge["to"].as_str(), edge["loop"].as_bool()), (Some("idle_polite"), Some("chest_hold"), Some(false)));
        assert_eq!(edge["url"], "/avatar-assets/c1/animation/vrma/idle_to_chest.vrma");
    }

    #[test]
    fn game_set_links_parts_through_neutral() {
        let part = |f: &str| json!({ "file": format!("vrma/{f}.vrma"), "duration": 1.0 });
        let manifest = json!({ "model": "../../assets/characters/c1/vrm/X.vrm", "default": "idle", "motions": [
            { "id": "idle", "enter_from": "neutral", "exit_to": "neutral", "parts": { "s": part("idle_s"), "loop": part("idle_loop"), "e": part("idle_e") } },
            { "id": "hope", "enter_from": "idle", "exit_to": "neutral", "parts": { "s": part("hope_s"), "loop": part("hope_loop"), "e": part("hope_e") } },
            { "id": "bow", "enter_from": "neutral", "exit_to": "neutral", "parts": { "s": part("bow_s"), "loop": part("bow_loop"), "e": part("bow_e") } } ] });
        assert_eq!(game_character(&manifest).as_deref(), Some("c1"));
        let set = game_set("c1", &manifest, &behavior()).unwrap();
        let find = |id: &str| set["clips"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap().clone();
        assert_eq!(set["rest"], "idle_loop");
        assert_eq!((find("bow_s")["from"].clone(), find("bow_s")["to"].clone()), (json!("neutral"), json!("bow_loop")));
        assert_eq!(find("hope_s")["from"], "idle_loop");
        assert_eq!(find("bow_loop")["gestures"], json!(["bow", "greet"]));
        assert_eq!(find("bow_e")["url"], "/avatar-assets/c1/game/vrma/bow_e.vrma");
    }

    #[test]
    fn reply_intent_carries_the_cue() {
        let cue = Cue { emotion: "happy", intensity: 0.666, gesture: Some("clap"), source: CueSource::Model, from_model: None };
        let v = reply_intent(AvatarState::Explaining, "message.completed", &cue);
        assert_eq!((v["emotion"].as_str(), v["intensity"].as_f64(), v["gesture"].as_str()), (Some("happy"), Some(0.67), Some("clap")));
        assert_eq!(v["cue_source"], "model");
        assert!(v.get("emotion_from_model").is_none());
        let thinking = intent(AvatarState::Thinking, "run.started");
        assert_eq!((thinking["gesture"].as_str(), thinking["cue_source"].as_str()), (Some("think"), Some("state")));
    }
}
