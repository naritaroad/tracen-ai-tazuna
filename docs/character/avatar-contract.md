# 角色播放介面契約

日期：2026-10-25。**狀態：已實作 3D 角色互動模組**（使用者 2026-10-25 要求解除暫緩）。後端依 run 的生命週期與回覆內容送出 `avatar.intent`；前端以 three-vrm 播放 VRM／VRMA，載入失敗或沒有素材時回到 2D 狀態卡。模型仍然**不能**指定動畫檔或骨架，只能從固定詞彙選情緒與手勢，由伺服器驗證。

## 1. 責任分工

| 層 | 負責 | 不負責 |
|---|---|---|
| Rust API（`backend/src/avatar/`） | 依 run 的生命週期（開始、呼叫工具、回覆、轉接、錯誤）送出 `avatar.intent`；回覆時以 `cues.rs` 決定情緒、強度與語意手勢；提供角色清單、行為對照、動作組與素材檔 | 不逐幀傳骨架，不讓 LLM 指定動畫檔 |
| 前端控制器 `states.ts` | 把 intent 轉成快照，處理優先權、最短停留、打斷；記住正在「說」的回覆文字 | 不呼叫模型 |
| 前端規劃層 `behavior.ts`、`speech.ts`、`motionGraph.ts` | 純函式：intent → 表情目標、手勢步驟、視線、頭部傾斜、點頭／鞠躬；回覆文字 → 口型時間軸與句末節拍；動作圖找路 | 不碰 three.js |
| 前端呈現層 `player/`（`VrmStage.tsx` 按需載入） | 動作圖播放與交叉淡化、表情混合、自動眨眼、口型、視線與頭部跟隨、程序疊加動作、指標互動、畫質與暫停 | 不決定業務狀態 |
| 角色資料 `assets/characters/<id>/` | `character.json`（模型資訊）、`animation/animations.json`（影片擬合動作）、`behavior.json`（情緒 → 表情、手勢替代、動作組的圖） | 不知道網站狀態 |
| Blender／動作加工 | 交付 VRMA 與清單 | 不知道網站狀態 |

## 2. 互動狀態

| `state` | 觸發 | 預設情緒／手勢 | 視線 | 可被打斷 |
|---|---|---|---|---|
| `idle` | 無事件、run 結束後 | `neutral`／— | 看使用者 | 是 |
| `listening` | 使用者正在輸入或錄音（前端自行送出） | `gentle`／— | 看向對話欄 | 是 |
| `thinking` | 使用者送出、`run.started` | `neutral`／`think`（0.9 秒後才擺姿勢） | 往上方側邊 | 是 |
| `working` | `tool.started` | `serious`／`think` | 往下（翻資料） | 是 |
| `explaining` | 回覆（`message.completed` 之前送出） | 依回覆內容（第 3 節） | 看使用者 | 是 |
| `confirming` | 等使用者確認（之後的預約流程） | `happy`／`hope` | 看使用者 | 是 |
| `handoff` | 建立理事長室案件 | `sincere`／`bow` | 看使用者 | 否，至少 1.5 秒 |
| `apologizing` | 查無資料 | `apologetic`／`hand_chest` | 看使用者 | 是 |
| `error` | `run.failed` | `troubled`／— | 看使用者 | 是 |

伺服器端的表在 `backend/src/avatar/mod.rs` 的 `STATES`，`GET /api/v1/avatar/states` 回傳整張表與詞彙。

## 3. `avatar.intent` 事件

```json
{ "state": "explaining", "expression": "smile", "priority": 2, "min_hold_ms": 0,
  "interruptible": true, "reason": "message.completed", "character_id": "9001_hayakawa_tazuna",
  "emotion": "gentle", "intensity": 0.6, "gesture": "guide", "cue_source": "model",
  "emotion_from_model": { "emotion": "gentle", "intensity": 0.6, "gesture": "guide" } }
```

- `priority`：0 idle、1 thinking/working、2 explaining/confirming/apologizing、3 handoff/error。低優先權不打斷仍在 `min_hold_ms` 內的高優先權狀態。新的使用者訊息一律打斷舊狀態並回到 `thinking`。
- `emotion`、`intensity`、`gesture`：每個事件都有。非回覆事件用狀態預設值（`cue_source: "state"`）。舊欄位 `expression` 保留給舊前端。
- 詞彙（`backend/src/avatar/cues.rs` 的 `EMOTIONS`、`GESTURES`）：
  - `emotion`：`neutral` `gentle` `happy` `excited` `sincere` `apologetic` `troubled` `surprised` `serious` `playful`
  - `gesture`（語意名稱，不是檔名）：`greet` `bow` `wave` `present` `guide` `explain` `hand_chest` `think` `hope` `raise_hand` `happy` `clap`
- **回覆的情緒怎麼決定**（`cues::infer`）：
  1. 系統提示詞第 15 條要求模型在回覆最後加一行 `[[mood:EMOTION INTENSITY GESTURE]]`。伺服器**一律刪除**這一行（訓練員、對話紀錄、記憶都看不到），只接受詞彙內的值，放進 `emotion_from_model`。
  2. 轉接理事長室（`sincere`＋`bow`）與查無資料（`apologetic`）是 run 的事實，**優先於模型**。
  3. 其他情況用模型的值；模型沒給（例如 `mock`）就用規則：道歉、拒絕、恭喜、謝謝、問候、道別、好消息、提醒注意、條列、地點、反問、驚嘆，三種語言的關鍵字，預設 `gentle`＋`hand_chest`。`cue_source` 標示用了哪一種。
- **保留欄位**：`look_at`、`viseme`（有語音回應時改用 `message.audio.visemes`，見 [voice-contract.md](voice-contract.md)）。

## 4. 端點

| 端點 | 用途 | 狀態 |
|---|---|---|
| `GET /api/v1/avatar/characters` | 角色清單（id、名稱、是否有本機素材） | 已實作 |
| `GET /api/v1/avatar/characters/{id}` | 角色清單檔：表情、視線範圍、授權、素材 URL、`behavior`（behavior.json 除了動作組）、`motion_sets`（見下） | 已實作；素材相關欄位只在設定 `AVATAR_ASSET_DIR` 時出現 |
| `GET /api/v1/avatar/states` | 狀態表與詞彙 | 已實作（`version: 2`） |
| `GET /avatar-assets/{character}/vrm/*.vrm`、`/animation/vrma/*.vrma` | 本機 VRM／VRMA | 只在設定 `AVATAR_ASSET_DIR` 時提供 |
| `GET /avatar-assets/{character}/scene/*.glb` | 本機背景場景（對話頁角色窗） | 只在另設 `AVATAR_SCENE_DIR` 時提供；清單檔多出 `scene`（網址、位置、轉向、`unlit`、霧），見 [`_staging/scene/README.md`](../../_staging/scene/README.md) |
| `GET /avatar-assets/{character}/game/vrma/*.vrma` | 本機遊戲原始動作 | 只在另設 `AVATAR_GAME_MOTION_DIR` 時提供（研究用，不可公開，見 [character-assets.md](character-assets.md) 第 7 節） |
| SSE `avatar.intent` | 角色狀態與回覆提示 | 已送出 |
| `POST /api/v1/avatar/sessions/{id}/events`（保留） | 前端回報播放完成、使用者打斷 | 未實作（回 501） |

### `motion_sets`

伺服器把動作正規化成一張圖：**循環片段是節點**（`from` = `to` = 自己的 id），其他片段是兩個節點之間的邊；`neutral` 是沒有循環的中立站姿節點。播放器用廣度優先搜尋找最少片段的路徑。

```json
{ "id": "game", "rest": "idle_loop", "clips": [
  { "id": "bow_s", "url": "/avatar-assets/…/game/vrma/bow_s.vrma", "loop": false, "duration": 1.33, "from": "neutral", "to": "bow_loop", "gestures": [] },
  { "id": "bow_loop", "url": "…", "loop": true, "duration": 1.67, "from": "bow_loop", "to": "bow_loop", "gestures": ["bow", "greet"] } ] }
```

| 動作組 | 來源 | 圖的來源 | 順序 |
|---|---|---|---|
| `game` | `AVATAR_GAME_MOTION_DIR/game_motions.json`（12 個動作 × 開始／循環／結束） | 伺服器依 `parts`、`enter_from`、`exit_to` 產生；手勢對照在 behavior.json | 有設定時排第一（前端用第一組） |
| `video_fit` | `animation/animations.json`（版本控制內的 9 段） | behavior.json 的 `loops`、`transitions`（`animations.json` 由 Blender 腳本重新產生，所以不放在那裡） | 一定有 |

`gestures` 也可以是 `emotion:<名稱>`，表示這個循環適合某種情緒（例如托腮適合 `playful`）。

## 5. 前端行為

- **手勢步驟**：一次性手勢（`behavior.json` 的 `oneshot`：鞠躬、揮手、拍手…）播指定的循環次數後，換成依情緒挑選的說話姿勢（`postures`），說完再回到待機。目前動作組沒有的手勢依 `gesture_fallbacks` 找替代；鞠躬沒有片段時改用程序鞠躬（脊椎、胸、頸依序前傾）。最近用過的循環降低權重，避免重複。
- **說話**：沒有語音時，`speech.ts` 依回覆文字產生口型（假名對母音、漢字與英文音節給穩定的近似母音）與句末節拍，句末點頭；最多 10 秒（先加速至多 20%，再截斷）。回覆語言用 `run.started.locale`。
- **表情**：`behavior.json` 的 `emotions`（強度 1 時的權重，依 `intensity` 縮放）加上 `state_expressions`；淡入快、淡出慢，保持時有微小起伏。自動眨眼（偶爾連眨、視線大幅移動時眨眼），閉眼表情時不眨。
- **視線**：眼睛快速跳動到目標並有小幅掃視，頭部以臨界阻尼彈簧跟隨眼睛到不了的角度。滑鼠在角色附近（畫布中心向外 8 個半寬以內，幾乎整個視窗）時看向游標，6 秒不動或離開視窗就回頭看訓練員；手指只在點到角色時算數。
- **程序疊加與動作片段**：點頭、視線、呼吸等偏移在算繪後撤回，不在混合器之前把骨骼歸零。three.js 的混合器只在片段數值改變時寫入骨骼，片段遇到保持不變的關鍵影格時，先歸零會讓頭部（或整個上半身）停在靜止姿勢一格，造成每隔幾秒 11–14° 的抖動（2026-10-25 修正）。
- **光影**：MToon 卡通著色仿遊戲：暖色主光從她右上方偏側面打，讓遊戲的陰影貼圖露出來；冷色天光補陰影面；背光加上 MToon 的邊緣光；描邊比匯出值粗 35%。
- **互動（`taps.ts`）**：點角色時，她用固定詞彙裡的一個手勢回應（揮手、開心、拍手、介紹、引導…，動作組沒有時依 `gesture_fallbacks` 替代），並「說」一句短台詞：台詞顯示在頭上的小對話框，同時帶動口型。點身體是一般招呼或與目前頁面有關的一句；點頭部會開心或驚訝；4 秒內連點頭部三次會害羞。同一句不會連續出現；1.5 秒內的連點只累計害羞次數，不重播手勢。台詞在語言檔 `avatar.tap.*`，是介面文案，不是校務事實。只在待命與聆聽時反應，不打斷說明；`prefers-reduced-motion` 時只說台詞、不做手勢。2D 狀態卡點頭像也會回一句並輕跳一下。
- **常駐諮詢（`concierge/`）**：全站只有一個角色實例（`ConciergeProvider`），3D 模型只載入一次，在「位置」（`CharacterSlot`）之間移動：對話頁右側欄底部（桌機寬約 29vw、隨視窗縮放；手機在輸入框上方），或其他訓練員頁面右下角的諮詢窗；沒有位置顯示時暫停。對話頁的位置帶背景場景（`backdrop`），諮詢窗維持透明。位置用 CSS 變數告訴播放器怎麼取景：`--avatar-headroom`（頭頂上方留給對話泡泡的高度比例）、`--avatar-body`（頭部以下拍多少公尺）。諮詢窗沒有另外的文字區：她的話顯示在頭頂留白處的對話泡泡裡（長回覆在泡泡內捲動），頁面的快捷問題疊在角色下半身上。諮詢窗與對話頁共用同一個狀態控制器與同一段對話（對話頁最後開啟的對話，或在諮詢窗開始的新對話），所以問題一樣經過 Agent 與工具，答案只來自資料庫與工具；查詢中顯示正在用的工具，送交理事長室時附連結，完整回覆與處理紀錄在對話頁看。快捷問題依頁面而定（`concierge.ask.<頁面>.*`）。桌機收合狀態記在瀏覽器；手機從按鈕開成下方面板。管理站沒有諮詢窗。
- **新對話問候**：空白對話第一次顯示角色時，鞠躬或揮手並「說」出問候語。
- **畫質（Q11）**：粗指標、記憶體或核心數 ≤ 4、省流量時用標準畫質（像素比 ≤ 1.5、30 FPS），否則高畫質（≤ 2、60 FPS）；分頁隱藏或角色捲出畫面時暫停。`prefers-reduced-motion` 時只換表情，不做手勢、點頭與擺動。
- **回退**：沒有素材、沒有 WebGL、模型載入失敗時顯示 2D 狀態卡並說明原因（2026-10-25 起不再提供手動切換簡易顯示的按鈕）。three.js 與 three-vrm 是獨立的程式塊（約 200 KB gzip），只有要顯示 3D 時才下載。

## 6. 測試與驗收

| 項目 | 方式 |
|---|---|
| 情緒規則、`[[mood:…]]` 解析與刪除、動作組正規化 | `cd backend && cargo test` |
| 規劃層、動作圖、口型時間軸 | `cd web && npm test`（Node 內建測試，直接執行 TypeScript） |
| 實際畫面 | 本機 `AVATAR_ASSET_DIR=assets/characters`（可加 `AVATAR_GAME_MOTION_DIR=_staging/game-motion`），登入訓練員站對話 |

待實測（沿用[計畫評估第 6 節](../reviews/2026-10-25-plan-review.md)）：桌機 60 FPS／手機 30 FPS；Sony 實機（Q01 加註）；遊戲共用動作的手部接觸與穿模。
