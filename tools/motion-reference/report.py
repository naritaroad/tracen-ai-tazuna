"""Generate a self-contained local index; no service or external assets required."""
from __future__ import annotations

import json
import csv
import os
from pathlib import Path
from urllib.parse import quote

from core import dump


def build_report(output, project):
    entries = []
    for path in sorted(output.glob('*/manifest.json')):
        folder = path.parent
        manifest = json.loads(path.read_text())
        if not isinstance(manifest, dict) or 'input_signature' not in manifest:
            continue
        poses = [json.loads(line) for line in (folder/'poses.jsonl').read_text().splitlines()]
        items = []
        for p in poses:
            if not ('overview' in p or p.get('overlay')):
                continue
            items.append({key: p.get(key) for key in ['id', 't', 'frame', 'shot', 'target_present',
                'flags', 'overview', 'body_image', 'overlay', 'mask_preview', 'details',
                'body_confidence', 'face_confidence', 'left_hand_confidence', 'right_hand_confidence']})
        source = project/manifest['source_relative_to_project']
        entries.append({'id': folder.name, 'manifest': manifest, 'samples': items,
            'frame_times': [round(float(r['video_time_seconds']), 6) for r in csv.DictReader((folder/'frame_timestamps.csv').open())],
            'shots': json.loads((folder/'shots.json').read_text()),
            'source_url': quote(os.path.relpath(source, output).replace(os.sep, '/'), safe='/'),
            'windows': json.loads((folder/'windows.json').read_text()),
            'dialogue': json.loads((folder/'dialogue.json').read_text()),
            'masks': json.loads((folder/'masks.json').read_text())})
    data = {'schema_version': '1.0', 'videos': entries, 'start_here': 'START_HERE.md' if (output/'START_HERE.md').exists() else None,
            'note': 'Machine-generated reference data. Review annotations before animation authoring.'}
    dump(output/'catalog.json', data)
    (output/'catalog.js').write_text('window.REFERENCE_DATA = '+json.dumps(data, ensure_ascii=False).replace('</', '<\\/')+';\n', encoding='utf-8')
    (output/'index.html').write_text((Path(__file__).parent/'viewer.html').read_text(), encoding='utf-8')
    total = {'videos': len(entries), 'duration_seconds': round(sum(e['manifest']['source_duration'] for e in entries), 3),
        'decoded_frames': sum(e['manifest']['decoded_frames'] for e in entries),
        'pose_samples': sum(e['manifest']['pose_samples'] for e in entries),
        'masks': sum(e['manifest']['mask_samples'] for e in entries),
        'ocr_samples': sum(e['manifest']['ocr_samples'] for e in entries),
        'dialogue_events': sum(e['manifest']['dialogue_events'] for e in entries),
        'windows': sum(e['manifest']['windows'] for e in entries)}
    dump(output/'summary.json', total)
    handoff = f'''# Claude 動畫加工交接

處理日期：2026-10-25（Asia/Taipei）。請先開啟同目錄的 index.html；若有 START_HERE.md，先讀其中的初步目視選段。

## 現有交付

- {total['videos']} 支來源影片；合計 {total['duration_seconds']} 秒。
- 已解碼 {total['decoded_frames']} 格，保留原始 PTS/time_base 與來源影格編號。
- {total['pose_samples']} 組 10 Hz 姿勢樣本；{total['masks']} 組角色遮罩。
- {total['ocr_samples']} 次 OCR；合併為 {total['dialogue_events']} 段候選文字事件。
- {total['windows']} 段候選動作參考區間與影片裁切。

上述數量表示資料產出，不代表所有機器辨識正確，也不表示已完成動畫。

## 請依序閱讀

1. summary.json、verification.json、provenance.json：範圍、驗證與模型來源。
2. 任一影片的 manifest.json、shots.json、windows.json：鏡頭與候選時間區間。
3. ACTION_INDEX.md、各影片 timeline.csv、action_candidates.json：挑選參考區間；motion_events.json 可找局部移動起點／高峰／終點。
4. 選定區間的 sheets/window_*.jpg：少量影格總覽，再看 clips/window_*.mp4。
5. dialogue.json、text/*.jpg：核對日文原文、說話者、情境。OCR 尚未人工逐字校正。
6. poses.jsonl 的對應時間區間、overlay/*.jpg、details/*/*.jpg：定位關節與手臉細節。
7. masks.json、masks/*.png：遮罩 bbox_source 定義其回到來源畫面的座標；mask_quality.json 列出遮罩快速變化的複核點。

## 動畫製作方式

- 加工目標位於專案根目錄下 assets/characters/9001_hayakawa_tazuna/blend/HayakawaTazuna_VRM.blend，保留原檔，另存加工版本。
- 優先選一段同鏡頭、目標清楚、起收完整的短動作。
- 若能取得 VMD，先匯入並對應到現有骨架，再拿本資料集校對；這次輸入只有影片，尚無 VMD。
- 影片姿勢是 2D 推估，不可把 x/y 直接當 3D 骨頭旋轉。observability、confidence、UI 遮擋必須共同參考。
- 裁切後角色大小在同一鏡頭內固定。跨鏡頭、特寫、縮放不能直接接成連續動作。
- 先調整起手、重心、節奏、停頓與收勢，再補手指、視線、表情。
- 被字幕覆蓋或在畫面外的部位標為未知；不可把辨識器猜測當作原作真值。
- 為各動作人工標記名稱、情緒、適用情境、可否循環、進入/退出姿勢與可中斷時點。
- 完成後輸出可編輯 .blend 與 .vrma；在網站確認循環和切換，不要同時讓測試頁的 applyPose 覆寫同一骨骼。

## 低 token 審閱

先讀一個 window 的 JSON 和接觸表；只讀該區間需要的 poses.jsonl 行。
發現問題再以來源完整影格率抽取局部區間，例如（從專案根目錄執行）：

```bash
bash tools/motion-reference/run.sh extract --video qQ40c6kgV8M --start 16.2 --end 17.0
```

輸出的 PNG 保留來源影格率，frames.json 保存真實時間；不要一次把全部圖片與數字送進對話。
可在 index.html 載入 Blender 算圖影片做同步視覺比較，並匯出人工審閱註記。
音訊與能量包絡已保留；本次沒有語音辨識或音素對齊，OCR 時間不可直接作為精準對嘴時間。

## 待人工複核

全身/手臉關鍵點、前景遮罩、說話者、日文 OCR、動作語意與循環性都屬機器候選結果。
雙人片段已限制目標在畫面右側的綠色制服角色；仍應檢查手臂交疊與其他角色混入。
mKNl_wQwpn0 為剪輯影片，需特別排除標題卡與轉場。
'''
    (output/'CLAUDE_HANDOFF.md').write_text(handoff, encoding='utf-8')
    return total
