# 遊戲原始動作（暫存區）— 駿川たづな

**狀態：暫存，檔案不進版本控制；網站可在本機載入。** 在 `.env` 設 `AVATAR_GAME_MOTION_DIR=_staging/game-motion`（另需 `AVATAR_ASSET_DIR`），API 會把這裡的動作當成 `game` 動作組提供，網站優先使用（見 `docs/character/avatar-contract.md` 第 4 節）。公開部署不可設定。既有的 `assets/characters/9001_hayakawa_tazuna/animation/`（影片擬合版）與 `tools/vrm-test-viewer/` 都沒有被修改。

## 內容

| 路徑 | 說明 |
|---|---|
| `vrma/*.vrma` | 36 段 VRMA（12 個動作 × 開始 `_s`／循環 `_loop`／結束 `_e`） |
| `game_motions.json` | 動作清單：`motions`（依動作分組，含來源資產名、進出姿勢）與 `clips`（相容既有測試頁的平面清單） |
| `viewer/index.html` | 獨立測試頁，直接雙擊即可開啟（共用 `tools/vrm-test-viewer/dist/vrm-data.js` 的模型） |
| `viewer/game-viewer.js`、`viewer/game-pack.js` | 測試頁程式與內嵌動作包（由 `viewer/src/` 建置） |

## 動作一覽

| id | 名稱 | 來源 | 建議用途 |
|---|---|---|---|
| `idle` | 待機（雙手交握） | たづな專屬 `chr9001_00_idle02` | 預設待機、聆聽 |
| `present` | 介紹手勢 | たづな專屬 `idle01` | 介紹功能 |
| `hope` | 雙手合十 | たづな專屬 `mot01` | 請求、期待 |
| `raise_hand` | 舉手 | たづな專屬 `mot02` | 「是的！」 |
| `bow` | 鞠躬 | 共用 `type00_bow05` | 問候、致謝、致歉 |
| `wave` | 揮手 | 共用 `type00_hello03` | 打招呼、道別 |
| `hand_chest` | 手放胸前 | 共用 `type00_muneate06` | 說明、安撫 |
| `think` | 思考 | 共用 `type00_think03` | 查詢中 |
| `guide` | 指引方向 | 共用 `type00_point04` | 引導到頁面某處 |
| `explain` | 豎指說明 | 共用 `type00_advice06` | 補充、提醒重點 |
| `happy` | 開心 | 共用 `type00_happy01` | 好消息 |
| `clap` | 拍手 | 共用 `type00_clap01` | 完成、恭喜 |

## 切換規則（遊戲原生設計）

- 每段 `_s` 的第一格與 `_e` 的最後一格是同一個「中立站姿」，所以：
  **目前循環播完 → 目前 `_e`（回到中立）→ 目標 `_s` → 目標 `_loop`**，接點姿勢一致，不需要補製轉換。
- 例外：`hope_s` 從 `idle` 循環姿勢開始（`enter_from: "idle"`），要先經過 `idle_s`。
- 遊戲中有兩組中立站姿，最大差 5.4°，以 0.2 秒交叉淡化吸收。
- 實測：循環首尾接縫 ≤ 0.11°；測試頁自動驗收 4/4 通過。

## 已知限制

- 只轉了身體骨架。表情、耳朵、嘴型（`_face`、`_ear`、`lipsync`）還沒轉換；測試頁的眨眼是自動產生的。
- VRMA 只含 VRM 標準人形骨骼，遊戲骨架的扭轉輔助骨（`ArmRoll`、`ShoulderRoll`）與 IK 目標不會帶過去；手腕大幅自轉時前臂可能有些許扭曲。
- 共用 `type00` 動作是給標準體型設計的，遊戲執行時會用 IK 修正手的接觸；目前未套用 IK；已目視檢查的雙手合十、手放胸前、思考未見明顯穿模，其餘接觸動作（如拍手）尚待逐一確認。

## 日後整合進專案時

1. 把需要的 `vrma/*.vrma` 與清單搬到 `assets/characters/9001_hayakawa_tazuna/animation/`（或新的 `animation_game/`）。
2. 網站播放器採用上面的切換規則（測試頁 `viewer/src/game-motion.js` 的 `planTo`／`onLoop`／`onFinished` 可直接參考）。
3. 若要在網站上公開，先確認權利：這些動作是遊戲（Cygames, Inc.）的原始資料。

## 重新產生

讀取與轉換工具在 `C:\Users\admin\dev_open\umaviewer\umaviewer\tools\uma_motion\`（含解密邏輯，刻意不放進本專案）；
Blender 端為 `assets/characters/9001_hayakawa_tazuna/animation/work/tz_game.py`（`export_list`）。
