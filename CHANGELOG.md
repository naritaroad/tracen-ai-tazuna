# Changelog

All notable changes to this project are recorded here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Local Gemini TTS module and command-line tool for rendering single-speaker demo audio from text or replaying a saved request with the project API key.
- Claude Code plugin `plugins/tracen-academy`: the `tracen-public` skill plus the local MCP server (`tracen-api mcp-stdio`), installable through `.claude-plugin/marketplace.json` (`/plugin install tracen-academy@tracen-ai-tazuna`). `export-skills` writes the skill to both `skills/tracen-public/` (kept for other agent systems) and the plugin, and a test checks both copies.
- `plugins/tracen-academy.zip`: the plugin packed with its root at the top level, importable directly in Claude (plugin upload) or with `claude --plugin-dir`. Built by `python tools/package-plugin/package_plugin.py` (deterministic; `--check` fails when it is stale; it refuses to pack when the plugin's skill copy differs from `skills/tracen-public/`).
- The API finds `.env` from `TRACEN_HOME` or from the executable's folders when the working directory has none, so an MCP client can start it from anywhere.
- `docs/engineering/manual-test-plan.md`: manual acceptance plan for 1.1 (skills inventory, cases, answer key, record sheets).

- Information topic `dormitory` (學生宿舍與舍規): the Ritto and Miho dorms, self-government under their dorm leaders, no entry for outsiders or, as a rule, trainers (dorm leader approval in special cases), and cafeteria meals allowed in the dorm. Marked `community`; the dorm leaders are checked against the official character pages. Two new `tool_choice` questions and three `eval-rag` questions.

### Changed

- Version numbers in `backend/Cargo.toml` and `web/package.json` now read 1.1.0-beta, matching this file.
- Curfew is now 22:00, with no going out until 05:30 (was 21:00). Facilities now close at 21:30 instead of 20:30, and training or activities after 21:30 need a late-return request (`LAST_USE` in the plan calculator follows). Handbook §6.3: trainers do not enter the dorms, and special cases need the dorm leader's approval instead of the Director's Office.

### Fixed

- Outside feedback: long numbers such as case numbers (`DIR-20261108-001`), order numbers and timestamps with a 0 inside are no longer refused as phone numbers, and full-width phone numbers and e-mail addresses are now refused.

## [1.1.0-beta] - 2026-11-07

The course midterm (Unit H): retrieval over a trainer handbook, a five-part system prompt and tests for staying in scope; plus built-in skills and an MCP server for outside agents. Plan and results: `docs/engineering/upgrade-plan-1.1.md`.

### Added

- Trainer Handbook for RAG: `data/rag/trainer_handbook.zh-TW.md`, 13 chapters and 53 sections of demo rules (procedures, conditions, exceptions, penalties, appeals), written so that it never repeats numbers the CSV files own. A start-up check refuses bad headings, empty or duplicate sections, unknown visibility values, and language files whose sections differ.
- `handbook_chunks` (migration 0005): the handbook and the information topics, split by section and embedded with `gemini-embedding-2` (768 dimensions). An embedding is kept while the text and the model stay the same, so a restart does not pay again; indexing runs in the background and search falls back to keywords until it is done.
- Tool `search_academy_rules` and the *Trainer Handbook* page (`GET /api/v1/handbook`, `/handbook/search`). Search ranks by cosine distance with a small boost for shared wording, cuts off at 0.31, and returns whole sections in the reader's language where one exists. The activity log lists the sections found.
- Five-part system prompt (role and goal, scope, interaction rules, refusals and special cases, opening line), with three fixed sentences per language for not found, out of scope and keeping the rules, and an internal marker that tests check never appears in a reply. The chat page shows the same opening line, and a sixth sample question asks the handbook.
- Built-in skills (`data/skills/`): seven skills in the Agent Skills format plus `registry.json` for tools, MCP tools, roles, surfaces, visibility and tests. The prompt's tool rules (4–9), the agent's tool list and the MCP tool list come from it; the registry is checked at start. Admins can switch a skill off and on again (`PUT /api/v1/admin/skills/{name}`).
- `visibility` on catalog rows (migration 0006; `public`, `trainer` or `staff`, default `trainer`) and a `visibility` column in the four seed CSV files.
- MCP server: five tools over the public views (`search_public_rules`, `list_public_events`, `search_public_services`, `calculate_training_plan`, `submit_public_feedback`), over stdio (`tracen-api mcp-stdio`) and Streamable HTTP at `/mcp` (`rmcp` 3.9.0). Remote access needs the read-only role `tracen_mcp` (`MCP_DATABASE_URL`) and an admin switch; it checks the Host and refuses browser origins that are not listed, limits calls per address or client key in memory, and has its own monthly search budget.
- Migration 0007: client keys (`mcp_clients`, SHA-256 only), outside feedback (`external_feedback`, no contact fields, rejected text emptied after 30 days), a call log without arguments or addresses (`mcp_calls`), public-only views, and the `tracen_mcp` role.
- Feedback from outside agents: length and format checks, refusal of e-mail addresses, phone numbers and ID or student numbers, a flag for likely prompt injection, and de-duplication. Staff review it on the new management page *MCP and skills*, which also has the remote switch, limits, client keys, call counts and the skills with their latest test results.
- `tracen-api export-skills` writes `skills/tracen-public/SKILL.md` for outside agents from the registry; a test fails when the committed file is out of date.
- Tests: `eval-rag` (46 questions), `eval-boundary` (12), `eval-mcp` (13 cases with a canary in private tables, plus checks that the read-only role cannot read them); five new `tool_choice` questions; unit tests for parsing, the registry, feedback checks, limits and the prompt parts (62 in total).
- Documents: `docs/engineering/upgrade-plan-1.1.md` and `docs/engineering/mcp-next.md` (load-on-demand skills, OAuth, the MCP Skills extension, cross-language search); product spec, data model, path map, development, API and decisions updated.

### Changed

- **Breaking:** without a session, `/api/v1/catalog/*` returns only `visibility = public` rows. `support_contacts` is for signed-in trainers now.
- **Breaking:** the API needs `data/rag/` and `data/skills/` and stops at start if either is invalid; the Docker image copies both.
- **Breaking:** `avatar::cues::prompt_rule` takes the rule number, and the prompt's rules are renumbered 1–17 by part.
- Routing: rules and "can I" questions go to `search_academy_rules`; after a not-found answer, Tazuna offers the Director's Office and opens a case only when the trainer agrees, complains, asks for an exception or asks for a person.
- Replies use no Markdown, because the chat page shows text as written.
- Course homework section of the three READMEs rewritten for Unit H; feature lists mention the handbook, skills and MCP.
- `tool_choice.csv`: q09, q10 and q13 also accept `search_academy_rules`, and q02, q03 and q21 accept a second relevant lookup (facility rules or the calendar), because the new routing rule 5 tells the model to check the handbook when a short topic does not cover the detail. With `gemini-3.8-flash`: 18/21 calls within the accepted tools and 21/21 answers with the expected value; the three misses (q02, q07, q20) made an extra lookup before or after the right tool.
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
