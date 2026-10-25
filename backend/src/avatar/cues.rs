//! Reply cues for the character (docs/character/avatar-contract.md §3): an emotion, how strong it is and a
//! semantic gesture. The model may suggest them with one `[[mood:EMOTION INTENSITY GESTURE]]` line;
//! the line is checked against the fixed vocabulary and always removed from the reply. Rules over
//! the reply and its context fill in when the model gives nothing, and win where a fact of the run
//! (a hand-over, data not found) has to show. Never a file name or a bone: the browser maps words to clips.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

pub const EMOTIONS: [&str; 10] = ["neutral", "gentle", "happy", "excited", "sincere", "apologetic", "troubled", "surprised", "serious", "playful"];
pub const GESTURES: [&str; 12] = ["greet", "bow", "wave", "present", "guide", "explain", "hand_chest", "think", "hope", "raise_hand", "happy", "clap"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CueSource {
    /// The model's `[[mood:...]]` line.
    Model,
    /// Keyword and context rules in this file.
    Rules,
    /// The default of the lifecycle state.
    State,
}

/// What the model suggested, already validated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mood {
    pub emotion: &'static str,
    pub intensity: f32,
    pub gesture: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cue {
    pub emotion: &'static str,
    pub intensity: f32,
    pub gesture: Option<&'static str>,
    pub source: CueSource,
    /// Kept for `emotion_from_model` even when a rule overrides it.
    pub from_model: Option<Mood>,
}

/// The instruction added to the system prompt, built from the same vocabulary the parser accepts.
pub fn prompt_rule() -> String {
    format!(
        "15. End every final reply with one extra line `[[mood:EMOTION INTENSITY GESTURE]]` for your on-screen character.\n    \
         EMOTION is one of: {}. INTENSITY is 0.0 to 1.0. GESTURE is one of: {}, none.\n    \
         The system removes this line before the trainer sees it; never mention it or put anything else in it.",
        EMOTIONS.join(" "),
        GESTURES.join(" "),
    )
}

static MOOD_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\[\[\s*mood\s*[:：]([^\[\]]*)\]\]").expect("mood pattern"));

fn word_in(list: &[&'static str], word: &str) -> Option<&'static str> {
    list.iter().copied().find(|w| w.eq_ignore_ascii_case(word))
}

/// `gentle 0.6 hand_chest` -> Mood. Unknown words are ignored; without a known emotion there is no mood.
pub fn parse_mood(body: &str) -> Option<Mood> {
    let (mut emotion, mut intensity, mut gesture) = (None, None, None);
    for token in body.split(|c: char| c.is_whitespace() || matches!(c, ',' | '/' | '|' | '、')).filter(|t| !t.is_empty()) {
        if let Ok(x) = token.parse::<f32>() {
            if intensity.is_none() && x.is_finite() {
                intensity = Some(x.clamp(0.0, 1.0));
            }
            continue;
        }
        // "happy" is both an emotion and a gesture: the first one is the emotion.
        if emotion.is_none()
            && let Some(e) = word_in(&EMOTIONS, token)
        {
            emotion = Some(e);
            continue;
        }
        if gesture.is_none() {
            gesture = word_in(&GESTURES, token);
        }
    }
    Some(Mood { emotion: emotion?, intensity: intensity.unwrap_or(0.5), gesture })
}

/// Removes every mood line from a reply and returns the last valid one.
pub fn split_mood(text: &str) -> (String, Option<Mood>) {
    let mood = MOOD_LINE.captures_iter(text).filter_map(|c| parse_mood(&c[1])).last();
    if mood.is_none() && !MOOD_LINE.is_match(text) {
        return (text.trim().to_string(), None);
    }
    let cleaned = MOOD_LINE.replace_all(text, "");
    let cleaned = cleaned.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
    (cleaned.trim().to_string(), mood)
}

/// What the rules look at.
pub struct Context<'a> {
    pub question: &'a str,
    pub reply: &'a str,
    pub handed_off: bool,
    pub not_found: bool,
}

pub fn infer(ctx: &Context, model: Option<Mood>) -> Cue {
    let rules = Cue { from_model: model, ..rules(ctx) };
    if ctx.handed_off || ctx.not_found {
        return rules;
    }
    match model {
        Some(m) => Cue { emotion: m.emotion, intensity: m.intensity, gesture: m.gesture.or(rules.gesture), source: CueSource::Model, from_model: Some(m) },
        None => rules,
    }
}

const APOLOGY: &[&str] = &["抱歉", "對不起", "不好意思", "很遺憾", "すみません", "申し訳", "ごめんなさい", "残念", "sorry", "apologies", "apologise", "apologize", "unfortunately", "i'm afraid"];
const REFUSAL: &[&str] = &["無法協助", "沒辦法協助", "無法回答", "お手伝いできません", "お答えできません", "can't help", "cannot help", "unable to help"];
const CONGRATS: &[&str] = &["恭喜", "おめでとう", "congratulations", "congratulate"];
const GOOD_NEWS: &[&str] = &["太好了", "好消息", "よかった", "良かった", "great news", "good news", "wonderful"];
const THANKS: &[&str] = &["謝謝", "感謝", "多謝", "ありがとう", "thank you", "thanks", "cheers"];
const GREETING: &[&str] = &["你好", "您好", "早安", "午安", "晚安", "哈囉", "嗨", "こんにちは", "おはよう", "こんばんは", "はじめまして", "hello", "hi", "hey", "good morning", "good afternoon", "good evening"];
const FAREWELL: &[&str] = &["再見", "掰掰", "拜拜", "さようなら", "またね", "お疲れ様", "goodbye", "bye", "see you"];
const CAUTION: &[&str] = &["請注意", "注意", "務必", "禁止", "不得", "ご注意", "必ず", "いけません", "please note", "note that", "must", "not allowed", "make sure"];
const PLACE: &[&str] = &["位於", "前往", "請到", "請至", "樓", "頁面", "にあります", "へお越し", "ページ", "階", "located", "head to", "go to", "floor", "page"];

/// `word` in `text`; ASCII words must stand alone ("hi" is not in "this").
fn contains_word(text: &str, word: &str) -> bool {
    if !word.is_ascii() {
        return text.contains(word);
    }
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric()) && !after.is_some_and(|c| c.is_ascii_alphanumeric())
    })
}

fn has(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| contains_word(text, w))
}

/// Short messages that are only a greeting, a thank-you or a goodbye.
fn short(text: &str) -> bool {
    text.chars().count() <= 30
}

fn is_list(reply: &str) -> bool {
    let items = reply
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with(['-', '・', '•', '*']) || l.chars().next().is_some_and(|c| c.is_ascii_digit()) && l.chars().nth(1).is_some_and(|c| matches!(c, '.' | '、' | ')' | '．')))
        .count();
    items >= 2
}

fn asks_back(reply: &str) -> bool {
    let end = reply.trim_end();
    end.ends_with(['?', '？']) || end.ends_with("か。") || end.ends_with("ますか")
}

fn rules(ctx: &Context) -> Cue {
    let q = ctx.question.to_lowercase();
    let r = ctx.reply.to_lowercase();
    let cue = |emotion: &'static str, intensity: f32, gesture: &'static str| Cue { emotion, intensity, gesture: Some(gesture), source: CueSource::Rules, from_model: None };
    let bangs = r.matches(['!', '！']).count().min(3) as f32 * 0.1;

    if ctx.handed_off {
        return cue("sincere", 0.6, "bow");
    }
    if ctx.not_found {
        return cue("apologetic", 0.5, "hand_chest");
    }
    if has(&r, REFUSAL) {
        return cue("apologetic", 0.4, "hand_chest");
    }
    if has(&r, APOLOGY) {
        return cue("apologetic", 0.55, "bow");
    }
    if has(&r, CONGRATS) {
        return cue("excited", 0.7 + bangs, "clap");
    }
    if short(&q) && has(&q, THANKS) {
        return cue("happy", 0.55, "bow");
    }
    if short(&q) && has(&q, GREETING) {
        return cue("happy", 0.5, "greet");
    }
    if short(&q) && has(&q, FAREWELL) {
        return cue("gentle", 0.6, "wave");
    }
    if has(&r, GOOD_NEWS) {
        return cue("happy", 0.6 + bangs, "happy");
    }
    if has(&r, CAUTION) {
        return cue("serious", 0.45, "explain");
    }
    if is_list(ctx.reply) {
        return cue("sincere", 0.5, "explain");
    }
    if has(&r, PLACE) {
        return cue("gentle", 0.5, "guide");
    }
    if asks_back(ctx.reply) {
        return cue("gentle", 0.5, "present");
    }
    if bangs > 0.0 {
        return cue("happy", 0.45 + bangs, "present");
    }
    cue("gentle", 0.45, "hand_chest")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(question: &'a str, reply: &'a str) -> Context<'a> {
        Context { question, reply, handed_off: false, not_found: false }
    }

    #[test]
    fn mood_line_is_parsed_and_removed() {
        let (text, mood) = split_mood("受理時間是 9:00–17:00。\n[[mood:gentle 0.6 hand_chest]]");
        assert_eq!(text, "受理時間是 9:00–17:00。");
        assert_eq!(mood, Some(Mood { emotion: "gentle", intensity: 0.6, gesture: Some("hand_chest") }));
    }

    #[test]
    fn mood_accepts_loose_forms_and_rejects_unknown_words() {
        assert_eq!(parse_mood(" Happy, 1.7 / clap "), Some(Mood { emotion: "happy", intensity: 1.0, gesture: Some("clap") }));
        assert_eq!(parse_mood("gentle happy"), Some(Mood { emotion: "gentle", intensity: 0.5, gesture: Some("happy") }));
        assert_eq!(parse_mood("furious 0.9 dance"), None);
        assert_eq!(parse_mood("sincere 0.4 none").and_then(|m| m.gesture), None);
        // An invalid marker still disappears from the reply.
        let (text, mood) = split_mood("はい。[[mood: furious]]");
        assert_eq!((text.as_str(), mood), ("はい。", None));
    }

    #[test]
    fn reply_without_marker_is_unchanged() {
        let (text, mood) = split_mood("  Hello [mood] there  ");
        assert_eq!((text.as_str(), mood), ("Hello [mood] there", None));
    }

    #[test]
    fn run_facts_outrank_the_model() {
        let happy = Some(Mood { emotion: "excited", intensity: 0.9, gesture: Some("clap") });
        let handoff = infer(&Context { handed_off: true, ..ctx("我要申訴", "已建立案件 D-0001。") }, happy);
        assert_eq!((handoff.emotion, handoff.gesture, handoff.source), ("sincere", Some("bow"), CueSource::Rules));
        assert_eq!(handoff.from_model, happy);
        let missing = infer(&Context { not_found: true, ..ctx("游泳池開到幾點？", "目前資料裡沒有游泳池。") }, None);
        assert_eq!(missing.emotion, "apologetic");
    }

    #[test]
    fn model_mood_wins_otherwise_and_keeps_rule_gesture_when_it_has_none() {
        let cue = infer(&ctx("門禁幾點？", "門禁是 22:00。"), Some(Mood { emotion: "serious", intensity: 0.3, gesture: None }));
        assert_eq!((cue.emotion, cue.source), ("serious", CueSource::Model));
        assert_eq!(cue.gesture, Some("hand_chest"));
    }

    #[test]
    fn rules_cover_the_common_turns() {
        let pick = |q: &str, r: &str| {
            let c = infer(&ctx(q, r), None);
            (c.emotion, c.gesture.unwrap_or("-"))
        };
        assert_eq!(pick("你好", "訓練員您好！今天需要什麼協助呢？"), ("happy", "greet"));
        assert_eq!(pick("thanks!", "You're welcome."), ("happy", "bow"));
        assert_eq!(pick("またね", "お疲れ様でした。"), ("gentle", "wave"));
        assert_eq!(pick("能退費嗎？", "很抱歉，這項費用不能退。"), ("apologetic", "bow"));
        assert_eq!(pick("this week?", "Please note that the pool is closed."), ("serious", "explain"));
        assert_eq!(pick("有哪些設施？", "目前可預約：\n- 坂路\n- 泳池"), ("sincere", "explain"));
        assert_eq!(pick("秘書室在哪？", "秘書室位於本館二樓。"), ("gentle", "guide"));
        assert_eq!(pick("想預約", "請問您想預約哪一天呢？"), ("gentle", "present"));
        assert_eq!(pick("模擬賽結果", "恭喜！"), ("excited", "clap"));
        assert_eq!(pick("門禁幾點？", "門禁是 22:00。"), ("gentle", "hand_chest"));
    }

    #[test]
    fn ascii_words_need_word_boundaries() {
        assert!(!contains_word("this is it", "hi"));
        assert!(contains_word("oh, hi there", "hi"));
        assert!(contains_word("嗨 hi", "hi"));
    }

    #[test]
    fn prompt_rule_lists_the_vocabulary() {
        let rule = prompt_rule();
        assert!(EMOTIONS.iter().chain(GESTURES.iter()).all(|w| rule.contains(w)));
    }
}
