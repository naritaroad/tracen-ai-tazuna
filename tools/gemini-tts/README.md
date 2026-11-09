# Gemini TTS for local voice production

This tool renders single-speaker WAV audio with Gemini 3.8 Flash TTS through the Interactions API. It reads `GEMINI_API_KEY` from the environment or the project's `.env`. It uses Python's standard library.

Run from the repository root to render a new line.

```powershell
python tools/gemini-tts/gemini_tts.py --voice en-gb-assistant-1 --text "Good morning! How can I help?" --style "Warm and quietly upbeat" --output greeting.wav
```

Pass a saved request to reproduce its text, voice and delivery settings. Give each take a fresh WAV filename.

```powershell
python tools/gemini-tts/gemini_tts.py --request dome_video/example_voice_3/zh-TW/01_greeting.request.json --output greeting-zh.wav
```

The Python module exposes `speech_request()`, `voice_design_request()`, `GeminiTts.synthesize()` and `GeminiTts.design_voice()`. A designed voice ID belongs to the Google project that created it. `--env-file` accepts an alternate path to that project's API key file.

[Google Gemini API speech generation](https://ai.google.dev/gemini-api/docs/speech-generation)
