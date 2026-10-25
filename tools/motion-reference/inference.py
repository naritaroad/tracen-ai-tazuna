"""Small adapters keep model dependencies out of the dataset format."""
from __future__ import annotations

import json
import os
import urllib.request
from pathlib import Path

import cv2
import numpy as np
import onnxruntime as ort

from core import box_clip, expand, green_fraction, sha256, contains

TOOL = Path(__file__).resolve().parent
RUNTIME = Path(os.environ.get('TRACEN_MOTION_RUNTIME', Path.home()/'.cache/tracen-motion-ref'))


def session_options():
    options = ort.SessionOptions()
    options.intra_op_num_threads = int(os.environ.get('OMP_NUM_THREADS', '4'))
    options.inter_op_num_threads = 1
    options.log_severity_level = 3
    return options


def model_path():
    spec = json.loads((TOOL/'models.lock.json').read_text())['anime_segmentation']
    path = RUNTIME/'models'/spec['file']
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists() or path.stat().st_size != spec['bytes'] or sha256(path) != spec['sha256']:
        temp = path.with_suffix('.download')
        print('Downloading pinned anime segmentation model', flush=True)
        urllib.request.urlretrieve(spec['url'], temp)
        if sha256(temp) != spec['sha256']:
            raise RuntimeError('Anime segmentation checksum mismatch')
        temp.replace(path)
    return path


class Segmenter:
    def __init__(self):
        self.path = model_path()
        self.session = ort.InferenceSession(str(self.path), sess_options=session_options(), providers=['CPUExecutionProvider'])
        self.input = self.session.get_inputs()[0].name
        shape = self.session.get_inputs()[0].shape
        self.size = int(shape[2]) if isinstance(shape[2], int) else 1024

    def __call__(self, image):
        h0, w0 = image.shape[:2]
        s = self.size
        h, w = (s, int(s*w0/h0)) if h0 > w0 else (int(s*h0/w0), s)
        ph, pw = s-h, s-w
        rgb = cv2.cvtColor(image, cv2.COLOR_BGR2RGB).astype('float32') / 255
        canvas = np.zeros((s, s, 3), np.float32)
        canvas[ph//2:ph//2+h, pw//2:pw//2+w] = cv2.resize(rgb, (w, h))
        prediction = self.session.run(None, {self.input: canvas.transpose(2, 0, 1)[None]})[0][0, 0]
        prediction = prediction[ph//2:ph//2+h, pw//2:pw//2+w]
        return cv2.resize(prediction, (w0, h0)).clip(0, 1)


class PoseEstimator:
    def __init__(self):
        # rtmlib does not expose thread settings. Supply SessionOptions only while
        # constructing its sessions, then restore the public API immediately.
        from rtmlib import Wholebody
        original = ort.InferenceSession
        def limited(*args, **kwargs):
            kwargs.setdefault('sess_options', session_options())
            return original(*args, **kwargs)
        ort.InferenceSession = limited
        try:
            self.model = Wholebody(mode='balanced', to_openpose=False, backend='onnxruntime', device='cpu')
        finally:
            ort.InferenceSession = original
        self.tracked = None
        self.last_detection = -100
        self.last_green = 0

    def reset(self):
        self.tracked = None
        self.last_detection = -100

    def __call__(self, image, t, region):
        h, w = image.shape[:2]
        rx, ry, rx2, ry2 = region
        roi = image[ry:ry2, rx:rx2]
        if self.tracked is None or t-self.last_detection >= .45:
            boxes = self.model.det_model(roi)
            candidates = []
            for box in boxes:
                b = box_clip(box[:4], rx2-rx, ry2-ry)
                x, y, x2, y2 = b
                # The central upper body is more selective than background greenery.
                torso = roi[int(y+(y2-y)*.15):int(y+(y2-y)*.65), x:x2]
                green = green_fraction(torso)
                if green < .025:
                    continue
                source_box = [x+rx, y+ry, x2+rx, y2+ry]
                continuity = 0
                if self.tracked:
                    ac = np.array([(source_box[0]+source_box[2])/2, (source_box[1]+source_box[3])/2])
                    bc = np.array([(self.tracked[0]+self.tracked[2])/2, (self.tracked[1]+self.tracked[3])/2])
                    continuity = .12 * max(0, 1-float(np.linalg.norm(ac-bc))/max(w, h))
                candidates.append((green+continuity, green, source_box))
            self.tracked = max(candidates, default=(0, 0, None))[2]
            self.last_green = max(candidates, default=(0, 0, None))[1]
            self.last_detection = t
        if self.tracked is None:
            return None
        # Run pose on a generous target box. Retain coordinates in the source image.
        points, scores = self.model.pose_model(image, bboxes=np.array([self.tracked], dtype=np.float32))
        if not len(points):
            return None
        return points[0], scores[0], self.tracked.copy(), self.last_green


class OCR:
    def __init__(self):
        from rapidocr import RapidOCR, OCRVersion, ModelType, EngineType
        self.engine = RapidOCR(params={
            'Global.use_cls': False, 'Global.text_score': .35,
            'Rec.lang_type': 'japan', 'Rec.ocr_version': OCRVersion.PPOCRV4,
            'Rec.model_type': ModelType.MOBILE, 'Rec.engine_type': EngineType.ONNXRUNTIME,
            'Det.ocr_version': OCRVersion.PPOCRV5, 'Det.model_type': ModelType.MOBILE,
            'EngineConfig.onnxruntime.intra_op_num_threads': 4,
            'EngineConfig.onnxruntime.inter_op_num_threads': 1})

    def __call__(self, image, region):
        x, y, x2, y2 = region
        crop = image[y:y2, x:x2]
        scale = 2.0 if crop.shape[0] < 250 else 1.0
        if scale != 1:
            crop = cv2.resize(crop, None, fx=scale, fy=scale, interpolation=cv2.INTER_CUBIC)
        result = self.engine(crop)
        if result.txts is None:
            return []
        out = []
        for text, score, polygon in zip(result.txts, result.scores, result.boxes):
            polygon = np.asarray(polygon, dtype=float)/scale + np.array([x, y])
            out.append({'text': str(text), 'confidence': round(float(score), 4),
                        'polygon_source': polygon.round(2).tolist(),
                        'bbox_source': [round(float(polygon[:, 0].min()), 2), round(float(polygon[:, 1].min()), 2),
                                        round(float(polygon[:, 0].max()), 2), round(float(polygon[:, 1].max()), 2)]})
        return sorted(out, key=lambda r: (round(r['bbox_source'][1]/18), r['bbox_source'][0]))
