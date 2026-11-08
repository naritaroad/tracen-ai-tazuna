#!/usr/bin/env python3
"""Pack plugins/tracen-academy into a zip that Claude can import directly.

The zip has the plugin root at its top level (.claude-plugin/plugin.json, .mcp.json,
skills/...), which is what the plugin upload dialog and `claude --plugin-dir` expect.
Entries use forward slashes and a fixed timestamp, so the same sources always give the
same bytes.

    python tools/package-plugin/package_plugin.py            # -> plugins/tracen-academy.zip
    python tools/package-plugin/package_plugin.py --check    # fail if that zip is stale
    python tools/package-plugin/package_plugin.py --out X.zip

Run `cargo run -- export-skills` first if the skill changed; this script refuses to pack
a plugin whose skill copy differs from skills/tracen-public/SKILL.md.
"""
from __future__ import annotations

import argparse
import io
import json
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "plugins" / "tracen-academy"
SKILL_COPY = PLUGIN / "skills" / "tracen-public" / "SKILL.md"
SKILL_SOURCE = ROOT / "skills" / "tracen-public" / "SKILL.md"
FIXED_TIME = (2026, 1, 1, 0, 0, 0)  # zip cannot store dates before 1980
REQUIRED = [".claude-plugin/plugin.json", ".mcp.json", "skills/tracen-public/SKILL.md"]


def collect() -> list[Path]:
    files = [p for p in PLUGIN.rglob("*") if p.is_file()]
    return sorted(files, key=lambda p: p.relative_to(PLUGIN).as_posix())


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", type=Path, help="output zip (default: plugins/<name>.zip)")
    ap.add_argument("--check", action="store_true", help="rebuild in memory and compare with the committed zip")
    args = ap.parse_args()

    manifest = json.loads((PLUGIN / ".claude-plugin" / "plugin.json").read_text(encoding="utf-8"))
    name, version = manifest["name"], manifest.get("version", "0")

    for rel in REQUIRED:
        if not (PLUGIN / rel).is_file():
            print(f"missing {rel} in {PLUGIN}", file=sys.stderr)
            return 1
    if SKILL_COPY.read_bytes() != SKILL_SOURCE.read_bytes():
        print("plugin skill differs from skills/tracen-public/SKILL.md; run `cargo run -- export-skills`", file=sys.stderr)
        return 1

    out = args.out or ROOT / "plugins" / f"{name}.zip"
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for path in collect():
            info = zipfile.ZipInfo(path.relative_to(PLUGIN).as_posix(), FIXED_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, path.read_bytes())
    data = buf.getvalue()

    if args.check:
        if not out.is_file() or out.read_bytes() != data:
            print(f"{out} is stale; run `python tools/package-plugin/package_plugin.py`", file=sys.stderr)
            return 1
        print(f"{out.name} is up to date (v{version})")
        return 0

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(data)
    print(f"{out.relative_to(ROOT) if out.is_relative_to(ROOT) else out}  v{version}  ({len(data)} bytes)")
    for n in zipfile.ZipFile(out).namelist():
        print(f"  {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
