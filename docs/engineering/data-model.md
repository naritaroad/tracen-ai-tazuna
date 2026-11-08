# 資料模型

結構定義在 [`backend/migrations/0001_init.sql`](../../backend/migrations/0001_init.sql)。這裡只記錄擁有者與生命週期。

| 表 | 擁有者 | 生命週期 | 說明 |
|---|---|---|---|
| `academy_info` | 展示資料（CSV） | `CATALOG_SEED_MODE=reset` 時每次啟動重建 | 校務資訊：`topic`＋`content`，另有日文、英文與來源 |
| `training_services` | 展示資料（CSV） | 同上 | 訓練設施與資源：名稱、數值（`unit_minutes`、`capacity`、`fee_jpy`）、`available` |
| `academy_events` | 展示資料（CSV） | 同上 | 校務日曆；`affects` 為受影響設施 id，`all` 表全校 |
| `races` | 展示資料（`race_schedule.csv`） | 同上 | 遊戲的 G1／G2 賽程：等級、賽場、場地、距離、方向、`month`＋`half`（1＝前半、2＝後半）、可出賽的 `classes`。不存日期；`catalog::list_events` 依查詢區間把每場賽事展開成每年一筆 `kind = race` 的日曆項目（前半 1–15 日、後半 16 日到月底），不封鎖任何設施 |
| `handbook_chunks` | 手冊檔（`data/rag/*.md`）與 `academy_info` | 每次啟動依檔案與資料表同步；`content_hash` 與 `embed_model` 不變就保留向量 | RAG 的段落：`doc`（`handbook`／`academy_info`）、`section`（`§3.2` 或主題代碼）、`part`、`lang`、`embedding vector(768)`、`visibility`（0005） |
| `external_feedback` | 外部 agent（經 MCP）送來，由人審核 | 被拒絕的 30 天後清除 `text`，保留筆數 | 沒有聯絡欄位；`client_key` 是客戶端 id 或 `anonymous`，不存 IP；`text_hash` 去重（0007） |
| `mcp_clients` | 管理員 | 永久；可停用 | 外部客戶端金鑰，只存 SHA-256，金鑰只顯示一次（0007） |
| `mcp_calls` | 系統 | 永久（稽核） | 每次 MCP 工具呼叫的客戶端、工具、狀態、耗時、參數長度；不存參數原文與 IP（0007） |
| `users`、`sessions` | 帳號 | 永久；工作階段到期刪除 | 只存 token 的 SHA-256 |
| `trainer_profiles`、`memory_items` | 訓練員本人（`source = agent` 為自動整理） | 本人可改、可刪；刪除會覆寫內容並清除向量；被取代的保留為 `superseded` | 不是校務事實來源；`embedding vector(768)` 用於去重 |
| `memory_events` | 訓練員本人 | 本人可刪（連同向量） | 過去事件的一句話筆記與向量，供語意回想 |
| `app_settings` | 管理員 | 永久 | `ai`：模型、價格、每月預算與超額處理；`conversation`：回覆長度、題外話、思考深度、溫度、輸出上限、每則訊息的模型呼叫次數；`mcp`：遠端開關、MCP 預算與限流（1.1）；`skills`：管理員關掉的 skill（1.1） |
| `model_usage` | 系統 | 永久（帳務） | 每次付費模型呼叫的 tokens 與依管理站價格算出的費用 |
| `message_audio` | 系統 | 保留，目前為空 | AI 語音回應（[voice-contract.md](../character/voice-contract.md)） |
| `conversations`、`messages` | 訓練員本人 | 永久 | `messages.role`：user／assistant／director（真人）／notice |
| `agent_runs`、`tool_calls` | 系統 | 永久（稽核） | 每次工具呼叫的名稱、參數、結果與狀態 |
| `director_tickets`、`ticket_events` | 理事長室 | 永久（稽核） | `idempotency_key` 防止重送開兩件；`actor_user_id` 是實際操作者 |

## 可見度與公開檢視表（1.1）

`academy_info`、`training_services`、`academy_events`、`races`、`handbook_chunks` 都有 `visibility`（`public`／`trainer`／`staff`，預設 `trainer`）。`source_type` 說資料從哪來，`visibility` 說誰能讀。

遷移 0007 建了只含公開列的檢視表：`mcp_info`、`mcp_services`（`notes` 一律為空）、`mcp_events`、`mcp_races`、`mcp_handbook`，以及 `mcp_settings`（只有 `ai`、`mcp`、`skills`）、`mcp_usage`（沒有使用者與 run）、`mcp_client_keys`、`mcp_feedback_keys`（沒有回饋內文）。唯讀角色 `tracen_mcp` 只能讀這些檢視表，只能新增 `model_usage`、`mcp_calls`、`external_feedback`，讀不到其他資料表。未登入的人呼叫 `/api/v1/catalog/*` 時也讀這些檢視表。

## 規則

- 展示資料重建不碰帳號、對話、案件與記憶。
- 所有讀寫都帶伺服器端的使用者身分；訓練員只能讀自己的對話、案件與記憶。
- 正式查詢使用 PostgreSQL，業務查詢集中在 `catalog` 模組；手冊檢索在 `handbook` 模組。
- 外部回饋不進 `memory_*`、任何提示詞或 `handbook_chunks`；內部 agent 沒有讀它的工具。
