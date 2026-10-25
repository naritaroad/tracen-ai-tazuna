"""Geometry and temporal utilities. All persisted coordinates refer to source pixels."""
from __future__ import annotations

import hashlib
import json
import math
import re
from difflib import SequenceMatcher
from pathlib import Path

import cv2
import numpy as np

BODY_NAMES = ['nose', 'left_eye', 'right_eye', 'left_ear', 'right_ear',
              'left_shoulder', 'right_shoulder', 'left_elbow', 'right_elbow',
              'left_wrist', 'right_wrist', 'left_hip', 'right_hip', 'left_knee',
              'right_knee', 'left_ankle', 'right_ankle']
KEYPOINT_NAMES = BODY_NAMES + ['left_big_toe', 'left_small_toe', 'left_heel',
    'right_big_toe', 'right_small_toe', 'right_heel'] + [f'face_{i:02}' for i in range(68)]
for side in ['left', 'right']:
    KEYPOINT_NAMES += [f'{side}_hand_wrist'] + [f'{side}_{finger}_{j}'
        for finger in ['thumb', 'index', 'middle', 'ring', 'pinky'] for j in range(1, 5)]
BODY_EDGES = [(5, 6), (5, 7), (7, 9), (6, 8), (8, 10), (5, 11),
              (6, 12), (11, 12), (11, 13), (13, 15), (12, 14), (14, 16), (0, 1), (0, 2)]
HAND_EDGES = [(base + a, base + b) for base in [91, 112]
              for finger in range(5) for a, b in [(0, 1 + 4 * finger),
              (1 + 4 * finger, 2 + 4 * finger), (2 + 4 * finger, 3 + 4 * finger),
              (3 + 4 * finger, 4 + 4 * finger)]]


def dump(path: Path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + '.tmp')
    temp.write_text(json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False), encoding='utf-8')
    temp.replace(path)


def sha256(path: Path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def box_clip(box, width, height):
    x1, y1, x2, y2 = box
    x1, y1 = max(0, min(width - 1, int(x1))), max(0, min(height - 1, int(y1)))
    return [x1, y1, max(x1 + 1, min(width, int(math.ceil(x2)))),
            max(y1 + 1, min(height, int(math.ceil(y2))))]


def relative_box(region, content):
    x, y, x2, y2 = content
    return [int(x + region[0] * (x2-x)), int(y + region[1] * (y2-y)),
            int(x + region[2] * (x2-x)), int(y + region[3] * (y2-y))]


def expand(box, margin, width, height):
    x1, y1, x2, y2 = box
    return box_clip([x1-(x2-x1)*margin, y1-(y2-y1)*margin,
                     x2+(x2-x1)*margin, y2+(y2-y1)*margin], width, height)


def intersection(a, b):
    return max(0, min(a[2], b[2])-max(a[0], b[0])) * max(0, min(a[3], b[3])-max(a[1], b[1]))


def contains(box, x, y):
    return box[0] <= x < box[2] and box[1] <= y < box[3]


def green_fraction(image):
    if image.size == 0:
        return 0.0
    hsv = cv2.cvtColor(image, cv2.COLOR_BGR2HSV)
    return float(np.mean((hsv[..., 0] >= 32) & (hsv[..., 0] <= 90) &
                         (hsv[..., 1] > 85) & (hsv[..., 2] > 45)))


def visible_content(images):
    """Only remove persistent near-black edge bands, never interior dark clothing."""
    h, w = images[0].shape[:2]
    rows, cols = [], []
    for image in images:
        gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        rows.append(np.mean(gray > 40, axis=1) > .08)
        cols.append(np.mean(gray > 40, axis=0) > .08)
    r = np.where(np.mean(rows, axis=0) >= .5)[0]
    c = np.where(np.mean(cols, axis=0) >= .5)[0]
    return [int(c[0]), int(r[0]), int(c[-1]+1), int(r[-1]+1)] if len(r) and len(c) else [0, 0, w, h]


def text_ui_boxes(image):
    """Conservative white dialogue-panel candidates; inference, not ground truth."""
    h, w = image.shape[:2]
    hsv = cv2.cvtColor(image, cv2.COLOR_BGR2HSV)
    white = ((hsv[..., 1] < 45) & (hsv[..., 2] > 215)).astype('uint8') * 255
    white[:int(h*.45)] = 0
    white = cv2.morphologyEx(white, cv2.MORPH_CLOSE, np.ones((11, 17), np.uint8))
    contours, _ = cv2.findContours(white, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
    boxes = []
    for contour in contours:
        x, y, bw, bh = cv2.boundingRect(contour)
        if bw > w*.32 and bh > h*.045 and bw/bh > 1.7 and bw*bh < h*w*.35:
            boxes.append([x, y, x+bw, y+bh])
    return boxes


def draw_pose(image, points, scores, threshold=.35, observable=None):
    canvas = image.copy()
    valid = scores >= threshold
    if observable is not None:
        valid &= np.asarray(observable, dtype=bool)
    for a, b in BODY_EDGES + HAND_EDGES:
        if valid[a] and valid[b]:
            cv2.line(canvas, tuple(np.round(points[a]).astype(int)),
                     tuple(np.round(points[b]).astype(int)), (70, 240, 255), 2, cv2.LINE_AA)
    for i, ((x, y), score) in enumerate(zip(points, scores)):
        if valid[i]:
            color = (255, 180, 50) if 23 <= i < 91 else (60, 80, 255)
            cv2.circle(canvas, (int(x), int(y)), 2 if 23 <= i < 91 else 3, color, -1)
    return canvas


def normalized_text(text):
    return ''.join(c for c in text if not c.isspace())


def split_ocr_rows(rows, ui, source_height, green_scores=None):
    """Separate nameplates using position as well as color; subtitle backgrounds can be green."""
    green_scores = green_scores or [0.] * len(rows)
    nameplates, dialogue, eligible = [], [], []
    top = min((box[1] for box in ui), default=-100000)
    for row, green in zip(rows, green_scores):
        text = row['text'].strip()
        b = row['bbox_source']; cx, cy = (b[0]+b[2])/2, (b[1]+b[3])/2
        name_zone = bool(ui) and cy <= top+source_height*.01
        name_text = 'たづな' in text or '駿川' in text
        if name_zone and len(text) <= 16 and (name_text or green > .22):
            nameplates.append(row)
            continue
        if re.fullmatch(r'(AUTO|SKIP|MENU|NEXT|Now\s*Loading[.\s]*|オート|メニュー)', text, re.I):
            continue
        if not re.search(r'[ぁ-ゟ゠-ヿ一-鿿A-Za-z0-9]', text):
            continue
        eligible.append(row)
        if ui and any(contains(panel, cx, cy) for panel in ui) and row['confidence'] >= .35:
            dialogue.append(row)
    if not dialogue:
        dialogue = [r for r in eligible if r['confidence'] >= .5 and len(r['text']) >= 3]
    speaker = max(nameplates, key=lambda r: r['confidence'])['text'] if nameplates else None
    return dialogue, speaker


def merge_ocr(samples, sample_step=.5):
    """Merge stable/typewriter variants; retain observations and confidence evidence."""
    events = []
    for sample in samples:
        text = sample.get('dialogue_text', '').strip()
        if not text:
            continue
        norm = normalized_text(text)
        prev = events[-1] if events else None
        old = normalized_text(prev['text']) if prev else ''
        same_speaker = bool(not prev or not sample.get('speaker') or not prev.get('speaker') or sample['speaker'] == prev['speaker'])
        related = bool(prev and same_speaker and sample['t']-prev['last_seen'] <= sample_step*2.2 and
                       (norm.startswith(old) or old.startswith(norm) or
                        SequenceMatcher(None, norm, old).ratio() >= .84))
        if related:
            prev['last_seen'] = sample['t']
            prev['end'] = sample['t'] + sample_step
            prev['observations'].append(sample['id'])
            if len(norm) > len(old) or (len(norm) == len(old) and sample['mean_score'] > prev['confidence']):
                prev['text'], prev['confidence'] = text, sample['mean_score']
                prev['representative'] = sample['id']
        else:
            events.append({'id': f'text_{len(events):04}', 'start': sample['t'],
                'end': sample['t']+sample_step, 'last_seen': sample['t'], 'text': text,
                'confidence': sample['mean_score'], 'representative': sample['id'],
                'observations': [sample['id']], 'status': 'machine_ocr_unreviewed',
                'speaker': sample.get('speaker'), 'translation': None, 'context_tags': []})
    return events


def write_srt(path, events):
    def stamp(t):
        ms = max(0, round(t*1000)); s, ms = divmod(ms, 1000); m, s = divmod(s, 60); h, m = divmod(m, 60)
        return f'{h:02}:{m:02}:{s:02},{ms:03}'
    path.write_text('\n\n'.join(f"{i+1}\n{stamp(e['start'])} --> {stamp(e['end'])}\n{e['text']}"
                              for i, e in enumerate(events))+'\n', encoding='utf-8')


def video_frames(path):
    """Decoded presentation order; retain actual PTS/time_base (including VFR)."""
    import av
    with av.open(str(path)) as container:
        stream = container.streams.video[0]
        stream.thread_type = 'AUTO'
        for index, frame in enumerate(container.decode(stream)):
            if frame.pts is None:
                raise RuntimeError(f'Frame {index} has no presentation timestamp: {path}')
            yield index, float(frame.pts * frame.time_base), int(frame.pts), str(frame.time_base), frame.to_ndarray(format='bgr24')
