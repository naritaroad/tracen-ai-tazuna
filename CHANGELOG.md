# Changelog

All notable changes to this project are recorded here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Interface copy in all three languages. The empty chat page now names Tazuna and says what she can look up instead of greeting the trainer; the reply trace is labelled "Activity log" in English instead of "What I did"; the plan calculator, Director's Office and AI settings introductions describe what each page does; English error messages for missing records, invalid requests and changed records are more specific.
- README wording in English, Traditional Chinese and Japanese, and the README screenshots retaken with the new copy. The README now gives the actual time limits (90 s per model call; 45 s per allowed call for the whole run, between 120 and 600 s) instead of the old 60 s and 120 s.
- README screenshots retaken with the local setup the site actually runs: the academy crest in the sidebar, and on the chat page Tazuna in front of the corridor scene with the game motions.
- The Traditional Chinese character card shows 駿川手綱, the name used everywhere else in the zh-TW interface, instead of 駿川たづな.

### Fixed

- The management site badge in the sidebar no longer slides under the collapse button, and in English it reads "Admin" so "Tracen Office" fits beside it.
- On phones, a chat start screen taller than the space (English, for example) scrolls instead of cutting off its heading.
- Label colons on the case detail and the plan formula follow the interface language (full-width in Chinese and Japanese).

## [1.0.0-beta] - 2026-10-26

First release.

### Added

- Rust API (Axum 0.8, SQLx 0.9) with accounts, cookie sessions and role checks, on PostgreSQL 18.6 with pgvector (`pgvector/pgvector:0.8.7-pg18-trixie`). Migrations are append-only.
- A bounded agent loop over five whitelisted tools: academy information, academy calendar, facility search, training-plan calculation and Director's Office hand-over. Replies stream over SSE, and every tool call is stored and listed under the reply.
- Model gateway on genai: Gemini (`gemini-3.8-flash`, embeddings `gemini-embedding-2`), Claude, or the offline `mock`.
- Demo data rebuilt from CSV at start: 10 information topics, 17 training services and 10 calendar entries. Every row carries a `source_type` (`official`, `anime`, `community`, `reference` or `project`).
- The game's race schedule in the academy calendar: 34 G1 and 42 G2 races (`data/seed/race_schedule.csv`, `races` table in migration 0004) with grade, racecourse, turf or dirt, distance, direction and the classes that may enter, named as in the Taiwanese, Japanese and English versions of the game. Each race is placed on its half month (1st–15th or 16th–end), never closes a facility, and can be filtered to G1 only or hidden. `list_academy_events` returns the races too. Source: GameTora, checked against BiliGame (`community`).
- Training-plan calculation on the server: total minutes, demo fees, end time, and clashes with calendar events, facility closures and the 21:00 curfew. Missing or ambiguous facility names are reported in words.
- Director's Office hand-over: `transfer_to_director` opens a case that staff handle on the management site as Director Akikawa (staffed). The AI does not reply while a person holds the case.
- Tazuna's persona, taken from the official character page and community summaries of the game text (`docs/product/worldbuilding.md` §3). She can chat off topic; with the default setting she returns to academy work in one sentence after about three off-topic exchanges. She declines large jobs such as essays, code and homework, and does not invent academy facts, news or weather.
- Trainer memory: preferences, conversation summaries and semantic recall of past events, curated in the background with the history of superseded entries kept. All of it is visible, editable and deletable under My details.
- Microphone input with speech to text on the server; the endpoints for spoken replies are reserved.
- Trainer site: chat, academy information, calendar, training facilities, plan calculator, Director's Office and My details (including a profile picture with manual cropping).
- Sidebar layout with new conversation (Ctrl/⌘+Shift+O), conversation search (Ctrl/⌘+K), the pages, pinned and recent conversations, and an account menu for language, theme and sign-out. The sidebar collapses to an icon rail. On phones it becomes a drawer, and the pages sit in a draggable bubble.
- Management site, which uses no AI: case inbox, demo data editing, AI run log, accounts, and AI settings and budget. The settings cover models, per-token prices, a monthly budget (block or warn) with usage per model, reply length, small talk, thinking depth, temperature, output limit per call and model calls per message. Defaults are natural length, balanced small talk, the model's own thinking depth and temperature, 8,192 output tokens and 6 calls; `AGENT_MAX_STEPS` sets only the starting value. Temperature is not forced lower because Google recommends 1.0 for Gemini 3, and Gemini's output limit includes thinking tokens.
- Interface text in Traditional Chinese, Japanese and English, kept in JSON language files and loaded per language. `npm run i18n:check`, also run by `npm run build`, checks key and placeholder parity and rejects hard-coded text.
- Light and dark themes in Tracen violet and white with wisteria tints and a gold horseshoe accent. Japanese pages use Japanese fonts first.
- Capacitor 8 Android app that loads the site from the API server and returns to the last page after the WebView reloads.
- 3D character player (three 0.186.1, @pixiv/three-vrm 3.5.5), loaded on demand. It shows thinking, looking things up, explaining, a bow on hand-over and a greeting on an empty chat, with nods, lip sync from the reply text, blinking, gaze that follows the pointer, and tap reactions with short spoken lines. High and standard quality run at 60 and 30 FPS; the player pauses when hidden and honours reduced motion. Without character files or WebGL, after a load failure, or when the trainer picks the simple view, a 2D state card is shown. If the API sends a character without motion data, the card asks for an API restart.
- One character instance across the trainer site: it sits in the chat page's column, and on other pages it becomes a concierge in the bottom-right corner (a sheet on phones) that continues the open conversation with the same agent and tools.
- Reply cues in `avatar.intent`: emotion, intensity and a gesture from a fixed vocabulary. The model can suggest them with a `[[mood:…]]` line, which the server validates and strips from the reply; otherwise trilingual rules decide.
- `behavior.json` per character (emotion to expression weights, gesture fallbacks, one-shot gestures, the motion graph) and `motion_sets` in the character manifest. `AVATAR_GAME_MOTION_DIR` serves a local game motion library for research use only.
- Tests: Rust unit tests, `npm test` for the character planner, motion graph and lip-sync timing, and a 16-question tool-choice test (`cargo run -- eval-tools`).
- Local animation-reference preprocessing for Tazuna videos the user supplies (`tools/motion-reference/`): source timestamps, fixed character crops, whole-body landmarks, anime masks, Japanese OCR, audio, reference clips, contact sheets, a review page, and locked dependency and model records.
- Docker Compose for PostgreSQL, and a three-stage release image for the API and the built site.
- Project documents under `docs/`: product spec and decisions, worldbuilding and sources, architecture, data model, API and SSE events, character player and voice contracts.
