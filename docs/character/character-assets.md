# 角色資產的取得與運用

日期：2026-10-25。對象：駿川たづな（`9001_hayakawa_tazuna`）與 Q 版（`9001_hayakawa_tazuna_mini`）。本文件說明模型、動作、表情等角色資產**從哪裡來、怎麼加工、放在哪裡、怎麼用、哪些不能公開**。各項的詳細加工紀錄見文末連結。

## 1. 總覽

| 資產 | 來源 | 加工工具 | 交付位置 | 進版本控制 |
|---|---|---|---|---|
| VRM 1.0 模型 | 本機 PMX＋貼圖（`vrm/9001_Hayakawa Tazuna/`） | Blender＋MMD Tools＋VRM Add-on | `assets/characters/9001_hayakawa_tazuna/vrm/HayakawaTazuna.vrm` | 是（`.vrm`） |
| 影片擬合動作（9 段） | 本機原片＋`animation_reference/` 關鍵點 | `tools/motion-reference/`、Python 擬合、Blender | `assets/characters/9001_hayakawa_tazuna/animation/` | 是（`.vrma`、`animations.json`） |
| 遊戲原始動作（12 個動作、36 段） | 本機遊戲資料（日服用戶端） | `umaviewer/tools/uma_motion/`（專案外）、Blender | `_staging/game-motion/`（暫存區，未整合） | **否**（`.gitignore` 排除） |
| 表情 | VRM 內 27 個表情；遊戲 124 組預設表情尚未轉換 | Blender（VRM 表情） | 內含於 `.vrm` | 隨 `.vrm` |
| Q 版 VRM 1.0 模型（預備素材） | 本機 Q 版 PMX＋貼圖（`vrm/9001_Hayakawa Tazuna_mini/`） | Blender＋MMD Tools＋VRM Add-on | `assets/characters/9001_hayakawa_tazuna_mini/vrm/HayakawaTazunaMini.vrm` | 是（`.vrm`） |
| Q 版遊戲動作（5 個動作、15 段） | 本機遊戲資料 | 同動作來源 B | `_staging/mini/`（暫存區） | **否** |
| 背景場景：學園走廊（特寫過場 1053） | 本機場景 PMX＋3 張貼圖集（`vrm/scene/`） | Blender（`_staging/scene/build_scene.py`，無介面執行） | `_staging/scene/corridor.glb` | **否**（只有 `scene.json`、腳本與說明進版本控制） |
| 原始素材（PMX、.blend、貼圖、渲染、原片、對照影格、遊戲資料） | — | — | 只留本機 | **否**（`AGENTS.md` 第 4 條） |

## 2. 模型：PMX → VRM 1.0

- **工具（實際使用版本）**：Blender 5.1.2、[MMD Tools](https://github.com/MMD-Blender/blender_mmd_tools) 4.5.14、[VRM Add-on for Blender](https://github.com/saturday06/VRM-Addon-for-Blender) 4.10.2，透過 mcp-for-blender 操作 Blender。
- **流程**：匯入 PMX（0.08 比例、身高 1.672 m、T 字姿勢）→ 清理重複網格與無用骨骼（373 → 209 根）→ MToon 1.0 材質 → 54 根人形骨對應 → 表情（13 個預設、9 個客服用自訂）→ 骨骼驅動視線 → 自建 24 條彈簧鏈與 12 個碰撞體 → 匯出並重新匯入驗證。
- **原檔不動**：PMX 加工前後 MD5 一致；中間存檔在 `blend/`。
- **驗收**：Blender 重新匯入數值全部吻合；瀏覽器自動驗收 15/15。
- **詳細紀錄**：[`assets/characters/9001_hayakawa_tazuna/docs/加工紀錄.md`](../../assets/characters/9001_hayakawa_tazuna/docs/加工紀錄.md)。網站用的模型資訊（表情清單、授權、視線範圍）在 [`character.json`](../../assets/characters/9001_hayakawa_tazuna/character.json)。

## 3. 動作來源 A：從影片擬合

- **輸入**：`example_pose/9001_Hayakawa Tazuna/` 的原片（唯讀）與 [`tools/motion-reference/`](../../tools/motion-reference/README.md) 產生的參考資料（全身 133 點關鍵點、遮罩、OCR）。
- **流程**：還原原片相機 → 逐格擬合身體與手指 → 接觸修正（手貼腹部、胸口、下巴）→ 手掌方向修正 → 平滑、重取樣 30 fps → 切成循環與轉換段 → Blender 建立動作 → 匯出 VRMA。
- **成果**：9 段 VRMA 與 `animations.json`，為目前 avatar-contract 狀態表使用的動作（`idle_polite`、`chest_hold` 等）。
- **限制**：單一視角無法唯一決定手的深度與翻轉；被對話框遮住的左手與下半身靠推估；部分轉換段是反向播放補製。
- **詳細紀錄**：[`assets/characters/9001_hayakawa_tazuna/animation/動作加工紀錄.md`](../../assets/characters/9001_hayakawa_tazuna/animation/動作加工紀錄.md)；擬合程式與中間資料在同目錄 `work/`。

## 4. 動作來源 B：遊戲原始資料（本機研究用）

### 4.1 取得方式

- 以使用者本機安裝的遊戲用戶端資料為來源（`C:\Users\admin\Umamusume\umamusume_Data\Persistent`），**不下載、不修改**遊戲檔案。
- 讀取流程移植自 UmaViewer（上游開源專案 [katboi01/UmaViewer](https://github.com/katboi01/UmaViewer)；使用者另有反編譯重建版 `C:\Users\admin\dev_open\umaviewer\umaviewer`）：開啟資產清單 → 依清單找出資產檔 → 解密 → 解析 Unity 資產 → 解碼動畫曲線。
- 工具放在**專案外**的 `C:\Users\admin\dev_open\umaviewer\umaviewer\tools\uma_motion\`。它含解密邏輯，刻意不進本專案；程式內不存金鑰，執行時從 UmaViewer 的設定檔讀取。
- 依賴（實際使用版本）：[UnityPy](https://pypi.org/project/UnityPy/) 1.25.4、[apsw-sqlite3mc](https://pypi.org/project/apsw-sqlite3mc/) 3.54.0.0、numpy、scipy。

### 4.2 轉換到本模型

- 遊戲骨架與本模型骨架的骨骼名稱、靜止姿勢一致（僅差 1.033 倍縮放），以「Unity 世界旋轉差量 → Blender 骨骼旋轉」換算後烘焙成 Blender 動作，再用 VRM Add-on 匯出 VRMA。
- Blender 端程式：`assets/characters/9001_hayakawa_tazuna/animation/work/tz_game.py`（`build_action`、`export_list`）。
- 驗證：四元數長度 1±0.0001（解碼正確）；循環接縫 ≤ 0.11°；所有開始段的首格與結束段的末格為同一個中立站姿（兩組之間最大差 5.4°）。

### 4.3 可用素材（2026-10-25 盤點）

| 類別 | 數量 | 備註 |
|---|---|---|
| 共用事件動作 `type00` | 3,473 個（431 類） | 為標準體型設計，接觸動作需逐一檢查 |
| 角色專屬事件動作 | 169 位角色、1,410 個 | 駿川たづな：`idle01`、`idle02`、`mot01`、`mot02` |
| 駿川たづな其他 | Q 版事件 5 個、卡片過場 9 段（含表情、耳朵）、賽後結果 1 段 | 尚未轉換 |
| 預設表情 | 124 組（另有 Q 版 36 組） | 在主資料庫 `face_type_data`；尚未轉換 |
| 劇情時間軸 | 約 43,011 個 | 台詞、動作、表情、視線的對照，可做為情境對應的統計來源 |

### 4.4 目前成果（暫存區）

- `_staging/game-motion/`：12 個動作（待機、介紹手勢、雙手合十、舉手、鞠躬、揮手、手放胸前、思考、指引方向、豎指說明、開心、拍手）× 開始／循環／結束，共 36 段 VRMA，以及清單 `game_motions.json`。
- 測試頁 `_staging/game-motion/viewer/index.html`（直接雙擊開啟），切換方式照遊戲原生設計；自動驗收 4/4。
- 詳細說明：[`_staging/game-motion/README.md`](../../_staging/game-motion/README.md)。

## 5. Q 版模型（預備素材）

- **來源**：`vrm/9001_Hayakawa Tazuna_mini/` 的 PMX 與 7 張貼圖；流程與正式版相同（Blender 5.1.2、MMD Tools 4.5.14、VRM Add-on 4.10.2）。
- **與正式版的差異**：
  - 身高 1.13 m；人形骨 31 根（手指只有拇指、食指、無名指各 2 節）。
  - **表情靠貼圖格切換**：眼、眉、嘴是貼圖集，以 VRM 1.0 的 `textureTransformBinds` 實作 13 個預設表情與遊戲的 36 組 Q 版預設表情（`mini_*`），全部是二元表情。
  - 臉頰貼圖由乘法疊加改成半透明疊加；視線不轉動（眼睛是貼圖）。
- **動作**：Q 版骨架比例與正式版不同，使用遊戲的 Q 版動作庫。たづな的 5 個 Q 版事件動作已轉成 VRMA，放在 `_staging/mini/`；轉換方式同動作來源 B（`uma2blend.convert(..., body_bundle='3d/chara/mini/body/mbdy9001_00/pfb_mbdy9001_00', rest_file='blender_rest_mini.json')`）。
- **狀態**：預備素材，尚未接進網站。
- **詳細紀錄**：[`assets/characters/9001_hayakawa_tazuna_mini/docs/加工紀錄.md`](../../assets/characters/9001_hayakawa_tazuna_mini/docs/加工紀錄.md)、[`_staging/mini/README.md`](../../_staging/mini/README.md)。

## 6. 運用方式

### 6.1 測試

| 用途 | 位置 |
|---|---|
| 模型驗收、表情、視線、彈簧骨、影片擬合動作 | `tools/vrm-test-viewer/index.html` |
| 遊戲原始動作（含表情按鈕） | `_staging/game-motion/viewer/index.html` |
| Q 版模型、Q 版動作、Q 版表情 | `_staging/mini/viewer/index.html` |

兩個測試頁都以 base64 內嵌資料，可用 `file://` 直接開啟；內嵌檔（`dist/*.js`、`game-pack.js`）不進版本控制。

### 6.2 網站載入

- 依 [avatar-contract.md](avatar-contract.md)：角色素材只在設定 `AVATAR_ASSET_DIR` 時由 `GET /avatar-assets/{character}/...` 提供；公開部署預設不提供，網站在沒有角色素材時仍可運作（Q16）。
- 情緒對表情的權重、手勢的替代順序、影片擬合動作的圖都在 [`behavior.json`](../../assets/characters/9001_hayakawa_tazuna/behavior.json)；改這裡不用改程式。
- 遊戲原始動作只在本機設定 `AVATAR_GAME_MOTION_DIR=_staging/game-motion` 時，由 `GET /avatar-assets/{id}/game/...` 提供，網站會優先使用（手勢較多）；公開部署不得設定（第 7 節）。
- 對話頁角色窗的背景場景只在本機設定 `AVATAR_SCENE_DIR=_staging/scene` 時，由 `GET /avatar-assets/{id}/scene/...` 提供；公開部署不得設定（見「權利與版本控制規則」一節）。加工方式見 [`_staging/scene/README.md`](../../_staging/scene/README.md)。
- 動作與表情如何對應到對話情境，見 [avatar-contract.md](avatar-contract.md) 與 [avatar-behavior-design.md](avatar-behavior-design.md)。

### 6.3 重新產生

| 項目 | 方式 |
|---|---|
| VRM | 開啟 `blend/HayakawaTazuna_VRM.blend` → VRM Add-on 匯出 |
| 影片擬合動作 | Blender 載入 `animation/work/` 的 `tz_clips_build.py` → `build_all(export=True)` |
| 遊戲原始動作 | `uma_motion/uma2blend.py` 轉出 JSON → Blender `tz_game.export_list()` → `_staging/game-motion/vrma/` |
| Q 版 VRM | 開啟 `assets/characters/9001_hayakawa_tazuna_mini/blend/HayakawaTazunaMini_VRM.blend` → VRM Add-on 匯出 |

## 7. 權利與版本控制規則

1. 原始模型、貼圖與遊戲資料的著作權屬 Cygames, Inc.。`.vrm` 的中繼資料已設為不可再散布、非商用。
2. 原始素材（PMX、.blend、貼圖、渲染、原片、對照影格、遊戲資料）只留本機（`AGENTS.md` 第 4 條）。
3. 遊戲衍生資料（暫存區的 VRMA、內嵌動作包、清單）以 `_staging/game-motion/.gitignore`、`_staging/mini/.gitignore` 排除；解密工具與金鑰不進本專案。
4. 官方校徽（`assets/brand/tracen-crest*.svg`，依官方商品照片重新描繪）依使用者 2026-10-26 的決定納入 git，README 橫幅 `docs/brand/banners/banner.*.webp` 也放了校徽，聲明見 `NOTICE.md`。Docker 映像也放了校徽（`Dockerfile` 複製 `tracen-crest.svg`，`compose.yaml` 設定 `BRAND_CREST_FILE`），公開版顯示校徽；網站只在檔案存在時顯示（`BRAND_CREST_FILE`），否則改用自製的蹄鐵標誌。
5. 解密遊戲資產涉及規避技術保護措施的法律問題（臺灣著作權法第 80 條之 2），只做本機研究使用。**公開部署前必須另行確認權利**，此前公開版不含角色素材（Q16）。

## 8. 已知限制

| 項目 | 狀況 |
|---|---|
| 瞳孔縮放、眼睛高光、眼睛圖案變化 | PMX 未保留，需補製（見模型加工紀錄） |
| 眉毛透過瀏海顯示 | 標準 VRM／MToon 無法表達原作的 stencil 效果 |
| 扭轉輔助骨、IK | VRMA 只含 VRM 標準骨骼；遊戲的 `ArmRoll`／`ShoulderRoll` 與手部 IK 不會帶過去 |
| 窄裙 | 沒有骨骼，大幅抬腿時會拉伸，動作庫應避開 |
| 遊戲表情、耳朵、口型 | 尚未轉換；目前眨眼與口型由網站端產生 |
| 遊戲改版 | 金鑰或格式改變時，讀取工具需要跟著更新 |

## 9. 相關文件

- 模型加工紀錄：[`assets/characters/9001_hayakawa_tazuna/docs/加工紀錄.md`](../../assets/characters/9001_hayakawa_tazuna/docs/加工紀錄.md)
- 影片擬合動作紀錄：[`assets/characters/9001_hayakawa_tazuna/animation/動作加工紀錄.md`](../../assets/characters/9001_hayakawa_tazuna/animation/動作加工紀錄.md)
- 遊戲原始動作暫存區：[`_staging/game-motion/README.md`](../../_staging/game-motion/README.md)
- Q 版模型加工紀錄：[`assets/characters/9001_hayakawa_tazuna_mini/docs/加工紀錄.md`](../../assets/characters/9001_hayakawa_tazuna_mini/docs/加工紀錄.md)；Q 版暫存區：[`_staging/mini/README.md`](../../_staging/mini/README.md)
- 影片參考前處理：[`tools/motion-reference/README.md`](../../tools/motion-reference/README.md)
- 角色播放介面契約：[avatar-contract.md](avatar-contract.md)
- 角色行為系統設計：[avatar-behavior-design.md](avatar-behavior-design.md)
