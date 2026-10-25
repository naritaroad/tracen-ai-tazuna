# 專案路徑導航（給 AI 工程代理）

先讀這份，再讀 [decisions.md](../product/decisions.md) 與 [product-spec.md](../product/product-spec.md)；角色與語音介面見 [avatar-contract.md](../character/avatar-contract.md)、[voice-contract.md](../character/voice-contract.md)。角色資產的來源與規則見 [character-assets.md](../character/character-assets.md)；情境對應動作與表情的設計提案見 [avatar-behavior-design.md](../character/avatar-behavior-design.md)。日期：2026-10-25。

## 1. 目錄

| 路徑 | 內容 | 可以改嗎 |
|---|---|---|
| `README.md` | 正式專案介紹（英文） | 可改；與 `docs/readme/` 的繁中、日文版同步，章節結構相同 |
| `README.txt` | 原始專案計畫（本機，`.gitignore` 排除） | **計畫文字不可改** |
| `LICENSE`、`NOTICE.md`、`CONTRIBUTING.md`、`SECURITY.md`、`.github/` | 授權、權利聲明、貢獻與安全政策、Issue／PR 範本 | 可改 |
| `AGENTS.md` | 專案級代理規則 | 可補充，不刪原有規則 |
| `docs/` | 文件目錄，入口是 [docs/README.md](../README.md) | 子目錄見下 |
| `docs/product/` | 施工決策、產品規格、世界觀 | 可改 |
| `docs/engineering/` | 路徑導航、架構、資料模型、開發與執行 | 可改 |
| `docs/character/` | 角色契約、行為設計、資產規則、語音契約 | 可改 |
| `docs/frontend/` | 介面文案與多語言 | 可改 |
| `docs/api/` | HTTP API、串流事件 | 可改 |
| `docs/brand/` | 標誌概念稿 | 可改 |
| `docs/readme/` | 專案介紹的繁中版 `README.zh-TW.md`、日文版 `README.ja.md` | 可改；與根目錄 `README.md` 同步 |
| `docs/assets/` | README 用的截圖（橫幅在 `docs/brand/banners/`） | 可改；截圖不得含遊戲衍生場景或遊戲動作 |
| `docs/reviews/` | 歷史評估 | **不改寫原文** |
| `backend/` | Rust API（Axum＋SQLx＋genai） | 可改 |
| `backend/migrations/` | PostgreSQL 遷移 | **已套用的檔案不可改**，新增 `000N_*.sql` |
| `web/` | Vite＋React＋TS 前端（訓練員站＋管理站） | 可改 |
| `web/android/` | Capacitor Android 專案（App 從 API 伺服器載入網站） | 可改；`app/build/` 是建置產物 |
| `data/seed/*.csv` | 展示資料（資訊表、紀錄表、日曆、遊戲賽程 `race_schedule.csv`） | 可改；每列都要有 `source_type` |
| `data/eval/tool_choice.csv` | 工具選擇題庫 | 可增題，不刪既有題 |
| `assets/characters/<id>/` | 角色清單、動作清單、行為對照、加工紀錄、.vrm／.vrma | `character.json`、`animations.json`、`behavior.json`（情緒 → 表情、手勢替代、動作組的圖）可改；**模型與動畫檔不可覆蓋**，`work/` 是動作擬合中間資料 |
| `tools/vrm-test-viewer/` | 角色測試頁 | 獨立工具 |
| `tools/motion-reference/` | 影片動作參考前處理 | 獨立工具，Python，不進容器 |
| `example_pose/`、`/vrm/` | 原始影片、參考資料、PMX | **唯讀**，不進 git |
| `_staging/game-motion/` | 遊戲原始動作暫存區（VRMA、清單、測試頁）；網站可用 `AVATAR_GAME_MOTION_DIR` 在本機載入 | 可改；遊戲衍生資料不進 git（見目錄內 `.gitignore`） |
| `_staging/scene/` | 對話頁角色窗的背景場景（遊戲走廊 GLB、`scene.json`、建置腳本）；網站可用 `AVATAR_SCENE_DIR` 在本機載入 | 可改；GLB 與 .blend 不進 git |
| `_staging/mini/` | Q 版たづな的遊戲動作暫存區與測試頁（模型本體在 `assets/characters/9001_hayakawa_tazuna_mini/`），尚未整合 | 可改；遊戲衍生資料不進 git |

## 2. 後端模組（`backend/src/`）

| 檔案 | 責任 |
|---|---|
| `main.rs` | 啟動、路由組裝、`eval-tools` 子指令 |
| `config.rs` | 環境變數 |
| `seed.rs` | CSV → 展示資料表；測試帳號 |
| `auth.rs` | 帳號、cookie 工作階段、`CurrentUser`／`StaffUser`／`AdminUser` 擷取器 |
| `catalog/` | 校務資訊、日曆、設施查詢（`matching.rs` 寬鬆名稱比對）、`plan.rs` 訓練計畫計算、`http.rs` 手動頁面 API |
| `tools.rs` | 工具規格、參數驗證、分派（白名單） |
| `agent/` | 有界 Agent 迴圈、系統提示詞 |
| `llm/` | `Ai` 服務：對話（`ModelGateway`）、嵌入、語音轉文字；genai（真模型）與 `mock`（離線） |
| `settings.rs` | 管理站可調的模型、價格、每月預算；用量紀錄與預算檢查 |
| `speech.rs` | 麥克風輸入轉文字；語音回應的保留端點 |
| `conversations.rs` | 對話、SSE 串流 |
| `handoff.rs` | 理事長室案件（訓練員端與管理站端） |
| `memory/` | 偏好與事件記憶、語意回想（`mod.rs`）、背景自動整理與對話摘要（`curate.rs`） |
| `brand.rs` | 官方校徽 `/brand/crest.svg`：本機有 `BRAND_CREST_FILE` 才回傳，否則 404 |
| `picture.rs` | 訓練員頭像（`/me/avatar`：上傳、讀取、移除；以檔頭判斷 PNG／JPEG／WebP，最大 512 KB） |
| `admin.rs` | 管理站：展示資料維護、帳號、執行紀錄 |
| `avatar/mod.rs` | 角色介面：狀態表、`avatar.intent`、角色清單檔與動作組（`motion_sets`）、素材檔 |
| `avatar/cues.rs` | 回覆的情緒／強度／語意手勢：解析並刪除模型的 `[[mood:…]]`，或用三語規則推定 |
| `eval.rs` | 工具選擇測試 |

新增工具：在 `tools.rs` 的 `TOOL_NAMES`、`specs`、`dispatch` 三處同時加，業務邏輯放在對應模組，手動頁面共用同一函式；再補 `data/eval/tool_choice.csv` 題目與前端三個語言檔的 `tools.<name>`。

## 3. 前端（`web/src/`）

`api.ts`（型別與 SSE 解析）、`pages/`（訓練員站）、`admin/`（管理站，不用 AI）、`avatar/`（`states.ts` 狀態機；純函式的 `behavior.ts` 規劃層、`taps.ts` 點擊反應、`speech.ts` 口型、`motionGraph.ts` 動作圖；`AvatarStage.tsx` 選 3D 或 2D；`VrmStage.tsx` 與 `player/` 是按需載入的 three-vrm 播放器）、`i18n/`（`locales/*.json` 介面文案，`zh-TW.json` 是參考語言；`errors.ts` 翻譯 API 錯誤代碼）、`format.ts`（日期、數字、日圓、清單）、`components/Layout.tsx`（側邊欄外框：新對話、搜尋、頁面、對話紀錄（可釘選）、帳號選單（頭像、稱呼與帳號名稱、「我的資料」、語言、主題、登出）；手機版是抽屜加可拖曳的頁面泡泡）、`components/PictureCropper.tsx`（頭像手動裁切：拖曳、縮放、雙指縮放、方向鍵）、`conversations.tsx`（側邊欄與對話頁共用的對話清單）、`concierge/`（全站唯一的角色實例與在各頁之間移動的位置、各訓練員頁右下角的諮詢窗）、`components/Icon.tsx`（SVG 圖示）、`brand.ts`（本機有校徽時，`Mark` 與 favicon 換成校徽，否則保留蹄鐵）、`styles.css`（設計變數，紫白配色）。文案放哪裡、鍵的寫法與檢查見 [frontend-i18n.md](../frontend/frontend-i18n.md)。

## 4. 指令

```bash
docker compose up -d db              # PostgreSQL 18.6
cd backend && cargo run              # API，讀專案根目錄 .env
cd web && npm run dev                # 前端，/api 代理到 127.0.0.1:8080
cd backend && cargo test             # 單元測試（不需要資料庫）
cd backend && cargo run -- eval-tools   # 工具選擇測試，結果寫到 data/eval/results/
cd web && npm run i18n:check         # 三語鍵值一致、無寫死文字
cd web && npm test                   # 角色規劃層、動作圖、口型（node --test）
cd web && npm run build              # 文案檢查＋型別檢查＋建置
docker compose --profile app up -d --build   # 驗收：API 映像＋資料庫
```

規則：不要每改一行就編譯 Rust；完成一個工作單位再 `cargo test`。Rust 只用靜態 SQL（SQLx 0.9 要求 `&'static str`），組字串的查詢必須是常數拼接並包 `AssertSqlSafe`。

## 5. 不要做的事

- 不要把模型金鑰放進前端或提交 `.env`。
- 不要讓模型送自由 SQL、任意 JavaScript 或逐幀骨架。
- 不要把 `project`／`reference` 資料寫成官方設定（見 [worldbuilding.md](../product/worldbuilding.md)）。
- 不要建立 git 分支，直接在 `main` 上工作（使用者指示）。
