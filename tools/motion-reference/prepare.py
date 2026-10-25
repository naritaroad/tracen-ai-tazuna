#!/usr/bin/env python3
"""Build an auditable, local animation reference dataset from source videos."""
from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.metadata
import json
import os
import platform
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import quote

import cv2
import numpy as np
from PIL import Image, ImageDraw, ImageFont, ImageOps

from core import (BODY_NAMES, KEYPOINT_NAMES, box_clip, contains, draw_pose, dump,
                  expand, merge_ocr, relative_box, sha256, text_ui_boxes,
                  video_frames, visible_content, write_srt, split_ocr_rows)

TOOL = Path(__file__).resolve().parent
PROJECT = TOOL.parent.parent
DEFAULT_SOURCE = PROJECT/'example_pose/9001_Hayakawa Tazuna/exsample_source_vedio'
DEFAULT_OUTPUT = DEFAULT_SOURCE.parent/'animation_reference'
SCHEMA = '1.0'


def probe(path):
    return json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-show_format',
        '-show_streams', '-of', 'json', str(path)], text=True))


def save_jpg(path, image, quality=88, max_side=None):
    if image.size == 0:
        raise ValueError(f'Empty crop: {path}')
    if max_side and max(image.shape[:2]) > max_side:
        scale = max_side/max(image.shape[:2])
        image = cv2.resize(image, (round(image.shape[1]*scale), round(image.shape[0]*scale)))
    path.parent.mkdir(parents=True, exist_ok=True)
    if not cv2.imwrite(str(path), image, [cv2.IMWRITE_JPEG_QUALITY, quality]):
        raise OSError(f'Cannot write {path}')


def sample_preview(path, duration):
    cap = cv2.VideoCapture(str(path))
    images = []
    for fraction in [.1, .3, .5, .7, .9]:
        cap.set(cv2.CAP_PROP_POS_MSEC, duration*fraction*1000)
        ok, image = cap.read()
        if ok:
            images.append(image)
    cap.release()
    if not images:
        raise RuntimeError(f'Cannot decode {path}')
    return images


def scan(path, output, profile):
    cached = output/'scan.json'
    if cached.exists():
        return json.loads(cached.read_text())
    info = probe(path)
    stream = next(s for s in info['streams'] if s['codec_type'] == 'video')
    duration = float(info['format']['duration'])
    content = visible_content(sample_preview(path, duration))
    roi = relative_box(profile['target_region'], content)
    records, cuts = [], [0.0]
    previous = None
    origin = None
    started = time.monotonic()
    timestamp_path = output/'frame_timestamps.csv'
    with timestamp_path.open('w', newline='', encoding='utf-8') as f:
        writer = csv.writer(f)
        writer.writerow(['source_frame', 'pts', 'time_base', 'source_timestamp_seconds',
                         'video_time_seconds', 'target_region_pixel_change', 'global_pixel_change'])
        for idx, absolute_t, pts, time_base, image in video_frames(path):
            if origin is None:
                origin = absolute_t
            t = absolute_t-origin
            small = cv2.resize(image, (240, 180))
            gray = cv2.cvtColor(small, cv2.COLOR_BGR2GRAY)
            if previous is None:
                change = motion = 0.
            else:
                diff = cv2.absdiff(gray, previous)
                change = float(np.mean(diff))/255
                rx, ry, rx2, ry2 = [round(v*s) for v, s in zip(roi, [240/stream['width'], 180/stream['height']]*2)]
                motion = float(np.mean(diff[ry:ry2, rx:rx2]))/255
                if change > profile['shot_cut_threshold'] and t-cuts[-1] > .35:
                    cuts.append(round(t, 6))
            previous = gray
            writer.writerow([idx, pts, time_base, f'{absolute_t:.9f}', f'{t:.9f}', f'{motion:.6f}', f'{change:.6f}'])
            if idx % 3 == 0 or change > .1:
                records.append({'frame': idx, 't': round(t, 6), 'pixel_change': round(motion, 6),
                                'global_change': round(change, 6)})
    last_t = t
    end = max(duration, last_t+1/float(eval_fraction(stream.get('avg_frame_rate', '60/1'))))
    shots = [{'id': f'shot_{i:03}', 'start': start, 'end': cuts[i+1] if i+1 < len(cuts) else end}
             for i, start in enumerate(cuts)]
    result = {'schema_version': SCHEMA, 'metadata': info, 'sha256': sha256(path),
              'duration': duration, 'source_time_origin': origin, 'decoded_frames': idx+1,
              'last_decoded_t': last_t, 'content_box': content, 'target_region_source': roi,
              'shots': shots, 'motion_samples': records, 'scan_seconds': time.monotonic()-started}
    dump(cached, result)
    print(f'{path.name}: decoded {idx+1} frames; {len(shots)} shots', flush=True)
    return result


def eval_fraction(value):
    from fractions import Fraction
    return Fraction(value) if value != '0/0' else 60


def ocr_sample(image, engine, profile, content, ui, sample_id, t, output):
    region = relative_box(profile['text_region'], content)
    if ui:
        # Include the nameplate just above the dialogue panel.
        region = box_clip([min(b[0] for b in ui)-12, min(b[1] for b in ui)-90,
                           max(b[2] for b in ui)+12, max(b[3] for b in ui)+12], image.shape[1], image.shape[0])
    rows = engine(image, region)
    green_scores = []
    for row in rows:
        bx, by, bx2, by2 = box_clip(row['bbox_source'], image.shape[1], image.shape[0])
        patch = image[by:by2, bx:bx2]
        from core import green_fraction
        green_scores.append(green_fraction(patch))
    dialogue_rows, speaker = split_ocr_rows(rows, ui, image.shape[0], green_scores)
    text = '\n'.join(r['text'] for r in dialogue_rows)
    scores = [r['confidence'] for r in dialogue_rows]
    record = {'id': sample_id, 't': round(t, 6), 'region_source': region, 'lines': rows,
              'dialogue_text': text, 'speaker': speaker, 'mean_score': round(float(np.mean(scores)), 4) if scores else 0.,
              'timing_uncertainty_seconds': 1/profile['ocr_hz'], 'review_required': True,
              'image': f'text/{sample_id}.jpg'}
    x, y, x2, y2 = region
    save_jpg(output/record['image'], image[y:y2, x:x2], quality=93)
    return record


def infer(path, output, scan_data, profile, models, limit=None):
    pose_engine, segmenter, ocr_engine = models
    pose_engine.reset()
    poses, samples, masks = [], [], []
    shot_index = 0
    next_pose = next_mask = next_ocr = next_overview = 0.
    cache = output/'_frames'
    cache.mkdir(exist_ok=True)
    ui = []
    content = scan_data['content_box']
    region = scan_data['target_region_source']
    origin = scan_data['source_time_origin']
    start = time.monotonic()
    last_log = -100
    raw_path = output/'poses.raw.jsonl'
    with raw_path.open('w', encoding='utf-8') as raw:
        for idx, absolute_t, pts, time_base, image in video_frames(path):
            t = absolute_t-origin
            if limit and t > limit:
                break
            while shot_index+1 < len(scan_data['shots']) and t >= scan_data['shots'][shot_index+1]['start']:
                shot_index += 1
                pose_engine.reset()
            shot_id = scan_data['shots'][shot_index]['id']
            if t+1e-6 < next_pose:
                continue
            while next_pose <= t+1e-6:
                next_pose += 1/profile['pose_hz']
            sample_id = f'f{idx:06}_t{round(t*1000):07}'
            if profile.get('dialogue_panel_region'):
                ui = [relative_box(profile['dialogue_panel_region'], content)]
            elif profile.get('detect_dialogue_ui', True):
                ui = text_ui_boxes(image)
            else:
                ui = []
            result = pose_engine(image, t, region)
            record = {'id': sample_id, 'frame': idx, 't': round(t, 6), 'pts': pts, 'time_base': time_base,
                      'shot': shot_id, 'target_present': result is not None,
                      'ui_occlusion_candidates': ui, 'ui_bounds_source': 'profile_estimate' if profile.get('dialogue_panel_region') else 'image_heuristic',
                      'coordinate_space': 'source_pixels',
                      'source': 'estimated_2d_not_ground_truth', 'flags': []}
            if result is not None:
                points, scores, target_box, green = result
                in_frame = (points[:, 0] >= 0) & (points[:, 0] < image.shape[1]) & (points[:, 1] >= 0) & (points[:, 1] < image.shape[0])
                occluded = np.array([any(contains(b, x, y) for b in ui) for x, y in points])
                observable = (scores >= .35) & in_frame & ~occluded
                record.update({'keypoints': np.round(points, 2).tolist(), 'scores': np.round(scores, 4).tolist(),
                    'observable': observable.tolist(), 'target_box': target_box,
                    'green_uniform_fraction': round(green, 4), 'body_confidence': round(float(np.mean(scores[:17])), 4),
                    'face_confidence': round(float(np.mean(scores[23:91])), 4),
                    'left_hand_confidence': round(float(np.mean(scores[91:112])), 4),
                    'right_hand_confidence': round(float(np.mean(scores[112:133])), 4)})
                if np.mean(observable[:17]) < .55:
                    record['flags'].append('body_partly_unobservable')
                if any(occluded):
                    record['flags'].append('dialogue_ui_occlusion_candidate')
                if min(record['left_hand_confidence'], record['right_hand_confidence']) < .4:
                    record['flags'].append('hand_detail_unreliable')
                if record['face_confidence'] < .5:
                    record['flags'].append('face_detail_unreliable')
                if target_box[0] <= 2 or target_box[2] >= image.shape[1]-2 or target_box[3] >= image.shape[0]-2:
                    record['flags'].append('body_truncated_by_frame')
            else:
                record['flags'].append('target_not_detected')
            save_jpg(cache/(sample_id+'.jpg'), image, quality=92)
            if t+1e-6 >= next_overview:
                while next_overview <= t+1e-6:
                    next_overview += 1/profile['overview_hz']
                record['overview'] = f'overview/{sample_id}.jpg'
                x, y, x2, y2 = content
                save_jpg(output/record['overview'], image[y:y2, x:x2], max_side=1280)
            if t+1e-6 >= next_ocr:
                while next_ocr <= t+1e-6:
                    next_ocr += 1/profile['ocr_hz']
                samples.append(ocr_sample(image, ocr_engine, profile, content, ui, sample_id, t, output))
            if t+1e-6 >= next_mask:
                while next_mask <= t+1e-6:
                    next_mask += 1/profile['mask_hz']
                if result is not None:
                    # Apply anime segmentation to the chosen subject ROI, not both characters.
                    bounds = expand(record['target_box'], .06, image.shape[1], image.shape[0])
                    bounds[0] = max(region[0], bounds[0]); bounds[2] = min(region[2], bounds[2])
                    x, y, x2, y2 = bounds
                    alpha = segmenter(image[y:y2, x:x2])
                    (output/'masks/raw').mkdir(parents=True, exist_ok=True)
                    cv2.imwrite(str(output/'masks/raw'/(sample_id+'.png')), (alpha*255).astype('uint8'))
                    for b in ui:
                        bx, by, bx2, by2 = box_clip([b[0]-x, b[1]-y, b[2]-x, b[3]-y], x2-x, y2-y)
                        if b[2] > x and b[0] < x2 and b[3] > y and b[1] < y2:
                            alpha[by:by2, bx:bx2] = 0
                    mask = (alpha*255).astype('uint8')
                    (output/'masks').mkdir(exist_ok=True)
                    cv2.imwrite(str(output/'masks'/(sample_id+'.png')), mask)
                    foreground = image[y:y2, x:x2].copy()
                    bg = np.full_like(foreground, 42)
                    preview = (foreground*alpha[..., None]+bg*(1-alpha[..., None])).astype('uint8')
                    save_jpg(output/'masked'/f'{sample_id}.jpg', preview)
                    masks.append({'id': sample_id, 't': round(t, 6), 'bbox_source': bounds,
                                  'path': f'masks/{sample_id}.png', 'preview': f'masked/{sample_id}.jpg',
                                  'raw_path': f'masks/raw/{sample_id}.png',
                                  'foreground_fraction': round(float(np.mean(alpha > .5)), 4),
                                  'quality': 'machine_mask_unreviewed',
                                  'ui_removed': bool(ui), 'hidden_body_not_reconstructed': True})
                    record['mask_preview'] = f'masked/{sample_id}.jpg'
            poses.append(record)
            raw.write(json.dumps(record, ensure_ascii=False, allow_nan=False)+'\n')
            raw.flush()
            if t-last_log >= 10:
                elapsed = time.monotonic()-start
                print(f'{path.stem}: {t:.1f}s / {scan_data["duration"]:.1f}s; {len(poses)} pose samples; elapsed {elapsed:.0f}s', flush=True)
                last_log = t
    dump(output/'ocr.raw.json', samples)
    dump(output/'masks.json', masks)
    return poses, samples, masks


def enrich_images(output, poses, scan_data):
    shots = scan_data['shots']
    for shot in shots:
        present = [p for p in poses if p['shot'] == shot['id'] and p['target_present']]
        if present:
            boxes = np.array([p['target_box'] for p in present])
            union = [boxes[:, 0].min(), boxes[:, 1].min(), boxes[:, 2].max(), boxes[:, 3].max()]
            stream = next(s for s in scan_data['metadata']['streams'] if s['codec_type'] == 'video')
            fixed = expand(union, .09, stream['width'], stream['height'])
            # Keep a fixed crop within this shot; do not rescale each individual pose.
            region = scan_data['target_region_source']
            fixed[0] = max(region[0], fixed[0]); fixed[2] = min(region[2], fixed[2])
            shot['character_crop_source'] = fixed
        else:
            shot['character_crop_source'] = scan_data['target_region_source']
        shot['target_detection_fraction'] = round(len(present)/max(1, sum(p['shot'] == shot['id'] for p in poses)), 4)
    by_shot = {s['id']: s for s in shots}
    for p in poses:
        image_path = output/'_frames'/(p['id']+'.jpg')
        image = cv2.imread(str(image_path))
        crop = by_shot[p['shot']]['character_crop_source']
        x, y, x2, y2 = crop
        p['character_crop_source'] = crop
        p['body_image'] = f'body/{p["id"]}.jpg'
        save_jpg(output/p['body_image'], image[y:y2, x:x2], max_side=1100)
        if p['target_present']:
            points, scores = np.array(p['keypoints']), np.array(p['scores'])
            if 'overview' in p or p.get('mask_preview'):
                overlay = draw_pose(image, points, scores, observable=p['observable'])
                p['overlay'] = f'overlay/{p["id"]}.jpg'
                save_jpg(output/p['overlay'], overlay[y:y2, x:x2], max_side=1100)
                p['details'] = {}
                for part, begin, end in [('face', 23, 91), ('left_hand', 91, 112), ('right_hand', 112, 133)]:
                    valid = (scores[begin:end] >= .4) & np.array(p['observable'][begin:end])
                    pts = points[begin:end][valid]
                    if len(pts) < (12 if part == 'face' else 8):
                        continue
                    bounds = [pts[:, 0].min(), pts[:, 1].min(), pts[:, 0].max()+1, pts[:, 1].max()+1]
                    bounds = expand(bounds, .5, image.shape[1], image.shape[0])
                    dx, dy, dx2, dy2 = bounds
                    if min(dx2-dx, dy2-dy) < 18:
                        continue
                    rel = f'details/{part}/{p["id"]}.jpg'
                    save_jpg(output/rel, image[dy:dy2, dx:dx2], quality=95)
                    p['details'][part] = {'image': rel, 'bbox_source': bounds, 'estimated_location': True}
        image_path.unlink()
    (output/'_frames').rmdir()


def temporal_metrics(poses):
    prev = None
    for p in poses:
        if not p['target_present']:
            prev = None
            continue
        if prev and p['shot'] == prev['shot'] and 0 < p['t']-prev['t'] < .4:
            a, b = np.array(p['keypoints']), np.array(prev['keypoints'])
            valid = np.array(p['observable']) & np.array(prev['observable'])
            height = max(1, p['target_box'][3]-p['target_box'][1])
            speeds = np.linalg.norm(a-b, axis=1)/(p['t']-prev['t'])/height
            p['motion'] = {}
            for part, start, end in [('body', 0, 17), ('face', 23, 91), ('left_hand', 91, 112), ('right_hand', 112, 133)]:
                part_valid = valid[start:end]
                p['motion'][part+'_height_per_second'] = round(float(np.median(speeds[start:end][part_valid])), 5) if np.any(part_valid) else None
            p['motion']['interpretation'] = '2d_image_motion_includes_camera_and_estimation_noise'
        prev = p


def contactsheet(output, records, name, cols=4):
    font_path = '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc'
    font = ImageFont.truetype(font_path, 15) if Path(font_path).exists() else ImageFont.load_default()
    sheets = []
    for offset in range(0, len(records), 16):
        batch = records[offset:offset+16]
        canvas = Image.new('RGB', (cols*300, ((len(batch)+cols-1)//cols)*330), '#191f26')
        draw = ImageDraw.Draw(canvas)
        for i, record in enumerate(batch):
            im = Image.open(output/record['body_image']).convert('RGB')
            im.thumbnail((292, 295))
            x, y = (i % cols)*300, (i//cols)*330
            canvas.paste(im, (x+(300-im.width)//2, y))
            draw.text((x+5, y+298), f'{record["t"]:.3f}s  {record["shot"]}', font=font, fill='white')
            if record['flags']:
                draw.text((x+5, y+316), '需複核' if record['target_present'] else '未偵測到目標', font=font, fill='#f0bc72')
        rel = f'sheets/{name}_{offset//16:03}.jpg'
        (output/'sheets').mkdir(exist_ok=True)
        canvas.save(output/rel, quality=90)
        sheets.append(rel)
    return sheets


def make_windows(poses, shots, events, duration, max_window):
    windows = []
    for shot in shots:
        if shot['target_detection_fraction'] < .15:
            continue
        a, end = shot['start'], min(shot['end'], duration)
        while a < end-.15:
            b = min(end, a+max_window)
            # Prefer an observed dialogue boundary near the end of long windows.
            candidates = [e['start'] for e in events if a+max_window*.6 < e['start'] <= b]
            if b < end and candidates:
                b = candidates[-1]
            selected = [p for p in poses if a <= p['t'] < b and p['target_present']]
            if selected:
                overview = [p for p in selected if 'overview' in p]
                peaks = sorted(selected, key=lambda p: (p.get('motion', {}).get('body_height_per_second') or 0), reverse=True)
                key_ids = [p['id'] for p in overview]
                for p in peaks:
                    if all(abs(p['t']-q['t']) > .3 for q in selected if q['id'] in key_ids):
                        key_ids.append(p['id'])
                    if len(key_ids) >= 16:
                        break
                windows.append({'id': f'window_{len(windows):03}', 'shot': shot['id'], 'start': round(a, 6), 'end': round(b, 6),
                    'duration': round(b-a, 6), 'status': 'candidate_window_not_semantically_verified_action',
                    'name': None, 'context_tags': [], 'loopable': None, 'animation_ready': False,
                    'keyframes': sorted(key_ids, key=lambda i: next(p['t'] for p in selected if p['id'] == i)),
                    'dialogue_events': [e['id'] for e in events if e['start'] < b and e['end'] > a],
                    'flags': sorted({f for p in selected for f in p['flags']}),
                    'suggested_review': ['timing', 'weight_shift', 'hands', 'face', 'entry_exit', 'camera_and_ui_occlusion']})
            a = b
    return windows


def export_clip(source, dest, start, end):
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists():
        return
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin', '-y',
        '-ss', f'{start:.6f}', '-i', str(source), '-t', f'{end-start:.6f}',
        '-map', '0:v:0', '-map', '0:a:0?', '-vf', "scale='min(1080,iw)':-2",
        '-c:v', 'libx264', '-threads', '2', '-preset', 'veryfast', '-crf', '20',
        '-c:a', 'aac', '-b:a', '128k', '-movflags', '+faststart', str(dest)], check=True)


def audio_analysis(source, output, scan_data):
    if not any(s['codec_type'] == 'audio' for s in scan_data['metadata']['streams']):
        return {'present': False}
    audio_path = output/'audio.m4a'
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin', '-y', '-i', str(source),
                    '-vn', '-c:a', 'aac', '-b:a', '128k', str(audio_path)], check=True)
    pcm = subprocess.check_output(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin',
        '-i', str(source), '-vn', '-ac', '1', '-ar', '16000', '-f', 'f32le', 'pipe:1'])
    samples = np.frombuffer(pcm, dtype='<f4')
    rows = []
    for i in range(0, len(samples), 320):
        chunk = samples[i:i+320]
        rows.append([round(i/16000, 4), round(float(np.sqrt(np.mean(chunk**2))), 6)])
    dump(output/'audio_envelope.json', {'columns': ['audio_time_seconds', 'rms'], 'step': .02,
        'interpretation': 'energy_only_not_phoneme_or_speech_alignment', 'samples': rows})
    return {'present': True, 'audio': 'audio.m4a', 'envelope': 'audio_envelope.json',
            'speech_transcription': 'not_performed', 'lip_sync_alignment': 'not_performed',
            'note': 'Retains all audio, including music/effects. OCR timestamps are visual text timings.'}


def provenance(models):
    from inference import RUNTIME
    packages = {}
    for name in ['rapidocr', 'rtmlib', 'onnxruntime', 'opencv-python-headless', 'av', 'numpy', 'Pillow', 'scipy']:
        packages[name] = importlib.metadata.version(name)
    hashes = []
    expected = json.loads((TOOL/'models.lock.json').read_text())['expected_artifacts']
    roots = [RUNTIME/'models', Path.home()/'.cache/rtmlib', Path(sys.prefix)/'lib/python3.12/site-packages/rapidocr']
    seen = set()
    for root in roots:
        if root.exists():
            for file in root.rglob('*.onnx'):
                if file.resolve() not in seen and file.name in expected:
                    actual = sha256(file)
                    if actual != expected[file.name]:
                        raise RuntimeError(f'Model checksum mismatch: {file.name}')
                    hashes.append({'file': file.name, 'bytes': file.stat().st_size, 'sha256': actual})
                    seen.add(file.resolve())
    if {r['file'] for r in hashes} != set(expected):
        raise RuntimeError('Not all expected inference model files were found')
    return {'schema_version': SCHEMA, 'checked_at': '2026-10-25', 'python': sys.version,
            'platform': platform.platform(), 'packages': packages, 'models': hashes,
            'providers': ['CPUExecutionProvider'], 'external_inference_calls': False,
            'source_videos_modified': False, 'pose_model': 'RTMW Wholebody balanced, COCO-WholeBody 133',
            'ocr': 'RapidOCR Japanese PP-OCRv4 mobile recognition + PP-OCRv5 mobile detection',
            'mask': 'SkyTNT ISNetIS, native model input resolution; ROI target selection; heuristic UI exclusion'}


def process(source, output, profile, models, args):
    output.mkdir(parents=True, exist_ok=True)
    expected = {'source_sha256': sha256(source), 'profile': profile, 'schema_version': SCHEMA}
    complete = output/'manifest.json'
    if complete.exists() and not args.force:
        existing = json.loads(complete.read_text())
        if existing.get('input_signature') == expected:
            print(f'{source.name}: reusing verified input signature', flush=True)
            return existing
        raise RuntimeError(f'Existing output differs from inputs. Use a new --output or --force: {output}')
    scan_data = scan(source, output, profile)
    if scan_data['sha256'] != expected['source_sha256']:
        raise RuntimeError('Cached scan is from a different source; use a new output directory')
    poses, samples, masks = infer(source, output, scan_data, profile, models, args.limit)
    enrich_images(output, poses, scan_data)
    temporal_metrics(poses)
    events = merge_ocr(samples, 1/profile['ocr_hz'])
    duration = min(scan_data['duration'], args.limit) if args.limit else scan_data['duration']
    for e in events:
        e['end'] = min(e['end'], duration)
    windows = make_windows(poses, scan_data['shots'], events, duration, profile['max_window_seconds'])
    record_by_id = {p['id']: p for p in poses}
    for window in windows:
        keyframes = [record_by_id[k] for k in window['keyframes']]
        window['sheets'] = contactsheet(output, keyframes, window['id'])
        if not args.no_clips:
            window['clip'] = f'clips/{window["id"]}.mp4'
            export_clip(source, output/window['clip'], window['start'], window['end'])
    overview_sheets = contactsheet(output, [p for p in poses if 'overview' in p], 'overview')
    with (output/'poses.jsonl').open('w', encoding='utf-8') as f:
        for p in poses:
            f.write(json.dumps(p, ensure_ascii=False, allow_nan=False)+'\n')
    dump(output/'keypoint_schema.json', {'name': 'COCO-WholeBody-133', 'names': KEYPOINT_NAMES,
        'source': 'estimated 2D image landmarks, not Blender bones or recovered 3D truth',
        'confidence_threshold': .35, 'observable': 'confidence>=.35 and in frame and outside heuristic UI panels'})
    dump(output/'shots.json', scan_data['shots'])
    dump(output/'dialogue.json', events)
    dump(output/'windows.json', windows)
    write_srt(output/'dialogue.ocr.srt', events)
    audio = audio_analysis(source, output, scan_data)
    manifest = {'schema_version': SCHEMA, 'input_signature': expected, 'source_name': source.name,
        'source_relative_to_project': source.relative_to(PROJECT).as_posix(), 'duration': duration,
        'source_duration': scan_data['duration'], 'full_source_processed': not bool(args.limit),
        'decoded_frames': scan_data['decoded_frames'], 'pose_samples': len(poses),
        'target_detected_samples': sum(p['target_present'] for p in poses), 'mask_samples': len(masks),
        'ocr_samples': len(samples), 'dialogue_events': len(events), 'windows': len(windows),
        'shots': len(scan_data['shots']), 'overview_sheets': overview_sheets, 'audio': audio,
        'profile': profile, 'files': {'poses': 'poses.jsonl', 'dialogue': 'dialogue.json',
        'windows': 'windows.json', 'masks': 'masks.json', 'timestamps': 'frame_timestamps.csv'},
        'limitations': ['No inferred pose is exact 3D motion.', 'UI and occluded joints remain unknown.',
            'Keypoint count is not a measure of accuracy.', 'Window labels and context tags require review.',
            'OCR is unreviewed original-language text; visual timing is not speech alignment.',
            'Masks can include another subject or omit thin hair; manually inspect.',
            'VMD input was not supplied; no VMD/VRMA animation has been authored in this stage.']}
    dump(complete, manifest)
    print(f'{source.name}: complete: {len(poses)} poses, {len(masks)} masks, {len(events)} OCR events, {len(windows)} windows', flush=True)
    return manifest


def load_models():
    from inference import OCR, PoseEstimator, Segmenter
    print('Loading local pose model...', flush=True)
    pose = PoseEstimator()
    print('Loading local anime segmentation...', flush=True)
    segmenter = Segmenter()
    print('Loading Japanese OCR...', flush=True)
    ocr = OCR()
    return pose, segmenter, ocr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['models', 'build', 'curate', 'report', 'verify', 'extract'])
    parser.add_argument('--source', type=Path, default=DEFAULT_SOURCE)
    parser.add_argument('--output', type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument('--profiles', type=Path, default=TOOL/'profiles.json')
    parser.add_argument('--video', help='Optional filename stem')
    parser.add_argument('--videos', help='Optional comma-separated filename stems')
    parser.add_argument('--limit', type=float, help='Pilot only; limit inference seconds (marked incomplete)')
    parser.add_argument('--force', action='store_true')
    parser.add_argument('--no-clips', action='store_true')
    parser.add_argument('--start', type=float, default=0)
    parser.add_argument('--end', type=float, default=1)
    args = parser.parse_args()
    if args.limit is not None and args.limit <= 0:
        parser.error('--limit must be positive')
    cv2.setNumThreads(2)
    args.output.mkdir(parents=True, exist_ok=True)
    if args.command == 'models':
        models = load_models()
        warmup = np.full((120, 500, 3), 255, np.uint8)
        cv2.putText(warmup, 'TEST TEXT', (20, 80), cv2.FONT_HERSHEY_SIMPLEX, 1.5, (0, 0, 0), 3)
        models[2](warmup, [0, 0, 500, 120])
        dump(args.output/'provenance.json', provenance(models))
        return
    if args.command == 'extract':
        if not args.video or args.end <= args.start:
            parser.error('extract requires --video and --end > --start')
        source = args.source/(args.video+'.mp4')
        dest = args.output/args.video/f'detail_{args.start:.3f}_{args.end:.3f}'
        dest.mkdir(parents=True, exist_ok=True)
        # Full original frame rate, original PTS stored in a sidecar.
        rows = []
        origin = None
        for idx, ts, pts, tb, image in video_frames(source):
            origin = ts if origin is None else origin
            t = ts-origin
            if t > args.end:
                break
            if t >= args.start:
                file = f'f{idx:06}_t{round(t*1000):07}.png'
                cv2.imwrite(str(dest/file), image)
                rows.append({'file': file, 'frame': idx, 't': t, 'pts': pts, 'time_base': tb})
        dump(dest/'frames.json', rows)
        export_clip(source, dest/'source_clip.mp4', args.start, args.end)
        print(dest)
        return
    if args.command == 'build':
        settings = json.loads(args.profiles.read_text())
        paths = sorted(args.source.glob('*.mp4'))
        if args.video:
            paths = [p for p in paths if p.stem == args.video]
        if args.videos:
            chosen = set(args.videos.split(','))
            paths = [p for p in paths if p.stem in chosen]
        if not paths:
            raise FileNotFoundError('No matching input videos')
        models = load_models()
        for source in paths:
            profile = settings['defaults'] | settings['videos'].get(source.stem, {})
            process(source, args.output/source.stem, profile, models, args)
        dump(args.output/'provenance.json', provenance(models))
    if args.command in ['build', 'curate']:
        from curate import curate_dataset
        curate_dataset(args.output)
    if args.command in ['build', 'curate', 'report']:
        from report import build_report
        build_report(args.output, PROJECT)
    if args.command in ['build', 'verify']:
        from verify import verify_dataset
        result = verify_dataset(args.output, PROJECT)
        dump(args.output/'verification.json', result)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        if not result['passed']:
            raise SystemExit(1)


if __name__ == '__main__':
    main()
