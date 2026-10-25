// Crops a profile picture by hand: the photo sits under a round window; drag to move it, and zoom
// with the slider, the mouse wheel, a two-finger pinch or the + and - keys. The result is a square
// of `size` pixels, the window's contents, encoded as WebP (JPEG where WebP cannot be written).

import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useTranslation } from "react-i18next";

const MAX_ZOOM = 5;

interface View {
  /** Zoom over the smallest scale that still fills the window. */
  zoom: number;
  /** Image point at the window's centre, in image pixels. */
  cx: number;
  cy: number;
}

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

export function PictureCropper({ image, size, onCancel, onDone }: { image: ImageBitmap; size: number; onCancel: () => void; onDone: (blob: Blob) => void }) {
  const { t } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const frame = useRef<HTMLDivElement>(null);
  const [side, setSide] = useState(300);
  const [view, setView] = useState<View>({ zoom: 1, cx: image.width / 2, cy: image.height / 2 });
  const [busy, setBusy] = useState(false);
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const pinch = useRef<{ dist: number; zoom: number } | null>(null);

  const baseScale = side / Math.min(image.width, image.height);

  /** Keeps the window inside the photo whatever the zoom. */
  const fit = useCallback(
    (v: View): View => {
      const zoom = clamp(v.zoom, 1, MAX_ZOOM);
      const half = side / (baseScale * zoom) / 2;
      return { zoom, cx: clamp(v.cx, half, image.width - half), cy: clamp(v.cy, half, image.height - half) };
    },
    [side, baseScale, image],
  );

  useEffect(() => {
    dialog.current?.showModal();
  }, []);

  useLayoutEffect(() => {
    const el = frame.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSide(Math.round(el.clientWidth)));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => setView((v) => fit(v)), [fit]);

  // Draw the photo as it sits under the window.
  useEffect(() => {
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = Math.round(side * dpr);
    c.height = Math.round(side * dpr);
    const s = baseScale * view.zoom * dpr;
    ctx.imageSmoothingQuality = "high";
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.drawImage(image, c.width / 2 - view.cx * s, c.height / 2 - view.cy * s, image.width * s, image.height * s);
  }, [image, side, baseScale, view]);

  /** Zooms keeping the image point under (fx, fy) — window pixels from its centre — in place. */
  const zoomAt = (zoom: number, fx = 0, fy = 0) =>
    setView((v) => {
      const s0 = baseScale * v.zoom;
      const z = clamp(zoom, 1, MAX_ZOOM);
      const s1 = baseScale * z;
      return fit({ zoom: z, cx: v.cx + fx / s0 - fx / s1, cy: v.cy + fy / s0 - fy / s1 });
    });

  const fromCentre = (x: number, y: number) => {
    const r = canvas.current!.getBoundingClientRect();
    return [x - r.left - r.width / 2, y - r.top - r.height / 2] as const;
  };

  const down = (e: PointerEvent<HTMLCanvasElement>) => {
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      /* the pointer is already gone; the drag still works while it stays over the photo */
    }
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.current.size === 2) {
      const [a, b] = [...pointers.current.values()];
      pinch.current = { dist: Math.hypot(a.x - b.x, a.y - b.y), zoom: view.zoom };
    }
  };
  const move = (e: PointerEvent<HTMLCanvasElement>) => {
    const last = pointers.current.get(e.pointerId);
    if (!last) return;
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.current.size >= 2 && pinch.current) {
      const [a, b] = [...pointers.current.values()];
      const [fx, fy] = fromCentre((a.x + b.x) / 2, (a.y + b.y) / 2);
      zoomAt((pinch.current.zoom * Math.hypot(a.x - b.x, a.y - b.y)) / pinch.current.dist, fx, fy);
      return;
    }
    const s = baseScale * view.zoom;
    setView((v) => fit({ ...v, cx: v.cx - (e.clientX - last.x) / s, cy: v.cy - (e.clientY - last.y) / s }));
  };
  const up = (e: PointerEvent<HTMLCanvasElement>) => {
    pointers.current.delete(e.pointerId);
    if (pointers.current.size < 2) pinch.current = null;
  };

  // React attaches wheel listeners as passive; this one must stop the page from scrolling.
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const [fx, fy] = fromCentre(e.clientX, e.clientY);
      setView((v) => {
        const z = clamp(v.zoom * Math.exp(-e.deltaY * 0.0015), 1, MAX_ZOOM);
        const s0 = baseScale * v.zoom;
        const s1 = baseScale * z;
        return fit({ zoom: z, cx: v.cx + fx / s0 - fx / s1, cy: v.cy + fy / s0 - fy / s1 });
      });
    };
    c.addEventListener("wheel", wheel, { passive: false });
    return () => c.removeEventListener("wheel", wheel);
  }, [baseScale, fit]);

  const key = (e: KeyboardEvent<HTMLCanvasElement>) => {
    const step = 12 / (baseScale * view.zoom);
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (moves[e.key]) {
      e.preventDefault();
      const [dx, dy] = moves[e.key];
      setView((v) => fit({ ...v, cx: v.cx + dx, cy: v.cy + dy }));
    } else if (e.key === "+" || e.key === "=") zoomAt(view.zoom * 1.15);
    else if (e.key === "-") zoomAt(view.zoom / 1.15);
  };

  const confirm = async () => {
    setBusy(true);
    try {
      const out = document.createElement("canvas");
      out.width = out.height = size;
      const ctx = out.getContext("2d");
      if (!ctx) throw new Error("canvas");
      const span = side / (baseScale * view.zoom);
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(image, view.cx - span / 2, view.cy - span / 2, span, span, 0, 0, size, size);
      const encode = (type: string) => new Promise<Blob | null>((done) => out.toBlob(done, type, 0.9));
      const webp = await encode("image/webp");
      const blob = webp?.type === "image/webp" ? webp : await encode("image/jpeg");
      if (!blob) throw new Error("encode");
      onDone(blob);
    } finally {
      setBusy(false);
    }
  };

  return (
    <dialog
      ref={dialog}
      className="cropper"
      aria-labelledby="cropper-title"
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
    >
      <h2 id="cropper-title">{t("memory.cropTitle")}</h2>
      <p className="muted small">{t("memory.cropHint")}</p>
      <div className="cropper-frame" ref={frame}>
        <canvas
          ref={canvas}
          tabIndex={0}
          role="img"
          aria-label={t("memory.cropLabel")}
          onPointerDown={down}
          onPointerMove={move}
          onPointerUp={up}
          onPointerCancel={up}
          onKeyDown={key}
        />
        <div className="cropper-window" aria-hidden="true" />
      </div>
      <label className="cropper-zoom">
        <span>{t("memory.cropZoom")}</span>
        <input type="range" min={1} max={MAX_ZOOM} step={0.01} value={view.zoom} onChange={(e) => zoomAt(Number(e.target.value))} />
      </label>
      <div className="row cropper-actions">
        <button className="btn ghost" onClick={onCancel} disabled={busy}>
          {t("memory.cropCancel")}
        </button>
        <button className="btn" onClick={() => void confirm()} disabled={busy}>
          {busy ? t("memory.avatarSaving") : t("memory.cropConfirm")}
        </button>
      </div>
    </dialog>
  );
}
