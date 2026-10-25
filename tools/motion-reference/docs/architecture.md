# Local video-to-animation-reference pipeline

Checked 2026-10-25, Asia/Taipei. This is an offline authoring tool, not a production web backend.

## Scope and data flow

Original MP4 → SHA-256/ffprobe → presentation timestamps + per-frame image change → shot candidates →
10 Hz selected-character 2D whole-body landmarks → 2 Hz anime mask and Japanese OCR →
fixed crop per shot + hand/face details → merged OCR intervals → candidate reference windows →
clips, contact sheets, local review page and Claude handoff.

Original video stays at its source frame rate. The extraction command can recover any short interval
at the complete original frame rate. The 10 Hz pose dataset is an analysis sample, not a replacement
for original motion. UI and off-screen body parts remain unknown.

Coordinate convention: all bboxes are [left, top, right, bottom], right/bottom exclusive, in source
image pixels. Cropped mask images use masks.json bbox_source for reconstruction. Body crop is fixed
within a shot. Image resize changes display size only; persisted pose coordinates remain source pixels.
Each pose retains decoded frame index, PTS, time_base and normalized video time. The source time origin
is recorded in scan.json. Audio energy uses audio stream time and does not imply speech alignment.

## Third-party evaluation

| Capability | Choice | Reason / exit path |
|---|---|---|
| Decode and trim | FFmpeg + PyAV | Actual PTS, mature decoders; subprocess for exact clip encoding |
| Whole-body pose | rtmlib 0.0.16, RTMW balanced | 133 body/feet/face/hands landmarks, CPU ONNX; avoids full MMPose dependency stack |
| Anime foreground | SkyTNT ISNetIS ONNX | Anime-specific; pinned revision and SHA-256; replace adapter if video boundaries are unstable |
| Japanese OCR | RapidOCR 3.10.0, Japanese PP-OCRv4 mobile recognition, PP-OCRv5 mobile detection | Explicit Japanese route; local CPU; retain raw crops and confidence for correction |
| Runtime | ONNX Runtime 1.30.0 | Supported Python 3.12 build; established previous line rather than immediately moving to 1.31 |
| Computer vision | OpenCV 4.13.0.92 headless | Compatible existing API line rather than new OpenCV 5 major |
| Alternatives | SAM 2, MediaPipe Holistic, SAM 3/Sapiens2 | Not needed as simultaneous dependencies. SAM 2 is fallback for temporal masks; human-based landmark models need domain testing. SAM 3/Sapiens2 have custom licensing and higher integration costs. |

rtmlib's wheel declares three overlapping OpenCV distributions. We deliberately install its wheel
with --no-deps after supplying NumPy, ONNX Runtime, headless cv2 and tqdm explicitly. Do not run an
uncontrolled resolver that replaces this with three conflicting cv2 wheels. Exact resolved package
versions are in requirements.lock.txt; actual ONNX SHA-256 values are in generated provenance.json.

rtmlib/model declarations and anime-segmentation use Apache-2.0. RapidOCR and its PaddleOCR model
routes use Apache-2.0. Record origin notices in THIRD_PARTY_NOTICES.md. Do not treat source video
or game assets as licensed by the tooling license.

## Docker evaluation

Scope: none. Use an isolated Python environment in WSL's Linux cache, with source and output in the
user's Windows project. This single-machine batch pipeline benefits from direct file access and
needs no Docker Desktop service. Environment reuse avoids repeated dependency installs. The RTX 3070
has 8 GiB and was already substantially occupied, so this run uses CPU inference with bounded threads.
Revisit containers when deploying a shared processing worker.

## Rust evaluation

Level 0 for this preprocessing tool: Python glues ML models and native FFmpeg/OpenCV/ONNX kernels.
There is no demonstrated Python hot path that warrants a Rust rewrite. This does not change the
planned Rust website backend. Revisit if profiling identifies a material orchestration bottleneck.

## PostgreSQL evaluation

Use filesystem + JSON/JSONL/CSV for this single-user immutable reference batch. Large images and video
belong in files, not database rows. The schema can later import paths, hashes, review status and action
metadata into the planned PostgreSQL website database. No database service is needed for this handoff.

## Quality boundaries

Target selection uses explicit per-video region plus green-uniform appearance and temporal continuity.
That is a character-specific heuristic, not general face recognition. Multiple people, foliage,
cropped bodies and title cards can still produce false detections. Keep raw confidence and annotate failures.
Shot boundaries use a conservative image-change heuristic and need review, especially fades and zooms.
OCR times have 0.5-second sampling uncertainty and describe visible text, not phonemes.
Candidate windows are review units, not verified complete or loopable animation clips.

## Sources

- https://github.com/Tau-J/rtmlib
- https://github.com/open-mmlab/mmpose
- https://github.com/SkyTNT/anime-segmentation
- https://huggingface.co/skytnt/anime-seg
- https://rapidai.github.io/RapidOCRDocs/main/en/model_list/
- https://github.com/facebookresearch/sam2
- https://vrm.dev/en/vrma/
