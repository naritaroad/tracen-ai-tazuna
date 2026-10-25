#!/usr/bin/env bash
set -euo pipefail
tool_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
runtime_dir="${TRACEN_MOTION_RUNTIME:-${XDG_CACHE_HOME:-$HOME/.cache}/tracen-motion-ref}"
export OMP_NUM_THREADS="${OMP_NUM_THREADS:-4}"
export OPENBLAS_NUM_THREADS="${OPENBLAS_NUM_THREADS:-4}"
exec "$runtime_dir/.venv/bin/python" -u "$tool_dir/prepare.py" "$@"
