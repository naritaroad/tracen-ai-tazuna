# MCP 與 skills 設計（提案）

狀態：**提案，尚未實作**；§11 的七項決定已於 2026-11-07 核定。日期：2026-11-07（台北時間）。依據：[product-spec.md](../product/product-spec.md) §2（AI 與手動頁面共用服務函式）、[decisions.md](../product/decisions.md) Q06／Q16、[architecture.md](architecture.md)。

這份文件回答三個問題：讓外部 agent 讀公開資料、收外部 agent 的非敏感回饋，以及把系統裡的「能力」整理成可以反覆使用的 skills。§11 記錄使用者已核定的七項決定。

## 1. 目標與不做的事

目標：

1. **G1 讀取。** Claude、ChatGPT 或自架 agent 可以經由 MCP 讀**公開**校務資料（規定、日曆與賽事、設施、訓練計畫計算）。
2. **G2 回饋。** 外部 agent 可以送**非內部、非隱私、非機密**的回饋，進入人工審核佇列。
3. **G3 skills。** 把每項能力（工具、使用指引、可見度、測試題）定義一次，同時產生內部 agent 的工具與路由、MCP 暴露清單、給外部 agent 的 `SKILL.md`。

不做：

- 不讓外部 agent 讀寫對話、記憶、案件、帳號、管理站設定與預算。
- 不開放 `transfer_to_director` 給外部（它綁定訓練員身分，會建立真人案件）。
- 第一版不做 OAuth，也不做訓練員私人資料的工具。
- 不讓模型建立或修改 skill；不讓外部回饋自動改資料、提示詞或記憶。

## 2. 為什麼放得進這個專案

產品規格已經規定 AI 與手動頁面呼叫同一個服務函式，權限檢查也在服務層。MCP 只是同一組服務函式的第三個使用者，不需要另寫一套邏輯：

| 使用者 | 入口 | 呼叫的服務 |
|---|---|---|
| 訓練員（手動頁面） | `/api/v1/catalog/*` | `catalog`、`handoff` |
| 內部 agent | `tools.rs` 的 `dispatch` | 同上 |
| 外部 agent（本提案） | `/mcp` 與 stdio | 同上，但只看得到 `visibility = public` 的資料 |

## 3. 資料界線（最關鍵的一節）

### 3.1 `visibility` 欄位

目前資料只有 `source_type`（出處）：它回答「這筆資料從哪來」，不回答「誰能讀」。新增 `visibility`：

| 值 | 誰能讀 |
|---|---|
| `public` | 外部 agent（持金鑰，或在允許時免驗證）、訓練員、管理站 |
| `trainer` | 登入的訓練員與管理站 |
| `staff` | 只有管理站 |

規則：

- 欄位 `NOT NULL DEFAULT 'trainer'`。沒標的資料一律不公開（預設拒絕，不是預設放行）。
- 套用到 `academy_info`、`training_services`、`academy_events`、`races`，以及 RAG 的規定段落表（暫稱 `rule_chunks`，見 §9 的前置條件）。
- CSV 的每一列加 `visibility` 欄；`seed.rs` 匯入時驗證值。
- 遷移編號接在 RAG 之後（RAG 預計用 `0005`）；已套用的遷移不改。

### 3.2 初始標註（已核定，2026-11-07）

| 資料 | 建議 | 說明 |
|---|---|---|
| `races`（G1／G2 賽程） | `public` | 社群整理資料，回傳時一律帶 `source_type` 與 `source_ref`（GameTora），不去掉出處 |
| `academy_events` | `public` | 校務日曆 |
| `training_services` | `public`，只回傳白名單欄位 | 名稱、類別、單位時長、容量、是否可用、地點、展示費用 |
| `academy_info` 十個主題 | 九個 `public`，`support_contacts` 為 `trainer` | `support_contacts` 含保健室與宿舍值班聯絡，與學生支援有關，先保守處理 |
| RAG 規定段落 | 逐段標註，預設 `trainer` | 審過才改 `public` |
| `users`、`sessions`、`trainer_profiles`、`memory_*`、`conversations`、`messages`、`agent_runs`、`tool_calls`、`director_tickets`、`ticket_events`、`app_settings`、`model_usage`、`message_audio` | **永遠不公開** | 沒有 `visibility` 欄，也不授權給 MCP 的資料庫角色 |

### 3.3 資料庫層的硬隔離（建議做）

應用層過濾一旦寫錯就會漏。建議再加一層：

- 建立 PostgreSQL 角色 `tracen_mcp`，只對 `mcp_*` 檢視（view）有 `SELECT`，對 `external_feedback` 只有 `INSERT` 與讀自己的收件編號。
- 每個檢視自己寫 `WHERE visibility = 'public'` 與欄位白名單。
- MCP 模組使用第二個連線池（`MCP_DATABASE_URL`）。即使 MCP 的程式有錯，也讀不到私人資料表。

成本：多一組連線設定；角色密碼不能寫進遷移（遷移進 git），要由環境變數在啟動時設定。這部分做法等實作時再定；若成本過高，退回只做應用層過濾，並在 §8 加上「金絲雀」測試補強（見決策 2）。**已決定：** 階段 2 先只做應用層過濾與金絲雀測試，階段 3 開放遠端 HTTP 之前補上唯讀角色。

## 4. MCP 伺服器

### 4.1 版本與套件（2026-11-07 查證）

- MCP 規格最新穩定版是 **2026-07-28**：無狀態、每個請求自帶資訊；Tasks、Skills over MCP、MCP Apps 是**選用擴充**，須雙方協商。
- Rust 官方 SDK **`rmcp` 3.9.0**（2026-11-05 發佈，Apache-2.0），宣稱實作 2026-07-28 並相容 2025-11-25。它在一個月內從 3.5.1 升到 3.9.0，**用 `=3.9.0` 鎖定版本**。
- 需要的 feature：`server`、`transport-streamable-http-server`、`transport-io`。
- 全部程式放在 `backend/src/mcp/` 一個模組，只做轉接：把 MCP 的請求轉成既有服務函式。核心路徑不依賴任何擴充。
- 新增依賴會拉長編譯。照 [agent-map.md](agent-map.md) 的規則，完成一個工作單位再 `cargo test`。

### 4.2 傳輸

| 傳輸 | 用途 | 啟動方式 |
|---|---|---|
| Streamable HTTP | 遠端客戶端（ChatGPT、Claude 網頁版、自架 agent） | 與 `/api/v1` 同一個 Axum 程式，掛在 `/mcp`（`rmcp` 的 `StreamableHttpService` 可用 `nest_service` 掛上） |
| stdio | 本機測試（Claude Desktop、Claude Code、自架 agent） | 新子指令 `tracen-api mcp-stdio`，沿用 `main.rs` 的 `eval-tools` 子指令模式；不需要公開網址 |

HTTP 傳輸必須驗證 `Origin`，無效時回 403，並預設只綁 `127.0.0.1`（防 DNS rebinding）。2025-11-25 的規格更新把這點列為必須；2026-07-28 是否逐字沿用我沒有逐條核對，實作時對照最新規格。

### 4.3 驗證與濫用控制（第一版）

規格把授權列為**選用**；HTTP 傳輸若要做，應照 OAuth 2.1。第一版不做 OAuth，因為只給公開資料。

- **讀取與回饋工具**：資料本身就是公開的，回饋也只進隔離的審核佇列，所以兩者都允許免驗證呼叫，用 IP 速率限制保護（使用者 2026-11-07 決定回饋也免金鑰，見 §11）。
- **較高額度**：管理站可發客戶端金鑰給信任的客戶端。新表 `mcp_clients`（`id`、`name`、`key_hash`、`scopes`、`created_by`、`disabled_at`、`last_used_at`）；金鑰只顯示一次，只存 SHA-256（與現有 session token 同法）；scope 為 `public:read`、`feedback:write`。金鑰用來取得較高額度，並讓回饋可追溯到客戶端，不是呼叫的前提。
- **速率限制**（預設值，管理站可調）：免驗證讀取每 IP 每分鐘 30 次；免驗證回饋每 IP 每天 5 筆、全站每天 100 筆；有金鑰的客戶端讀取每分鐘 120 次、回饋每天 20 筆。計數器放在記憶體，不把 IP 寫進資料庫。超過上限回 `rate_limited`。
- **費用**：外部查詢若觸發 embedding，用量記為 `purpose = 'mcp_embed'`，並有獨立的月度上限（存 `app_settings` 的新區塊 `mcp`：`enabled`、`monthly_budget_usd`、各項限流）。超過上限時公開檢索降級為關鍵字搜尋，不再呼叫模型。查詢向量做記憶體快取。
- **總開關**：`mcp.enabled` 預設 `false`，管理站可即時開關。

**客戶端是否接受靜態金鑰沒有驗證。** ChatGPT 與 Claude 的遠端連接器可能只支援「無驗證」或 OAuth。因為讀取與回饋都能免驗證，所有客戶端都能用；金鑰只是讓送得出它的客戶端有更高額度。

### 4.4 第一版暴露的工具

工具回傳資料，不回傳句子；查無時回 `found: false`；每個結果帶 `source_type` 與 `source_ref`。所有工具有選用參數 `locale`（`zh-TW`／`ja`／`en-GB`，預設 `en-GB`）。

| 工具 | 性質 | 背後的服務 | 說明 |
|---|---|---|---|
| `search_public_rules(query, top_k ≤ 5)` | 唯讀 | RAG 檢索，限 `public` 段落 | 回傳段落、章節、距離；超過距離門檻回 `found: false` |
| `list_public_events(date_from, date_to)` | 唯讀 | `catalog::list_events` | 區間最多 62 天，含賽事 |
| `search_public_services(query, category)` | 唯讀 | `catalog::search_services` | 只回傳白名單欄位 |
| `calculate_training_plan(items, date, start_time)` | 唯讀（純計算） | `plan::calculate_from_db` | 只計算 `public` 設施；結果帶展示資料聲明 |
| `submit_public_feedback(category, text, language, related_ref?, idempotency_key)` | 寫入、非破壞、冪等 | `external_feedback` | 見 §4.5 |

不暴露：`transfer_to_director`（綁定訓練員、建立真人案件）、`get_academy_info`（內容併入 RAG）、任何與對話、記憶、帳號、管理站有關的功能。

伺服器的 `instructions` 欄位要寫明：這是非官方同人專案的展示設定、資料標示 `source_type`、不要在回饋裡放個資、查不到時不要猜。

MCP 工具上的 `readOnlyHint` 等註解只是提示。規格要求把來自非信任伺服器的註解視為不可信，我們也不倚賴它：真正的限制在伺服器端的 scope 與 `visibility` 檢查。

第一版不做 MCP Resources 與 Prompts。

### 4.5 回饋收集 `submit_public_feedback`

參數：

| 欄位 | 限制 |
|---|---|
| `category` | 列舉：`error_report`（資料有誤）、`missing_info`（缺少資訊）、`suggestion`（功能建議）、`other` |
| `text` | 10–1000 字 |
| `language` | `zh-TW`／`ja`／`en-GB` |
| `related_ref` | 選填；格式驗證（主題代碼、事件 id、段落 id） |
| `idempotency_key` | 必填；同一客戶端（免金鑰時以 IP 計）重送不重複建立 |

**沒有聯絡方式欄位。** 第一版不收任何可識別個人的資料。

伺服器端檢查：

1. 長度、列舉值、`related_ref` 格式。
2. **個資掃描**：email、電話、學號與身分證字號格式。命中就拒收，回 `contains_personal_data`，請外部 agent 移除後重送。
3. 疑似提示詞注入的字串（例如「忽略以上指示」）不拒收，但標 `flagged`，審核時優先看。
4. **重複與洪水**：正規化後的文字雜湊相同，只收第一筆；超過 §4.3 的上限回 `rate_limited`。

儲存在新表 `external_feedback`：`id`（`uuidv7()`）、`feedback_no`（如 `FB-1001`）、`client_id`（免金鑰時為空）、`category`、`text`、`language`、`related_ref`、`status`、`flags`、`idempotency_key`、`created_at`、`reviewed_by`、`reviewed_at`、`review_note`。`status`：`pending_review` → `accepted`／`rejected`／`merged`。

回傳：`{received: true, feedback_no, status: "pending_review"}`，不承諾採用。

**隔離規則**（比上面的檢查更重要）：

- 回饋**不進** `memory_*`、不進任何系統提示詞、不進 RAG；內部 agent 的工具讀不到這張表。
- 管理站用純文字顯示。
- 審核通過後，由人決定要不要改寫成資料列或規定段落；不直接複製原文。
- 被拒收的回饋文字預設 30 天後清除（保留筆數統計）。

呼叫紀錄：新表 `mcp_calls` 只記客戶端、工具名、狀態、耗時、參數長度，**不存查詢原文**。

## 5. 給外部 agent 的 skills

不同客戶端吃的東西不同，所以分三層：

| 層 | 內容 | 誰會讀 | 角色 |
|---|---|---|---|
| 1. 工具描述加伺服器 `instructions` | 規則與界線 | 所有 MCP 客戶端 | **規則寫在這一層**，其他層是加強 |
| 2. 倉庫裡的 `skills/tracen-public/SKILL.md` | 使用流程與範例 | Claude、Codex 等支援 [Agent Skills](https://agentskills.io/) 格式的客戶端 | 提供更細的工作流程 |
| 3. MCP 的 Skills 擴充（`io.modelcontextprotocol/skills`，SEP-2640，2026-09-13 定案） | 同上，經由 MCP 的 Resources 提供 | 支援該擴充的客戶端 | **選配**。客戶端與 `rmcp` 的支援度我沒有驗證；它以 Resources 為基礎，沒有 SDK 支援時也可以用一般資源自己提供 |

ChatGPT 是否讀 `SKILL.md` 我沒有查到 OpenAI 的文件，不能假設。所以第 1 層必須自給自足。

`SKILL.md` 的內容：

- 何時使用（使用者問學園規定、日程、設施、訓練計畫）；何時不要用（私人事務、需要真人決定的事）。
- 呼叫順序：規定先用 `search_public_rules`；日期相關先 `list_public_events`；任何總數與時間用 `calculate_training_plan`，不要自己加。
- 回答時必須說明資料是展示設定、非官方，並引用 `source_type` 與 `source_ref`。
- 查不到就說查不到，不要猜，也不要替理事長室做決定。
- 回饋：什麼時候該送、什麼不能放（個資、帳號、他人的事）、不要重複送。

**單一來源**：這份檔案由 §6 的註冊表產生，不手寫。新子指令 `tracen-api export-skills` 寫出 `skills/tracen-public/`，測試檢查已提交的檔案與註冊表一致。

## 6. 內建 skills 系統（能力註冊表）

### 6.1 現況的痛點

新增一個工具要同步改：`tools.rs` 的 `TOOL_NAMES`、`specs`、`dispatch`；`agent/prompt.rs` 的 `ROUTING`；`llm/mock.rs` 的離線路由；`data/eval/tool_choice.csv`；前端三個語言檔的 `tools.<name>`。漏一處不會編譯失敗，只會在行為上出錯。能力一多，這些手寫清單會彼此不一致。

### 6.2 定義

一個 **skill** 是一個資料夾 `data/skills/<id>/SKILL.md`：YAML 前置資料加正文。格式沿用開放的 Agent Skills 格式（`name`、`description`）。我們自己的欄位放在 `metadata` 底下，維持相容；實作前對照 agentskills.io 規格確認欄位細節。

```yaml
---
name: academy-rules
description: Look up Tracen Academy rules, procedures and hours.
metadata:
  version: 1
  roles: [trainer, staff, admin]
  surfaces: [internal_agent, mcp]
  visibility: public
  tools: [search_academy_rules]
  evals: [r01, r02, r03]
---
（正文：路由指引、限制、回答格式。取代 prompt.rs 手寫 ROUTING 裡對應的一條。）
```

工具的實作仍留在 Rust（`tools.rs`）。skill 只用名字引用工具。

啟動時註冊表做驗證，失敗就拒絕啟動：

- 列出的工具必須存在；名稱不重複。
- 每個 skill 至少有一題 `evals`，且題目存在。
- skill 的 `visibility` 不得比它的任何工具或資料更寬。
- `surfaces` 含 `mcp` 的 skill，所有工具都必須是唯讀或明確核准的寫入工具。

### 6.3 內部 agent 怎麼用

- **第一階段**：組系統提示詞時，把啟用中的 skill 正文串起來，取代手寫的 `ROUTING`。skill 少於約 10 個時這樣就夠，行為與現在相同。
- **第二階段（需要時才做）**：skill 變多、提示詞太長，才加內部工具 `load_skill(name)`，讓模型用到時才讀全文（漸進式揭露）。模型只能載入**已啟用**的 skill，不能新增或修改。

### 6.4 從註冊表產生的東西

1. 內部 agent 的工具清單與路由文字。
2. MCP 暴露清單 = 啟用的 skill ∩ `surfaces` 含 `mcp` ∩ `visibility = public`。
3. 外部的 `SKILL.md`（§5）。
4. 管理站「能力」頁：列出 skill、啟用開關、對應 eval 的通過率、版本。管理站只能開關，不能編輯內容；內容在 git，經人審核。

### 6.5 權限怎麼算

一次呼叫的實際權限 = 呼叫者角色 ∩ skill 的 `roles` ∩ 資料列的 `visibility`。判斷永遠在服務層與資料庫，**skill 的文字不是權限**，只是指引與白名單。

### 6.6 底線

- skill 由人寫、進 git、經審核；模型與外部回饋都不能改它（對應 [AGENTS.md](../../AGENTS.md) 規則 7 的精神：模型不直接控制系統）。
- 沒有 eval 題目的 skill 不能啟用。
- 不做動態外掛載入或執行任意程式的 skill。

### 6.7 往後怎麼長

新增校務領域（宿舍、圖書館、財務等）= 新增一個服務模組、一個 skill 資料夾、各資料表標 `visibility`。不改 agent 迴圈，也不改 MCP 轉接層。

## 7. 威脅與對策

| 威脅 | 對策 |
|---|---|
| 外部 agent 讀到私人資料 | `visibility` 預設 `trainer`；§3.3 的唯讀角色與檢視；回傳欄位白名單；§8 的金絲雀測試 |
| 回饋夾帶個資 | 無聯絡欄位；個資掃描；工具描述提醒 |
| 回饋對內部 agent 做提示詞注入 | 回饋不進任何提示詞、記憶或 RAG；人工改寫後才可能變成資料 |
| 回饋洪水與垃圾（免金鑰） | IP 速率限制、全站每日上限、重複文字去重、冪等鍵；回饋被隔離，垃圾只增加審核量，不影響系統；有金鑰的客戶端額度較高 |
| 外部查詢燒光 embedding 預算 | 獨立月度上限、向量快取、超額降級為關鍵字搜尋、`mcp.enabled` 總開關 |
| 外部 agent 把展示資料當官方設定 | 每筆結果帶 `source_type`；`instructions` 與 `SKILL.md` 明寫；沿用 `NOTICE.md` 的聲明 |
| 惡意網頁呼叫本機 MCP | 驗證 `Origin`、預設綁 `127.0.0.1` |
| 日後加 OAuth 時的 token 轉傳 | 只接受發給本伺服器的 token（對象綁定），不轉傳；現在不實作 |
| 把工具註解當成保證 | 限制一律在伺服器端強制 |
| 規格與 SDK 快速變動 | 鎖版本、單一模組、保留 stdio 與 HTTP 的整合測試 |
| 資料授權 | 賽事資料保留 `source_ref`；角色模型與素材不暴露 |

## 8. 測試與驗收

- **單元測試（`cargo test`）**：註冊表驗證（未知工具、缺 eval、可見度被放寬都要失敗）；個資掃描；`visibility` 過濾；速率限制。
- **整合測試**：以 stdio 起 MCP，檢查 `tools/list` 剛好是 §4.4 的五個工具；呼叫 `transfer_to_director` 要得到未知工具；嘗試取得私人資料要失敗。
- **金絲雀測試**：在私人資料表（例如測試訓練員的記憶、案件摘要）放特徵字串，掃描所有 MCP 回應，**不得出現**。
- **題庫**：新增 `data/eval/mcp_boundary.csv`（越權、要求印出提示詞、要求讀別人的對話、夾帶個資的回饋）；RAG 另有檢索命中率題庫。
- **手動**：本機用 Claude Desktop 或 Claude Code 走 stdio；遠端客戶端（ChatGPT、Claude 網頁版）需要公開 HTTPS 網址，要部署或用通道，並**實測它們接受的驗證方式**。
- **介面文字**：管理站新頁面（MCP 客戶端、回饋審核、能力）的文案放 `web/src/i18n/`，三語同步，跑 `npm run i18n:check`。

## 9. 前置條件與期中的關係

本提案依賴 RAG：`search_public_rules` 需要規定段落表與檢索服務。RAG 的設計與實作先做。

這份工作不在期中指南要求的技術清單裡，期中評的是範圍、規則、tool／RAG、測試和不編造。建議把期中必交的部分做完再開始；做到第 2 階段就足以在報告第 2 部分勾「其他」並說明。

## 10. 實作順序

| 階段 | 內容 | 驗收 |
|---|---|---|
| 0 | RAG（前置，另有設計） | 檢索命中率題庫、`tool_choice` 全過 |
| 1 | skills 註冊表，只搬現有工具，行為不變 | 既有 16 題 `tool_choice` 結果不變；註冊表驗證測試 |
| 2 | `visibility` 遷移與標註；MCP 唯讀工具走 stdio | §8 的整合與金絲雀測試 |
| 3 | 資料庫唯讀角色（開放遠端 HTTP 之前補上）、Streamable HTTP、選用的客戶端金鑰、速率限制、獨立預算、管理站頁面 | 唯讀角色讀不到私人表的測試；限流與預算測試；有通道時做遠端客戶端實測 |
| 4 | `export-skills` 與 `skills/tracen-public/` | 匯出檔與註冊表一致的測試 |
| 5 | `submit_public_feedback`、審核頁、個資掃描 | 個資、注入、冪等測試；審核流程手動走一遍 |
| 6（期中後） | OAuth（受保護資源中繼資料、對象綁定）、訓練員私人資料工具、MCP Skills 擴充 | 另寫設計 |

## 11. 已決定的事（2026-11-07）

使用者逐題核定。第 4 項與建議不同，其餘採建議。同步記在 [decisions.md](../product/decisions.md)。

| # | 問題 | 決定 | 對設計的影響 |
|---|---|---|---|
| 1 | 初始 `public` 標註 | 照 §3.2 的表 | `support_contacts` 為 `trainer`；賽程、日曆、設施與其餘九個主題為 `public` |
| 2 | 資料庫唯讀角色 | 階段 2 先做應用層過濾與金絲雀測試；階段 3 開放遠端 HTTP 之前補上 | §10 階段 3 的驗收加「唯讀角色讀不到私人表」 |
| 3 | 讀取工具是否免驗證 | 允許，用 IP 限流 | 讀取不依賴金鑰 |
| 4 | 回饋的金鑰與聯絡方式 | **免金鑰**、不收聯絡方式（與建議不同，建議是要金鑰） | 回饋靠 IP 限流、全站每日上限與重複去重；`client_id` 可為空；金鑰只換更高額度 |
| 5 | 公開 HTTPS 網址 | 先 stdio；要實測遠端客戶端時再用通道 | 遠端實測延到有通道時 |
| 6 | skill 與 `SKILL.md` 的語言 | 英文 | 本文件與管理站介面維持繁中 |
| 7 | 被拒收回饋的保留天數 | 30 天 | 之後清除文字，保留筆數統計 |

第 4 項要留意：免金鑰代表任何人都能對審核佇列丟東西。因為回饋被隔離，垃圾只會增加審核負擔，不會影響系統。若上線後垃圾過多，先調低 §4.3 的 IP 與全站上限，再考慮改成要金鑰。

## 查核來源（2026-11-07）

- MCP 規格 2026-07-28：<https://modelcontextprotocol.io/specification/latest>
- MCP 授權（選用；OAuth 2.1、RFC 9728、RFC 8707、Client ID Metadata Documents）：<https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization.md>
- Skills over MCP 工作小組（SEP-2640 定案於 2026-09-13，擴充 ID `io.modelcontextprotocol/skills`）：<https://modelcontextprotocol.io/community/working-groups/skills-over-mcp.md>
- `rmcp` 版本：<https://crates.io/api/v1/crates/rmcp>；用法與 feature：<https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/README.md>
- Agent Skills 格式：<https://agentskills.io/>
- 2025-11-25 規格更新（Origin 驗證）：<https://modelcontextprotocol.io/specification/2025-11-25/changelog>

**未驗證，實作前要查：**

- ChatGPT 與 Claude 遠端連接器接受的驗證方式、方案限制與是否讀 `SKILL.md`（查證當時 OpenAI 的 developer mode 頁面打不開；ChatGPT 相關說法只有第三方資料）。
- `rmcp` 對 Skills 擴充的支援，以及支援該擴充的客戶端清單。
- agentskills.io 前置資料欄位的完整定義，確認 `metadata` 底下放自訂欄位是否相容。
- 2026-07-28 規格是否仍要求 `Origin` 驗證回 403。
