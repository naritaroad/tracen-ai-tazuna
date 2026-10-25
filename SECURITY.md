# Security Policy

**English** | [繁體中文](#繁體中文) | [日本語](#日本語)

## Supported versions

| Version | Supported |
|---|---|
| 1.0.0-beta | Yes |

Security fixes go to the `main` branch.

## Reporting a vulnerability

Please do not open a public issue for a security problem. Report it privately through GitHub: open the repository's **Security** tab and choose **Report a vulnerability**.

Include:

- what is affected (endpoint, page, tool or file) and the commit you tested;
- steps to reproduce, with a minimal request or conversation if relevant;
- the impact you observed.

You will get an acknowledgement within 7 days. Once a fix is ready, the report is credited in [CHANGELOG.md](CHANGELOG.md) unless you prefer otherwise.

## Scope

In scope:

- authentication, sessions and role checks (`trainer`, `staff`, `admin`);
- one trainer reading another trainer's conversations, cases or memories;
- prompt injection that makes the agent call a tool outside its rules, act as another user, or write data without the user's confirmation;
- leaking API keys, password hashes or session tokens;
- SQL injection or path traversal in the asset and picture endpoints.

Out of scope:

- the demo accounts and passwords in `.env.example`, which are for local development only and must be disabled (`SEED_DEMO_ACCOUNTS=false`) on any shared deployment;
- denial of service through model cost; use the monthly budget on the management site;
- findings that need a modified client or a compromised host.

---

## 繁體中文

### 支援的版本

| 版本 | 支援 |
|---|---|
| 1.0.0-beta | 是 |

安全性修正會進入 `main` 分支。

### 回報漏洞

請不要以公開 Issue 回報安全性問題。請透過 GitHub 私下回報：在儲存庫的 **Security** 分頁選擇 **Report a vulnerability**。

請附上：

- 受影響的範圍（端點、頁面、工具或檔案）與測試時的 commit；
- 重現步驟，必要時附上最小的請求或對話內容；
- 觀察到的影響。

我們會在 7 天內回覆確認。修正完成後，除非你另有意願，會在 [CHANGELOG.md](CHANGELOG.md) 致謝。

### 範圍

在範圍內：

- 登入、工作階段與角色權限檢查（`trainer`、`staff`、`admin`）；
- 訓練員讀取到其他訓練員的對話、案件或記憶；
- 透過提示詞注入讓 Agent 違規呼叫工具、冒用其他使用者，或未經使用者確認就寫入資料；
- 洩漏 API 金鑰、密碼雜湊或工作階段 token；
- 素材與頭像端點的 SQL 注入或路徑穿越。

不在範圍內：

- `.env.example` 中的測試帳號與密碼：僅供本機開發，任何共用部署都必須關閉（`SEED_DEMO_ACCOUNTS=false`）；
- 以模型費用造成的阻斷服務：請使用管理站的每月預算；
- 需要修改用戶端或主機已遭入侵才能成立的問題。

---

## 日本語

### サポート対象のバージョン

| バージョン | サポート |
|---|---|
| 1.0.0-beta | あり |

セキュリティ修正は `main` ブランチに反映します。

### 脆弱性の報告

セキュリティ上の問題は公開 Issue で報告しないでください。GitHub から非公開で報告してください：リポジトリの **Security** タブで **Report a vulnerability** を選びます。

次の情報を含めてください。

- 影響を受ける対象（エンドポイント、ページ、ツール、ファイル）と、確認したコミット
- 再現手順（必要に応じて最小限のリクエストや会話内容）
- 確認できた影響

7 日以内に受領のご連絡をします。修正が完了したら、ご希望がない限り [CHANGELOG.md](CHANGELOG.md) で謝辞を記載します。

### 対象範囲

対象：

- ログイン、セッション、ロールによる権限チェック（`trainer`、`staff`、`admin`）
- あるトレーナーが別のトレーナーの会話、案件、記憶を閲覧できてしまう問題
- プロンプトインジェクションにより、エージェントがルール外のツール呼び出し、他の利用者へのなりすまし、利用者の確認なしでのデータ書き込みを行ってしまう問題
- API キー、パスワードハッシュ、セッショントークンの漏えい
- 素材・プロフィール画像のエンドポイントにおける SQL インジェクションやパストラバーサル

対象外：

- `.env.example` にあるテスト用アカウントとパスワード（ローカル開発専用です。共有する環境では必ず `SEED_DEMO_ACCOUNTS=false` にしてください）
- モデルの利用料金によるサービス妨害（管理サイトの月間予算をご利用ください）
- クライアントの改変やホストの侵害を前提とする問題
