# 施工決策紀錄

建立：2026-10-25（臺灣時間）。對應[決策問卷](../reviews/2026-10-25-decisions-questionnaire.md)。

使用者於 2026-10-25 13:01 存檔填寫結果：在 Q01、Q04、Q08 的選項標 O，Q13 加註；**沒有標記的題目採用【建議】選項**。另一項使用者指示：**AI 驅動角色動畫的部分先暫緩，但各端點的介面要保留**（見 [avatar-contract.md](../character/avatar-contract.md)）。同日稍後使用者要求「建立專案網站用的 3D 角色互動模塊並接到系統上，使角色能應對話有自然的動作與表情」，暫緩解除。依 Q01 的選擇，「暫緩」是施工順序，角色動畫仍在整體驗收範圍內。

| 題號 | 決定 | 施工上的具體做法 | 對目前程式的影響 |
|---|---|---|---|
| Q01 驗收範圍 | **C**（使用者選） | README 全部目標完成後才一起驗收，期間不設「首版」驗收點。使用者加註：Android app 完成後不驗收，等使用者用自己的 Sony 手機以 ADB 連電腦再實測 | 目前完成的是第一個可運作的切片，不是可交付版本 |
| Q02 資料庫 | A（預設） | 正式系統只用 PostgreSQL。業務查詢集中在 `catalog` 模組。Android 端若有離線需求才加 SQLite 快取 | 無 |
| Q03 首批服務 | A（預設） | 校務資訊、校務日曆、訓練設施／資源查詢、訓練計畫時間與衝突計算、理事長室案件 | 已實作 |
| Q04 世界觀 | **A 為主（OO），並採 C（O）** | 以日服遊戲與官方資料為基準，也允許動畫與社群整理作為來源；衝突時依[來源優先順序](worldbuilding.md#1-來源層級與優先順序)取高者；本專案設定照舊明確標示 | `source_type` 需新增 `anime`、`community`（新遷移） |
| Q05 前端 | A（預設） | Vite＋React＋TypeScript，建置為靜態檔，由 Rust API 同站提供 | 已實作 |
| Q06 AI 權限 | A（預設） | 查詢、計算、準備申請摘要；預約／申請寫入由使用者確認後才提交；人工轉接可直接建立案件 | 寫入工具待做 |
| Q07 真人服務 | A（預設） | 理事長室案件＋管理站收件匣；接手後暫停該對話的 AI 回覆 | 已實作 |
| Q08 記憶 | **B**（使用者選） | 近期對話＋摘要＋可編輯偏好，**再加上歷史事件語意檢索與自動記憶整理**，接受嵌入模型成本。見[記憶設計](product-spec.md#7-記憶q08-b) | 需 pgvector、嵌入模型、背景整理工作 |
| Q09 帳號 | A（預設） | 測試帳號（訓練員／工作人員／管理員）；公開註冊之後再做 | 已實作 |
| Q10 模型 | A（預設）；**先接 Gemini** | rust-genai 包在 `Ai` 服務後；預設對話模型 `gemini-3.8-flash`（官方模型頁標為 Stable、2026-09 更新、支援函式呼叫與音訊輸入）；之後用同一題庫比較 Claude。模型、價格與每月預算都在管理站「AI 設定與預算」調整，存在 `app_settings`，不需重啟 | 已實作設定與預算；待填金鑰實測 |
| Q11 3D 畫質 | A（預設） | 桌機與一般 Android 實機分高／標準畫質；桌機 60 FPS、手機 30 FPS 為待實測目標；保留角色收合 | 已實作高／標準畫質、收合；手動改用簡易顯示的按鈕已於 2026-10-25 依使用者要求移除（載入失敗仍自動回退 2D）；實機 FPS 待測 |
| Q12 角色動作 | A（預設） | 先完成待機、處理中、說明、確認、轉接／錯誤等互動狀態，各有進入、保持、退出與打斷規則 | 已實作（[avatar-contract.md](../character/avatar-contract.md)） |
| Q13 語音 | **A，加註** | 先文字＋3D 狀態與表情；**但要有麥克風輸入功能**；AI 語音回應先在後端與資料庫預留介面。見[語音契約](../character/voice-contract.md) | 需語音轉文字端點與前端錄音；語音回應只留介面 |
| Q14 Android | A（預設） | 手機網頁先行，之後以 Capacitor 共用前端交付 APK；實機驗收依 Q01 加註延到使用者接上 Sony 手機 | 未開始 |
| Q15 三語與主題 | A（預設） | 一開始抽離字串；繁中先驗收，同一功能定稿後補齊日文與英文 | 已實作三語 |
| Q16 素材公開 | A（預設） | 程式碼與 CSV 可公開；角色素材以本機路徑載入（`AVATAR_ASSET_DIR`），公開版可無角色運行 | 已符合 |

## 矛盾與需要補的資料

2026-10-25 使用者回覆：先完成專案。語音辨識照建議。先接 Gemini API。每月預算不用先定，做成可在管理站調整。Sony 手機已接上電腦（型號 XQ-EC72，Android 14／API 34，1080×2340）。嵌入模型隨「先接 Gemini」採用 `gemini-embedding-2`。


1. **Q04 A＋C 的衝突處理**：已訂優先順序（官方／日服遊戲 > 官方動畫資料 > 社群整理 > 本專案設定），社群資料不能覆蓋官方設定。若使用者希望動畫設定優先於遊戲，需另行說明。
2. **Q08 B 的嵌入模型**：Anthropic 不提供嵌入 API，所以即使對話用 Claude，嵌入也需另一個供應商。提案用 Gemini 的 `gemini-embedding-2`（官方模型頁標為 Stable、2026 年 4 月更新、可輸出 128–3072 維；本案取 768 維），經 genai 呼叫，需要 `GEMINI_API_KEY`。資料庫改用含 pgvector 的映像 `pgvector/pgvector:0.8.7-pg18-trixie`（pgvector 0.8.7，Docker Hub 2026-10-01 更新）。**使用者若不想送對話內容到 Google 嵌入，可改本機嵌入模型，需另選。**
3. **Q08 B「自動記憶整理」的範圍**：問卷的「不希望自動記住的資料」空白。預設不自動記住：擔當馬娘的健康與醫療狀況、他人的聯絡方式、帳號密碼與金鑰。訓練員可以在「我的資料」看到、修改、刪除所有自動記住的項目。
4. **Q13 麥克風輸入的語音辨識引擎**：瀏覽器內建語音辨識（Web Speech API）在 Android WebView 支援不穩定，不適合之後的 APK。提案改為「前端錄音→後端 `/api/v1/speech/transcribe`」，後端可換引擎；第一個引擎用支援音訊輸入的 Gemini 模型（genai 可傳音訊）。**需確認可接受把語音送到 Google。**
5. **Q10 先接哪一家、每月預算**：空白。依預設先跑通一家；若 Q08、Q13 都用 Gemini，建議先接 Gemini 以減少金鑰與帳單數量，再用同一題庫比 Claude。
6. **Q11 測試裝置**：已知 Sony 手機，型號未提供；桌機與瀏覽器未提供。

## 本次另行決定的工程細節

- 後端為單一 Rust 模組化單體（`backend/`），Axum 0.8＋SQLx 0.9（PostgreSQL）＋genai 0.7。
- PostgreSQL 18 的 `uuidv7()` 作為主鍵預設值。
- 前後端同站：靜態檔在 `/`，API 在 `/api/v1`，避免跨站設定。登入用 HttpOnly、SameSite=Lax 的工作階段 cookie；改變狀態的請求只收 `application/json`。
- 對話回覆以 SSE 傳送事件（[事件契約](../api/stream-events.md)）。首版模型呼叫不串流，事件格式已預留逐段文字。
- 回覆語言由程式判斷，不交給模型：含假名→日文；含漢字、不含假名→繁體中文；其他→英文。
- 展示資料重建模式由 `CATALOG_SEED_MODE` 控制：`reset`（預設，每次啟動由 CSV 重建展示資料）或 `if_empty`（保留管理站修改）。
- Git：直接在 `main` 上工作，不建立分支（使用者指示）。
- 2026-10-26 使用者要求：對話要更像在和駿川說話、開放題外話但太多時適時拉回、不要太限制字數與思考程度，並在管理站加控制項。做法：提示詞改為「角色＋說話方式＋四類規則」；新增 `AiSettings.conversation`（回覆長度、題外話、思考深度、溫度、輸出上限、每則訊息模型呼叫次數），預設為自然長度、適度閒聊、思考「自動」、溫度用模型預設、輸出上限 8192、6 次呼叫。原本固定的溫度 0.2 與輸出上限 1200 不再用於對話：Gemini 3 官方建議溫度保持 1.0，且輸出上限包含思考用的 tokens，1200 可能讓回覆在思考途中被截斷。記憶整理與語音轉文字仍用原本的固定值。

## 查核來源（2026-10-25）

- Gemini 模型列表：https://ai.google.dev/gemini-api/docs/models ；Gemini 3.8 Flash：https://ai.google.dev/gemini-api/docs/models/gemini-3.8-flash ；音訊輸入格式（含 WebM）：https://ai.google.dev/gemini-api/docs/audio
- `gemini-3.5-transcribe` 是專用轉寫模型，但官方頁未標穩定狀態、未寫明呼叫方式，暫不採用；之後可在管理站把「語音轉文字」模型改成它，前提是 genai 能以一般對話介面呼叫。
- Capacitor 8 環境需求：https://capacitorjs.com/docs/getting-started/environment-setup （實測需 JDK 21、Android SDK API 36）

- Gemini 思考設定（2026-10-26 查閱）：https://ai.google.dev/gemini-api/docs/thinking （`gemini-3.8-flash` 預設 medium，可選 low／medium／high，不支援 minimal；`max_output_tokens` 含思考 tokens）；Gemini 3 開發指南：https://ai.google.dev/gemini-api/docs/gemini-3 （溫度建議保持 1.0）
- Gemini Embedding 2 模型頁：https://ai.google.dev/gemini-api/docs/models/gemini-embedding-2
- pgvector 映像標籤：https://hub.docker.com/r/pgvector/pgvector/tags ；版本：https://github.com/pgvector/pgvector/tags
