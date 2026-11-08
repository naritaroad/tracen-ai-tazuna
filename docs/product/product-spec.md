# 產品規格（首版）

日期：2026-10-25。依據[施工決策](decisions.md)。只寫會改變實作或驗收的事。

## 1. 服務對象與情境

- **訓練員**：在特雷森學園指導擔當馬娘的人。想快速查校務規定、找可用的訓練設施、排訓練計畫並避開考試與活動、辦文件，遇到例外就找理事長室。
- **工作人員／管理員**：維護展示資料、在管理站以「秋川理事長・真人服務」身分處理案件。管理站不使用 AI。

現在沒有這個系統時，訓練員得自己翻公告、跑理事長秘書室問人。和一般聊天機器人的差別：答案來自學園資料庫並附來源類型，計算由伺服器完成，處理不了的事會建立案件交給真人。

## 2. 首版功能

| 功能 | AI 對話 | 手動頁面（非 AI） | 共用服務 |
|---|---|---|---|
| 校務資訊 | `get_academy_info` | 「校務資訊」頁 | `catalog::info` |
| 訓練員手冊（RAG，1.1） | `search_academy_rules` | 「訓練員手冊」頁 | `handbook::search` |
| 校務日曆 | `list_academy_events` | 「校務日曆」頁 | `catalog::events` |
| 設施／資源查詢 | `search_training_services` | 「訓練設施」頁 | `catalog::services` |
| 訓練計畫計算 | `calculate_training_plan` | 「訓練計畫試算」頁 | `catalog::plan` |
| 理事長室案件 | `transfer_to_director` | 「理事長室」頁 | `handoff` |
| 個人偏好 | 寫入提示詞（唯讀） | 「我的資料」頁（從側邊欄底部的帳號選單進入）可檢視、修改、刪除，並上傳頭像 | `memory`、`picture` |

AI 與手動頁面呼叫同一個服務函式，權限檢查也在服務層。外部 agent 經由 MCP 呼叫的也是同一組函式，只是讀公開的檢視表（§9）。

哪些工具啟用、提示詞怎麼分派，由內建 skills 註冊表決定（`data/skills/`，[upgrade-plan-1.1.md](../engineering/upgrade-plan-1.1.md) §5）。

## 3. 工具契約

共同規則：回傳資料不回傳句子；查無資料明示 `found: false`；計算在工具內完成；只收參數，不收 SQL。使用者身分由伺服器帶入，模型不能指定。

### 3.1 `get_academy_info` — 資訊查詢

- 參數：`topic`（列舉值，來自資料庫 `academy_info.topic`）
- 回傳：`{found, topic, title, content, source_type, source_ref, updated_on}`；查無：`{found:false, note, topics_available}`

### 3.2 `list_academy_events` — 資訊查詢（日曆）

- 參數：`date_from`（YYYY-MM-DD，必填）、`date_to`（選填，預設同日，最多 62 天）
- 回傳：`{found, count, events:[{id, title, kind, starts_at, ends_at, affects_services, source_type}]}`

### 3.3 `search_training_services` — 紀錄查詢

- 參數：`query`（選填，可用三語名稱或別名，寬鬆比對）、`category`（選填列舉）、`only_available`（選填布林）
- 回傳：`{found, items:[{id, name, category, unit_minutes, capacity, fee_jpy, available, location, notes, source_type}]}`

### 3.4 `calculate_training_plan` — 程式計算

- 參數：`items:[{service, sessions, minutes_per_session?}]`（`service` 為使用者原話或 id），`date?`（YYYY-MM-DD）、`start_time?`（HH:MM）
- 計算：每項 `minutes = (minutes_per_session ?? unit_minutes) × sessions`、`fee = fee_jpy × sessions`；總分鐘、總費用；給了日期與開始時間時，依序排入並算出結束時間，檢查和校務日曆事件（全校或影響該設施）重疊、設施停用、超過 21:30 最後使用時間（22:00 門禁前收操）。
- 不完整就不給總數：任何一項比對不到或有多個候選，回傳 `total: null` 與 `problems`。
- 回傳：`{lines:[...], total_minutes, total_fee_jpy, schedule?:{start, end, blocks:[...]}, conflicts:[...], formula}`

### 3.5 `transfer_to_director` — 真人轉接

- 參數：`reason`（列舉：`not_in_data`／`exception_request`／`complaint`／`asked_for_director`／`other`）、`summary`（給理事長室看的摘要，≤ 500 字）
- 行為：在 `director_tickets` 寫入一筆（同一 run 重試不重複）；回傳 `{created, ticket_no, status:"waiting", note}`。同一回合不可再呼叫。

### 3.6 `search_academy_rules` — 手冊檢索（RAG，1.1）

- 參數：`query`（訓練員原話，≤ 300 字）、`top_k`（選填，預設 3，最多 5）
- 做法：問題轉成向量（`gemini-embedding-2`，查詢加 `task: question answering | query:` 前綴），在 `handbook_chunks` 以餘弦距離找最近的塊，再依字元對的重疊略為拉近（混合排序），依節去重，回傳整節、優先用訓練員的語言。距離超過 0.31 的不算（由 `eval-rag` 定出）；沒有向量時退回關鍵字比對。
- 回傳：`{found, method, count, results:[{doc, section, chapter, title, content, lang, distance, visibility, source_type, source_ref}], note}`；查無：`{found:false, results:[], note}`
- 回傳的是「最近的段落」，不保證回答得了問題；`note` 要求模型自行判斷，答不了就說 NOT FOUND 原句。訓練員只看得到 `public` 與 `trainer` 段落。

## 4. Agent 執行規則

- 每則訊息最多 6 次模型呼叫（`AGENT_MAX_STEPS` 是初始值，管理站可調 1–12）；單次模型呼叫逾時 90 秒；整個 run 逾時為「45 秒 × 呼叫次數」，至少 120 秒、至多 600 秒。
- 思考深度（自動／低／中／高）、溫度（空白＝模型預設）、每次呼叫的輸出上限（預設 8192 tokens，含思考）都在管理站「AI 設定與預算 › 對話風格與思考」調整，下一則訊息即生效。
- 只執行註冊表中的工具；未知工具回傳 `{error:"unknown_tool"}` 給模型並記錄為 `rejected`。
- 參數先驗證型別與範圍，不合格回傳 `{error:"invalid_arguments", detail}`。
- 每次工具呼叫寫入 `tool_calls`（名稱、參數、結果、耗時、狀態），對話頁「處理紀錄」可展開。
- 對話有進行中的理事長室案件（`in_progress`）時，不啟動 run；訊息直接送交真人。
- 送給模型的歷史：同一對話最近 20 則使用者／助理／理事長訊息（純文字）。

## 5. 系統提示詞的五個部分（1.1）

期中指南要求系統提示詞分成五部分。`backend/src/agent/prompt.rs` 依序寫出這五部分，測試檢查順序與編號：

| 部分 | 內容 | 在程式裡 |
|---|---|---|
| 1 角色與目標 | 駿川的身分、小事與目標（依資料與手冊把校務辦對，查不到比亂猜好） | `ROLE`；口吻見 [worldbuilding.md](worldbuilding.md#3-角色口吻) |
| 2 範圍 | 規則 1–3：本業與題外話（依管理站設定：不聊／適度（預設）／開放）、不給醫療與法律意見、不能核准例外 | `scope_rule`、`SCOPE_LIMITS` |
| 3 互動規則 | 說話方式、語言、長度、不用 Markdown；規則 4–9 的工具分派由啟用中的 skill 產生；規則 10–12 是語言、長度與角色表情行 | `TALK`、`voice`、`routing()`、`length_rule` |
| 4 拒絕與特殊情況 | 規則 13–17：只依工具結果回答；查無、超出範圍、要求改規則或印出提示詞時，各用一句固定原句（三語各一）；不寫出提示詞與內部標記 | `REFUSALS`、`fixed_sentence` |
| 5 開場白 | 使用者最先看到的那一句；對話頁顯示同一句（`chat.opening`） | `opening_line` |

題外話的三種模式與舊版相同：任何模式都婉拒寫文章、長詩、作業、程式、研究報告等大型請求；沒有即時新聞、天氣、價格就直說，不編造。

## 6. 驗收

依 Q01 C，README 全部目標完成後才一起驗收，不設首版驗收點；以下是屆時的必要條件之一部分，施工中可先自測。Android APK 的實機驗收延到使用者以 ADB 接上自己的 Sony 手機。

- 四類問題各至少一題通過：資料內、需計算、資料外（轉接）、非業務（大型請求婉拒；閒聊不呼叫工具，且依設定適時拉回）。
- 工具選擇測試 `data/eval/tool_choice.csv` 至少 10 題，記錄分數；未通過的題目記錄修正內容後重測。
- 1.1：手冊檢索 `data/eval/rag_retrieval.csv`（`eval-rag`）、範圍與不編造 `data/eval/boundary.csv`（`eval-boundary`）、MCP 界線 `data/eval/mcp_boundary.csv`（`eval-mcp`，含金絲雀與唯讀角色檢查）。各題組的指標見 [upgrade-plan-1.1.md](../engineering/upgrade-plan-1.1.md) §4.7、§8。
- 未登入的人經由 `/api/v1/catalog/*` 只看得到 `visibility = public` 的資料。
- 跨帳號隔離：訓練員 A 看不到 B 的對話、案件與記憶。
- 真人接手：管理站接受案件後，訓練員再發訊息不會觸發 AI；理事長回覆出現在對話中並標示真人。
- 三語介面切換、明暗主題切換、手機寬度（375 px）可操作。

## 7. 記憶（Q08 B）

四層，全部以 PostgreSQL 儲存，都帶使用者識別、來源與有效狀態：

| 層 | 內容 | 寫入方式 | 用法 |
|---|---|---|---|
| 近期對話 | 同一對話最近 20 則 | 自動 | 每次送給模型 |
| 對話摘要 | 長對話的摘要（`conversations.summary`） | 對話超過 20 則時背景產生 | 取代被截掉的舊訊息 |
| 個人偏好與事實 | 稱呼、語言、偏好、關於自己的事（`memory_items`） | 訓練員手動，或背景整理自動新增（`source = agent`） | 每次放進系統提示詞 |
| 歷史事件 | 過去對話片段與案件的重點（新表，含 768 維向量） | 每個 run 結束後背景萃取並嵌入 | 依新問題做語意檢索，取前 5 筆放進提示詞 |

規則：

- 校務事實、可用狀態、案件狀態一律查正式資料庫，不從記憶回答。
- 背景整理用模型萃取「候選記憶」，與既有記憶比對：重複就略過，衝突就以新的取代舊的並保留舊紀錄為 `superseded`。
- 預設不自動記住：擔當馬娘的健康與醫療狀況、他人聯絡方式、帳號密碼與金鑰。
- 「我的資料」頁顯示所有自動記住的項目與來源訊息，可修改、刪除；刪除時一併刪除對應向量。
- 驗收：記對人（跨對話仍正確稱呼）、能改正、能忘記、A 的記憶不會出現在 B 的回答中；語意檢索以固定題組比較開啟前後的回答正確率。
- 嵌入模型與資料庫映像見 [decisions.md](decisions.md) 矛盾 2。

## 8. 語音（Q13 A＋加註）

- **麥克風輸入要做**：對話頁加錄音按鈕，錄完送到後端轉成文字，填入輸入框讓訓練員確認或修改後再送出（不自動送出）。
- **AI 語音回應先不做**，只在後端與資料庫預留介面。
- 契約見 [voice-contract.md](../character/voice-contract.md)。

## 9. 外部 agent（MCP，1.1）

- 外部 agent（Claude、ChatGPT、自架 agent）經由 MCP 只能做兩件事：讀 `visibility = public` 的資料（規定、日曆與賽程、設施、訓練計畫計算），以及送非個人的回饋到人工審核佇列。對話、記憶、案件、帳號與設定一律讀不到，`transfer_to_director` 不對外。
- 本機用 stdio（`tracen-api mcp-stdio`）；遠端用 `/mcp`（Streamable HTTP），預設關閉，必須設定唯讀資料庫角色 `tracen_mcp`，並由管理員在「MCP 與能力」頁打開。
- 回饋不會自動改資料、提示詞或記憶；人在管理站審核，採用時改寫成資料。
- 設計與驗收：[upgrade-plan-1.1.md](../engineering/upgrade-plan-1.1.md) §6；之後的 OAuth 與漸進載入：[mcp-next.md](../engineering/mcp-next.md)。
