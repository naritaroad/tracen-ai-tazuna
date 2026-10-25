# Trance Academy idministrative information system - AI Hayakawa Tazuna

## 外部版本資訊查證

1. 當涉及下列項目的版本、下載、採用建議時，必須先以當下時間查證最新且穩定資訊，再提供建議：
   - 軟體、模型、套件、專案、業界標準、標準規範、架構、法律、設計、流程、電腦零組件、文獻、網址、版本、型號。
2. 避免直接沿用過時資訊；需優先提供符合當前情境且成熟穩定的版本建議。
3.不確定的專有名詞或概念要上網查圖查文確認清楚

## 你也可以增加些

給做專案的ai agent增加的
### 施工規則（2026-10-25 起）

1. 開工前先讀 `docs/engineering/agent-map.md`（路徑導航）、`docs/product/decisions.md`（決策）、`docs/product/product-spec.md`（規格）。
2. 直接在 `main` 上工作，不要建立 git 分支。
3. README 的「專案實作計畫」文字保留不改。
4. 角色原始素材（PMX、.blend、貼圖、渲染、對照影格、原片）只留本機，不進 git；只有 `.vrm`、`.vrma`、清單與腳本納入版本控制。
5. 不要頻繁編譯 Rust；完成一個工作單位再 `cargo test`，驗收時才建 release／映像。
6. 校務事實只能來自資料庫與工具結果；新資料要標 `source_type`，不把專案設定寫成官方設定（`docs/product/worldbuilding.md`）。
7. AI 角色動畫目前暫緩，只維護 `docs/character/avatar-contract.md` 的介面；不要讓模型直接控制動畫檔或骨架。（2026-10-25 使用者要求實作 3D 角色互動，暫緩解除；模型仍只能從固定詞彙選情緒與手勢，由伺服器驗證，見 `docs/character/avatar-contract.md`。）
8. 介面文字一律放 `web/src/i18n/`，三語同步；繁中使用臺灣用語。語言檔是 `web/src/i18n/locales/*.json`，元件不寫死文字，改完跑 `npm run i18n:check`（見 `docs/frontend/frontend-i18n.md`）。