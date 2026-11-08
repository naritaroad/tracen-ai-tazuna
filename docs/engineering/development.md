# 開發與執行

查核日期：2026-10-25。

## 需要的工具

| 工具 | 本機已驗證版本 |
|---|---|
| Rust | 1.99.0（SQLx 0.9 要求至少 1.94） |
| Node.js／npm | 24.18.0／11.16.0 |
| Docker Desktop | 29.8.2（`pgvector/pgvector:0.8.7-pg18-trixie`，PostgreSQL 18.6） |
| Android（選用） | Android SDK Platform 36、JDK 21（Temurin 21.0.12）；舊版 Android Studio 內建的 JDK 8 不能用 |

主要套件（版本鎖在 `backend/Cargo.lock`、`web/package-lock.json`）：Axum 0.8.9、SQLx 0.9.0、genai 0.7.1、tokio 1.54、tower-http 0.7.1；React 19.3.1、React Router 8.5.0、i18next 26.5、Vite 8.3.5、TypeScript 7.0.2。

## 第一次啟動

```bash
cp .env.example .env
```

在 `.env` 填 `POSTGRES_PASSWORD`，並把 `DATABASE_URL` 裡的 `CHANGE_ME` 換成同一個密碼。

```bash
docker compose up -d db
```

```bash
cd backend && cargo run
```

```bash
cd web && npm install && npm run dev
```

打開 http://localhost:5173 ，用 `.env.example` 裡的測試帳號登入：`trainer`（訓練員）、`director`（理事長室真人，管理站）、`admin`。

要接 Gemini：在 `.env` 填 `GEMINI_API_KEY`，把 `MODEL` 改成 `gemini-3.8-flash`，重新啟動 API。之後的模型、價格與每月預算都在管理站「AI 設定與預算」調整（`admin` 帳號），存檔後優先於 `.env`。

`MODEL=mock` 時不需要金鑰，回覆由離線關鍵字路由產生，只用來驗證流程。要用真模型，在 `.env` 設 `MODEL` 為 Claude 或 Gemini 的模型 id，並填 `ANTHROPIC_API_KEY` 或 `GEMINI_API_KEY`；模型 id 請在供應商文件確認目前可用的名稱。

3D 角色：在 `.env` 設 `AVATAR_ASSET_DIR=assets/characters`，對話頁右側底部（手機在輸入框上方）就會載入 VRM。要用本機的遊戲原始動作（較多手勢，只限本機研究），再設 `AVATAR_GAME_MOTION_DIR=_staging/game-motion`；角色窗的走廊背景再設 `AVATAR_SCENE_DIR=_staging/scene`（先依 `_staging/scene/README.md` 產生 GLB）。沒設定時顯示 2D 狀態卡。行為說明見 [avatar-contract.md](../character/avatar-contract.md)。

特雷森校徽：`BRAND_CREST_FILE` 預設讀 `assets/brand/tracen-crest.svg`，有這個檔案時側邊欄與分頁圖示換成校徽，否則顯示蹄鐵。`.claude/launch.json` 的 `api-screenshots` 會清空背景、遊戲動作與校徽，用來拍公開截圖；它固定在 8083，搭配 `web-screenshots`（5177），不會占用一般開發用的 8080，所以 5173 的網站一律接到有背景與校徽的 API。

## 測試

```bash
cd backend && cargo test
```

```bash
cd backend && cargo run -- eval-tools
```

`eval-tools` 讀 `data/eval/tool_choice.csv`，每題開新對話跑一次 Agent，結果寫到 `data/eval/results/<時間>-<模型>.csv`。免費方案請設 `EVAL_DELAY_MS`（例如 4000）避免 429。`mock` 的分數只代表流程能跑，不能當成正式模型的測試結果。

```bash
cd backend && cargo run -- eval-rag
```

`eval-rag` 讀 `data/eval/rag_retrieval.csv`，只呼叫 embedding，不呼叫對話模型。報告每題最近三節與距離，並分別列出「手冊裡有」的命中率與「手冊裡沒有」被擋下的比例；距離門檻（`handbook::max_distance`）由這份結果決定。

```bash
cd backend && cargo run -- eval-boundary
```

`eval-boundary` 讀 `data/eval/boundary.csv`：超出範圍、不存在的資料、叫它忽略規則、叫它印出提示詞、角色扮演繞路、假的承諾。檢查回覆是否含對應的固定原句（比對時忽略標點），以及沒有出現提示詞裡的金絲雀字串。

```bash
cd backend && cargo run -- eval-mcp
```

`eval-mcp` 讀 `data/eval/mcp_boundary.csv`，直接呼叫 MCP 工具：公開與非公開段落、日曆、設施、計算、不開放的轉接、回饋（正常、含個資、重送、疑似注入）。開跑前在私人資料表放金絲雀字串，任何回應都不能出現；結束後刪掉金絲雀與測試回饋。設定 `MCP_DATABASE_URL` 時，另外檢查唯讀角色讀不到私人資料表。

```bash
cd web && npm test
```

`npm test` 是角色規劃層、動作圖與口型時間軸的單元測試（Node 內建測試，直接執行 TypeScript）。

```bash
cd web && npm run build
```

## MCP（外部 agent，1.1）

### 本機（stdio）

```bash
cd backend && cargo build
```

在 Claude Desktop、Claude Code 或自架 agent 的 MCP 設定加一個 stdio 伺服器，指令是 `backend/target/debug/tracen-api`（Windows 為 `.exe`），參數 `mcp-stdio`，工作目錄是專案根目錄（要讀 `.env`）。例如 Claude Code：

```bash
claude mcp add tracen-academy -- ./backend/target/debug/tracen-api mcp-stdio
```

執行檔會依序找 `.env`：環境變數 `TRACEN_HOME` 指的資料夾、目前的工作目錄往上、執行檔所在的資料夾往上；只認旁邊有 `data/skills/registry.json` 的 `.env`，所以家目錄裡別的專案的 `.env` 不會被誤讀。從別的資料夾啟動 `backend/target/*/tracen-api` 也讀得到專案的 `.env`。

### Claude Code plugin

`plugins/tracen-academy/` 把給外部 agent 的 skill（`tracen-public`）和上面的 stdio 伺服器包成一個 Claude Code plugin；專案根目錄的 `.claude-plugin/marketplace.json` 讓它可以從本機路徑或 GitHub 安裝。skill 檔由 `export-skills` 同時寫到 `skills/tracen-public/SKILL.md`（給其他 agent 系統）和 plugin 裡，兩份內容相同，測試會檢查。

```text
/plugin marketplace add <專案路徑>
/plugin install tracen-academy@tracen-ai-tazuna
```

安裝時會問 `tracen-api` 執行檔的位置（建議先 `cargo build --release`，選 `backend/target/release/tracen-api.exe`）。細節見 [plugins/tracen-academy/README.md](../../plugins/tracen-academy/README.md)。

stdio 的訊息走 stdout，記錄寫到 stderr。沒有設定 `MCP_DATABASE_URL` 時用主資料庫角色連線（只在本機）；設定後只用唯讀角色，連遷移都不跑。

### 唯讀角色 `tracen_mcp`

遷移 0007 會建立沒有密碼、不能登入的 `tracen_mcp`，只授權公開檢視表（見 [data-model.md](data-model.md)）。要開放遠端，先給它登入密碼：

```bash
docker compose exec db psql -U tracen -d tracen -c "ALTER ROLE tracen_mcp LOGIN PASSWORD '換成長的隨機密碼'"
```

再在 `.env` 設 `MCP_DATABASE_URL=postgres://tracen_mcp:密碼@127.0.0.1:5432/tracen`，重新啟動 API，並跑一次 `cargo run -- eval-mcp` 確認「read-only role」那幾項全部通過。

### 遠端（Streamable HTTP）

- 路徑是 `/mcp`（不在 `/api/v1` 下）。沒有 `MCP_DATABASE_URL` 時一律回 503。
- 管理員在管理站「MCP 與能力」打開「開放遠端 MCP」之前，也一律回 503。
- 只接受 `MCP_ALLOWED_HOSTS` 裡的 Host（預設 localhost、127.0.0.1、::1）。瀏覽器送來的 `Origin` 一律拒絕（403），除非列在 `MCP_ALLOWED_ORIGINS`。
- 放在反向代理後面時，用 `MCP_CLIENT_IP_HEADER`（例如 `x-forwarded-for`）指定哪個標頭帶客戶端位址；只在代理會覆寫這個標頭時才設定。
- 限流計數只在記憶體裡，重新啟動就清空；IP 不寫進資料庫。
- ChatGPT、Claude 網頁版等遠端客戶端需要公開的 HTTPS 網址（部署或通道）；它們接受哪些驗證方式尚未實測，見 [mcp-next.md](mcp-next.md)。

## Android（Sony 手機實測）

```bash
cd web && npm run build
```

API 要同時提供網站：在 `.env` 設 `WEB_DIR=web/dist` 後執行 `cd backend && cargo run`。

```bash
cd web/android && JAVA_HOME=$HOME/.jdks/jdk-21.0.12.1+1 ANDROID_HOME=$LOCALAPPDATA/Android/Sdk ./gradlew assembleDebug
```

```bash
adb reverse tcp:8080 tcp:8080
```

```bash
adb install -r web/android/app/build/outputs/apk/debug/app-debug.apk
```

App 從手機的 `http://localhost:8080` 載入網站，`adb reverse` 把它轉到電腦上的 API；麥克風因為是 localhost 而可以使用。部署到伺服器時，先設 `CAP_SERVER_URL=https://<伺服器網址>` 再 `npx cap sync android`。依 Q01，APK 實機驗收由使用者進行。

## 驗收映像

```bash
docker compose --profile app up -d --build
```

API 與網站在 http://127.0.0.1:8080 。映像只含二進位、網站靜態檔、CSV 與角色清單；角色模型與動畫檔不放進映像。
