//! Shared font inventory and save preparation for native documents and PPTX.
use deckcraft_fonts::{
    FontDb,
    embedding::{self, Permission},
};
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::text::{Bullet, ListStyle, RunProps, TextBody};
use deckcraft_model::{EmbeddedFont, Presentation, Shape, ShapeKind};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Usage {
    pub family: String,
    pub style: String,
    pub pages: Vec<usize>,
    pub available: bool,
    pub embedded: bool,
    pub stored_unreadable: bool,
    pub permission: Permission,
    pub replacement: String,
    pub missing_chars: String,
    pub style_substituted: bool,
}

#[derive(Default)]
struct Request {
    family: String,
    style: String,
    pages: Vec<usize>,
    chars: std::collections::BTreeSet<char>,
}
type Requests = BTreeMap<(String, String), Request>;

fn style(r: &RunProps) -> &'static str {
    match (r.bold.unwrap_or(false), r.italic.unwrap_or(false)) {
        (true, true) => "Bold Italic",
        (true, false) => "Bold",
        (false, true) => "Italic",
        _ => "Regular",
    }
}
fn add(out: &mut Requests, family: String, style: &str, page: usize, text: &str) {
    let family = family.trim().to_string();
    if family.is_empty() || family.starts_with('+') {
        return;
    }
    let item = out.entry((family.to_lowercase(), style.to_string())).or_insert_with(|| Request { family, style: style.into(), ..Default::default() });
    if page > 0 && !item.pages.contains(&page) {
        item.pages.push(page);
    }
    item.chars.extend(text.chars().filter(|c| {
        !c.is_whitespace()
            && !c.is_control()
            && !matches!(*c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}')
    }));
}
fn props(ctx: &Ctx, r: &RunProps, out: &mut Requests, page: usize, text: &str) {
    let latin = resolve::font_family(ctx, r);
    let major = r.font.as_deref().is_some_and(|f| f.starts_with("+mj"));
    let set = if major { &ctx.master.theme.fonts.major } else { &ctx.master.theme.fonts.minor };
    add(out, latin.clone(), style(r), page, "");
    for f in [&r.font_ea, &r.font_cs, &r.font_sym].into_iter().flatten() {
        add(out, ctx.master.theme.font(f), style(r), page, "");
    }
    let chars: Vec<char> = text.chars().collect();
    for (c, (script, slot)) in chars.iter().zip(deckcraft_fonts::script::resolve(&chars)) {
        let explicit = match slot {
            deckcraft_fonts::FontSlot::EastAsian => r.font_ea.as_deref().or(Some(set.ea.as_str())),
            deckcraft_fonts::FontSlot::ComplexScript => r.font_cs.as_deref().or(Some(set.cs.as_str())),
            _ => None,
        };
        let script_font = script.as_ref().and_then(|s| std::str::from_utf8(s).ok()).and_then(|s| set.script(s));
        let family = explicit
            .map(|f| ctx.master.theme.font(f))
            .filter(|f| !f.is_empty())
            .or_else(|| script_font.map(String::from))
            .unwrap_or_else(|| latin.clone());
        let family =
            if ('\u{f000}'..='\u{f0ff}').contains(c) { r.font_sym.as_deref().map(|f| ctx.master.theme.font(f)).unwrap_or(family) } else { family };
        add(out, family, style(r), page, &c.to_string());
    }
}
fn body(ctx: &Ctx, shape: &Shape, body: &TextBody, out: &mut Requests, page: usize) {
    for para in &body.paragraphs {
        for run in &para.runs {
            let r = resolve::run(ctx, shape, para, &run.props);
            props(ctx, &r, out, page, &run.text);
        }
        let p = resolve::para(ctx, shape, para);
        if let Some(Bullet::Char { char: ch }) = p.bullet {
            let f = p.bullet_font.as_deref().unwrap_or("+mn-lt");
            add(out, ctx.master.theme.font(f), "Regular", page, &ch.to_string());
        }
    }
}
fn shapes(ctx: &Ctx, list: &[Shape], out: &mut Requests, page: usize) {
    deckcraft_model::walk(list, &mut |s, _| {
        if let Some(t) = &s.text {
            body(ctx, s, t, out, page);
        }
        if let ShapeKind::Table(t) = &s.kind {
            for c in t.rows.iter().flat_map(|r| &r.cells) {
                body(ctx, s, &c.text, out, page);
            }
        }
    });
}
fn list(ctx: &Ctx, ls: &ListStyle, out: &mut Requests) {
    for l in ls.levels.iter().flatten() {
        props(ctx, &l.run, out, 0, "");
        if let Some(f) = &l.para.bullet_font {
            add(out, ctx.master.theme.font(f), "Regular", 0, "");
        }
    }
}
fn requests(p: &Presentation) -> Requests {
    let mut out = Requests::new();
    for (i, s) in p.slides.iter().enumerate() {
        if let Some(ctx) = Ctx::for_slide(p, s) {
            shapes(&ctx, &s.shapes, &mut out, i + 1);
            if !s.notes.is_empty() {
                let notes_ctx = p.notes_master.as_ref().map(|m| Ctx::for_master(p, m));
                body(notes_ctx.as_ref().unwrap_or(&ctx), &Shape::default(), &s.notes, &mut out, i + 1);
            }
        }
    }
    for m in p.masters.iter().chain(p.notes_master.iter()).chain(p.handout_master.iter()) {
        let ctx = Ctx::for_master(p, m);
        shapes(&ctx, &m.shapes, &mut out, 0);
        for ls in [&m.title_style, &m.body_style, &m.other_style, &p.default_text_style] {
            list(&ctx, ls, &mut out);
        }
        for fs in [&m.theme.fonts.major, &m.theme.fonts.minor] {
            for f in [&fs.latin, &fs.ea, &fs.cs] {
                add(&mut out, f.clone(), "Regular", 0, "");
            }
        }
        for l in &m.layouts {
            shapes(&Ctx::for_layout(p, m, l), &l.shapes, &mut out, 0);
        }
    }
    out
}

pub fn inspect(p: &Presentation) -> Vec<Usage> {
    let db = FontDb::global();
    let mut budget = 0usize;
    requests(p)
        .into_values()
        .map(|r| {
            let embedded = embedded_face(p, &r.family, &r.style);
            let available = embedded.is_some() || db.has_family(&r.family);
            let face = embedded.clone().unwrap_or_else(|| db.document_face(&r.family, &r.style));
            let stored_unreadable = embedded.is_none()
                && (p.embedded_font_faces.iter().any(|f| f.family.eq_ignore_ascii_case(&r.family) && f.style == r.style)
                    || p.embedded_fonts.iter().any(|(f, _)| f.eq_ignore_ascii_case(&r.family)));
            let mut permission = if available { embedding::permission(face.data(), face.index()) } else { Permission::Unknown };
            if permission == Permission::Editable && embedded.is_none() {
                budget = budget.saturating_add(face.data().len());
                if budget > 256 << 20 {
                    permission = Permission::Budget;
                }
            }
            let missing_chars = r.chars.iter().filter(|c| !face.covers(**c)).take(12).collect();
            let style_substituted = (r.style.contains("Bold") && face.weight < 550.0) || (r.style.contains("Italic") && !face.italic);
            Usage {
                family: r.family,
                style: r.style,
                pages: r.pages,
                available,
                embedded: embedded.is_some(),
                stored_unreadable,
                permission,
                replacement: format!("{} / {}", face.family, face.style),
                missing_chars,
                style_substituted,
            }
        })
        .collect()
}

/// Clone-on-write preparation: saving never marks the live document dirty or alters its fonts.
pub fn prepare(p: &Presentation) -> Presentation {
    let mut result = p.clone();
    if !p.auto_embed_fonts {
        return result;
    }
    let db = FontDb::global();
    let mut total = 0usize;
    for request in requests(p).into_values() {
        if result.embedded_font_faces.iter().any(|f| f.family.eq_ignore_ascii_case(&request.family) && f.style == request.style)
            || p.embedded_fonts.iter().any(|(f, _)| f.eq_ignore_ascii_case(&request.family))
        {
            continue;
        }
        if !db.has_family(&request.family) {
            continue;
        }
        let face = db.face(&request.family, &request.style);
        if embedding::permission(face.data(), face.index()) != Permission::Editable {
            continue;
        }
        total = total.saturating_add(face.data().len());
        if total > 256 << 20 {
            continue;
        }
        let Some(bytes) = embedding::standalone(face.data(), face.index()) else {
            continue;
        };
        let id = result.add_media(&format!("{}-{}.ttf", request.family, request.style), "font/ttf", bytes);
        result.embedded_font_faces.push(EmbeddedFont { family: request.family, style: request.style, media: id });
    }
    result
}

/// Find a font only in this presentation, without changing global family lookup.
pub fn embedded_face(p: &Presentation, family: &str, style: &str) -> Option<std::sync::Arc<deckcraft_fonts::FontFace>> {
    let id = p
        .embedded_font_faces
        .iter()
        .find(|f| f.family.eq_ignore_ascii_case(family) && f.style.eq_ignore_ascii_case(style))
        .map(|f| f.media)
        .or_else(|| p.embedded_fonts.iter().find(|(f, _)| f.eq_ignore_ascii_case(family)).map(|(_, id)| *id))?;
    FontDb::global().embedded_data(&p.media(id)?.data, family, style)
}

pub fn for_document(p: &Presentation, family: &str, style: &str) -> std::sync::Arc<deckcraft_fonts::FontFace> {
    embedded_face(p, family, style).unwrap_or_else(|| FontDb::global().document_face(family, style))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparing_fonts_is_opt_out_idempotent_and_keeps_live_document_unchanged() {
        let mut p = Presentation::default();
        std::sync::Arc::make_mut(&mut p.masters[0]).theme.fonts.minor.latin = "Ubuntu".into();
        let prepared = prepare(&p);
        assert!(p.embedded_font_faces.is_empty());
        assert!(prepared.embedded_font_faces.iter().any(|f| f.family == "Ubuntu"));
        assert_eq!(prepare(&prepared).embedded_font_faces, prepared.embedded_font_faces);
        p.auto_embed_fonts = false;
        assert!(prepare(&p).embedded_font_faces.is_empty());
    }
    #[test]
    fn localized_system_family_resolves_when_installed() {
        let db = FontDb::global();
        if db.has_family("Microsoft YaHei") {
            assert!(db.has_family("微软雅黑"));
            assert_eq!(db.face("微软雅黑", "Bold").family, "Microsoft YaHei");
            assert!(db.face("微软雅黑", "Bold").weight >= 550.0);
        }
    }
}
