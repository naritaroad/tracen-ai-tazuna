# Notice

**English** | [繁體中文](#繁體中文) | [日本語](#日本語)

## Unofficial doujin project

This is an unofficial doujin project (a fan work). It is not affiliated with, endorsed by, or sponsored by Cygames, Inc.

*Uma Musume Pretty Derby*, Tracen Academy, Hayakawa Tazuna, Director Akikawa and all related names, characters, artwork, 3D models, motion data and logos are trademarks or copyrighted works of Cygames, Inc. Race names belong to their respective owners.

## What the MIT License covers

The [MIT License](LICENSE) applies only to the original source code, configuration, scripts and documentation written for this project.

It does **not** apply to, and grants no rights in:

| Material | Location | Rule |
|---|---|---|
| Character models and animation clips | `assets/characters/*/vrm/`, `assets/characters/*/animation/vrma/` | Derived from Cygames works. VRM metadata sets `allowRedistribution: false` and `commercialUsage: personalNonProfit`. |
| Game-derived motions and scenes | `_staging/` (local only, not in git) | Local research use only. Never served by a public deployment. The README screenshots show the corridor scene and game motions as they appear locally; the files themselves are not distributed. |
| Official academy crest | `assets/brand/tracen-crest*.svg`, and the README banners in `docs/brand/banners/` | Redrawn from a photo of official merchandise; a Cygames emblem. Included in the Docker image as the site mark and favicon; without the file the site uses the project's own horseshoe mark. |
| Setting facts in the demo data | `data/seed/*.csv` rows marked `official`, `anime` or `community` | Facts about the work belong to their sources. Every row records its `source_type`; information topics and races also record a `source_ref`. |

Apart from the crest, the public deployment profile runs without any of the above: the site falls back to a 2D status card when no character files are configured. See [docs/character/character-assets.md](docs/character/character-assets.md) for the full rights and version-control rules.

## Third-party software

Third-party libraries are used under their own licenses, recorded in `backend/Cargo.lock`, `web/package-lock.json` and [tools/motion-reference/THIRD_PARTY_NOTICES.md](tools/motion-reference/THIRD_PARTY_NOTICES.md).

---

## 繁體中文

### 同人專案作品

本專案為同人專案作品，與 Cygames, Inc. 沒有任何關係，也未獲其認可或贊助。

《賽馬娘 Pretty Derby》、特雷森學園、駿川手綱、秋川理事長及所有相關名稱、角色、美術、3D 模型、動作資料與標誌，均為 Cygames, Inc. 的商標或著作。賽事名稱屬於各自的權利人。

### MIT 授權的範圍

[MIT 授權](LICENSE)只涵蓋本專案自行撰寫的原始碼、設定檔、腳本與文件。

下列素材**不在**授權範圍內，也不授予任何權利：

| 素材 | 位置 | 規則 |
|---|---|---|
| 角色模型與動畫片段 | `assets/characters/*/vrm/`、`assets/characters/*/animation/vrma/` | 衍生自 Cygames 著作。VRM 中繼資料設定為 `allowRedistribution: false`、`commercialUsage: personalNonProfit`。 |
| 遊戲衍生動作與場景 | `_staging/`（只在本機，不進 git） | 僅限本機研究，公開部署不得提供。README 截圖呈現本機畫面中的走廊場景與遊戲動作，但不散布這些檔案。 |
| 官方校徽 | `assets/brand/tracen-crest*.svg`，以及 `docs/brand/banners/` 的 README 橫幅 | 依官方商品照片重新描繪，屬 Cygames 的標誌。隨 Docker 映像提供，作為網站標誌與 favicon；沒有這個檔案時，網站改用本專案自製的蹄鐵標誌。 |
| 展示資料中的作品設定 | `data/seed/*.csv` 中標為 `official`、`anime`、`community` 的資料列 | 作品設定屬於原出處。每列都記錄 `source_type`，校務資訊與賽程另記錄 `source_ref`。 |

除了校徽，公開部署設定不含上述任何素材；沒有設定角色檔時，網站改顯示 2D 狀態卡。完整的權利與版本控制規則見 [docs/character/character-assets.md](docs/character/character-assets.md)。

### 第三方軟體

第三方函式庫依各自授權使用，紀錄於 `backend/Cargo.lock`、`web/package-lock.json` 與 [tools/motion-reference/THIRD_PARTY_NOTICES.md](tools/motion-reference/THIRD_PARTY_NOTICES.md)。

---

## 日本語

### 同人プロジェクト作品

本プロジェクトは非公式の同人プロジェクト作品であり、Cygames, Inc. とは一切関係がなく、同社による承認や後援も受けていません。

『ウマ娘 プリティーダービー』、トレセン学園、駿川たづな、秋川理事長、および関連するすべての名称、キャラクター、アートワーク、3D モデル、モーションデータ、ロゴは Cygames, Inc. の商標または著作物です。レース名はそれぞれの権利者に帰属します。

### MIT ライセンスの適用範囲

[MIT ライセンス](LICENSE)が適用されるのは、本プロジェクトのために書かれた独自のソースコード、設定ファイル、スクリプト、文書のみです。

以下の素材にはライセンスが適用されず、いかなる権利も許諾しません。

| 素材 | 場所 | ルール |
|---|---|---|
| キャラクターモデルとアニメーションクリップ | `assets/characters/*/vrm/`、`assets/characters/*/animation/vrma/` | Cygames の著作物から派生したもの。VRM のメタデータは `allowRedistribution: false`、`commercialUsage: personalNonProfit` に設定されています。 |
| ゲーム由来のモーションとシーン | `_staging/`（ローカルのみ、git 管理外） | ローカルでの研究用途に限ります。公開環境では提供しません。README のスクリーンショットにはローカル環境の廊下シーンとゲームモーションが写っていますが、ファイル自体は配布しません。 |
| 公式校章 | `assets/brand/tracen-crest*.svg`、および `docs/brand/banners/` の README バナー | 公式グッズの写真から描き起こした Cygames の校章です。サイトのロゴと favicon として Docker イメージに含めます。ファイルがない場合は、本プロジェクト独自の蹄鉄マークを使用します。 |
| デモデータ中の作品設定 | `data/seed/*.csv` のうち `official`、`anime`、`community` の行 | 作品設定は各出典に帰属します。すべての行に `source_type` を記録し、校務案内とレースには `source_ref` も記録しています。 |

校章を除き、公開用の構成には上記の素材を一切含みません。キャラクターファイルが設定されていない場合、サイトは 2D の状態カードを表示します。権利とバージョン管理の詳しいルールは [docs/character/character-assets.md](docs/character/character-assets.md) をご覧ください。

### サードパーティソフトウェア

サードパーティのライブラリはそれぞれのライセンスに従って使用しており、`backend/Cargo.lock`、`web/package-lock.json`、[tools/motion-reference/THIRD_PARTY_NOTICES.md](tools/motion-reference/THIRD_PARTY_NOTICES.md) に記録しています。
