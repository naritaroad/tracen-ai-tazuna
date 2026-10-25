#!/usr/bin/env bash
set -euo pipefail
tool_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
runtime_dir="${TRACEN_MOTION_RUNTIME:-${XDG_CACHE_HOME:-$HOME/.cache}/tracen-motion-ref}"
command -v uv >/dev/null || { echo 'Install uv before running setup.' >&2; exit 1; }
uv venv --python 3.12 --allow-existing "$runtime_dir/.venv"
uv pip install --python "$runtime_dir/.venv/bin/python" --no-deps -r "$tool_dir/requirements.lock.txt"
"$runtime_dir/.venv/bin/python" "$tool_dir/prepare.py" models
