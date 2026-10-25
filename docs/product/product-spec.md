# 產品規格（首版）

日期：2026-10-25。依據[施工決策](decisions.md)。只寫會改變實作或驗收的事。

## 1. 服務對象與情境

- **訓練員**：在特雷森學園指導擔當馬娘的人。想快速查校務規定、找可用的訓練設施、排訓練計畫並避開考試與活動、辦文件，遇到例外就找理事長室。
- **工作人員／管理員**：維護展示資料、在管理站以「秋川理事長・真人服務」身分處理案件。管理站不使用 AI。

現在沒有這個系統時，訓練員得自己翻公告、跑理事長秘書室問人。和一般聊天機器人的差別：答案來自學園資料庫並附來源類型，計算由程式完成，處理不了的事會真的建立案件給真人。

## 2. 首版功能

| 功能 | AI 對話 | 手動頁面（非 AI） | 共用服務 |
|---|---|---|---|
| 校務資訊 | `get_academy_info` | 「校務資訊」頁 | `catalog::info` |
| 校務日曆 | `list_academy_events` | 「校務日曆」頁 | `catalog::events` |
| 設施／資源查詢 | `search_training_services` | 「訓練設施」頁 | `catalog::services` |
| 訓練計畫計算 | `calculate_training_plan` | 「訓練計畫試算」頁 | `catalog::plan` |
| 理事長室案件 | `transfer_to_director` | 「理事長室」頁 | `handoff` |
| 個人偏好 | 寫入提示詞（唯讀） | 「我的資料」頁（從側邊欄底部的帳號選單進入）可檢視、修改、刪除，並上傳頭像 | `memory`、`picture` |

AI 與手動頁面呼叫同一個服務函式，權限檢查也在服務層。

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
- 計算：每項 `minutes = (minutes_per_session ?? unit_minutes) × sessions`、`fee = fee_jpy × sessions`；總分鐘、總費用；給了日期與開始時間時，依序排入並算出結束時間，檢查和校務日曆事件（全校或影響該設施）重疊、設施停用、超過 21:00 門禁前收操時間。
- 不完整就不給總數：任何一項比對不到或有多個候選，回傳 `total: null` 與 `problems`。
- 回傳：`{lines:[...], total_minutes, total_fee_jpy, schedule?:{start, end, blocks:[...]}, conflicts:[...], formula}`

### 3.5 `transfer_to_director` — 真人轉接

- 參數：`reason`（列舉：`not_in_data`／`exception_request`／`complaint`／`asked_for_director`／`other`）、`summary`（給理事長室看的摘要，≤ 500 字）
- 行為：真的建立 `director_tickets` 一筆（同一 run 重試不重複）；回傳 `{created, ticket_no, status:"waiting", note}`。同一回合不可再呼叫。

## 4. Agent 執行規則

- 每則訊息最多 6 次模型呼叫（`AGENT_MAX_STEPS` 是初始值，管理站可調 1–12）；單次模型呼叫逾時 90 秒；整個 run 逾時為「45 秒 × 呼叫次數」，至少 120 秒、至多 600 秒。
- 思考深度（自動／低／中／高）、溫度（空白＝模型預設）、每次呼叫的輸出上限（預設 8192 tokens，含思考）都在管理站「AI 設定與預算 › 對話風格與思考」調整，下一則訊息即生效。
- 只執行註冊表中的工具；未知工具回傳 `{error:"unknown_tool"}` 給模型並記錄為 `rejected`。
- 參數先驗證型別與範圍，不合格回傳 `{error:"invalid_arguments", detail}`。
- 每次工具呼叫寫入 `tool_calls`（名稱、參數、結果、耗時、狀態），對話頁「處理紀錄」可展開。
- 對話有進行中的理事長室案件（`in_progress`）時，不啟動 run；訊息直接送交真人。
- 送給模型的歷史：同一對話最近 20 則使用者／助理／理事長訊息（純文字）。

## 5. 系統提示詞的四類規則

系統提示詞先寫角色（駿川的身分、小事與說話方式，見 [worldbuilding.md](worldbuilding.md#3-角色口吻)），再寫四類規則：

1. **接地**：校務、設施、日曆、費用只能依工具結果回答；工具說查無就說查無並提議轉接。個人偏好只用來稱呼與調整語氣，不能當校務事實。閒聊中也一樣。
2. **範圍**：本業是特雷森學園訓練員的事務。題外話依管理站設定（2026-10-26 使用者要求開放閒聊，預設「適度」）：
   - 不聊：一句帶過並拉回校務（舊行為）。
   - 適度（預設）：可以聊訓練員的日常、飲食、興趣、賽事與馬娘、駿川自己的小事；連續約三輪題外話或話題告一段落時，用一句銜接的話自然拉回工作，不打斷、不說教。
   - 開放：訓練員想聊就陪著聊，只在有自然的切入點或聊了很久（約六輪）時輕輕提起工作。
   - 任何模式都婉拒寫文章、長詩、作業、程式、研究報告等大型請求；沒有即時新聞、天氣、價格就直說，不編造。不給醫療、法律意見。
3. **分派**：規定與辦法→`get_academy_info`；日期與活動→`list_academy_events`；設施與資源→`search_training_services`；時間、費用、衝突→`calculate_training_plan`；查無、例外、抱怨、要求理事長→`transfer_to_director`。
4. **語言與格式**：以程式指定的語言回答；長度依管理站設定（簡短一到三句／自然＝看問題決定、不設上限（預設）／詳細）；數值照工具結果；標示為 project 的資料要說明是本系統的示範設定。

## 6. 驗收

依 Q01 C，README 全部目標完成後才一起驗收，不設首版驗收點；以下是屆時的必要條件之一部分，施工中可先自測。Android APK 的實機驗收延到使用者以 ADB 接上自己的 Sony 手機。

- 四類問題各至少一題通過：資料內、需計算、資料外（轉接）、非業務（大型請求婉拒；閒聊不呼叫工具，且依設定適時拉回）。
- 工具選擇測試 `data/eval/tool_choice.csv` 至少 10 題，記錄分數；未通過的題目記錄修正內容後重測。
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
