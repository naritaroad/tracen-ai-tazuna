"""Derive review priorities and candidate movement intervals from completed batches."""
from __future__ import annotations

import csv
import json
from pathlib import Path

import cv2
import numpy as np
from scipy.ndimage import median_filter

from core import dump, draw_pose, box_clip, green_fraction, split_ocr_rows, merge_ocr, write_srt


def movement_intervals(poses):
    """2D torso-normalized movement bouts; never label them as recovered actions."""
    results = []
    shots = sorted({p['shot'] for p in poses})
    groups = [('upper_body', list(range(5, 13))), ('left_hand', list(range(91, 112))),
              ('right_hand', list(range(112, 133))), ('face', list(range(23, 91)))]
    for shot in shots:
        seq = [p for p in poses if p['shot'] == shot and p['target_present']]
        if len(seq) < 6:
            continue
        for part, indices in groups:
            signal = np.zeros(len(seq))
            for i in range(1, len(seq)):
                a, b = seq[i-1], seq[i]
                dt = b['t']-a['t']
                if not 0 < dt < .25:
                    continue
                pa, pb = np.array(a['keypoints']), np.array(b['keypoints'])
                # Translation and uniform image scale cancellation is approximate,
                # and does not recover perspective or true 3D pose.
                ca, cb = (pa[5]+pa[6])/2, (pb[5]+pb[6])/2
                sa, sb = np.linalg.norm(pa[5]-pa[6]), np.linalg.norm(pb[5]-pb[6])
                if min(sa, sb) < 12 or min(a['scores'][5:7]+b['scores'][5:7]) < .45:
                    continue
                valid = np.array(a['observable'])[indices] & np.array(b['observable'])[indices]
                if np.sum(valid) < max(3, len(indices)//3):
                    continue
                delta = (pb[indices]-cb)/sb-(pa[indices]-ca)/sa
                signal[i] = np.median(np.linalg.norm(delta[valid], axis=1))/dt
            signal = median_filter(signal, size=5)
            positive = signal[signal > 0]
            if not len(positive):
                continue
            threshold = max(.12 if part != 'face' else .08, float(np.percentile(positive, 65)))
            active = signal > threshold
            # Bridge very short gaps while retaining continuous timestamps.
            for i in range(1, len(active)-1):
                if active[i-1] and active[i+1] and seq[i+1]['t']-seq[i-1]['t'] < .3:
                    active[i] = True
            starts = np.where(active & ~np.r_[False, active[:-1]])[0]
            ends = np.where(active & ~np.r_[active[1:], False])[0]
            for a, b in zip(starts, ends):
                if seq[b]['t']-seq[a]['t'] < .25:
                    continue
                peak = a+int(np.argmax(signal[a:b+1]))
                start_i, end_i = max(0, a-2), min(len(seq)-1, b+3)
                if any(seq[i+1]['t']-seq[i]['t'] > .25 for i in range(start_i, end_i)):
                    continue
                results.append({'id': '', 'shot': shot, 'part': part,
                    'start': seq[start_i]['t'], 'peak': seq[peak]['t'], 'end': seq[end_i]['t'],
                    'start_frame': seq[start_i]['frame'], 'peak_frame': seq[peak]['frame'], 'end_frame': seq[end_i]['frame'],
                    'keyframes': [seq[start_i]['id'], seq[peak]['id'], seq[end_i]['id']],
                    'peak_speed_shoulder_widths_per_second': round(float(signal[peak]), 5),
                    'threshold': round(threshold, 5), 'action_name': None, 'context_tags': [],
                    'review_status': 'candidate_2d_movement_not_semantic_action',
                    'limitations': ['estimation_noise', 'perspective_and_camera_not_fully_removed', 'visibility_filter_is_heuristic']})
    results.sort(key=lambda e: (e['start'], e['part']))
    for i, e in enumerate(results):
        e['id'] = f'movement_{i:04}'
    return results


def mask_quality(folder, masks, shots, source_shape):
    h, w = source_shape
    scale = 256/max(h, w)
    ph, pw = round(h*scale), round(w*scale)
    prev = None
    rows = []
    for m in masks:
        image = cv2.imread(str(folder/m['path']), cv2.IMREAD_GRAYSCALE)
        x, y, x2, y2 = [round(v*scale) for v in m['bbox_source']]
        x2, y2 = max(x+1, x2), max(y+1, y2)
        canvas = np.zeros((ph, pw), np.uint8)
        patch = cv2.resize(image, (x2-x, y2-y)) > 127
        canvas[y:y2, x:x2] = patch[:min(y2, ph)-y, :min(x2, pw)-x]
        shot = next((s['id'] for s in shots if s['start'] <= m['t'] < s['end']), None)
        iou = None
        flags = []
        if prev and prev[0] == shot and m['t']-prev[1] < .8:
            union = np.sum((canvas > 0) | (prev[2] > 0))
            iou = float(np.sum((canvas > 0) & (prev[2] > 0))/union) if union else 0.
            if iou < .5:
                flags.append('mask_change_review')
        if m['foreground_fraction'] < .08:
            flags.append('foreground_mostly_missing')
        rows.append({'id': m['id'], 't': m['t'], 'shot': shot,
            'previous_mask_iou': round(iou, 4) if iou is not None else None,
            'flags': flags, 'interpretation': 'change_can_be_true_motion_or_camera_change_not_only_mask_error'})
        prev = shot, m['t'], canvas
    return rows


def curate_dataset(root):
    summary = []
    for manifest_path in sorted(root.glob('*/manifest.json')):
        folder = manifest_path.parent
        manifest = json.loads(manifest_path.read_text())
        if not isinstance(manifest, dict) or 'input_signature' not in manifest:
            continue
        poses = [json.loads(line) for line in (folder/'poses.jsonl').read_text().splitlines()]
        scan = json.loads((folder/'scan.json').read_text())
        video = next(s for s in scan['metadata']['streams'] if s['codec_type'] == 'video')
        by_id = {p['id']: p for p in poses}
        raw_ocr = json.loads((folder/'ocr.raw.json').read_text())
        for sample in raw_ocr:
            crop = cv2.imread(str(folder/sample['image']))
            ox, oy, _, _ = sample['region_source']
            greens = []
            for row in sample['lines']:
                x, y, x2, y2 = row['bbox_source']
                x, y, x2, y2 = box_clip([x-ox, y-oy, x2-ox, y2-oy], crop.shape[1], crop.shape[0])
                greens.append(green_fraction(crop[y:y2, x:x2]))
            ui = by_id[sample['id']]['ui_occlusion_candidates']
            dialogue_rows, speaker = split_ocr_rows(sample['lines'], ui, video['height'], greens)
            sample['dialogue_text'] = '\n'.join(row['text'] for row in dialogue_rows)
            sample['speaker'] = speaker
            sample['mean_score'] = round(float(np.mean([r['confidence'] for r in dialogue_rows])), 4) if dialogue_rows else 0.
            sample['text_kind'] = 'screen_text_not_verified_speech'
        dump(folder/'ocr.raw.json', raw_ocr)
        dialogue = merge_ocr(raw_ocr, 1/manifest['profile']['ocr_hz'])
        for event in dialogue:
            event['end'] = min(event['end'], manifest['duration'])
        dump(folder/'dialogue.json', dialogue)
        write_srt(folder/'dialogue.ocr.srt', dialogue)
        manifest['dialogue_events'] = len(dialogue)
        dump(manifest_path, manifest)
        for p in poses:
            if p.get('overlay') and p['target_present']:
                body = cv2.imread(str(folder/p['body_image']))
                x, y, x2, y2 = p['character_crop_source']
                points = (np.asarray(p['keypoints'])-np.array([x, y]))*np.array([body.shape[1]/(x2-x), body.shape[0]/(y2-y)])
                filtered = draw_pose(body, points, np.array(p['scores']), observable=p['observable'])
                cv2.imwrite(str(folder/p['overlay']), filtered, [cv2.IMWRITE_JPEG_QUALITY, 88])
        movements = movement_intervals(poses)
        dump(folder/'motion_events.json', movements)
        shots = json.loads((folder/'shots.json').read_text())
        masks = json.loads((folder/'masks.json').read_text())
        mask_qc = mask_quality(folder, masks, shots, (video['height'], video['width']))
        dump(folder/'mask_quality.json', mask_qc)
        windows = json.loads((folder/'windows.json').read_text())
        rows = []
        for window in windows:
            window['dialogue_events'] = [d['id'] for d in dialogue if d['start'] < window['end'] and d['end'] > window['start']]
            candidates = [p for p in poses if window['start'] <= p['t'] < window['end']]
            detected = [p for p in candidates if p['target_present']]
            body = float(np.mean([p['body_confidence'] for p in detected])) if detected else 0.
            hands = float(np.mean([min(p['left_hand_confidence'], p['right_hand_confidence']) for p in detected])) if detected else 0.
            face = float(np.mean([p['face_confidence'] for p in detected])) if detected else 0.
            visible_body = float(np.mean([np.mean(p['observable'][:17]) for p in detected])) if detected else 0.
            visible_hands = float(np.mean([np.mean(p['observable'][91:133]) for p in detected])) if detected else 0.
            text = [d for d in dialogue if d['id'] in window['dialogue_events']]
            mask_flags = [m['id'] for m in mask_qc if m['flags'] and window['start'] <= m['t'] < window['end']]
            movement_ids = [m['id'] for m in movements if m['start'] < window['end'] and m['end'] > window['start']]
            row = {'video': folder.name, 'window': window['id'], 'shot': window['shot'],
                'start': window['start'], 'end': window['end'], 'duration': window['duration'],
                'target_fraction': round(len(detected)/max(1, len(candidates)), 3),
                'mean_body_confidence': round(body, 3), 'mean_hand_confidence': round(hands, 3),
                'mean_face_confidence': round(face, 3), 'motion_event_ids': movement_ids,
                'visible_body_fraction': round(visible_body, 3), 'visible_hand_fraction': round(visible_hands, 3),
                'mask_review_ids': mask_flags, 'speaker_candidates': sorted({d['speaker'] for d in text if d.get('speaker')}),
                'ocr_text': '\n'.join(d['text'] for d in text), 'action_name': None, 'context_tags': [],
                'loopable': None, 'review_status': 'unreviewed', 'flags': window['flags'],
                'clip': window.get('clip'), 'sheets': window['sheets']}
            rows.append(row)
            window['motion_event_ids'] = movement_ids
            window['quality'] = {k: row[k] for k in ['target_fraction', 'mean_body_confidence', 'mean_hand_confidence', 'mean_face_confidence', 'visible_body_fraction', 'visible_hand_fraction', 'mask_review_ids']}
        dump(folder/'windows.json', windows)
        dump(folder/'action_candidates.json', rows)
        with (folder/'timeline.csv').open('w', newline='', encoding='utf-8-sig') as f:
            fields = ['video', 'window', 'shot', 'start', 'end', 'duration', 'target_fraction',
                'mean_body_confidence', 'mean_hand_confidence', 'mean_face_confidence', 'visible_body_fraction', 'visible_hand_fraction', 'speaker_candidates',
                'ocr_text', 'motion_event_ids', 'mask_review_ids', 'review_status']
            writer = csv.DictWriter(f, fieldnames=fields, extrasaction='ignore');writer.writeheader()
            writer.writerows(rows)
        summary.append({'video': folder.name, 'movement_intervals': len(movements),
            'candidate_windows': len(rows), 'mask_review_samples': sum(bool(m['flags']) for m in mask_qc)})
    dump(root/'curation_summary.json', summary)
    guide = ['# 動作候選索引', '', '先選完整單鏡頭的區間。以下皆為機器候選，尚未判定動作名稱、情緒或循環性。', '',
             '每支影片的 timeline.csv 可用試算表開啟；action_candidates.json 用於 agent 篩選；motion_events.json 提供上半身、手部、臉部的候選移動起點／高峰／終點。', '',
             '骨架已做近似軀幹平移／尺度正規化後計算移動區間；這無法完整消除透視與鏡頭影響，仍需看原片。', '']
    for item in summary:
        guide.append(f"- `{item['video']}`：{item['candidate_windows']} 個參考區間、{item['movement_intervals']} 段候選局部移動、{item['mask_review_samples']} 張遮罩變化需複核。")
    guide += ['', '建議第一輪只處理一個區間，先看接觸表和短片，再核對對應 motion_events、OCR 與手臉特寫。',
              '不同身形可共用人形骨架動作，但交握、摸臉、拿物與腳底接地須各角色校正。', '']
    (root/'ACTION_INDEX.md').write_text('\n'.join(guide), encoding='utf-8')
    return summary
