# HTTP API（v1）

前綴 `/api/v1`。JSON；登入後以 HttpOnly cookie `tracen_session` 識別。錯誤格式：`{"error":{"code":"...","message":"..."}}`。改變狀態的請求只收 `application/json`，cookie 為 SameSite=Lax。

| 方法與路徑 | 權限 | 用途 |
|---|---|---|
| `GET /health` | 公開 | 健康檢查 |
| `POST /auth/login`、`POST /auth/logout`、`GET /auth/me` | — | 登入、登出、目前使用者 |
| `GET /catalog/info?locale=`、`GET /catalog/info/{topic}` | 公開 | 校務資訊 |
| `GET /catalog/services?query=&category=&only_available=&locale=` | 公開 | 設施與資源 |
| `GET /catalog/events?from=&to=&locale=` | 公開 | 校務日曆（最多 92 天） |
| `POST /catalog/plan` | 公開 | 訓練計畫計算（與工具相同） |
| `GET/POST /conversations` | 訓練員 | 我的對話列表／新對話 |
| `GET /conversations/{id}` | 本人 | 訊息、run、工具軌跡、進行中的案件 |
| `PATCH /conversations/{id}` | 本人 | `{"pinned": true｜false}` 釘選或取消；列表釘選的排最前面 |
| `POST /conversations/{id}/messages` | 本人 | 送出訊息，回應為 SSE（[事件](stream-events.md)） |
| `GET/POST /director/tickets`、`GET /director/tickets/{id}` | 本人 | 理事長室案件（手動表單需 `request_id`） |
| `GET/PUT /me/profile`、`GET/POST /me/memory`、`PATCH/DELETE /me/memory/{id}` | 本人 | 稱呼、回覆語言、記憶 |
| `GET/PUT/DELETE /me/avatar` | 本人 | 頭像：`PUT` 以 multipart 的 `image` 欄位上傳（PNG／JPEG／WebP，≤ 512 KB，訓練員在瀏覽器手動裁切後縮成 256 px 正方形，見 `web/src/components/PictureCropper.tsx`）；`GET /auth/me` 的 `avatar_version` 是快取鍵，`preferred_name`（「我的資料」的稱呼）顯示在帳號按鈕 |
| `GET /admin/director/tickets`、`GET .../{id}` | staff | 案件收件匣 |
| `POST /admin/director/tickets/{id}/accept｜reply｜resolve｜return` | staff | 接手、回覆、結案、交回駿川 |
| `GET /admin/catalog/services`、`PATCH /admin/catalog/services/{id}` | staff | 設施可用狀態、名額、展示費用 |
| `GET /admin/catalog/info`、`PUT /admin/catalog/info/{topic}` | staff | 校務資訊內容 |
| `GET /admin/runs` | staff | AI 執行紀錄 |
| `GET /admin/users` | admin | 帳號 |
| `GET /avatar/states`、`GET /avatar/characters`、`GET /avatar/characters/{id}` | 公開 | 角色介面（[契約](../character/avatar-contract.md)） |
| `POST /avatar/sessions/{id}/events` | — | 保留，回 501 |
| `GET /avatar-assets/{id}/vrm/*.vrm`、`.../animation/vrma/*.vrma` | 公開 | 只在設定 `AVATAR_ASSET_DIR` 時存在 |
| `GET /brand/crest.svg` | 公開 | 官方校徽（不在 `/api/v1` 下）；只在 `BRAND_CREST_FILE`（預設 `assets/brand/tracen-crest.svg`）存在時回傳，否則 404，網站改用蹄鐵標誌 |
