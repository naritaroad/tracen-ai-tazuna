"""Render the README banners in src/*.html at 2x with headless Edge or Chrome.

GitHub does not load web fonts inside SVG images, so the banners ship as raster images:
WebP (quality 90) keeps the gradients and transparent corners at about a tenth of the PNG size.
Without Pillow the PNG is kept instead.

A source may declare, in <meta> tags:
  banner-name   output name, default the source file name
  banner-size   canvas size in CSS pixels, default 1280x400
  banner-langs  ?lang= variants to export, written as <name>.<lang>.webp

The README banner (src/02-blue-sky-turf.html, exported as banner.<lang>.webp) draws the academy
crest from assets/brand/tracen-crest.svg; without that file the banner falls back to the
horseshoe mark.

Usage: python docs/brand/banners/render.py [name-filter]
The browser is found at the usual Windows paths; set BROWSER to override.
"""

import os
import pathlib
import re
import shutil
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
CREST = HERE.parents[2] / "assets" / "brand" / "tracen-crest.svg"
CANDIDATES = [
    os.environ.get("BROWSER", ""),
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    shutil.which("chromium") or "",
    shutil.which("google-chrome") or "",
]


def browser() -> str:
    for path in CANDIDATES:
        if path and pathlib.Path(path).is_file():
            return path
    sys.exit("No Edge or Chrome found; set BROWSER to its executable.")


def meta(html: str, name: str) -> str | None:
    m = re.search(rf'<meta name="{name}" content="([^"]+)"', html)
    return m.group(1) if m else None


def to_webp(png: pathlib.Path) -> pathlib.Path:
    try:
        from PIL import Image
    except ImportError:
        return png
    out = png.with_suffix(".webp")
    with Image.open(png) as im:
        im.save(out, "WEBP", quality=90, method=6)
    png.unlink()
    return out


def shoot(exe: str, url: str, size: str, png: pathlib.Path) -> pathlib.Path:
    subprocess.run(
        [
            exe,
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--force-device-scale-factor=2",
            f"--window-size={size.replace('x', ',')}",
            "--default-background-color=00000000",
            "--allow-file-access-from-files",
            "--virtual-time-budget=8000",
            f"--screenshot={png}",
            url,
        ],
        check=True,
        capture_output=True,
    )
    return to_webp(png)


def render(exe: str, src: pathlib.Path) -> list[pathlib.Path]:
    html = src.read_text(encoding="utf-8")
    size = meta(html, "banner-size") or "1280x400"
    langs = (meta(html, "banner-langs") or "").split(",")
    name = meta(html, "banner-name") or src.stem
    out = []
    for lang in langs:
        stem = f"{name}.{lang}" if lang else name
        query = f"?lang={lang}" if lang else ""
        out.append(shoot(exe, src.as_uri() + query, size, HERE / f"{stem}.png"))
    return out


if __name__ == "__main__":
    pick = sys.argv[1] if len(sys.argv) > 1 else ""
    if not CREST.is_file():
        print(f"warning: {CREST} is missing; banners with the crest fall back to the horseshoe mark", file=sys.stderr)
    exe = browser()
    for src in sorted((HERE / "src").glob("*.html")):
        if pick in src.stem:
            for path in render(exe, src):
                print(path.relative_to(HERE))
