"""Single-speaker Gemini 3.8 Flash TTS for local voice production."""
from __future__ import annotations

import argparse
import base64
import io
import json
import os
from pathlib import Path
import tempfile
from urllib.error import HTTPError
from urllib.request import Request, urlopen
import wave


MODEL = "gemini-3.8-flash-tts"
API = "https://generativelanguage.googleapis.com/v1beta"


def load_api_key(env_file: Path) -> str:
    key = os.environ.get("GEMINI_API_KEY", "").strip()
    if key:
        return key
    if env_file.is_file():
        for line in env_file.read_text(encoding="utf-8-sig").splitlines():
            name, _, value = line.strip().removeprefix("export ").partition("=")
            if name.strip() == "GEMINI_API_KEY" and value.strip():
                return value.strip().strip("\"'")
    raise ValueError("Set GEMINI_API_KEY or add it to the project's .env file")


def speech_request(text: str, voice: str, style: str = "") -> dict:
    if not text.strip() or not voice.strip():
        raise ValueError("Speech requires a transcript and a voice")
    content = {"type": "text", "text": text}
    if style.strip():
        content["annotations"] = [{"type": "speech_metadata", "style": style}]
    return {
        "model": MODEL,
        "store": False,
        "input": [{"type": "user_input", "content": [content]}],
        "response_format": {"type": "audio", "mime_type": "audio/wav"},
        "generation_config": {"speech_config": [{"voice": voice}]},
    }


def voice_design_request(name: str, locale: str, description: str) -> dict:
    return {
        "store": True,
        "voice": {
            "model": MODEL,
            "type": "prompted",
            "display_name": name,
            "language_code": locale,
            "gender": "female",
            "prompted": {"input": description},
        },
    }


def wav_info(data: bytes) -> dict:
    with wave.open(io.BytesIO(data), "rb") as audio:
        frames = audio.getnframes()
        rate = audio.getframerate()
        pcm = audio.readframes(frames)
        if not frames or len(pcm) != frames * audio.getsampwidth() * audio.getnchannels():
            raise ValueError("Gemini returned an empty or truncated WAV")
        return {
            "duration_sec": round(frames / rate, 3),
            "sample_rate_hz": rate,
            "channels": audio.getnchannels(),
            "bits_per_sample": audio.getsampwidth() * 8,
        }


class GeminiTts:
    def __init__(self, api_key: str, timeout: int = 120):
        self.api_key = api_key
        self.timeout = timeout

    def _post(self, resource: str, body: dict) -> dict:
        request = Request(
            f"{API}/{resource}",
            data=json.dumps(body, ensure_ascii=False).encode("utf-8"),
            headers={"Content-Type": "application/json", "x-goog-api-key": self.api_key},
            method="POST",
        )
        try:
            with urlopen(request, timeout=self.timeout) as response:
                return json.load(response)
        except HTTPError as error:
            try:
                message = json.load(error).get("error", {}).get("message", error.reason)
            except ValueError:
                message = error.reason
            raise RuntimeError(
                f"Gemini {resource} HTTP {error.code}: {str(message).replace(self.api_key, '[redacted]')}"
            ) from None

    def design_voice(self, request: dict) -> dict:
        return self._post("voices", request)

    def synthesize(self, request: dict) -> tuple[bytes, dict]:
        if not isinstance(request, dict) or request.get("model") != MODEL:
            raise ValueError(f"Expected request model {MODEL}")
        response = self._post("interactions", request)
        blocks = [
            part
            for step in response.get("steps", [])
            if step.get("type") == "model_output"
            for part in step.get("content", [])
            if part.get("type") == "audio" and part.get("data")
        ]
        if response.get("status", "completed") != "completed" or len(blocks) != 1:
            raise RuntimeError(
                f"Expected one completed audio block; status={response.get('status')!r}, audio={len(blocks)}"
            )
        audio = base64.b64decode(blocks[0]["data"], validate=True)
        return audio, wav_info(audio)


def write_wav(path: Path, data: bytes) -> None:
    """Publish a complete WAV without replacing a take that already exists."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, suffix=".part", delete=False) as temp:
        temporary = Path(temp.name)
        try:
            temp.write(data)
            temp.close()
            os.link(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--env-file", type=Path, default=Path(__file__).resolve().parents[2] / ".env")
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--request", type=Path, help="Replay a saved Interactions API JSON request")
    inputs.add_argument("--text", help="Text to speak")
    parser.add_argument("--voice", help="Gemini voice ID for --text")
    parser.add_argument("--style", default="", help="Delivery for --text")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.text is not None and not args.voice:
        parser.error("--text requires --voice")
    if args.request and (args.voice or args.style):
        parser.error("--request already contains voice and style")
    if args.output.exists() or args.output.suffix.lower() != ".wav":
        parser.error("--output must be a new .wav path")
    try:
        body = (json.loads(args.request.read_text(encoding="utf-8-sig")) if args.request
                else speech_request(args.text, args.voice, args.style))
        audio, info = GeminiTts(load_api_key(args.env_file)).synthesize(body)
        write_wav(args.output, audio)
    except (ValueError, OSError, RuntimeError, wave.Error, EOFError) as error:
        parser.exit(1, f"{error}\n")
    print(f"{args.output} ({info['duration_sec']:.2f} s, {info['sample_rate_hz']} Hz)")


if __name__ == "__main__":
    main()
