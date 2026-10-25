import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from core import KEYPOINT_NAMES, expand, merge_ocr, relative_box, visible_content, split_ocr_rows


class ReferenceDataTests(unittest.TestCase):
    def test_typewriter_samples_merge_but_new_sentence_stays_separate(self):
        samples = [
            {'id': 'a', 't': 0, 'dialogue_text': 'トレーナーさん', 'mean_score': .9},
            {'id': 'b', 't': .5, 'dialogue_text': 'トレーナーさん、お疲れ様です！', 'mean_score': .95},
            {'id': 'c', 't': 1, 'dialogue_text': 'トレーナーさん、お疲れ様です！', 'mean_score': .96},
            {'id': 'd', 't': 1.5, 'dialogue_text': '今日は晴れですね。', 'mean_score': .9},
        ]
        events = merge_ocr(samples)
        self.assertEqual(len(events), 2)
        self.assertEqual(events[0]['text'], samples[2]['dialogue_text'])
        self.assertEqual(events[0]['observations'], ['a', 'b', 'c'])
        self.assertEqual(events[0]['end'], 1.5)
        self.assertEqual(events[1]['start'], 1.5)

    def test_same_text_after_gap_is_new_event(self):
        samples = [{'id': str(t), 't': t, 'dialogue_text': 'はい！', 'mean_score': .9} for t in [0, .5, 8]]
        self.assertEqual(len(merge_ocr(samples)), 2)

    def test_only_persistent_edge_bands_are_removed(self):
        images = []
        for x in [30, 50, 70]:
            a = np.full((100, 160, 3), 100, np.uint8)
            a[:, :12] = 0; a[:, 148:] = 0; a[:6] = 0; a[94:] = 0
            a[30:70, x:x+10] = 0  # Dark moving clothing is not an edge band.
            images.append(a)
        self.assertEqual(visible_content(images), [12, 6, 148, 94])

    def test_coordinate_mapping_keeps_source_offset(self):
        self.assertEqual(relative_box([.5, 0, 1, 1], [100, 20, 900, 620]), [500, 20, 900, 620])
        self.assertEqual(expand([5, 5, 95, 95], .2, 100, 100), [0, 0, 100, 100])

    def test_wholebody_schema_indices(self):
        self.assertEqual(len(KEYPOINT_NAMES), 133)
        self.assertEqual(KEYPOINT_NAMES[91], 'left_hand_wrist')
        self.assertEqual(KEYPOINT_NAMES[112], 'right_hand_wrist')

    def test_green_uniform_behind_subtitles_is_not_a_nameplate(self):
        rows = [
            {'text': '駿川たづな', 'confidence': .95, 'bbox_source': [200, 570, 450, 605]},
            {'text': 'トレーナーさん、大変です！', 'confidence': .96, 'bbox_source': [400, 620, 800, 660]},
            {'text': 'カレンチャンさんが失踪しました', 'confidence': .91, 'bbox_source': [400, 655, 850, 692]},
        ]
        dialogue, speaker = split_ocr_rows(rows, [[170, 590, 1100, 700]], 720, [.7, .6, .5])
        self.assertEqual(speaker, '駿川たづな')
        self.assertEqual(len(dialogue), 2)

    def test_english_title_card_is_preserved_but_loading_interface_is_not(self):
        rows = [{'text': 'BIG NEWS', 'confidence': .95, 'bbox_source': [10, 10, 200, 50]},
                {'text': 'Now Loading...', 'confidence': .99, 'bbox_source': [10, 100, 200, 150]}]
        dialogue, speaker = split_ocr_rows(rows, [], 720)
        self.assertEqual([r['text'] for r in dialogue], ['BIG NEWS'])
        self.assertIsNone(speaker)


if __name__ == '__main__':
    unittest.main()
