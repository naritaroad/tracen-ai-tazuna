// Mouth movement without audio: a reply's text becomes a timeline of syllables (vowel shapes
// aa/ih/ou/ee/oh) and "beats" at sentence ends, where the character nods. Kana map to their
// vowel; Han characters and Latin syllables get a stable pseudo-vowel. When the voice reply
// (docs/character/voice-contract.md, `message.audio.visemes`) exists, its visemes replace this estimate.

export type Vowel = "aa" | "ih" | "ou" | "ee" | "oh";

export interface Syllable {
  /** Start and length in ms from the start of speech. */
  t: number;
  d: number;
  /** `null` is a closed mouth (pause, ん, っ). */
  v: Vowel | null;
  /** How wide the mouth opens, 0–1. */
  w: number;
}

export interface SpeechPlan {
  durationMs: number;
  syllables: Syllable[];
  /** Sentence ends (ms): moments for a small nod. */
  beats: number[];
  truncated: boolean;
}

// One spoken unit per language, in ms.
const MORA_JA = 125;
const SYLLABLE_ZH = 215;
const SYLLABLE_EN = 195;
const PAUSE_COMMA = 200;
const PAUSE_COLON = 260;
const PAUSE_SENTENCE = 380;

const KANA_ROWS: [Vowel, string][] = [
  ["aa", "あかさたなはまやらわがざだばぱぁゃゎアカサタナハマヤラワガザダバパァャヮヵ"],
  ["ih", "いきしちにひみりぎじぢびぴぃイキシチニヒミリギジヂビピィ"],
  ["ou", "うくすつぬふむゆるぐずづぶぷぅゅウクスツヌフムユルグズヅブプゥュヴ"],
  ["ee", "えけせてねへめれげぜでべぺぇエケセテネヘメレゲゼデベペェヶ"],
  ["oh", "おこそとのほもよろをごぞどぼぽぉょオコソトノホモヨロヲゴゾドボポォョ"],
];
const KANA = new Map<string, Vowel>(KANA_ROWS.flatMap(([v, chars]) => [...chars].map((c) => [c, v] as [string, Vowel])));
/** Small kana that change the previous mora's vowel instead of adding one (きゃ = kya). */
const GLIDES = new Set("ゃゅょャュョぁぃぅぇぉァィゥェォ");
const PSEUDO: Vowel[] = ["aa", "oh", "ee", "aa", "ih", "ou", "oh", "aa", "ee", "ih"];
const LATIN_VOWEL: Record<string, Vowel> = { a: "aa", e: "ee", i: "ih", y: "ih", o: "oh", u: "ou" };

const pseudoVowel = (code: number): Vowel => PSEUDO[code % PSEUDO.length];
const openness = (code: number) => 0.55 + ((code * 7) % 31) / 100;

function langOf(lang: string): "ja" | "zh" | "en" {
  if (lang.startsWith("ja")) return "ja";
  if (lang.startsWith("zh")) return "zh";
  return "en";
}

/** Syllables of one Latin word: vowel groups, a silent final "e" dropped. */
function latinSyllables(word: string): Vowel[] {
  const lower = word.toLowerCase();
  const groups = lower.match(/[aeiouy]+/g) ?? [];
  if (groups.length > 1 && lower.length > 3 && lower.endsWith("e") && !/[aeiouy]e$/.test(lower)) groups.pop();
  if (groups.length === 0) return Array.from({ length: Math.max(1, Math.round(lower.length / 2)) }, (_, i) => pseudoVowel(lower.charCodeAt(i)));
  return groups.map((g) => LATIN_VOWEL[g[0]] ?? "aa");
}

export function planSpeech(text: string, lang: string, maxMs = 10000): SpeechPlan {
  const kind = langOf(lang);
  const unit = kind === "ja" ? MORA_JA : kind === "zh" ? SYLLABLE_ZH : SYLLABLE_EN;
  const clean = text.replace(/https?:\/\/\S+/g, " ").replace(/\[\[[^\]]*\]\]/g, " ");
  const out: Syllable[] = [];
  const beats: number[] = [];
  let t = 0;

  const voice = (v: Vowel | null, w: number, d = unit) => {
    out.push({ t, d, v, w });
    t += d;
  };
  const pause = (d: number, beat = false) => {
    const last = out[out.length - 1];
    if (!last) return;
    if (beat && beats[beats.length - 1] !== t) beats.push(t);
    // Several marks in a row ("。」\n") make one pause, not their sum.
    if (last.v === null && last.w === 0) {
      const grow = Math.max(0, d - last.d);
      last.d += grow;
      t += grow;
    } else voice(null, 0, d);
  };

  let word = "";
  const flushWord = () => {
    if (!word) return;
    for (const v of latinSyllables(word)) voice(v, openness(word.length + t));
    word = "";
  };

  for (const ch of clean) {
    const code = ch.codePointAt(0) ?? 0;
    if (/[A-Za-z]/.test(ch)) {
      word += ch;
      continue;
    }
    flushWord();
    if (ch === "っ" || ch === "ッ") voice(null, 0.05, unit * 0.8);
    else if (ch === "ん" || ch === "ン") voice(null, 0.12);
    else if (ch === "ー") {
      const last = out[out.length - 1];
      if (last && last.v) {
        last.d += unit;
        t += unit;
      }
    } else if (GLIDES.has(ch) && out.length && out[out.length - 1].v) out[out.length - 1].v = KANA.get(ch)!;
    else if (KANA.has(ch)) voice(KANA.get(ch)!, openness(code));
    else if (/[一-鿿㐀-䶿]/.test(ch)) {
      // A kanji is about two morae in Japanese, one syllable in Chinese.
      if (kind === "ja") {
        voice(pseudoVowel(code), openness(code));
        voice(pseudoVowel(code >> 3), openness(code >> 2));
      } else voice(pseudoVowel(code), openness(code));
    } else if (/[0-9０-９]/.test(ch)) voice(pseudoVowel(code), openness(code), kind === "en" ? unit * 1.2 : unit);
    else if (/[。．.!！?？]/.test(ch)) pause(PAUSE_SENTENCE, true);
    else if (ch === "\n") pause(PAUSE_SENTENCE * 0.8, true);
    else if (/[,，、]/.test(ch)) pause(PAUSE_COMMA);
    else if (/[:：;；…]/.test(ch)) pause(PAUSE_COLON);
    else if (ch === " " && kind === "en") pause(40);
  }
  flushWord();

  // Trailing silence is not speech.
  while (out.length && out[out.length - 1].v === null) {
    const last = out.pop()!;
    t = last.t;
  }
  let syllables = out;
  let total = t;
  let truncated = false;
  if (total > maxMs) {
    // Speak up to 20 % faster first, then stop at the limit.
    const speed = Math.min(1.2, total / maxMs);
    syllables = syllables.map((s) => ({ ...s, t: s.t / speed, d: s.d / speed }));
    for (let i = 0; i < beats.length; i++) beats[i] /= speed;
    total /= speed;
    if (total > maxMs) {
      syllables = syllables.filter((s) => s.t + s.d <= maxMs);
      total = syllables.length ? syllables[syllables.length - 1].t + syllables[syllables.length - 1].d : 0;
      truncated = true;
    }
  }
  return { durationMs: Math.round(total), syllables, beats: beats.filter((b) => b < total - 150).map(Math.round), truncated };
}

export function estimateSpeechMs(text: string, lang: string, maxMs = 10000): number {
  return planSpeech(text, lang, maxMs).durationMs;
}

/** Vowel and openness at `ms`, with each syllable opening then closing. */
export function mouthAt(plan: SpeechPlan, ms: number, hint = 0): { v: Vowel | null; w: number; index: number } {
  const s = plan.syllables;
  let i = Math.min(Math.max(hint, 0), s.length - 1);
  while (i > 0 && s[i].t > ms) i--;
  while (i < s.length - 1 && s[i].t + s[i].d <= ms) i++;
  const cur = s[i];
  if (!cur || ms < cur.t || ms >= cur.t + cur.d || !cur.v) return { v: null, w: 0, index: Math.max(i, 0) };
  const u = (ms - cur.t) / cur.d;
  return { v: cur.v, w: cur.w * Math.sin(Math.PI * Math.min(1, u * 1.15)), index: i };
}
