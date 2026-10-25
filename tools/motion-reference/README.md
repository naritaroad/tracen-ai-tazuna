# 動作參考素材處理工具

把本機影片整理成可供 Blender／Claude 動畫加工使用的參考資料。保留原始影片與時間戳，
輸出角色影格、手臉特寫、全身關鍵點、動漫角色遮罩、日文 OCR、候選參考片段和審閱頁面。

## 本次資料位置

- 輸入：專案下 `example_pose/9001_Hayakawa Tazuna/exsample_source_vedio/`
- 輸出：同層 `animation_reference/`
- 交接入口：輸出下 `CLAUDE_HANDOFF.md`
- 瀏覽入口：輸出下 `index.html`（可使用目前專案的本機 HTTP 伺服器）

## 環境與安裝

本次使用 Windows 上的 WSL，在 Linux 快取放隔離的 Python 3.12 環境，輸入和交付仍放在 Windows 專案。
需要 `uv`、`ffmpeg`、`ffprobe`。模型在首次使用時下載，影片與辨識運算都留在本機。
本次 CPU 推論，限制每個模型的執行緒，不占用已在使用中的遊戲／Blender GPU。

從專案根目錄執行：

```bash
bash tools/motion-reference/setup.sh
bash tools/motion-reference/run.sh build
```

PowerShell 可使用：

```powershell
& .\tools\motion-reference\run.ps1 build
```

`TRACEN_MOTION_RUNTIME` 可更換 Linux 環境與模型快取的位置；預設為 Linux 使用者的
`~/.cache/tracen-motion-ref/`。不要把 `.venv` 與模型快取複製到交付壓縮檔。
`requirements.lock.txt` 固定完整已驗證依賴；安裝採 `--no-deps` 避免 rtmlib 宣告的多份 OpenCV 互相覆寫。

## 常用操作

```bash
# 指定單支影片；開頭為 - 的檔名請使用等號。
bash tools/motion-reference/run.sh build --video=-WzWAQbzEHY

# 小批測試，資料會明確標記尚未處理完整來源。
bash tools/motion-reference/run.sh build --video qQ40c6kgV8M --limit 5 --output /tmp/tazuna-pilot

# 按來源完整影格率抽取局部區間，包含原始 PTS 與無損 PNG。
bash tools/motion-reference/run.sh extract --video qQ40c6kgV8M --start 16.2 --end 17.0

# 重新建立索引頁／驗證現有資料，不重新推論。
bash tools/motion-reference/run.sh report
bash tools/motion-reference/run.sh verify

# 幾何、時間區間與 OCR 合併測試。
~/.cache/tracen-motion-ref/.venv/bin/python -m unittest discover -s tools/motion-reference/tests -v
```

同一來源 SHA-256 與 profile 的已完成資料會重用；不同輸入請換輸出目錄。
`--force` 只適用刻意重新產生該影片的衍生資料；不修改原片。

## 每支影片輸出

| 路徑 | 內容 |
|---|---|
| `manifest.json` / `scan.json` | 原片識別、媒體資訊、設定、解碼與處理範圍 |
| `frame_timestamps.csv` | 每一來源影格的編號、PTS、time_base、時間與畫面變化 |
| `poses.jsonl` / `keypoint_schema.json` | 10 Hz、133 點的 2D 全身／手／臉估計與信心分數 |
| `overview/` | 約每秒一張的畫面總覽，去除持續黑邊 |
| `body/` | 10 Hz 角色裁切，同鏡頭固定裁切範圍 |
| `overlay/` | 關鍵點疊圖，用來檢查追蹤是否可靠 |
| `details/` | 有足夠信心與像素的臉部、左右手特寫 |
| `masks/` / `masks/raw/` / `masked/` | 排除對話框後遮罩、原始模型遮罩、去背預覽 |
| `text/` / `ocr.raw.json` | 文字區域圖片與逐次 OCR 結果 |
| `dialogue.json` / `dialogue.ocr.srt` | 合併重複／逐字顯示後的候選日文文字事件 |
| `shots.json` / `windows.json` | 鏡頭與候選參考區間；不是已完成動作標註 |
| `clips/` / `sheets/` | 原片裁切與低 token 審閱用接觸表 |
| `audio.m4a` / `audio_envelope.json` | 原片音訊與 20 ms 能量包絡；未做音素對齊 |

座標一律以來源影格像素為準。遮罩回原圖請使用 `masks.json` 的 `bbox_source`。
判斷可觀測關節時一起檢查 `scores`、`observable`、`flags` 和原圖。

## 交接邊界

本工具準備動畫參考資料，不會自動生成可靠 3D 動作，不會把 2D 座標當骨骼旋轉，
也不會自動證明動作適合某種情境。遮擋、說話者、日文文字、遮罩邊緣與動作起收仍須複核。
網站、角色模型及 Blender 原檔不由這個工具改動。

技術選擇、Docker／Rust／PostgreSQL 評估與限制請見 `docs/engineering/architecture.md`。
來源與授權見 `THIRD_PARTY_NOTICES.md`；實際模型雜湊在輸出下 `provenance.json`。
