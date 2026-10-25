# 架構決策

查核日期：2026-10-25（台北時間）。

## Docker

範圍：**網站伺服器進 Compose，客戶端與創作工具留在本機。** 主機目前的可用記憶體不參與這個決定。

- 開發內圈：Compose 只起 PostgreSQL。Rust API 在本機編譯、直接連這個資料庫，避免每次改碼都在容器裡重編。
- 驗收、交接與部署：同一個 Compose 再起 API 映像。映像用多階段建置，執行階段只留編譯好的二進位與網站靜態檔。PostgreSQL 與此映像一起起來，就是可重複的網站後端。
- 留在本機：瀏覽器裡的 3D／VRM 畫面、Android app、動作參考前處理。

動作參考管線維持不使用容器，見 `tools/motion-reference/docs/architecture.md`。原因是它要直接讀寫本機影片與檢閱檔，與主機記憶體無關。

### 理由

- 網站的伺服器依賴是 PostgreSQL 加上一個長駐 Rust API。帳號、訓練員記憶、客服對話與管理站都靠這組服務。把它們放進 Compose，換一台機器或部署時可以用同一組版本與資料卷跑起來。官方映像 `postgres:18.6` 已在 Docker Hub 提供（amd64，推送於 2026-10-21）。PostgreSQL 18 發佈於 2025-09-25，18.6 的官方釋出日是 2026-08-13，支援至 2030-11-14。
- 日常改 API 仍在本機編譯。專案位於 Windows 路徑，原始碼綁進 Linux 容器會拖慢編譯；專案也要求不要為了驗證而頻繁編譯 Rust。正式映像只在要驗收整條後端、或要部署時建置。
- 3D 角色要在一般電腦與手機的瀏覽器裡畫，Android 是裝置上的 app。容器畫不了使用者螢幕上的 WebGL，也取代不了手機上的 app。
- Docker Desktop 對個人、教育與非商業開源用途免費。

### 記憶體

容器記憶體依服務自己的工作集設定，不依這台電腦當下還剩多少記憶體來縮小。PostgreSQL 使用官方映像的預設組態，不預先壓到數百 MiB。API 與資料庫第一次以接近真實的帳號、對話與記憶資料跑起來之後，用 `docker stats --no-stream` 看尖峰，再把各服務上限設成觀測值的約 1.5 倍。映像檔大小（postgres amd64 壓縮層約 162 MB）不是執行時記憶體。

### 什麼情況要改這個決定

- 日常改 API 也要和部署環境完全同一套 Linux 工具鏈：把原始碼放進 WSL 檔案系統後，再把 API 的開發循環放進容器。
- 動作參考要變成共用的處理服務：回到該管線的架構文件重估。
- 量測顯示某個服務的工作集遠高於正式環境機器：調整該服務上限或正式環境機器，不把 3D 客戶端或 Android 放進容器。

### 查核來源

- https://docs.docker.com/desktop/setup/install/windows-install/
- https://learn.microsoft.com/en-us/windows/release-health/release-information
- https://endoflife.date/postgresql
- https://www.postgresql.org/docs/release/18.6/
- https://hub.docker.com/_/postgres

## 第三方方案評估（2026-10-25，待決策）

以下是 README 計畫評估的候選，**尚未核定或安裝**；不改變上面的 Docker 決策。完整理由見 [計畫評估](../reviews/2026-10-25-plan-review.md)，範圍選擇見 [決策問卷](../reviews/2026-10-25-decisions-questionnaire.md)。

| 能力 | 候選與採用方式 | 提案／主要成本 |
|---|---|---|
| Rust HTTP | [Axum](https://github.com/tokio-rs/axum)／[Actix Web](https://github.com/actix/actix-web) 作依賴 | 偏向 Axum 的組合方式；兩者皆可行，尚未做本案效能比較 |
| SQL 儲存 | [SQLx](https://github.com/transact-rs/sqlx) 作依賴 | 支援 SQLite／PostgreSQL；仍須分開處理 SQL、遷移及交易差異 |
| 模型介面 | [rust-genai](https://github.com/jeremychone/rust-genai) 作依賴，包在薄介面後 | 遵循 README 方向；先驗證鎖定版本的 Claude／Gemini 能力，Agent 執行規則另訂 |
| 角色顯示 | [three-vrm](https://github.com/pixiv/three-vrm)＋[R3F](https://github.com/pmndrs/react-three-fiber) 作依賴 | 沿用資產，網站整合播放器；接觸動作及手機效能仍須實測 |
| 記憶 | [Mem0](https://github.com/mem0ai/mem0)／[LangMem](https://github.com/langchain-ai/langmem) 先參考流程；[pgvector](https://github.com/pgvector/pgvector) 留作檢索候選 | 初期結構化記憶先用產品資料庫；增加其他服務前先證明改善，不預設全導入 |
| Android | [Capacitor](https://github.com/ionic-team/capacitor)／[Tauri](https://v2.tauri.app/start/) 作候選依賴 | 偏向手機網頁驗收後以 Capacitor 共用前端；不因伺服器使用 Rust 就自動選 Tauri |

已查核公開文件、維護狀態及主要授權；這不是完整依賴安全稽核或已相容的鎖版清單。定案時再依實際版本檢查依賴與工具鏈。


## 已確認更新：主庫與真人服務（2026-10-25）

- 正式產品以 PostgreSQL 為主；不再把「先 SQLite、再 PostgreSQL」當成已接受的順序。Android SQLite 僅依離線閱讀與草稿需求選用，正式預約、權限與案件仍由 Rust／PostgreSQL 裁決。
- 使用者已確認由真人管理員以秋川理事長身分接手；不是另一個 AI。即時聊天／留單的首版範圍未決；記錄實際操作者，接手後暫停該案件的 AI 回覆。
- 訓練員功能依作品與學校／JRA 實務提出；資料標示官方、現實參考與專案自訂，避免把參考流程當成作品既有規章。

詳見 [訓練員服務與轉接補充方案](../reviews/2026-10-25-trainer-services-and-handoff.md)。

| 新評估能力 | 候選 | 採用狀態與成本 |
|---|---|---|
| 共用前端的 Android SQLite | [Capacitor 社群外掛](https://github.com/capacitor-community/sqlite) | MIT；僅在確有離線需求時做版本相容驗證，尚未安裝 |
| 原生 Android 持久化 | [Android Room](https://developer.android.com/training/data-storage/room) | 適合原生資料層；共用 React 前端需另有橋接，不當成可直接替換的 JS 套件 |

## 施工採用（2026-10-25）

上方「第三方方案評估」的待決策項目，依[施工決策](../product/decisions.md)暫定如下，版本查證見[開發與執行](development.md)：

| 能力 | 採用 | 備註 |
|---|---|---|
| Rust HTTP | Axum 0.8.9 | 模組化單體，`backend/` |
| SQL 儲存 | SQLx 0.9.0（PostgreSQL） | 執行期查詢＋`sqlx::migrate!`；主鍵用 PostgreSQL 18 的 `uuidv7()` |
| 模型介面 | genai 0.7.1，包在 `ModelGateway` 後 | 另有離線 `mock` 閘道供開發與測試 |
| 前端 | Vite 8＋React 19＋TypeScript 7，React Router 8，i18next | 靜態檔由 Rust API 同站提供 |
| 角色顯示 | three 0.186.1＋@pixiv/three-vrm 3.5.5＋three-vrm-animation 3.5.5 | 單一畫布的命令式播放器 `web/src/avatar/player/`，按需載入；不用 R3F（一個畫布、每幀不經 React，Android WebView 同一份程式）。見 [avatar-contract.md](../character/avatar-contract.md) |
| 記憶 | PostgreSQL＋pgvector 0.8.7 | Q08 B：偏好、事件語意回想、自動整理、對話摘要；Mem0／LangMem 未導入 |
| Android | Capacitor 8.5.3（`web/android/`） | App 從 API 伺服器載入同一個網站；USB 測試用 `adb reverse`。建置需 JDK 21、Android SDK API 36 |

資料庫映像改為 `pgvector/pgvector:0.8.7-pg18-trixie`（PostgreSQL 18.6＋pgvector 0.8.7，資料目錄與官方映像相同，原資料卷可直接沿用）。

驗收映像為 `Dockerfile` 三階段建置（Node 建網站、Rust 建 release、Debian slim 執行），`compose.yaml` 的 `app` profile 啟動。映像另外帶有種子資料、角色清單檔與官方校徽 `assets/brand/tracen-crest.svg`；角色模型與動作檔不在映像裡。
