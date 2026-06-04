use super::*;

#[path = "ui_locale/catalog.rs"]
mod catalog;
#[path = "ui_locale/messages.rs"]
mod messages;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiLocale {
    En,
    Zh,
    Ja,
    Ko,
    Fr,
    De,
    Es,
    Pt,
    Ru,
}

impl UiLocale {
    pub(crate) fn detect() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = env::var(key) {
                let value = value.to_ascii_lowercase();
                if value.starts_with("zh") {
                    return Self::Zh;
                }
                if value.starts_with("ja") {
                    return Self::Ja;
                }
                if value.starts_with("ko") {
                    return Self::Ko;
                }
                if value.starts_with("fr") {
                    return Self::Fr;
                }
                if value.starts_with("de") {
                    return Self::De;
                }
                if value.starts_with("es") {
                    return Self::Es;
                }
                if value.starts_with("pt") {
                    return Self::Pt;
                }
                if value.starts_with("ru") {
                    return Self::Ru;
                }
            }
        }
        Self::En
    }
}
