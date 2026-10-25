//! The three interface/reply languages. The program, not the model, picks the reply language.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "zh-TW")]
    ZhTw,
    #[serde(rename = "ja")]
    Ja,
    #[serde(rename = "en-GB")]
    EnGb,
}

impl Locale {
    pub fn code(self) -> &'static str {
        match self {
            Self::ZhTw => "zh-TW",
            Self::Ja => "ja",
            Self::EnGb => "en-GB",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "zh-TW" | "zh-Hant" | "zh" => Some(Self::ZhTw),
            "ja" | "ja-JP" => Some(Self::Ja),
            "en-GB" | "en" | "en-US" => Some(Self::EnGb),
            _ => None,
        }
    }

    /// Kana -> Japanese; Han without kana -> Traditional Chinese; Latin letters -> English.
    /// Text with none of these (digits, emoji) falls back to `fallback`.
    pub fn detect(text: &str, fallback: Self) -> Self {
        let mut han = false;
        let mut latin = false;
        for ch in text.chars() {
            match ch {
                '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{ff66}'..='\u{ff9d}' => return Self::Ja,
                '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' => han = true,
                'a'..='z' | 'A'..='Z' => latin = true,
                _ => {}
            }
        }
        if han {
            Self::ZhTw
        } else if latin {
            Self::EnGb
        } else {
            fallback
        }
    }

    /// Pick one of three strings, ordered zh-TW, ja, en.
    pub fn pick<'a>(self, zh: &'a str, ja: &'a str, en: &'a str) -> &'a str {
        match self {
            Self::ZhTw => zh,
            Self::Ja => ja,
            Self::EnGb => en,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Locale;

    #[test]
    fn detects_reply_language() {
        assert_eq!(Locale::detect("坂路トレーニングは使えますか", Locale::EnGb), Locale::Ja);
        assert_eq!(Locale::detect("秘書室幾點開？", Locale::EnGb), Locale::ZhTw);
        assert_eq!(Locale::detect("What is the curfew?", Locale::ZhTw), Locale::EnGb);
        assert_eq!(Locale::detect("12:30?", Locale::Ja), Locale::Ja);
        // A Chinese sentence that quotes an English facility name is still Chinese.
        assert_eq!(Locale::detect("pool 可以用嗎", Locale::EnGb), Locale::ZhTw);
    }
}
