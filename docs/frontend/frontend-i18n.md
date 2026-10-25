# 前端文案與多語言架構

日期：2026-10-25。適用 `web/`（訓練員站、管理站、Android App 共用同一份網站）。

## 1. 原則

- **字串外部化（String Externalization）**：元件裡不寫任何使用者看得到的文字，只寫鍵（`t("director.intro")`）。
- **內容與呈現分離（Separation of Content and Presentation）**：文字在語言檔或資料庫，版面與配色在 `styles.css` 的設計變數，元件只負責結構與行為。
- 三種語言（`zh-TW`、`ja`、`en-GB`）同步；繁中使用臺灣用語；`zh-TW` 是參考語言。

## 2. 文字放哪裡

| 文字種類 | 放在 | 理由 |
|---|---|---|
| 介面文案：按鈕、標題、說明、提示、錯誤訊息、狀態與類別名稱 | `web/src/i18n/locales/<語言>.json` | 跟程式碼一起版本控制與審查；建置前自動檢查；不用多一次 API 往返；Android App 離線也能顯示 |
| 校務內容：校務資訊、設施、日曆、角色名稱 | PostgreSQL，經 API 以 `locale` 參數取得（`/catalog/*`、`/avatar/characters/{id}`） | 管理站可直接修改、不用重新發布；每筆資料要標 `source_type`（見 [worldbuilding.md](../product/worldbuilding.md)） |
| 使用者產生的內容：對話、案件、記憶、帳號顯示名稱 | PostgreSQL | 屬於資料，不翻譯 |
| API 錯誤 | 後端只回穩定的 `error.code`（`not_found`、`bad_request`…），前端以 `apiError.<code>` 翻譯 | 後端的英文 `message` 只當補充細節，不直接顯示 |
| 列舉值（狀態、類別、角色、用途、工具名稱） | 後端送代碼，前端以 `<區段>.<代碼>` 翻譯，查無時退回代碼本身 | 新代碼不會讓畫面壞掉，檢查腳本會提醒補翻譯 |

不把介面文案放資料庫：介面文案跟元件一起改版，放資料庫會多一次請求、需要快取與後台編輯介面，對這個規模沒有好處。若日後要讓非工程人員改文案，再評估把 JSON 交給翻譯平台管理，不必改架構。

## 3. 檔案與載入

```
web/src/i18n/
  index.ts          i18next 設定、語言清單 LOCALES、按需載入
  errors.ts         errorMessage(e)：API 錯誤代碼 → 當前語言的訊息
  locales/
    zh-TW.json      參考語言（Messages 型別的來源）
    ja.json
    en-GB.json
web/src/format.ts   日期、數字、日圓、清單、時數（Intl＋語言檔）
web/scripts/check-i18n.mjs
```

- 每個語言檔各自打包成一個小檔（gzip 約 5 KB），只下載目前語言與後備語言（`zh-TW`）。
- `main.tsx` 等語言檔載入後才畫面，切換語言時 i18next 先載入再切換，不會閃出鍵名。
- `<html lang>` 與頁面標題跟著語言變；`styles.css` 依 `:lang(ja)`、`:lang(en)` 換字型順序，避免日文漢字用到臺灣字形。

## 4. 鍵的寫法

- 依頁面或功能分區：`chat.*`、`director.*`、`admin.*`；共用的放 `common.*`。
- 由代碼組成的鍵用代碼原樣：`ticketStatus.waiting`、`notice.run.failed.budget`、`apiError.not_found`。
- 句子要整句翻譯，用 `{{變數}}` 插值，不要在元件裡拼字串（語序各語言不同）：`t("chat.stepMeta", { step, duration })`。
- 數字、金額、清單用 `format.ts`（`formatNumber`、`formatYen`、`formatList`），不要自己加「¥」或「, 」。
- 圖示用 `components/Icon.tsx` 的 SVG，不用文字符號；按鈕的名稱由文字或 `aria-label` 提供。
- 語言名稱（繁體中文、日本語、English）是設定，寫在 `index.ts` 的 `LOCALES`，在每種介面語言都顯示原文。

## 5. 新增或修改文案

1. 在三個語言檔的同一位置加鍵（先寫 `zh-TW.json`）。
2. 元件用 `t("區段.鍵")`。
3. `cd web && npm run i18n:check`（`npm run build` 也會先跑）。檢查：
   - 三個語言檔的鍵完全一致、沒有空字串；
   - 每個翻譯的 `{{變數}}` 與參考語言相同；
   - 程式用到的鍵都存在；沒用到的鍵列為警告；
   - 元件裡沒有直接寫出的文字（JSX 文字、`placeholder`／`title`／`alt`／`aria-label`）。

新增語言：加 `locales/<代碼>.json`（從 `zh-TW.json` 複製後翻譯），在 `LOCALES` 加一行，`toLocale()` 補瀏覽器語言對應；後端 `locale` 參數與角色清單的 `displayName` 也要支援該語言。

## 6. 視覺設計（`web/src/styles.css`）

特雷森學園的現代日系風格：制服紫（江戸紫）配白（胡粉），藤色當輔助，蹄鐵金色點綴；留白多、細線分隔、標題字距稍寬（`palt`）。

| 變數 | 明亮 | 深色 | 用途 |
|---|---|---|---|
| `--brand` | `#5b3a9e` | `#b59cf2` | 主要按鈕、選取中導覽、使用者氣泡 |
| `--accent` | `#7b62b5` | `#c4b2f0` | 處理紀錄、管理站 |
| `--gold` | `#a17a26` | `#e0b75a` | 蹄鐵標誌、標題下的短線、等候中 |
| `--director` | `#9d3760` | `#e59ac0` | 理事長室（真人回覆） |
| `--bg` / `--surface` | `#f8f6fc` / `#ffffff` | `#14111d` / `#1b1727` | 頁面底色、卡片 |

元件只用變數，不寫色碼；調整配色只改 `:root` 與兩個深色區塊（系統深色、手動深色）。`index.html` 的 `theme-color` 與 `public/favicon.svg` 用同一組紫色。
