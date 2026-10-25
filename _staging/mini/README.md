# Q 版駿川たづな（暫存區）

**狀態：暫存、尚未整合進專案。** 模型本體在 `assets/characters/9001_hayakawa_tazuna_mini/`（見該目錄 `docs/加工紀錄.md`）；這裡放由遊戲資料轉出的 Q 版動作與測試頁。

| 路徑 | 說明 |
|---|---|
| `vrma/*.vrma` | 15 段 VRMA（5 個 Q 版事件動作 × 開始／循環／結束） |
| `game_motions.json` | 動作清單（格式同 `_staging/game-motion/`） |
| `viewer/index.html` | 測試頁：動作切換＋49 個表情（含 36 組遊戲 Q 版預設表情）的按鈕；雙擊即可開啟 |
| `test/` | 表情驗證用的暫時檔案，可刪除 |

| id | 名稱 | 遊戲來源 |
|---|---|---|
| `idle` | 待機（雙手交握） | `anm_min_eve_chr9001_00_idle02` |
| `present` | 介紹手勢 | `idle01` |
| `kanban` | 指示看板 | `kanban01` |
| `hope` | 雙手合十 | `mot01` |
| `raise_hand` | 舉手 | `mot02` |

切換規則同正式版：目前循環播完 → 目前 `_e`（回到中立站姿）→ 目標 `_s` → 目標 `_loop`。測試頁自動驗收 4/4。

遊戲衍生資料不進版本控制（見 `.gitignore`）；權利說明見 `docs/character/character-assets.md`。
