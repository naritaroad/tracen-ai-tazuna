# 資料模型

結構定義在 [`backend/migrations/0001_init.sql`](../../backend/migrations/0001_init.sql)。這裡只記錄擁有者與生命週期。

| 表 | 擁有者 | 生命週期 | 說明 |
|---|---|---|---|
| `academy_info` | 展示資料（CSV） | `CATALOG_SEED_MODE=reset` 時每次啟動重建 | 校務資訊：`topic`＋`content`，另有日文、英文與來源 |
| `training_services` | 展示資料（CSV） | 同上 | 訓練設施與資源：名稱、數值（`unit_minutes`、`capacity`、`fee_jpy`）、`available` |
| `academy_events` | 展示資料（CSV） | 同上 | 校務日曆；`affects` 為受影響設施 id，`all` 表全校 |
| `races` | 展示資料（`race_schedule.csv`） | 同上 | 遊戲的 G1／G2 賽程：等級、賽場、場地、距離、方向、`month`＋`half`（1＝前半、2＝後半）、可出賽的 `classes`。不存日期；`catalog::list_events` 依查詢區間把每場賽事展開成每年一筆 `kind = race` 的日曆項目（前半 1–15 日、後半 16 日到月底），不封鎖任何設施 |
| `users`、`sessions` | 帳號 | 永久；工作階段到期刪除 | 只存 token 的 SHA-256 |
| `trainer_profiles`、`memory_items` | 訓練員本人（`source = agent` 為自動整理） | 本人可改、可刪；刪除會覆寫內容並清除向量；被取代的保留為 `superseded` | 不是校務事實來源；`embedding vector(768)` 用於去重 |
| `memory_events` | 訓練員本人 | 本人可刪（連同向量） | 過去事件的一句話筆記與向量，供語意回想 |
| `app_settings` | 管理員 | 永久 | `ai`：模型、價格、每月預算與超額處理；`conversation`：回覆長度、題外話、思考深度、溫度、輸出上限、每則訊息的模型呼叫次數 |
| `model_usage` | 系統 | 永久（帳務） | 每次付費模型呼叫的 tokens 與依管理站價格算出的費用 |
| `message_audio` | 系統 | 保留，目前為空 | AI 語音回應（[voice-contract.md](../character/voice-contract.md)） |
| `conversations`、`messages` | 訓練員本人 | 永久 | `messages.role`：user／assistant／director（真人）／notice |
| `agent_runs`、`tool_calls` | 系統 | 永久（稽核） | 每次工具呼叫的名稱、參數、結果與狀態 |
| `director_tickets`、`ticket_events` | 理事長室 | 永久（稽核） | `idempotency_key` 防止重送開兩件；`actor_user_id` 是實際操作者 |

## 規則

- 展示資料重建不碰帳號、對話、案件與記憶。
- 所有讀寫都帶伺服器端的使用者身分；訓練員只能讀自己的對話、案件與記憶。
- 正式查詢使用 PostgreSQL，業務查詢集中在 `catalog` 模組。
