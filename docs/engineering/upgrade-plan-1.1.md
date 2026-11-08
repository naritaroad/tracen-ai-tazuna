# 1.1 升級計畫：RAG、內建 skills 與 MCP

狀態：**A1–C3 已實作並驗收（2026-11-07）**；C4 另寫在 [mcp-next.md](mcp-next.md)。實作時和原計畫不同的地方記在 §10。日期：2026-11-07（台北時間）。施工進度與交接記在專案根目錄的 `.agent-context/`。

依據：課程期中指南（Unit H，`Midterm-Project-Guide-zh-TW.md`）、[product-spec.md](../product/product-spec.md)、[decisions.md](../product/decisions.md)、[architecture.md](architecture.md)、[data-model.md](data-model.md)、[agent-map.md](agent-map.md)。

## 1. 要做什麼

| # | 項目 | 為什麼 |
|---|---|---|
| 1 | **RAG**：一份長篇《訓練員手冊》，以 pgvector 檢索，做成第 6 個工具 `search_academy_rules` | 期中指南規定，延伸客服作業就要加 RAG。現在的 `academy_info` 只有 10 個短主題，屬於「放進表格的事實」，不算 RAG |
| 2 | **內建 skills**：能力註冊表 | 現在加一個工具要改六處（[agent-map.md](agent-map.md) §2），漏一處不會編譯失敗，只會在行為上出錯 |
| 3 | **MCP**：讓 Claude、ChatGPT、自架 agent 讀公開校務資料，之後收非內部、非隱私、非機密的回饋 | 使用者要求（2026-11-07），往 agent 式校務系統擴充 |

不做：

- 不讓外部 agent 讀寫對話、記憶、案件、帳號、管理站設定與預算。
- 不對外開放 `transfer_to_director`（它綁定訓練員身分，會建立真人案件）。
- 不讓模型或外部回饋建立、修改 skill、資料、提示詞或記憶。
- 本版不做 OAuth，也不做訓練員私人資料的 MCP 工具。

## 2. 期中要求對照

| 指南要求 | 本計畫 | 階段 |
|---|---|---|
| 加上 RAG（§3、§4.4） | §4 | A2–A3 |
| 分成五部分的 system prompt（§4.2） | §4.6 | A4 |
| 聊天畫面記得對話，顯示開場白與範例問題（§4.3） | 已有；補開場白與 RAG 範例問題 | A4 |
| 只用虛構資料（§4.5） | 手冊全部 `source_type = project` | A1 |
| 不編造的測試，報告的 Conversation 3（§5） | §4.7 | A5 |
| GitHub：public、`.env.example`、套件清單、README、`.gitignore`（§6.2） | `.gitignore` 七項已齊；三語 README 的作業區塊從 Unit G 改成 Unit H | A6 |
| PDF 報告（§6.1） | 填 `Midterm-Report-Template-zh-TW.docx`，存成 PDF | A6 |

MCP 與內建 skills 不在指南的技術清單裡，報告第 2 部分可以勾「其他」說明。

## 3. 施工順序

A 是期中必繳。B 要不要在期中前做，看繳交日期（§8 第 1 題）。C 在期中之後。每個階段做完才 `cargo test`（AGENTS.md 規則 5）。

| 階段 | 內容 | 驗收 | 建議 skill |
|---|---|---|---|
| A1 | 寫繁中版手冊 `data/rag/trainer_handbook.zh-TW.md` | 40–60 節，每節有節號；不寫會變的數字 | `human-writing-zh`、`zh-tw-localization` |
| A2 | 遷移 `0005_handbook.sql`；`seed.rs` 切塊、embed、快取 | 重啟不重新 embed 沒變的塊；`MODEL=mock` 可離線跑 | `current-version-check` |
| A3 | 工具 `search_academy_rules`；手動頁「訓練員手冊」；Activity log 顯示節號 | `cargo test`；三語 `tools.search_academy_rules`；`npm run i18n:check` | `code-review` |
| A4 | 提示詞改成五部分；固定原句；開場白 | `prompt.rs` 測試；product-spec §5 同步 | `human-writing-ui` |
| A5 | 題庫 `rag_retrieval.csv`、`boundary.csv`，`tool_choice.csv` 加題；子指令 `eval-rag`、`eval-boundary` | 真模型各跑一次並留下結果（mock 的結果不算） | `testing-strategy` |
| A6 | 三語 README 作業區塊、CHANGELOG、截圖、PDF 報告 | 指南 §6 的檢查清單；推上去前做安全檢查 | `human-writing-readme`、`project-changelog`、`docx`、`pdf`、`security-review` |
| B1 | skills 註冊表，搬入現有 6 個工具，行為不變 | `tool_choice` 結果不變；註冊表驗證測試 | `simplify` |
| B2 | 遷移 `0006_visibility.sql`；CSV 加 `visibility` | 沒標的資料一律是 `trainer` | |
| B3 | MCP 唯讀工具，走 stdio：`tracen-api mcp-stdio` | `tools/list` 剛好 4 個；金絲雀測試 | `current-version-check` |
| B4 | `tracen-api export-skills` 產生 `skills/tracen-public/SKILL.md` | 已提交的檔案與註冊表產生的結果一致 | |
| C1 | Streamable HTTP `/mcp`、`Origin` 驗證、資料庫唯讀角色 | 唯讀角色讀不到私人表 | `security-review` |
| C2 | 限流、MCP 獨立預算、選用的客戶端金鑰、管理站「MCP」與「能力」頁 | 限流與預算測試 | |
| C3 | `submit_public_feedback`、審核頁、個資掃描 | 個資、注入、冪等測試；審核流程手動走一遍 | |
| C4 | `load_skill` 漸進載入、OAuth、MCP 的 Skills 擴充 | 另寫設計 | `mcp-builder` |

`research-trail` 從 A1 開始全程記錄 AI 工具、時數與用途，對應報告第 4 部分。commit 用 `no-agent-attribution`，作者只留使用者一人。

## 4. RAG

### 4.1 內容放哪裡

| 內容 | 放哪裡 | 誰讀 |
|---|---|---|
| 會變的數字與狀態：時段長度、容量、費用、開放與否、日期 | 既有 CSV 與資料表 | 既有工具 |
| 10 個短主題（受理時間、門禁、文件…） | `academy_info.csv`，不動 | `get_academy_info`；同時編進檢索索引 |
| 長篇條文、流程、例外、罰則 | 新的手冊 | `search_academy_rules` |

手冊寫流程與條件，不寫會變的數字；要用到數字時寫「以設施資料為準」。同一件事只有一個來源。

### 4.2 手冊

- 路徑：`data/rag/trainer_handbook.{zh-TW,ja,en}.md`，三份的節號相同。依 Q15，先寫繁中並定稿，再補日文與英文；測試檢查已有的檔案節號一致。
- 格式：章用 `## 第 3 章 標題`，節用 `### §3.2 標題`，一節一個主題。可見度寫在節標題下一行的 HTML 註解 `<!-- visibility: public -->`；沒寫的是 `trainer`。HTML 註解不進索引。
- 章節草案（A1 定稿）：總則與適用範圍、訓練員職責、出賽報名與文件、傷病與健康通報（只寫通報流程，不給醫療意見）、設施使用細則與損壞賠償、宿舍與門禁例外申請、遠征與交通費報支、器材借用、媒體採訪與肖像、獎懲與申訴、本系統與帳號使用、附則。
- 全部是本專案設定：`source_type = project`，`source_ref = 訓練員手冊 §x.y`。不和遊戲與動畫的官方設定衝突，依 [worldbuilding.md](../product/worldbuilding.md) 的來源順序。

### 4.3 資料表（`0005_handbook.sql`）

`handbook_chunks`：`id`（`uuidv7()`）、`doc`（`handbook` 或 `academy_info`）、`section`（`§3.2` 或主題代碼）、`part`（同一節切成多塊時的序號）、`lang`、`title`、`content`、`content_hash`、`embedding vector(768)`（可為空）、`embed_model`、`visibility`（`NOT NULL DEFAULT 'trainer'`；其他表到 B2 才加）、`source_type`、`source_ref`。唯一鍵 `(doc, section, part, lang)`。

資料量約數百列，循序掃描就夠，先不建 hnsw 索引。

### 4.4 匯入與 embed

- 在 `seed.rs` 匯入展示資料之後執行，`CATALOG_SEED_MODE` 兩種模式都做。
- 切塊：依 `###` 標題切成節，章名併進塊的標題；一節超過約 800 字，再依段落切。不加新依賴；不夠用時再評估 `text-splitter`（2026-11-07 最新穩定版 0.33.1）。
- 快取：`content_hash` 與 `embed_model` 都沒變的列保留原向量，只 embed 新增或改過的塊；檔案裡已經不存在的塊刪除。重啟不會重新付費。
- 用量記進 `model_usage`，`purpose = 'embed'`，和記憶相同。
- embed 失敗（沒有金鑰、網路錯誤）不擋啟動：向量留空、記一筆警告，工具退回關鍵字比對。
- `MODEL=mock` 用 `mock::embed`，測試可以離線跑。

### 4.5 工具 `search_academy_rules`

- 參數：`query`（訓練員的原話）、`top_k`（預設 3，最多 5）。
- 做法：embed `query` → 用餘弦距離找最近的塊，三種語言一起找 → 依 `section` 去重 → 回傳訓練員語言的版本，沒有就回傳原本的語言。
- 回傳：`{found, results: [{section, title, content, distance, source_type, source_ref}]}`。最近一塊的距離超過門檻就回 `found: false`。門檻由 A5 的 `eval-rag` 決定，寫成常數，不交給模型判斷。
- 查詢的 embed 用量記 `purpose = 'rag'`。
- 業務邏輯放新模組 `backend/src/handbook/`，`tools.rs` 只做轉接。手動頁面 API `GET /api/v1/handbook/search` 呼叫同一個函式（product-spec §2：AI 與手動頁面呼叫同一個服務函式）。
- 前端：新頁「訓練員手冊」（搜尋與各節全文）；Activity log 顯示引用的節號；三語文案放 `web/src/i18n/`。

### 4.6 提示詞改成五部分

| 指南的部分 | 現在的位置（`backend/src/agent/prompt.rs`） | 要改的 |
|---|---|---|
| 角色與目標 | `PERSONA` 的 WHO YOU ARE | 加一句目標：依學園資料與手冊，幫訓練員把校務辦好 |
| 範圍 | `scope_rule`、`SCOPE_LIMITS` | 不變 |
| 互動規則 | HOW YOU TALK、`voice`、`length_rule`、`language_line` | 不變 |
| 拒絕與特殊情況 | `GROUNDING` 第 2 條只寫 "say so plainly" | 改成照念的固定原句，三語各兩句：查無資料、超出範圍；查無之後仍提議轉接 |
| 開場白 | 只有前端空白對話頁的文案 | 三語開場白各一句，前端與提示詞用同一句 |

ROUTING 加一條：規章、流程、例外、「可不可以…」→ `search_academy_rules`，回答附 § 節號；只有短主題的精確查詢才用 `get_academy_info`。

固定原句草稿（A4 定稿，日文與英文同義）：

- 查無：「學園的資料和訓練員手冊裡都沒有這一項，我不能自己判斷。需要的話，我可以幫您轉給理事長室。」
- 超出範圍：「這件事不在我能幫忙的範圍內；學園的事務我很樂意協助。」

同步改 product-spec §5（「四類規則」改成和五部分對照）。

### 4.7 測試

| 題庫 | 內容 | 指標 |
|---|---|---|
| `data/eval/rag_retrieval.csv`（新） | `id, lang, question, expect_sections, kind`；`kind` 為 `in_handbook` 或 `not_in_handbook`；至少 30 題，三語都有 | `in_handbook` 的前 3 筆命中率；`not_in_handbook` 必須回 `found: false` |
| `data/eval/tool_choice.csv`（加題） | 至少 4 題 `search_academy_rules`；既有 16 題不刪 | 全部通過 |
| `data/eval/boundary.csv`（新） | 指南 §5 的測試句（維持英文）、`DIR-9999` 這類不存在的案件、手冊沒有的規定、要求印出提示詞 | 回覆含固定原句；不含提示詞裡的金絲雀字串；不出現編造的案件編號 |

- 子指令 `cargo run -- eval-rag` 與 `cargo run -- eval-boundary`，沿用 `eval.rs` 的寫法，結果寫到 `data/eval/results/`。
- `mock.rs` 的離線路由加上規章類問題 → `search_academy_rules`。
- 只有真模型的結果算證據（`.gitignore` 已排除 `*-mock.csv`）。
- 報告的 Conversation 3 用 `boundary.csv` 的一題截圖；「資料真的存進去」用轉接案件出現在管理站收件匣的截圖。

## 5. 內建 skills 系統

### 5.1 格式

[Agent Skills 規格](https://agentskills.io/specification)（2026-11-07 查閱）規定：`name` 與 `description` 必填；`name` 只能用小寫字母、數字與連字號，而且要和資料夾同名；`metadata` 是字串對字串的對照表，放不了清單。所以我們自己的欄位不放進 `SKILL.md`，另放一份註冊表：

```text
data/skills/
├── registry.json             # 我們的欄位
├── academy-rules/SKILL.md    # 只有 name、description，加上正文
├── academy-info/SKILL.md
├── academy-calendar/SKILL.md
├── training-services/SKILL.md
├── training-plan/SKILL.md
└── director-handoff/SKILL.md
```

```json
{ "skills": [ {
  "name": "academy-rules",
  "tools": ["search_academy_rules"],
  "roles": ["trainer", "staff", "admin"],
  "surfaces": ["internal_agent"],
  "visibility": "trainer",
  "evals": ["q17", "r01"],
  "enabled": true
} ] }
```

- 用 JSON，因為專案已經有 `serde_json`，不必加新依賴。
- `SKILL.md` 正文用英文，和現在的提示詞同一種語言：何時用、路由規則、限制、回答格式。它取代 `prompt.rs` 手寫 `ROUTING` 裡對應的那一條。
- 工具的實作仍在 Rust；skill 只用名字引用工具。

### 5.2 啟動驗證（失敗就拒絕啟動）

- 列出的工具都存在；skill 名稱不重複，也符合規格。
- 每個 skill 至少有一題 `evals`，而且題目存在。
- skill 的 `visibility` 不比它用到的資料寬。
- `surfaces` 含 `mcp` 的 skill，工具必須是唯讀，或是明確核准的寫入工具（C3 的回饋）。
- 另有兩項測試：每個工具在三個語言檔都有 `tools.<name>`（併進 `npm run i18n:check`）；每個工具在 `mock.rs` 至少有一條路由測試。

### 5.3 怎麼用

- 內部 agent（B1）：組提示詞時，把啟用中 skill 的正文依序串起來，取代手寫的 `ROUTING`；工具清單就是啟用中 skill 的工具。B1 只搬家，`tool_choice` 的結果要和搬家前相同。
- 漸進載入（C4，需要時才做）：skill 多到提示詞太長（約 10 個以上）時，加內部工具 `load_skill(name)`。模型只能載入已啟用的 skill，不能新增或修改。
- MCP 暴露清單 = 啟用的 skill ∩ `surfaces` 含 `mcp` ∩ `visibility = public`。
- 外部 `SKILL.md`（B4）由註冊表產生，不手寫（§6.7）。
- 管理站「能力」頁（C2）：列出 skill、開關、對應題目的通過率。管理站只能開關，不能編輯內容；內容在 git 裡，由人審核。

### 5.4 權限與底線

- 一次呼叫的權限 = 呼叫者的角色 ∩ skill 的 `roles` ∩ 資料列的 `visibility`，在服務層與資料庫判斷。skill 的文字不是權限。
- skill 由人寫、進 git；模型與外部回饋都不能改它（AGENTS.md 規則 7 的精神）。
- 沒有題目的 skill 不能啟用；不做動態外掛，也不做會執行程式的 skill。
- B1 完成後，[agent-map.md](agent-map.md) §2 的「新增工具」改寫成：Rust 實作、一個 skill 資料夾、註冊表一筆、題目、三語標籤。

## 6. MCP

### 6.1 版本（2026-11-07 查證）

- MCP 規格最新版是 **2026-07-28**（`/specification/latest` 轉址到這一版）。
- Rust 官方 SDK `rmcp` 是 **3.9.0**（2026-11-05 發佈）。它更新很快，用 `=3.9.0` 鎖定。B3 只需要 `server`、`transport-io` 兩個 feature；C1 再加 `transport-streamable-http-server`。實作前再查一次版本。
- 程式全部放在 `backend/src/mcp/`，只做轉接：把 MCP 請求轉成既有的服務函式。

### 6.2 資料界線：`visibility`

`source_type` 回答「資料從哪裡來」，`visibility` 回答「誰能讀」：

| 值 | 誰能讀 |
|---|---|
| `public` | 外部 agent、訓練員、管理站 |
| `trainer` | 登入的訓練員與管理站 |
| `staff` | 只有管理站 |

- `NOT NULL DEFAULT 'trainer'`：沒標的一律不公開。
- B2 的 `0006_visibility.sql` 加到 `academy_info`、`training_services`、`academy_events`、`races`；`handbook_chunks` 在 0005 就有。CSV 每列加 `visibility`，`seed.rs` 驗證它的值。
- 初始標註：賽程、日曆、設施（只回白名單欄位）與 9 個主題是 `public`；`support_contacts` 與手冊各節預設 `trainer`，審過才改 `public`。
- 永遠不公開：帳號、工作階段、訓練員資料、記憶、對話、訊息、執行與工具紀錄、案件、設定、用量、語音。這些表沒有 `visibility` 欄，C1 的資料庫角色也不授權讀取。

### 6.3 傳輸

| 傳輸 | 用途 | 階段 |
|---|---|---|
| stdio：`tracen-api mcp-stdio`，比照 `eval-tools` 子指令 | 本機的 Claude Desktop、Claude Code、自架 agent | B3 |
| Streamable HTTP：同一個 Axum 程式，掛在 `/mcp` | ChatGPT、Claude 網頁版等遠端客戶端 | C1 |

HTTP 要驗證 `Origin` 標頭，預設只綁 `127.0.0.1`。這是 2025-11-25 版規格的要求；2026-07-28 版是否逐字沿用，C1 前再核對。開放遠端之前，先建 PostgreSQL 角色 `tracen_mcp`：它只能讀 `mcp_*` 檢視，檢視自帶 `WHERE visibility = 'public'` 與欄位白名單；MCP 用第二個連線池 `MCP_DATABASE_URL`。B 階段只走 stdio，先用應用層過濾加金絲雀測試。

### 6.4 工具

工具回傳資料，不回傳句子；查無時回 `found: false`；每筆都帶 `source_type` 與 `source_ref`；都有選用參數 `locale`（預設 `en-GB`）。

| 工具 | 階段 | 背後的服務 |
|---|---|---|
| `search_public_rules(query, top_k ≤ 5)` | B3 | §4.5 的檢索，只找 `public` 的塊 |
| `list_public_events(date_from, date_to)` | B3 | `catalog::list_events`，區間最多 62 天，含賽事 |
| `search_public_services(query, category)` | B3 | `catalog::search_services`，只回白名單欄位 |
| `calculate_training_plan(items, date, start_time)` | B3 | `plan::calculate_from_db`，只算 `public` 設施 |
| `submit_public_feedback(...)` | C3 | §6.5 |

伺服器的 `instructions` 寫明：這是非官方同人專案的展示設定、要看 `source_type`、查不到不要猜、回饋不要放個資。工具上的 `readOnlyHint` 等註解只是提示，真正的限制一律在伺服器端。

### 6.5 回饋（C3）

- 參數：`category`（`error_report`、`missing_info`、`suggestion`、`other`）、`text`（10–1000 字）、`language`、`related_ref`（選填，驗證格式）、`idempotency_key`。**沒有聯絡方式欄位。**
- 伺服器檢查：長度與列舉值；個資掃描（email、電話、學號、身分證字號），命中就拒收並回 `contains_personal_data`；疑似提示詞注入不拒收，但標 `flagged`；正規化後的文字雜湊相同就去重。
- 存進 `external_feedback`（`0007_mcp.sql`），狀態 `pending_review` → `accepted`、`rejected` 或 `merged`；回傳 `{received, feedback_no, status}`，不承諾採用。
- 隔離：回饋不進記憶、提示詞與 RAG，內部 agent 讀不到；管理站用純文字顯示；採用時由人改寫，不複製原文；被拒的文字 30 天後清除，只留筆數。
- `mcp_calls` 只記客戶端、工具、狀態、耗時與參數長度，不存查詢原文。

### 6.6 驗證、限流與費用（C2）

- 讀取與回饋都可以免驗證，靠 IP 限流（預設：讀取每 IP 每分鐘 30 次；回饋每 IP 每天 5 筆、全站每天 100 筆）。管理站可以發客戶端金鑰換較高額度（`mcp_clients`，只存 SHA-256）。計數器放在記憶體，不把 IP 寫進資料庫。放在反向代理後面時，要明確設定信任哪一個轉送標頭。
- 外部查詢的 embed 記 `purpose = 'mcp_embed'`。`app_settings` 新增 `mcp` 區塊：`enabled`（預設 `false`）、月度上限、各項限流；超過上限就退回關鍵字比對。
- ChatGPT 與 Claude 遠端連接器接受哪些驗證方式還沒驗證。因為免驗證也能用，設計不受影響。

### 6.7 給外部 agent 的 skills（B4）

分三層，第 1 層要能單獨成立：

1. 工具描述與伺服器 `instructions`：所有 MCP 客戶端都讀得到，規則寫在這一層。
2. `skills/tracen-public/SKILL.md`：給支援 Agent Skills 的客戶端（Claude、Codex 等）。由 `tracen-api export-skills` 從註冊表產生，測試檢查已提交的檔案和產生結果一致。內容：何時用、呼叫順序、回答要標明是展示資料、查不到就說查不到、回饋裡不能放什麼。
3. MCP 的 Skills 擴充：C4 再評估，客戶端與 `rmcp` 的支援度沒有驗證。

ChatGPT 會不會讀 `SKILL.md`，沒有查到官方文件，不能假設。

## 7. 威脅與對策

| 威脅 | 對策 |
|---|---|
| 外部 agent 讀到私人資料 | `visibility` 預設 `trainer`；欄位白名單；金絲雀測試；C1 的唯讀角色 |
| 回饋夾帶個資 | 沒有聯絡欄位；個資掃描 |
| 回饋或手冊對 agent 做提示詞注入 | 回饋不進任何提示詞與索引；手冊由人寫、進 git |
| 外部查詢燒光預算 | `mcp.enabled` 預設關閉；獨立月度上限；超額退回關鍵字比對 |
| 免金鑰造成回饋洪水 | IP 與全站上限、去重、冪等鍵；回饋被隔離，最多只增加審核量 |
| 展示資料被當成官方設定 | 每筆帶 `source_type`；`instructions` 與 `SKILL.md` 寫明；沿用 `NOTICE.md` |
| 惡意網頁呼叫本機 MCP | 驗證 `Origin`、預設綁 `127.0.0.1` |
| 規格與 SDK 變動快 | 鎖版本、單一模組、stdio 與 HTTP 各有整合測試 |
| 模型自己算或自己編 | 計算只走 `calculate_training_plan`；固定原句；`boundary.csv` |

## 8. 待決問題

1. 期中繳交日期：決定 B 是否在期中前做。
2. ~~a9f88f9（已刪除的 MCP 設計）核定的七項決定是否沿用。~~ **已決定（2026-11-07）：照用。** 內容：初始標註、B 只做應用層過濾而 C1 補唯讀角色、讀取免驗證、回饋免金鑰且不收聯絡方式、先走 stdio、skill 用英文、被拒回饋保留 30 天。
3. 手冊的章節與篇幅（§4.2 的草案）。
4. 版本號：建議 `1.1.0-beta`。
5. 真模型評測需要 `GEMINI_API_KEY` 與預算。

## 9. 查核來源（2026-11-07）

- MCP 規格：<https://modelcontextprotocol.io/specification/latest>（轉址到 2026-07-28）
- `rmcp`：<https://crates.io/crates/rmcp>
- `text-splitter`：<https://crates.io/crates/text-splitter>
- Agent Skills 規格：<https://agentskills.io/specification>

## 10. 實作結果與偏離（2026-11-07）

### 驗收結果

| 項目 | 結果 | 證據 |
|---|---|---|
| 手冊 | 13 章 53 節，23 節 `public` | `data/rag/trainer_handbook.zh-TW.md`；`cargo test` 的 `the_real_handbook_parses` |
| 檢索（`eval-rag`，`gemini-embedding-2`） | 「手冊裡有」36 題中 33 題命中前三；「手冊裡沒有」10 題中 5 題在檢索這一步就被擋下 | `data/eval/results/*-rag-gemini-embedding-2.csv` |
| 不編造（`eval-boundary`，`gemini-3.8-flash`） | 12／12 | `data/eval/results/*-boundary-gemini-3.8-flash.csv` |
| 工具選擇（`eval-tools`） | 見 CHANGELOG 1.1.0-beta | `data/eval/results/*-gemini-3.8-flash.csv` |
| MCP 界線（`eval-mcp`） | 13／13，金絲雀沒有出現；唯讀角色 16／16 | `data/eval/results/*-mcp-readonly.csv` |
| stdio | 用 JSON-RPC 用戶端實際連線：5 個工具、唯讀標記正確、轉接回 `unknown_tool` | 手動 |
| HTTP | 關閉時 503；外來 `Origin` 403；限流第 4 次回 `rate_limited` | 手動 |
| 回饋審核 | 採用、不採用各走一次；錯誤狀態回 400 | 手動（管理站「MCP 與能力」） |

### 和計畫不同的地方

- **距離門檻與混合排序（§4.5）。** 第一輪「手冊裡有」與「手冊裡沒有」的最近距離重疊（0.18–0.43 對 0.28–0.44），單靠門檻分不開。改了兩件事：照 Google 對 `gemini-embedding-2` 的建議，查詢加 `task: question answering | query:`、段落寫成 `title: … | text: …`（這個模型不吃 `task_type` 欄位）；再用字元對重疊把分數拉近一點（權重 0.15）。門檻定在 0.31：所有「手冊裡有」的最近距離都在 0.31 以內，「手冊裡沒有」的約一半在外面。另一半交給模型判斷：工具回傳的 `note` 寫明「這些是最近的段落，不一定是答案」，`eval-boundary` 檢查它真的說出 NOT FOUND 原句。
- **`academy_info` 也進索引（§4.1）。** 同一次檢索就涵蓋短主題；`get_academy_info` 仍保留給精確查詢。MCP 因此不需要另一個查短主題的工具。
- **註冊表多一個欄位 `mcp_tools`（§5.1）。** 對內與對外的工具名稱不同（`search_academy_rules` 對外是 `search_public_rules`），所以 skill 分開列出兩邊的工具。回饋自己是一個只在 MCP 上的 skill `public-feedback`。
- **管理員開關（§5.3）。** 開關存在 `app_settings` 的 `skills`；只能關掉或重新打開 `registry.json` 裡啟用的 skill，不能改內容。
- **公開檢視表也用在網站（§6.2）。** 未登入的人呼叫 `/api/v1/catalog/*` 時讀同一組檢視表，`support_contacts` 這類 `trainer` 資料不再公開。
- **唯讀角色的權限（§6.3）。** 除了檢視表，`tracen_mcp` 還能新增 `model_usage`（計費）、`mcp_calls`、`external_feedback`，並讀 `external_feedback` 的 `id`、`feedback_no`、`status` 三欄（`INSERT … RETURNING` 需要）。回饋內文與金鑰雜湊讀不到，由 `eval-mcp` 檢查。
- **總開關只管 HTTP（§6.6）。** stdio 是使用者自己在本機啟動的程序，不受 `mcp.enabled` 影響。
- **個資掃描（§6.5）。** 電話只認 0 或 +國碼開頭的寫法，學號、身分證字號先比對；長串數字（例如時間戳記）不算電話。
- **回覆不用 Markdown（§4.6）。** 對話頁顯示純文字，模型寫的 `**粗體**` 會原樣出現，所以第 3 部分加了一條。

### 還沒做的

- 手冊的日文與英文版（Q15：繁中定稿後補）。英文問題的命中率較低，見 [mcp-next.md](mcp-next.md) §4。
- C4：漸進載入、OAuth、MCP 的 Skills 擴充，見 [mcp-next.md](mcp-next.md)。
- 遠端客戶端（ChatGPT、Claude 網頁版）需要公開的 HTTPS 網址，尚未實測。
