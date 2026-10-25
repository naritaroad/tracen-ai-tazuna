//! Offline, deterministic stand-in for a model (`MODEL=mock`).
//! It picks a tool by keywords and turns tool results into a short reply, so the agent loop,
//! the trace panel, hand-over and the tool-choice test can run without an API key.
//! It is a development aid: real answers use a real model.

use std::sync::LazyLock;

use anyhow::Result;
use futures::future::BoxFuture;
use genai::chat::{ChatMessage, ChatRequest, ChatRole, ToolCall};
use regex::Regex;
use serde_json::{Value, json};

use super::{ModelGateway, ModelReply};
use crate::catalog::academy_now;
use crate::locale::Locale;

pub struct MockGateway;

/// What the offline speech engine "hears".
pub const TRANSCRIPT: &str = "秘書室的受理時間是幾點到幾點？";

/// Deterministic stand-in for an embedding: character bigrams hashed into 768 buckets,
/// normalised. Texts that share words land close together, which is enough to test recall.
pub fn embed(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; super::EMBED_DIMENSIONS];
    let chars: Vec<char> = text.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    for pair in chars.windows(2).map(|w| [w[0], w[1]]).chain(chars.iter().map(|c| [*c, ' '])) {
        let mut h: u64 = 1469598103934665603;
        for c in pair {
            h ^= c as u64;
            h = h.wrapping_mul(1099511628211);
        }
        v[(h % super::EMBED_DIMENSIONS as u64) as usize] += 1.0;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
    v.iter_mut().for_each(|x| *x /= norm);
    v
}

impl ModelGateway for MockGateway {
    fn model_name(&self) -> &str {
        "mock"
    }

    fn chat(&self, request: ChatRequest) -> BoxFuture<'_, Result<ModelReply>> {
        Box::pin(async move { Ok(reply(&request)) })
    }
}

fn reply_locale(request: &ChatRequest) -> Locale {
    request
        .system
        .as_deref()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("REPLY_LOCALE: ")))
        .and_then(|c| Locale::parse(c.trim()))
        .unwrap_or(Locale::ZhTw)
}

/// The admin's small-talk setting, from the `SMALL_TALK:` line of the system prompt.
fn small_talk_allowed(request: &ChatRequest) -> bool {
    request.system.as_deref().and_then(|s| s.lines().find_map(|l| l.strip_prefix("SMALL_TALK: "))).is_none_or(|v| v.trim() != "off")
}

fn reply(request: &ChatRequest) -> ModelReply {
    let locale = reply_locale(request);
    let Some(last) = request.messages.last() else {
        return text(locale.pick("請問有什麼需要協助的嗎？", "ご用件をどうぞ。", "How can I help?").into());
    };
    if last.role == ChatRole::Tool {
        // Every tool response since the last user message.
        let start = request.messages.iter().rposition(|m| m.role == ChatRole::User).map_or(0, |i| i + 1);
        let results: Vec<(String, Value)> = request.messages[start..]
            .iter()
            .flat_map(|m| m.content.tool_responses())
            .map(|r| (r.fn_name.clone().unwrap_or_default(), serde_json::from_str(&r.content).unwrap_or(Value::Null)))
            .collect();
        return text(summarize(&results, locale));
    }
    let question = last.content.first_text().unwrap_or_default().to_string();
    match route(&question) {
        Some((name, args)) => {
            let call = ToolCall { call_id: format!("mock_{name}"), fn_name: name.to_string(), fn_arguments: args, thought_signatures: None };
            ModelReply {
                text: None,
                assistant_message: Some(ChatMessage::from(vec![call.clone()])),
                tool_calls: vec![call],
                usage: json!({}),
            }
        }
        None if small_talk_allowed(request) => text(
            locale
                .pick(
                    "謝謝您跟我聊這些呢♪ 不過我這邊是離線測試模式，沒辦法好好陪您聊天。校務規定、訓練設施或訓練計畫的事，隨時都可以問我喔。",
                    "お話しいただけてうれしいです♪ ただ、今はオフラインのテストモードなので、雑談のお相手はうまくできません。学園の規則や施設、トレーニング計画のことなら、いつでもどうぞ。",
                    "Thank you for the chat♪ I'm in offline test mode, though, so I can't keep up a proper conversation. Academy rules, facilities or training plans, any time.",
                )
                .into(),
        ),
        None => text(
            locale
                .pick(
                    "不好意思，我只能協助特雷森學園訓練員的事務，例如校務規定、訓練設施與訓練計畫。",
                    "申し訳ありません。トレセン学園のトレーナー業務（学園の規則、施設、トレーニング計画など）のみお手伝いできます。",
                    "Sorry, I can only help with Tracen Academy trainer matters, such as academy rules, facilities and training plans.",
                )
                .into(),
        ),
    }
}

fn text(content: String) -> ModelReply {
    ModelReply { text: Some(content), tool_calls: Vec::new(), assistant_message: None, usage: json!({}) }
}

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| q.contains(w))
}

static SESSIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?P<name>[\p{Han}\p{Hiragana}\p{Katakana}A-Za-z_ ]+?)\s*(?P<n>\d+)\s*(?:次|回|個時段|個|sessions?|times|slots?)").expect("regex")
});
static MONTH_DAY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{1,2})\s*月\s*(\d{1,2})\s*日").expect("regex"));
static CLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d{1,2}):(\d{2})\b").expect("regex"));

fn dates(q: &str) -> Vec<String> {
    let year = academy_now().year();
    MONTH_DAY.captures_iter(q).map(|c| format!("{year}-{:0>2}-{:0>2}", &c[1], &c[2])).collect()
}

const INFO_KEYWORDS: [(&str, &[&str]); 10] = [
    ("curfew", &["門禁", "門限", "curfew"]),
    ("service_desk", &["受理時間", "窗口", "受付", "office hours", "秘書室"]),
    ("booking_policy", &["取消", "キャンセル", "cancel", "預約規則"]),
    ("race_paperwork", &["出賽", "校外", "出走", "race entr", "off-campus"]),
    ("documents", &["證明", "書類", "certificate"]),
    ("trainer_workshops", &["研習", "研修", "workshop"]),
    ("calendar_conflicts", &["衝突規則", "重なり", "clash"]),
    ("facility_use", &["使用規則", "利用ルール", "facility rule"]),
    ("support_contacts", &["保健室", "不適", "infirmary", "unwell"]),
    ("director_office", &["理事長室案件"]),
];

fn route(question: &str) -> Option<(&'static str, Value)> {
    let q = question.to_lowercase();

    if has(&q, &["理事長", "director", "manager", "申訴", "抱怨", "苦情", "complain", "延長門禁"]) {
        let reason = if has(&q, &["申訴", "抱怨", "苦情", "complain", "問題"]) { "complaint" } else if q.contains("延長") { "exception_request" } else { "asked_for_director" };
        return Some(("transfer_to_director", json!({ "reason": reason, "summary": question })));
    }

    let items: Vec<Value> = SESSIONS
        .captures_iter(question)
        .map(|c| json!({ "service": c["name"].trim(), "sessions": c["n"].parse::<u32>().unwrap_or(1) }))
        .collect();
    if !items.is_empty() {
        let mut args = json!({ "items": items });
        if let Some(d) = dates(question).first() {
            args["date"] = json!(d);
            if let Some(c) = CLOCK.captures(question) {
                args["start_time"] = json!(format!("{:0>2}:{}", &c[1], &c[2]));
            }
        }
        return Some(("calculate_training_plan", args));
    }

    if let Some((topic, _)) = INFO_KEYWORDS.iter().find(|(_, words)| has(&q, words)) {
        return Some(("get_academy_info", json!({ "topic": topic })));
    }

    let days = dates(question);
    if !days.is_empty() || has(&q, &["活動", "行事", "日曆", "calendar", "考試", "exam", "賽事", "レース", "g1", "g2"]) {
        let today = academy_now().date().to_string();
        let from = days.first().cloned().unwrap_or(today);
        let to = days.get(1).cloned().unwrap_or_else(|| from.clone());
        return Some(("list_academy_events", json!({ "date_from": from, "date_to": to })));
    }

    if has(&q, &["設施", "訓練", "可以用", "使え", "available", "room", "室", "トレーニング", "pool", "泳池"]) {
        return Some(("search_training_services", json!({ "query": question })));
    }

    if has(&q, &["學園", "學校", "餐廳", "食堂", "菜單", "menu", "cafeteria", "学園"]) {
        return Some(("transfer_to_director", json!({ "reason": "not_in_data", "summary": question })));
    }
    None
}

fn summarize(results: &[(String, Value)], locale: Locale) -> String {
    let mut parts = Vec::new();
    for (name, r) in results {
        let found = r.get("found").and_then(Value::as_bool);
        let line = match name.as_str() {
            "get_academy_info" if found == Some(true) => r["info"]["content"].as_str().unwrap_or_default().to_string(),
            "list_academy_events" if found == Some(true) => {
                let list: Vec<String> = r["events"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|e| format!("{}（{} – {}）", e["title"].as_str().unwrap_or(""), e["starts_at"].as_str().unwrap_or(""), e["ends_at"].as_str().unwrap_or("")))
                    .collect();
                list.join("；")
            }
            "search_training_services" if found == Some(true) => {
                let list: Vec<String> = r["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(5)
                    .map(|s| {
                        let ok = s["available"].as_bool().unwrap_or(false);
                        format!("{}：{}", s["name"].as_str().unwrap_or(""), locale.pick(if ok { "可使用" } else { "暫停開放" }, if ok { "利用可" } else { "利用停止中" }, if ok { "available" } else { "not available" }))
                    })
                    .collect();
                list.join("；")
            }
            "calculate_training_plan" if r["complete"].as_bool() == Some(true) => {
                let conflicts = r["conflicts"].as_array().map_or(0, Vec::len);
                format!(
                    "{} {} {}，{} {} {}。{}",
                    locale.pick("總共", "合計", "Total"),
                    r["total_minutes"],
                    locale.pick("分鐘", "分", "minutes"),
                    locale.pick("費用", "費用", "fee"),
                    r["total_fee_jpy"],
                    locale.pick("日圓", "円", "JPY"),
                    if conflicts > 0 { format!("{} {}", locale.pick("有衝突：", "重なり：", "Clashes:"), conflicts) } else { String::new() }
                )
            }
            "calculate_training_plan" => locale.pick("有項目無法確定，請問是指哪一項設施？", "どの施設か確認させてください。", "Which facility do you mean?").into(),
            "transfer_to_director" => format!(
                "{} {}",
                locale.pick("已送交理事長室，等候真人接手。案件編號", "理事長室へお送りしました。担当者の対応をお待ちください。案件番号", "I have sent this to the Director's Office; a person will pick it up. Case"),
                r["ticket_no"].as_str().unwrap_or("")
            ),
            _ => locale.pick("查無相關資料。", "該当する情報が見つかりませんでした。", "I could not find that information.").into(),
        };
        parts.push(line);
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{embed, route};

    #[test]
    fn mock_embedding_brings_related_text_closer() {
        let cos = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        let q = embed("泳池設備的問題");
        assert!(cos(&q, &embed("反映泳池設備壞掉")) > cos(&q, &embed("秘書室的受理時間")));
    }

    #[test]
    fn routes_course_questions() {
        assert_eq!(route("暖身伸展 1 次、草地基本訓練 5 次，總共要多少分鐘？").map(|r| r.0), Some("calculate_training_plan"));
        assert_eq!(route("請幫我轉給秋川理事長").map(|r| r.0), Some("transfer_to_director"));
        assert_eq!(route("What is the curfew for the dorms?").map(|r| r.0), Some("get_academy_info"));
        assert_eq!(route("11 月 4 日到 6 日有什麼校務活動？").map(|r| r.0), Some("list_academy_events"));
        assert_eq!(route("幫我寫一首關於天氣的詩"), None);
    }
}
