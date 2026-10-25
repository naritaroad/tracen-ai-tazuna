# 語音介面契約

日期：2026-10-25。依問卷 Q13（A＋使用者加註）：**麥克風輸入要做；AI 語音回應先在後端與資料庫預留介面。**

## 1. 語音輸入（要實作）

已實作（`backend/src/speech.rs`、`web/src/voice/useRecorder.ts`）。流程：前端 `MediaRecorder` 錄音 → `POST /api/v1/speech/transcribe` → 回傳文字 → 填入輸入框，訓練員確認後才送出。

| 項目 | 規格 |
|---|---|
| 請求 | `multipart/form-data`：`audio`（webm／ogg／mp4／wav，≤ 60 秒、≤ 10 MB），`locale`（選填，zh-TW／ja／en-GB） |
| 回應 | `{"text": "...", "locale": "zh-TW", "engine": "...", "duration_ms": 0}` |
| 權限 | 登入的訓練員 |
| 儲存 | 預設**不保存錄音**，只保存轉出的文字（以一般使用者訊息存檔） |
| 引擎 | 管理站「語音轉文字」模型，預設與對話模型相同（`gemini-3.8-flash`，支援音訊輸入），經 genai 以內嵌音訊呼叫；使用者已同意（[decisions.md](../product/decisions.md) 矛盾 4）。`mock` 回固定文字，只供測試 |
| 不採用 | 瀏覽器 Web Speech API：Android WebView 支援不穩定，不適合之後的 APK |
| 錯誤 | 太長、格式不支援、引擎未設定 → 400／503，前端提示改用打字 |

前端還要處理：麥克風權限被拒、錄音中顯示時間與停止鍵、角色狀態切到 `listening`。

## 2. AI 語音回應（只預留）

| 預留點 | 內容 | 狀態 |
|---|---|---|
| `GET /api/v1/speech/voices` | 可用聲音清單 | 回空清單 |
| `POST /api/v1/speech/synthesize` | `{message_id, voice}` → 音訊 URL | 回 501 |
| SSE `message.audio` | `{message_id, url, duration_ms, visemes?}` | 不送 |
| 資料表 `message_audio` | `message_id`、`voice`、`engine`、`storage_key`、`duration_ms`、`visemes jsonb`、`created_at` | 遷移時建立，空表 |
| `avatar.intent.viseme` | 口型，見 [avatar-contract.md](avatar-contract.md) | 保留欄位 |

角色聲音若要模仿作品配音，涉及聲優權利，實作前另行確認。
