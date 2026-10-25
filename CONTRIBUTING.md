# Contributing

**English** | [繁體中文](#繁體中文) | [日本語](#日本語)

Thank you for helping. This guide covers how to set up, the rules the code base depends on, and what to check before you open a pull request.

## Before you start

- For anything larger than a small fix, open an issue first so the approach can be agreed before you write code.
- Read [docs/engineering/agent-map.md](docs/engineering/agent-map.md) (where things live), [docs/product/decisions.md](docs/product/decisions.md) and [docs/product/product-spec.md](docs/product/product-spec.md). The project documents are in Traditional Chinese.
- Set up the project as described in [Getting started](README.md#getting-started). `MODEL=mock` runs the whole flow without an API key.

## Rules the code base depends on

### Back end

- **Migrations are append-only.** Never edit an applied file in `backend/migrations/`; add `000N_<name>.sql`.
- **Static SQL only.** SQLx 0.9 takes `&'static str`. A query assembled with `format!` may only join constants and must be wrapped in `sqlx::AssertSqlSafe`. User and model input always goes through bind parameters.
- **Adding an agent tool** touches `TOOL_NAMES`, `specs` and `dispatch` in `backend/src/tools.rs`. Put the business logic in its module so the manual page calls the same function, add questions to `data/eval/tool_choice.csv`, and add `tools.<name>` to all three language files.
- **The model chooses, the server decides.** The model never sends SQL, JavaScript, animation files or bone data. Character emotions and gestures come from the fixed vocabulary in [docs/character/avatar-contract.md](docs/character/avatar-contract.md) and are validated on the server.
- Build Rust when a unit of work is complete, then run `cargo test`; avoid rebuilding after every small edit.

### Front end

- **No hard-coded interface text.** Strings live in `web/src/i18n/locales/{zh-TW,ja,en-GB}.json`, added to all three files in the same change. `zh-TW.json` is the reference language and uses Taiwan wording. See [docs/frontend/frontend-i18n.md](docs/frontend/frontend-i18n.md).
- Dates, numbers, currency and lists go through `web/src/format.ts` (`Intl`), not string concatenation.

### Demo data and setting

- Every row in `data/seed/*.csv` carries a `source_type`: `official`, `anime`, `community`, `reference` or `project`.
- Facts about the academy come only from the database and tool results. Do not present `project` or `reference` data as an official setting; the source priority and wording rules are in [docs/product/worldbuilding.md](docs/product/worldbuilding.md).

### What stays out of the repository

- `.env` and any API key.
- Character source material: PMX, `.blend`, textures, renders, comparison frames, source videos.
- Game-derived motions and scenes (`_staging/`).
- Only `.vrm`, `.vrma`, manifests, notes and scripts are versioned under `assets/characters/`. See [NOTICE.md](NOTICE.md).

## Before you open a pull request

```bash
cd backend && cargo test
```

```bash
cd web && npm test && npm run build
```

`npm run build` also runs `npm run i18n:check` and the type check.

- Describe user-visible changes under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md) ([Keep a Changelog](https://keepachangelog.com/en/2.0.0/) format).
- Write commit subjects as a short imperative sentence that says what changed, for example `Add the race schedule to the academy calendar`.
- If the change affects the API, the SSE events or the character contract, update the matching document in `docs/` in the same pull request.

---

## 繁體中文

感謝參與。本文說明環境設定、程式碼依賴的規則，以及送出 Pull Request 前要檢查的事項。

### 開始之前

- 小修正以外的變更，請先開 Issue 討論做法再動手。
- 先讀 [docs/engineering/agent-map.md](docs/engineering/agent-map.md)（路徑導航）、[docs/product/decisions.md](docs/product/decisions.md) 與 [docs/product/product-spec.md](docs/product/product-spec.md)。
- 依 [快速開始](docs/readme/README.zh-TW.md#快速開始) 建立環境。`MODEL=mock` 不需 API 金鑰即可跑完整個流程。

### 程式碼依賴的規則

**後端**

- **遷移只新增不修改。** 已套用的 `backend/migrations/` 檔案不可改，請新增 `000N_<名稱>.sql`。
- **只用靜態 SQL。** SQLx 0.9 只收 `&'static str`；以 `format!` 組成的查詢只能拼接常數，並包在 `sqlx::AssertSqlSafe` 中。使用者與模型的輸入一律走參數綁定。
- **新增 Agent 工具** 要同時改 `backend/src/tools.rs` 的 `TOOL_NAMES`、`specs`、`dispatch`；業務邏輯放在對應模組，讓手動頁面呼叫同一個函式；再補 `data/eval/tool_choice.csv` 題目，以及三個語言檔的 `tools.<name>`。
- **模型負責選擇，伺服器負責決定。** 模型不會送出 SQL、JavaScript、動畫檔或骨架資料；角色的情緒與手勢只能從 [docs/character/avatar-contract.md](docs/character/avatar-contract.md) 的固定詞彙選擇，並由伺服器驗證。
- 完成一個工作單位再編譯 Rust 並執行 `cargo test`，不要每改一行就重新編譯。

**前端**

- **元件不寫死介面文字。** 文字放在 `web/src/i18n/locales/{zh-TW,ja,en-GB}.json`，同一次變更補齊三語。`zh-TW.json` 是參考語言，使用臺灣用語。見 [docs/frontend/frontend-i18n.md](docs/frontend/frontend-i18n.md)。
- 日期、數字、幣值與清單一律經由 `web/src/format.ts`（`Intl`）格式化，不用字串拼接。

**展示資料與世界觀**

- `data/seed/*.csv` 每一列都要有 `source_type`：`official`、`anime`、`community`、`reference` 或 `project`。
- 校務事實只能來自資料庫與工具結果；`project`、`reference` 資料不可寫成官方設定。來源優先順序與用語規則見 [docs/product/worldbuilding.md](docs/product/worldbuilding.md)。

**不能放進儲存庫的檔案**

- `.env` 與任何 API 金鑰。
- 角色原始素材：PMX、`.blend`、貼圖、渲染圖、對照影格、原始影片。
- 遊戲衍生的動作與場景（`_staging/`）。
- `assets/characters/` 底下只版本控制 `.vrm`、`.vrma`、清單、說明與腳本。詳見 [NOTICE.md](NOTICE.md)。

### 送出 Pull Request 之前

```bash
cd backend && cargo test
```

```bash
cd web && npm test && npm run build
```

`npm run build` 也會執行 `npm run i18n:check` 與型別檢查。

- 使用者看得到的變更，寫進 [CHANGELOG.md](CHANGELOG.md) 的 `## [Unreleased]`（[Keep a Changelog](https://keepachangelog.com/en/2.0.0/) 格式）。
- Commit 標題用一句簡短的英文祈使句說明改了什麼，例如 `Add the race schedule to the academy calendar`。
- 變更影響 API、SSE 事件或角色契約時，請在同一個 Pull Request 更新 `docs/` 對應文件。

---

## 日本語

ご協力ありがとうございます。このガイドでは、環境の準備、コードベースが前提としているルール、Pull Request を出す前に確認することを説明します。

### はじめる前に

- 小さな修正を超える変更は、コードを書く前に Issue を作成して進め方を相談してください。
- [docs/engineering/agent-map.md](docs/engineering/agent-map.md)（パスの案内）、[docs/product/decisions.md](docs/product/decisions.md)、[docs/product/product-spec.md](docs/product/product-spec.md) をお読みください。プロジェクトの文書は繁体字中国語で書かれています。
- [はじめに](docs/readme/README.ja.md#はじめに) の手順で環境を準備します。`MODEL=mock` なら API キーなしで一連の流れを動かせます。

### コードベースが前提とするルール

**バックエンド**

- **マイグレーションは追加のみ。** 適用済みの `backend/migrations/` のファイルは変更せず、`000N_<名前>.sql` を追加します。
- **SQL は静的なものだけ。** SQLx 0.9 は `&'static str` しか受け付けません。`format!` で組み立てるクエリは定数の連結に限り、`sqlx::AssertSqlSafe` で包みます。利用者やモデルからの入力は必ずバインドパラメーターで渡します。
- **エージェントツールの追加** では、`backend/src/tools.rs` の `TOOL_NAMES`、`specs`、`dispatch` を同時に変更します。業務ロジックは対応するモジュールに置き、手動ページが同じ関数を呼べるようにします。さらに `data/eval/tool_choice.csv` に問題を追加し、3 つの言語ファイルに `tools.<name>` を追加します。
- **選ぶのはモデル、決めるのはサーバー。** モデルが SQL、JavaScript、アニメーションファイル、ボーンデータを送ることはありません。キャラクターの感情とジェスチャーは [docs/character/avatar-contract.md](docs/character/avatar-contract.md) の固定語彙から選ばれ、サーバーで検証されます。
- Rust のビルドは作業の区切りごとに行い、そのあとで `cargo test` を実行します。小さな変更のたびにビルドし直すことは避けてください。

**フロントエンド**

- **UI テキストを直書きしない。** 文言は `web/src/i18n/locales/{zh-TW,ja,en-GB}.json` に置き、同じ変更で 3 言語すべてに追加します。基準言語は `zh-TW.json`（台湾の用語）です。[docs/frontend/frontend-i18n.md](docs/frontend/frontend-i18n.md) を参照してください。
- 日付、数値、通貨、リストは文字列の連結ではなく、`web/src/format.ts`（`Intl`）で整形します。

**デモデータと作品設定**

- `data/seed/*.csv` のすべての行に `source_type`（`official`、`anime`、`community`、`reference`、`project` のいずれか）を付けます。
- 学園に関する事実はデータベースとツールの結果からのみ取得します。`project` や `reference` のデータを公式設定として扱わないでください。出典の優先順位と用語のルールは [docs/product/worldbuilding.md](docs/product/worldbuilding.md) にあります。

**リポジトリに含めないもの**

- `.env` と、あらゆる API キー。
- キャラクターの元素材：PMX、`.blend`、テクスチャー、レンダリング画像、比較用フレーム、元動画。
- ゲーム由来のモーションとシーン（`_staging/`）。
- `assets/characters/` でバージョン管理するのは `.vrm`、`.vrma`、マニフェスト、メモ、スクリプトだけです。[NOTICE.md](NOTICE.md) を参照してください。

### Pull Request を出す前に

```bash
cd backend && cargo test
```

```bash
cd web && npm test && npm run build
```

`npm run build` は `npm run i18n:check` と型チェックも実行します。

- 利用者から見える変更は [CHANGELOG.md](CHANGELOG.md) の `## [Unreleased]` に記載します（[Keep a Changelog](https://keepachangelog.com/en/2.0.0/) 形式）。
- コミットの件名は、何を変えたかを短い英語の命令文で書きます。例：`Add the race schedule to the academy calendar`。
- API、SSE イベント、キャラクターの契約に影響する変更では、同じ Pull Request で `docs/` の該当文書も更新してください。
