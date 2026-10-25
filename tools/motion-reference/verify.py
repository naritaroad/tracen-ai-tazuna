"""Validate real artifacts and source identities, not just successful tool exits."""
import csv
import json
from pathlib import Path

from PIL import Image
from core import sha256


def verify_dataset(output, project):
    errors, videos, image_count = [], [], 0
    for path in sorted(output.glob('*/manifest.json')):
        folder = path.parent
        manifest = json.loads(path.read_text())
        if not isinstance(manifest, dict) or 'input_signature' not in manifest:
            continue
        source = project/manifest['source_relative_to_project']
        source_matches = sha256(source) == manifest['input_signature']['source_sha256']
        if not source_matches:
            errors.append(f'{folder.name}: source identity changed')
        rows = list(csv.DictReader((folder/'frame_timestamps.csv').open()))
        times = [float(r['video_time_seconds']) for r in rows]
        if len(times) != manifest['decoded_frames'] or any(b < a for a, b in zip(times, times[1:])):
            errors.append(f'{folder.name}: source timestamp count/order')
        if manifest['full_source_processed'] and manifest['source_duration']-times[-1] > .2:
            errors.append(f'{folder.name}: decoded video ended too early')
        poses = [json.loads(line) for line in (folder/'poses.jsonl').read_text().splitlines()]
        if len(poses) != manifest['pose_samples']:
            errors.append(f'{folder.name}: pose sample count')
        if manifest['full_source_processed'] and poses and times[-1]-poses[-1]['t'] > .2:
            errors.append(f'{folder.name}: pose sampling ended too early')
        masks = json.loads((folder/'masks.json').read_text())
        ocr = json.loads((folder/'ocr.raw.json').read_text())
        dialogue = json.loads((folder/'dialogue.json').read_text())
        if len(masks) != manifest['mask_samples'] or len(ocr) != manifest['ocr_samples'] or len(dialogue) != manifest['dialogue_events']:
            errors.append(f'{folder.name}: mask/OCR event count')
        pose_ids = {p['id'] for p in poses}
        event_ids = {e['id'] for e in dialogue}
        for sample in ocr:
            if sample['id'] not in pose_ids or not (folder/sample['image']).is_file():
                errors.append(f'{folder.name}: OCR source linkage')
        for mask in masks:
            if mask['id'] not in pose_ids:
                errors.append(f'{folder.name}: mask source linkage')
            with Image.open(folder/mask['path']) as im:
                x, y, x2, y2 = mask['bbox_source']
                if im.size != (x2-x, y2-y):
                    errors.append(f'{folder.name}/{mask["id"]}: mask source transform')
        for p in poses:
            if p['target_present'] and not (len(p['keypoints']) == len(p['scores']) == len(p['observable']) == 133):
                errors.append(f'{folder.name}/{p["id"]}: keypoint shape')
            if p['target_present'] and not all(0 <= s <= 1.5 for s in p['scores']):
                errors.append(f'{folder.name}/{p["id"]}: invalid confidence')
            frame = int(p['frame'])
            if frame >= len(times) or abs(p['t']-times[frame]) > 2e-6:
                errors.append(f'{folder.name}/{p["id"]}: source frame time mismatch')
            for key in ['body_image', 'overview', 'overlay', 'mask_preview']:
                if p.get(key) and not (folder/p[key]).is_file():
                    errors.append(f'{folder.name}: missing {p[key]}')
            for detail in p.get('details', {}).values():
                if not (folder/detail['image']).is_file():
                    errors.append(f'{folder.name}: missing detail')
        windows = json.loads((folder/'windows.json').read_text())
        for w in windows:
            if any(k not in pose_ids for k in w['keyframes']) or any(e not in event_ids for e in w['dialogue_events']):
                errors.append(f'{folder.name}: invalid window references {w["id"]}')
            if not (0 <= w['start'] < w['end'] <= manifest['duration']+.001):
                errors.append(f'{folder.name}: invalid window {w["id"]}')
            if w.get('clip'):
                import subprocess
                d = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-show_format', '-of', 'json', str(folder/w['clip'])]))
                if abs(float(d['format']['duration'])-w['duration']) > .2:
                    errors.append(f'{folder.name}: clip duration {w["id"]}')
            for s in w['sheets']:
                if not (folder/s).is_file():
                    errors.append(f'{folder.name}: missing sheet')
        for image in list(folder.rglob('*.jpg'))+list(folder.rglob('*.png')):
            try:
                with Image.open(image) as im:
                    im.load()
                image_count += 1
            except Exception as exc:
                errors.append(f'{folder.name}: unreadable image {image.name}: {exc}')
        videos.append({'video': folder.name, 'full_source_processed': manifest['full_source_processed'],
            'source_sha256_verified': source_matches, 'decoded_frames': len(times), 'pose_samples': len(poses),
            'detected_fraction': round(sum(p['target_present'] for p in poses)/max(1, len(poses)), 4),
            'windows': len(windows), 'machine_predictions_require_review': True})
    if not videos:
        errors.append('No completed video manifests')
    decoder_audit = json.loads((output/'decoder_audit.json').read_text()) if (output/'decoder_audit.json').exists() else None
    if decoder_audit and any(row['returncode'] != 0 or row['decoder_messages'] for row in decoder_audit):
        errors.append('Independent FFmpeg source decode reported errors; inspect decoder_audit.json')
    return {'passed': not errors, 'checked_at': '2026-10-25', 'videos': videos,
            'independent_ffmpeg_decode': decoder_audit,
            'images_decoded_for_integrity': image_count, 'errors': errors,
            'verified_scope': ['source hashes', 'decoded timestamps', 'sample provenance', 'image integrity', 'clip durations', 'required references'],
            'not_verified': ['every OCR character', 'every pose landmark', 'every mask boundary', '3D animation equivalence', 'semantic action completeness']}
