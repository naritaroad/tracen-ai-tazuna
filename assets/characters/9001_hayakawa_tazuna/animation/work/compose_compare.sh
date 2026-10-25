#!/usr/bin/env bash
# Side-by-side comparison: source video | Blender render (same camera) | render over source (55%), with source audio.
# usage: compose_compare.sh <render_png_dir> <source_start_sec> <out.mp4> [fps]
set -euo pipefail
R="$1"; START="$2"; OUT="$3"; FPS="${4:-30}"
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
SRC="$ROOT/example_pose/9001_Hayakawa Tazuna/exsample_source_vedio/qQ40c6kgV8M.mp4"
N=$(ls "$R"/*.png | wc -l)
DUR=$(python3 -c "print(($N-1)/$FPS)")
ffmpeg -v error -y -ss "$START" -t "$DUR" -i "$SRC" -framerate "$FPS" -i "$R/%04d.png" \
  -f lavfi -i "color=c=0xc8cdd2:s=540x724:r=$FPS" \
  -filter_complex "\
[0:v]fps=$FPS,scale=540:724,setsar=1,format=yuva420p,split=2[sa][sb];\
[1:v]scale=540:724,setsar=1,format=rgba,split=2[ra][rb];\
[2:v][ra]overlay=0:0:shortest=1[p2];\
[rb]colorchannelmixer=aa=0.55[rb55];\
[sb][rb55]overlay=0:0:shortest=1[p3];\
[sa][p2][p3]hstack=inputs=3,format=yuv420p[v]" \
  -map "[v]" -map "0:a?" -c:v libx264 -preset veryfast -crf 20 -c:a aac -shortest "$OUT"
echo "wrote $OUT ($N frames)"
