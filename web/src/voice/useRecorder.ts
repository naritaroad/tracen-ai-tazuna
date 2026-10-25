// Microphone input (docs/character/voice-contract.md): record with MediaRecorder, send to the API,
// and hand back text for the trainer to check. The recording itself is not kept.

import { useCallback, useEffect, useRef, useState } from "react";

import { ApiError } from "../api";

export type RecorderState = "idle" | "recording" | "transcribing";

const MAX_SECONDS = 60;
// Chromium (desktop and Android WebView) records WebM/Opus; Safari records MP4/AAC.
const TYPES = ["audio/webm;codecs=opus", "audio/webm", "audio/mp4", "audio/ogg;codecs=opus"];

export function recordingSupported(): boolean {
  return typeof window !== "undefined" && !!navigator.mediaDevices?.getUserMedia && typeof MediaRecorder !== "undefined";
}

async function transcribe(blob: Blob, locale: string): Promise<string> {
  const form = new FormData();
  form.append("audio", blob, `speech.${blob.type.includes("mp4") ? "m4a" : "webm"}`);
  form.append("locale", locale);
  const res = await fetch("/api/v1/speech/transcribe", { method: "POST", credentials: "same-origin", body: form });
  const body = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(res.status, body.error?.code ?? "http_error", body.error?.message ?? res.statusText);
  return String(body.text ?? "");
}

export function useRecorder(locale: string, onText: (text: string) => void) {
  const [state, setState] = useState<RecorderState>("idle");
  const [seconds, setSeconds] = useState(0);
  const [error, setError] = useState<"denied" | "unsupported" | { message: string } | null>(null);
  const recorder = useRef<MediaRecorder | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const cleanup = useCallback(() => {
    window.clearInterval(timer.current);
    recorder.current?.stream.getTracks().forEach((t) => t.stop());
    recorder.current = null;
  }, []);

  useEffect(() => cleanup, [cleanup]);

  const stop = useCallback(() => {
    if (recorder.current?.state === "recording") recorder.current.stop();
  }, []);

  const start = useCallback(async () => {
    setError(null);
    if (!recordingSupported()) {
      setError("unsupported");
      return;
    }
    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } });
    } catch {
      setError("denied");
      return;
    }
    const mimeType = TYPES.find((t) => MediaRecorder.isTypeSupported(t));
    const rec = new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
    const chunks: Blob[] = [];
    rec.ondataavailable = (e) => e.data.size > 0 && chunks.push(e.data);
    rec.onstop = async () => {
      const type = rec.mimeType || mimeType || "audio/webm";
      cleanup();
      setState("transcribing");
      try {
        const text = await transcribe(new Blob(chunks, { type }), locale);
        if (text.trim()) onText(text.trim());
      } catch (e) {
        setError({ message: (e as Error).message });
      } finally {
        setState("idle");
      }
    };
    recorder.current = rec;
    rec.start();
    setSeconds(0);
    setState("recording");
    const began = Date.now();
    timer.current = window.setInterval(() => {
      const s = Math.floor((Date.now() - began) / 1000);
      setSeconds(s);
      if (s >= MAX_SECONDS) rec.stop();
    }, 250);
  }, [cleanup, locale, onText]);

  return { state, seconds, error, start, stop };
}
