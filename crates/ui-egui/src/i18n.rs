//! Interface localization. Missing translations fall back to English.
//! Command ids, stored tab names and presentation contents are never translated.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-Hans")]
    SimplifiedChinese,
}

impl Language {
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::English),
            "zh-Hans" => Some(Self::SimplifiedChinese),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::SimplifiedChinese => "简体中文",
        }
    }

    pub fn translate(self, english: &str) -> &str {
        if self == Self::SimplifiedChinese {
            ZH_HANS.binary_search_by(|(key, _)| key.cmp(&english)).ok().and_then(|index| ZH_HANS.get(index)).map_or(english, |(_, value)| *value)
        } else {
            english
        }
    }
}

pub fn set_language(ctx: &egui::Context, language: Language) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("interface_language"), language));
}

pub fn language(ctx: &egui::Context) -> Language {
    ctx.data(|data| data.get_temp(egui::Id::new("interface_language"))).unwrap_or_default()
}

pub fn tr<'a>(ctx: &egui::Context, english: &'a str) -> &'a str {
    language(ctx).translate(english)
}

pub fn placeholder_prompt(ctx: &egui::Context) -> Option<fn(deckcraft_model::PhType) -> &'static str> {
    fn chinese(kind: deckcraft_model::PhType) -> &'static str {
        Language::SimplifiedChinese.translate(kind.prompt())
    }
    (language(ctx) == Language::SimplifiedChinese).then_some(chinese)
}

pub fn chart_label(ctx: &egui::Context, kind: deckcraft_model::chart::ChartType) -> &'static str {
    if language(ctx) == Language::SimplifiedChinese {
        match kind {
            deckcraft_model::chart::ChartType::Line => return "折线图",
            deckcraft_model::chart::ChartType::Funnel => return "漏斗图",
            _ => {}
        }
    }
    tr(ctx, kind.label())
}

/// Both branches are checked by Rust's formatter, including captured names and number formats.
macro_rules! format_tr {
    ($ctx:expr, $english:literal, $chinese:literal $(, $($args:tt)*)?) => {
        if $crate::i18n::language($ctx) == $crate::i18n::Language::SimplifiedChinese {
            format!($chinese $(, $($args)*)?)
        } else {
            format!($english $(, $($args)*)?)
        }
    };
}
pub(crate) use format_tr;

/// Localize engine notices only at the display boundary, preserving control/API responses.
pub fn message(language: Language, english: &str) -> String {
    let exact = language.translate(english);
    if language != Language::SimplifiedChinese || exact != english {
        return exact.to_string();
    }
    for &(template, chinese) in MESSAGE_TEMPLATES {
        let Some((prefix, rest)) = template.split_once('{') else { continue };
        let Some((_, suffix)) = rest.split_once('}') else { continue };
        let Some(value) = english.strip_prefix(prefix).and_then(|s| s.strip_suffix(suffix)) else { continue };
        if let Some((before, rest)) = chinese.split_once('{')
            && let Some((_, after)) = rest.split_once('}')
        {
            return format!("{before}{value}{after}");
        }
    }
    english.to_string()
}

const MESSAGE_TEMPLATES: &[(&str, &str)] = &[
    ("Saved {path}", "已保存 {path}"),
    ("Version {version}", "版本 {version}"),
    ("Exported {path}", "已导出 {path}"),
    ("DeckCraft can't export “{other}”", "DeckCraft 无法导出“{other}”"),
    ("Couldn't save AutoRecover information: {e}", "无法保存自动恢复信息：{e}"),
    ("No sound: {err}", "无法播放声音：{err}"),
    ("{name}: DeckCraft can't open this kind of file", "{name}：DeckCraft 无法打开此类文件"),
    ("{name}: not a presentation DeckCraft can read", "{name}：DeckCraft 无法读取此演示文稿"),
    ("{name} is not a picture", "{name} 不是图片"),
    ("{name}: the picture can't be read", "{name}：无法读取图片"),
    ("{name} has no video", "{name} 没有视频轨道"),
    ("{name} has no audio", "{name} 没有音频轨道"),
    ("no poster frame: {e}", "无法获取封面帧：{e}"),
    ("missing `{parameter}`", "缺少 `{parameter}` 参数"),
    ("missing or invalid `{parameter}`", "缺少 `{parameter}` 参数或参数无效"),
    ("no slide {index}", "找不到幻灯片 {index}"),
    ("no layout {name}", "找不到版式 {name}"),
    ("unknown preset `{name}`", "未知预设 `{name}`"),
    ("unknown transition `{kind}`", "未知切换效果 `{kind}`"),
    ("unknown effect `{effect}`", "未知动画效果 `{effect}`"),
    ("Hard-to-read text contrast ({ratio}:1)", "文字对比度较低，难以阅读 ({ratio}:1)"),
];

/// Preload fallback faces before localized labels are painted (also on an empty start screen).
pub fn font_text() -> &'static str {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TEXT.get_or_init(|| {
        ZH_HANS
            .iter()
            .chain(MESSAGE_TEMPLATES)
            .map(|(_, value)| *value)
            .chain(["简体中文", "日本語", "العربية", "这些字体未安装，将使用相似字体显示。已恢复上次会话中未保存的份演示文稿"])
            .collect()
    })
}

pub const UNAVAILABLE_LANGUAGES: &[&str] = &["Español", "Français", "Deutsch", "Italiano", "Português", "日本語", "العربية"];

pub use crate::zh_hans::ZH_HANS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_unique_nonempty_entries_and_english_fallback() {
        let mut keys = std::collections::HashSet::new();
        for (key, value) in ZH_HANS {
            assert!(keys.insert(key), "duplicate translation: {key}");
            assert!(!value.is_empty());
        }
        assert_eq!(Language::English.translate("File"), "File");
        assert_eq!(Language::SimplifiedChinese.translate("File"), "文件");
        assert_eq!(Language::SimplifiedChinese.translate("Font Inspector…"), "字体检查器…");
        assert_eq!(Language::SimplifiedChinese.translate("Not translated yet"), "Not translated yet");
        assert_eq!(Language::from_code("fr"), None);
    }

    #[test]
    fn contexts_do_not_share_language() {
        let chinese = egui::Context::default();
        let english = egui::Context::default();
        set_language(&chinese, Language::SimplifiedChinese);
        assert_eq!(tr(&chinese, "File"), "文件");
        assert_eq!(tr(&english, "File"), "File");
    }

    #[test]
    fn preferences_switch_languages_and_disable_unavailable_choices() {
        use crate::{Services, SlideApp};
        use deckcraft_engine::Session;
        use egui_kittest::kittest::{NodeT, Queryable};
        let mut app = SlideApp::new(Session::with_new(), Services::default());
        app.run("app.preferences", serde_json::json!({})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
            |ui, app: &mut SlideApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            app,
        );
        h.run_steps(3);
        h.get_by_label("Interface language").click();
        h.run_steps(2);
        assert!(h.get_by_label("Français").accesskit_node().is_disabled());
        assert!(!h.get_by_label("简体中文").accesskit_node().is_disabled());
        h.get_by_label("简体中文").click();
        h.run_steps(3);
        assert_eq!(h.state().ui.language, Language::SimplifiedChinese);
        h.get_by_label("深色外观");
        h.get_by_label("界面语言").click();
        h.run_steps(2);
        h.get_by_label("English").click();
        h.run_steps(3);
        assert_eq!(h.state().ui.language, Language::English);
        h.get_by_label("Dark appearance");
        h.state_mut().run("app.dialog", serde_json::json!({"id": "language"})).unwrap();
        h.run_steps(3);
        assert!(h.get_by_label("中文").accesskit_node().is_disabled());
        h.get_by_label("Interface language…").click();
        h.run_steps(3);
        assert_eq!(h.state().dialog.as_ref().map(|d| d.id.as_str()), Some("preferences"));
    }
}
