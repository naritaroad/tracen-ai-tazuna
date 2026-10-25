# Changelog



The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-beta] - 2026-10-26

### Added

- The academy calendar now lists the game's race schedule: 34 G1 and 42 G2 races (`data/seed/race_schedule.csv`, new `races` table in migration 0004) with grade, racecourse, turf or dirt, distance, direction and the career years that may enter, in the Taiwan, Japanese and English game names. Each race is placed every year on its half month (1st–15th or 16th–end), never closes a facility, and can be filtered to G1 only or hidden. The `list_academy_events` tool returns the races too. Sources: GameTora's game data, checked against BiliGame (marked `community`).

- 3D character module on the chat page: a three-vrm player (three 0.186.1, @pixiv/three-vrm 3.5.5) that loads on demand and falls back to the 2D state card without character files, without WebGL, on a load failure, or when the trainer picks the simple view. The character thinks, looks things up, explains, bows when handing a case to the Director's Office, greets an empty chat, nods at sentence ends, moves her mouth along the reply text, blinks, follows the pointer with her eyes and head, and reacts when tapped. High and standard quality (60/30 FPS), pauses when hidden, honours reduced motion.
- Reply cues in `avatar.intent`: `emotion`, `intensity` and a semantic `gesture` from a fixed vocabulary. The model may suggest them with a `[[mood:…]]` line that the server validates and always strips from the reply; otherwise trilingual rules decide, and a hand-over or missing data always shows.
- `behavior.json` per character (emotion → expression weights, gesture fallbacks, one-shot gestures, the motion graph) and `motion_sets` in the character manifest; the local game motion library can be served with `AVATAR_GAME_MOTION_DIR` (research use only).
- `npm test` for the character planner, motion graph and lip-sync timing.

- Started construction with the questionnaire's recommended options as provisional decisions (`docs/product/decisions.md`), plus product spec, worldbuilding guide, data model, API and stream-event contracts, character-player contract and an AI-agent path map.
- Rust API (`backend/`): accounts and cookie sessions, PostgreSQL schema and migrations, demo catalog rebuilt from CSV at start, five agent tools (academy info, calendar, facility search, training-plan calculation, Director's Office hand-over), a bounded agent loop with recorded tool traces, SSE chat stream, trainer memory, Director's Office inbox for a real person, management endpoints, and a genai model gateway with an offline mock.
- Reserved character-player endpoints and `avatar.intent` events; AI-driven animation itself is postponed.
- Web front end (`web/`): trainer site (chat with tool trace, information, calendar, facilities, plan calculator, Director's Office, my details) and management site (case inbox, demo data, run log, accounts), in Traditional Chinese, Japanese and English with light and dark themes.
- Demo data: 10 information topics, 17 training services, 10 calendar entries; a 14-question tool-choice test and the `eval-tools` command.
- Docker Compose for PostgreSQL 18.6 and a multi-stage release image.
- Added a dated README plan review and a 16-question decision worksheet, covering scope, the database choice, staged delivery, avatar integration, memory and third-party candidates. Preserved the original README plan.

- Recorded the website Docker scope around the product: PostgreSQL plus a release image of the Rust API in Compose, with local Rust compiles during development, and the 3D client, Android app, and motion pipeline staying on the machine that displays or authors them. Summarized the same scope in README.md.
- Local animation-reference preprocessing for user-provided Tazuna videos: source timestamps, fixed character crops, whole-body landmarks, anime masks, Japanese OCR, audio, reference clips and contact sheets.
- A local review page, reproducible dependency/model records, data integrity checks and a Claude animation-authoring handoff.

### Changed

- Tazuna's system prompt now describes who she is and how she talks, from the official character page and community summaries of the game text (docs/product/worldbuilding.md §3): she answers what the trainer said first, varies her greetings, cares about tired or late-working trainers, uses her own small habits in chat and keeps her secrets. She can chat off topic; with the default setting she steers back in one bridging sentence after about three off-topic exchanges, and she still declines big jobs (essays, code, homework) and never invents academy facts, news or weather.
- The management site's AI settings gain "Conversation style and thinking": reply length (brief, natural, detailed), small talk (off, balanced, relaxed), thinking depth (auto, low, medium, high), temperature, output limit per call and model calls per message. The chat agent no longer forces temperature 0.2 and a 1,200-token limit: Google recommends temperature 1.0 for Gemini 3, and on Gemini the limit includes thinking tokens, so replies could be cut off. Defaults: natural length, balanced small talk, the model's own thinking and temperature, 8,192 tokens, 6 calls (`AGENT_MAX_STEPS` is now only the starting value). Model and run time-outs grow with the allowed calls. The tool-choice test gains a small-talk question. A line break that the model writes as the two characters `
` is turned into a real one. `.claude/launch.json` gains `api-alt2`/`web-alt2` (ports 8082/5176) for when other sessions hold the usual ports.

- Project documents that lived in the `docs/` root are grouped into `docs/product/` (decisions, spec, worldbuilding), `docs/engineering/` (path map, architecture, data model, how to run), `docs/character/` (player contract, behaviour design, assets, voice) and `docs/frontend/` (interface copy). Filenames are unchanged. `docs/reviews/` stays the historical record and was not rewritten. The index is `docs/README.md`.
- Tazuna is now on every trainer page: one character instance moves between the chat page's column and a concierge in the bottom-right corner of the other pages (a sheet from a button on phones), so the 3D model loads once. The concierge has a speech bubble, questions that fit the page, and continues the conversation last open on the chat page through the same agent and tools; it shows which tool she is using, links a Director's Office hand-over, and opens the full answer and trace in the chat page.
- When the API sends a character model without motion data (an API started before its character files changed), the 2D card now says so and asks for an API restart, instead of claiming there are no character files; the reason is also logged in the browser console. `.claude/launch.json` gains an `api` entry (`cargo run`) next to `web`.
- Tapping the character now gets a gesture and a short spoken line (shown above her and lip-synced): a greeting or a line about the current page, a reaction to a pat on the head, and shyness after three pats. The 2D card answers taps too.
- Site layout follows desktop AI assistants (Claude, ChatGPT): the top bar and page tabs moved into a left sidebar with new conversation, conversation search, the pages, the conversation history and an account menu (language, theme, sign-out). The sidebar collapses to an icon rail and remembers it; Ctrl/⌘+Shift+O starts a conversation and Ctrl/⌘+K searches. The chat page centres a greeting, the composer and suggestions on an empty chat, then reads as one column with plain-text replies, a copy button, a jump-to-latest button and a growing rounded composer. On phones the sidebar is a drawer, the pages sit in a draggable side bubble that steps aside while typing, and the character greets a new chat but hides after each reply (the character button brings her back).
- Web front end restyled in a modern Japanese look built on the Tracen uniform: violet and white with wisteria tints and a gold horseshoe accent, in light and dark themes; the favicon and browser theme colour follow. Japanese pages now use Japanese fonts first.
- UI copy moved to JSON language files (`web/src/i18n/locales/`), loaded on demand per language. Remaining hard-coded text in components (admin table headings, run statuses, roles, usage purposes, units, currency, list separators, the character name, API error messages, the page title) now comes from the language files, `Intl` formatting, or the API. `npm run i18n:check` (also run by `npm run build`) checks key and placeholder parity across the three languages and rejects hard-coded text. See `docs/frontend/frontend-i18n.md`.
- The Director's Office introduction now reads 「網頁沒有的資訊可以寫訊息找秋川理事長」 (with Japanese and English equivalents).
- The calendar names affected facilities instead of showing their ids, and the plan calculator explains missing or ambiguous facilities in words.

- Followed up the questionnaire: Gemini first (`gemini-3.8-flash`, `gemini-embedding-2`); AI models, prices and a monthly budget (block or warn) adjustable on the management site with usage per model; semantic memory with pgvector (past events, automatic curation with supersede history, conversation summaries, all visible and deletable by the trainer); microphone input with server-side speech to text and reserved spoken-reply endpoints; `anime` and `community` source types; a Capacitor 8 Android app built and installed over USB.
- Database image switched to `pgvector/pgvector:0.8.7-pg18-trixie` (PostgreSQL 18.6).
- Phones and the Android app: a conversation picker replaces the hidden sidebar on narrow screens, and the app returns to the last page after the WebView reloads (it was losing the open conversation around the microphone permission prompt). The username field no longer auto-capitalises or autocorrects.
- Recorded the filled-in questionnaire (Q01 C, Q04 A with C, Q08 B, Q13 A with microphone input; the rest default) in `docs/product/decisions.md`, with open conflicts and missing information; updated the worldbuilding source priority, the memory and voice sections of the product spec, and added a voice contract.
- Refined the plan around PostgreSQL as the product database, optional Android SQLite caching, source-backed trainer services, and confirmed human-admin handoff presented as Director Akikawa Yayoi; updated the review and questionnaire.
