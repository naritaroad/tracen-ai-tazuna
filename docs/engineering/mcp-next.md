# MCP 與 skills 的下一步（C4 設計）

狀態：**設計，尚未實作**。日期：2026-11-07（台北時間）。接在 [upgrade-plan-1.1.md](upgrade-plan-1.1.md) 之後；1.1 已完成的部分（stdio、Streamable HTTP、唯讀角色、限流、回饋、註冊表）不在這裡重寫。

這份處理四件 1.1 刻意留下的事：漸進載入 skill、遠端 MCP 的 OAuth、MCP 的 Skills 擴充，以及跨語言檢索。每一項都寫「什麼時候才做」，條件沒到就不做。

## 1. 漸進載入 skill（`load_skill`）

**現況。** 啟用中 skill 的 `## Routing` 段落全部串進系統提示詞（規則 4–9）。七個 skill 時大約 1.5 KB，不必省。

**什麼時候做。** 符合任一條：啟用的 skill 超過約 10 個；或 skill 正文（`## Limits` 與範例）需要放進提示詞才答得好，而全部放進去會讓提示詞超過約 8 KB。

**做法。**

- 提示詞只放每個 skill 一行：名稱與 `description`（前置資料原文）。
- 新增內部工具 `load_skill(name)`：回傳該 skill 的正文。只能載入啟用中、且呼叫者的角色在 `roles` 裡的 skill；不能新增或修改。
- 一個 run 內同一個 skill 只載入一次（伺服器端記錄，第二次回 `already_loaded`）。
- `load_skill` 本身屬於一個新 skill `skill-loader`，照 1.1 的規則要有題目：`tool_choice.csv` 加「先載入再呼叫」的題，`boundary.csv` 加「請載入一個不存在或已關閉的 skill」。
- 驗收：`tool_choice` 與 `boundary` 分數不低於改之前；提示詞長度記錄在報告。

**不做。** 讓模型自己寫或改 skill；從網路或外部檔案載入 skill。

## 2. 遠端 MCP 的 OAuth

**現況。** 讀取與回饋都可以免驗證，靠 IP 限流；管理站可以發客戶端金鑰換較高額度（1.1 決定 D9）。資料全部是公開的，所以這樣夠用。

**什麼時候做。** 符合任一條：要開放非公開資料（例如訓練員自己的對話、案件），或遠端客戶端只接受 OAuth、不接受無驗證或靜態金鑰。

**依據（2026-11-07 查閱 MCP 規格 2026-07-28 的 Authorization 一節）。**

- 授權對 MCP 是**選用**的；HTTP 傳輸若要做，應照這一節，stdio 不適用。
- 用 OAuth 2.1（草案 13）、Bearer Token（RFC 6750）、授權伺服器中繼資料（RFC 8414）、動態註冊（RFC 7591）、受保護資源中繼資料（RFC 9728）。
- 客戶端必須用 Resource Indicators（RFC 8707）在授權與換 token 時帶 `resource`；伺服器必須驗證 token 是發給自己的（audience），否則回 401。

**做法。**

- 本專案當**資源伺服器**，不自己當授權伺服器：授權交給既有的 IdP，伺服器只驗 token。
- 提供 `/.well-known/oauth-protected-resource`（RFC 9728），列出授權伺服器與支援的 scope：`public:read`、`feedback:write`，以及之後才開的 `trainer:self`。
- token 驗證：簽章、到期、`aud` 等於本伺服器的 canonical URI；token 不轉傳給任何下游服務。
- `trainer:self` 要對應到 `users` 的一個帳號；這時 MCP 的資料庫連線不能再只用 `tracen_mcp`，要另開一個「只讀該訓練員自己資料」的路徑（列層級安全或依使用者過濾的檢視表），設計時另寫一份。
- `rmcp` 3.9.0 有 `auth` feature（OAuth 用戶端與相關型別）；實作前再查證伺服器端可用的部分與版本。
- 驗收：沒有 token、token 過期、`aud` 不符各回 401；`public:read` 的 token 叫不動 `submit_public_feedback`；金絲雀測試照舊通過。

**不做。** 在 MCP 上開放 `transfer_to_director`；替使用者保存 IdP 的 refresh token。

## 3. MCP 的 Skills 擴充

**現況。** 給外部 agent 的說明分三層（1.1 §6.7）：工具描述與 `instructions`、`skills/tracen-public/SKILL.md`、MCP 擴充（未做）。

**查證。** `rmcp` 3.9.0 的 `ServerHandler` 已有 SEP-2640 的 `skills/list`、`skills/get` 與 `resources/directory/read`，預設回「未實作」（`-32601`）。哪些客戶端支援這個擴充，尚未查到可靠資料。

**什麼時候做。** 至少一個實際使用的客戶端支援，或課程／使用者要求。

**做法。**

- `skills/list` 只回 `tracen-public` 一個 skill；內容就是 `export-skills` 產生的同一份文字，不另外維護。
- 版本跟著 `SKILL.md` 的雜湊值走，客戶端快取可以失效。
- 驗收：`tools/list` 與 `skills/get` 描述的工具一致（新增測試）；`eval-mcp` 照舊通過。

## 4. 跨語言檢索與手冊翻譯

**現況（2026-11-07 的 `eval-rag`）。** 手冊只有繁中。36 題「手冊裡有」的題目中，33 題的前三筆有正確章節；沒中的 3 題裡有 2 題是英文問題（日文 3 題都命中），因為只能跨語言比對繁中段落。

**做法。**

- 依 Q15，繁中定稿後補 `trainer_handbook.ja.md` 與 `trainer_handbook.en.md`，節號與繁中相同（解析時就檢查）。翻譯用 human-writing 的日文、英文流程，並請 writing-reviewer 對照繁中盲審。
- 翻完重跑 `eval-rag`，比較前後的命中率與距離分布，再決定要不要調門檻（現在是 0.31，只對 `gemini-embedding-2` 有效）。
- 換 embedding 模型時，`embed_model` 不同的舊向量會在下次啟動全部重算；先在 `eval-rag` 比較，再改設定。

## 5. 查核來源（2026-11-07）

- MCP 規格 2026-07-28 Authorization：<https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization>
- `rmcp` 3.9.0 原始碼（`src/handler/server.rs` 的 SEP-2640 方法、`Cargo.toml` 的 `auth` feature）：<https://crates.io/crates/rmcp/3.9.0>
- Agent Skills 規格：<https://agentskills.io/specification>
