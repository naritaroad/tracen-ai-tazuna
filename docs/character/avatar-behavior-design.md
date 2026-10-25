# 角色行為系統設計（情境 → 動作與表情）

日期：2026-10-25。**狀態：第一版已實作（2026-10-25），實際行為以 [avatar-contract.md](avatar-contract.md) 為準。** 已做到：三層架構、固定詞彙的情緒與手勢（模型的 `[[mood:…]]` 經伺服器驗證，否則用規則）、行為規劃層（查表＋避免重複的加權隨機、一次性手勢後接說話姿勢）、遊戲原生的開始／循環／結束切換（以動作圖找路）、表情混合、自動眨眼、文字推估口型與句末點頭、視線與頭部跟隨、程序鞠躬與傾頭。尚未做：第 4 節的離線統計管線（`rules.json`、`motion_catalog.json`、劇情時間軸共現）、遊戲 124 組預設表情、口型以外的疊加動作庫、慣性化混合（目前用交叉淡化）。

## 1. 要解決的問題

遊戲原始資料可用的素材數量遠超過手動對照的範圍（2026-10-25 實際盤點本機遊戲資料，見 [character-assets.md](character-assets.md)）：

| 素材 | 數量 |
|---|---|
| 共用事件動作 `type00` | 3,473 個動作（431 類，23,267 個檔案） |
| 角色專屬事件動作 | 169 位角色共 1,410 個（駿川たづな 4 個） |
| 預設表情 `face_type_data` | 124 組（眉 29、眼 29、嘴 92 種零件組合）；Q 版 36 組 |
| 劇情時間軸 `storytimeline` | 約 43,011 個，每個區塊記錄台詞、說話者、動作名稱、表情零件、疊加動作、視線與混合時間 |

目標：讓系統**以最低延遲、最少模型輸出**把對話情境連到合適的動作與表情，同時維持遊戲原生的平順切換。

## 2. 設計原則

1. **模型不挑檔案。** 模型只從固定列舉輸出語意標籤（意圖、情緒、強度），由伺服器驗證；動作與表情由程式查表決定（延續 `AGENTS.md` 第 7 條與 avatar-contract 的 `emotion_from_model` 保留欄位）。
2. **執行時只查表。** 所有比對、標註、統計都在建置時完成，執行時是 O(1) 的對照表查詢加簡單加權隨機。
3. **沿用遊戲的結構。** 動作採「開始 `_s`／循環 `_loop`／結束 `_e`」與共同的中立站姿；表情採眉／眼／嘴零件組合；時間軸採劇情時間軸的分軌方式。
4. **分層、可回退。** 任一層失敗都退回上一層的安全狀態（最後回到 `PlaceholderAvatar`），不影響客服功能。

## 3. 三層架構

分層方式參考具身對話代理的經典架構：BEAT（Cassell、Vilhjálmsson、Bickmore，SIGGRAPH 2001）與 SAIBA／BML 框架（Kopp 等，IVA 2006）——「意圖規劃 → 行為規劃 → 行為呈現」。

```
使用者訊息
   │
① 意圖層（Rust API＋模型，函式呼叫的固定列舉）
   │   avatar.intent { state, emotion_from_model{emotion,intensity}, gesture? }
   ▼
② 行為規劃層（前端 AvatarController，純函式、可單元測試）
   │   (state, emotion, intensity, gesture) ──查表──▶ 候選池 ──選擇策略──▶ 時間軸
   │   時間軸分軌：body / face.eyebrow / face.eye / face.mouth / additive / gaze / viseme
   ▼
③ 呈現層（前端 AvatarStage，three-vrm）
       動作狀態機（中立站姿切換）・表情混合器・疊加層・口型・眨眼・視線・慣性化混合
```

### 3.1 意圖層（伺服器）

- 生命週期狀態（`state`）維持 avatar-contract 第 2 節的定義，由伺服器依 run 事件產生，不交給模型。
- 模型在產生回覆時，以函式呼叫的固定列舉附帶 `emotion` 與 `intensity`，必要時附帶 `gesture`；伺服器驗證是否在列舉內，不合法就丟棄並使用該狀態的預設值。
- 意圖欄位要在回覆內容之前送出，讓前端在文字或語音生成期間先播放動作的「開始」段（0.5–0.7 秒），掩蓋生成延遲。

#### `avatar.intent` 擴充（相容既有欄位）

```json
{ "state": "explaining", "expression": "smile", "priority": 2, "min_hold_ms": 0,
  "interruptible": true, "reason": "message.completed", "character_id": "9001_hayakawa_tazuna",
  "emotion_from_model": { "emotion": "gentle", "intensity": 0.6 },
  "gesture": "hand_chest" }
```

舊版前端忽略新欄位即可，行為與現在相同。

#### 詞彙表（初版提案，定案後放 `backend/src/avatar/vocab.rs` 與 `web/src/avatar/vocab.ts`，兩邊共用 `GET /api/v1/avatar/states` 回傳的表）

| 列舉 | 值 |
|---|---|
| `emotion` | `neutral`、`gentle`、`happy`、`excited`、`sincere`、`apologetic`、`troubled`、`surprised`、`serious`、`playful` |
| `gesture`（語意手勢） | `greet`、`bow`、`wave`、`present`、`guide`、`explain`、`hand_chest`、`think`、`hope`、`raise_hand`、`happy`、`clap` |
| `intensity` | 0–1，前端分成 `low`／`mid`／`high` 三段 |

`gesture` 是語意名稱，不是檔名；同一個手勢在目錄裡可以對應多個動作變體。

### 3.2 行為規劃層（前端）

1. **查表**：`rules.json` 預先算好 `state|emotion|intensityBand|gesture → 候選動作清單（含權重）` 與 `emotion|intensityBand → 表情預設清單`。
2. **選擇策略**（依序過濾、再加權隨機）：
   - 可用性：已下載、通過品質檢查（接觸、穿模）、進出姿勢可接上目前狀態；
   - 長度：動作的開始＋循環長度與這句話的預估語音長度相符；
   - 避免重複：最近播過的動作降低權重（冷卻）；
   - 權重：來自劇情時間軸的共現統計，駿川たづな本人的使用紀錄另外加權。
3. **時間軸**：每句話一個區塊，各軌各自排程（與遊戲劇情時間軸相同的分軌）：身體動作、眉、眼、嘴、疊加動作（點頭、歪頭）、視線、口型。
4. **優先權與停留**：沿用 avatar-contract 的 `priority`、`min_hold_ms`、`interruptible`；另加「至少停留一個循環」與「情緒延續」（同一回覆內不逐句翻轉情緒）。

### 3.3 呈現層（前端）

| 元件 | 做法 |
|---|---|
| 動作狀態機 | 目前循環播完 → 該動作 `_e`（回到中立站姿）→ 目標 `_s` → 目標 `_loop`；`enter_from: idle` 的動作先經過 `idle_s`。已在 `_staging/game-motion/viewer/` 驗證（中立站姿最大差 5.4°，以 0.2 秒交叉淡化吸收；循環接縫 ≤ 0.11°） |
| 表情混合器 | 預設表情（例如 `WaraiA`）→ 眉／眼／嘴零件與權重 → VRM 表情或形態鍵；移植自遊戲的 `FaceDrivenKeyTarget` 邏輯，零件可分別淡入淡出 |
| 疊加層 | 點頭、歪頭等疊加動作，權重另計，不中斷身體動作 |
| 口型 | 有語音時用 `message.audio.visemes`（[voice-contract.md](voice-contract.md) 已預留）；沒有語音時依文字節奏產生簡化口型 |
| 眨眼、視線 | 自動眨眼；視線跟隨鏡頭或時間軸指定的目標，限制在模型的視線範圍內（`character.json` 的 `lookAt.rangeDegrees`） |
| 混合 | 狀態機內部用交叉淡化；被打斷時改用慣性化混合（dead blending），避免動作突然跳換 |

## 4. 離線資料管線（建置時產生）

```
遊戲資料（本機）─▶ 動作轉換（Python／Blender）─▶ VRMA
                 └▶ 劇情時間軸統計 ─────────────▶ rules.json
人工審核（測試頁）──────────────────────────────▶ catalog.json 的 QA 欄位
```

### 4.1 `motion_catalog.json`（每個動作一筆）

| 欄位 | 說明 |
|---|---|
| `id`、`family`、`variant` | 例：`hand_chest`、`muneate`、`06` |
| `gestures` | 對應的語意手勢 |
| `parts` | `s`／`loop`／`e` 各自的檔案、格數、秒數、遊戲來源資產名 |
| `enter_from`、`exit_to` | `neutral`／`idle` |
| `contact`、`prop`、`mirror` | 是否有手部接觸、是否需要道具、是否有鏡像版 |
| `qa` | `ok`／`needs_ik`／`rejected`，附檢查日期與備註 |
| `origin` | `type00`／`chara/chr9001_00`／`video_fit` |

### 4.2 `face_catalog.json`

124 組預設表情的零件組合（眉、眼、嘴、權重），以及對應到本模型 VRM 表情或形態鍵的換算表。

### 4.3 `rules.json` 的權重來源

1. 動作類別名稱的語意翻譯（例如 `muneate` = 手放胸前、`think` = 思考）。
2. 劇情時間軸共現統計：台詞以情緒分類器標註後，統計「情緒 → 動作名稱」與「情緒 → 表情零件」的頻率；駿川たづな登場區塊另外統計。
3. 人工審核結果：`qa` 不是 `ok` 的動作不進候選池。

### 4.4 出貨子集

網站只放精選的 60–150 個動作（依詞彙表每個手勢 3–8 個變體），其餘留在本機資料庫，需要時再補。

## 5. 效能預算

| 項目 | 目標 |
|---|---|
| 模型額外輸出 | 每則回覆 < 30 個詞元（只有列舉值） |
| 規劃層 | 查表＋加權隨機，每次 < 1 毫秒 |
| 預先載入 | 待機、中立站姿與最常用的 10–15 個動作；每段 VRMA 約 60–260 KB，同一動作三段打包 |
| 按需載入 | 其他動作在首次使用時下載並快取；未載入完成時先用同手勢已載入的變體 |
| 畫面 | 沿用 Q11：桌機 60 FPS、手機 30 FPS 為待測目標；3D 載入失敗時回到 `PlaceholderAvatar` |

## 6. 測試與驗收

- 規劃層單元測試：同一輸入在固定亂數種子下輸出固定時間軸；所有詞彙組合都有候選；冷卻與停留規則生效。
- 動作資料測試（沿用暫存區測試頁的自動驗收）：每段可取樣、中立站姿相容、循環接縫、轉換鏈完整。
- 契約測試：伺服器只送列舉內的值；舊前端忽略新欄位仍可運作。
- 目視驗收：每個手勢至少一個變體通過接觸與穿模檢查。

## 7. 實作順序（建議）

1. 完整掃描劇情時間軸，產生共現統計與駿川たづな的使用紀錄。
2. 定案詞彙表與 `motion_catalog.json`／`face_catalog.json` 格式。
3. 表情混合器：124 組預設表情接上暫存區測試頁。
4. 規劃層原型：在暫存區測試頁輸入一句台詞與標籤，自動演出。
5. 動畫解除暫緩後，再把 `AvatarStage` 換成 three-vrm 播放器並接上 `avatar.intent`。

## 8. 未決事項

- **權利**：遊戲衍生的動作與表情資料是否能隨網站公開部署（Q16 預設角色素材只在本機載入）。
- **IK 與扭轉輔助骨**：VRMA 只含 VRM 標準骨骼；共用動作在遊戲中會用 IK 修正手部接觸，網站端是否補做簡化 IK 待評估。
- **情緒分類器**：劇情台詞的情緒標註用哪個模型、是否沿用專案的 Gemini 設定。

## 參考資料

- Cassell, Vilhjálmsson, Bickmore. *BEAT: the Behavior Expression Animation Toolkit*. SIGGRAPH 2001. [PDF](https://www.media.mit.edu/gnl/publications/siggraph2001.pdf)、[SIGGRAPH 檔案](https://history.siggraph.org/?p=116944)
- Kopp 等. *Towards a Common Framework for Multimodal Generation: The Behavior Markup Language*. IVA 2006. [PDF](https://alumni.media.mit.edu/~kris/ftp/BML-IVA-06-KoppEtAl.pdf)
- Unity Manual：[Animation transitions](https://docs.unity3d.com/2022.3/Documentation/Manual/class-Transition.html)
- Daniel Holden：[Dead Blending](https://theorangeduck.com/page/dead-blending)、[Spring-It-On](https://theorangeduck.com/page/spring-roll-call)
- [three-vrm](https://github.com/pixiv/three-vrm)
