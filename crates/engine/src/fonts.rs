//! Fonts a presentation uses that this machine doesn't have, so opening it can say so (its text
//! draws in a stand-in; see `FontDb::document_face`).

use deckcraft_fonts::FontDb;
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::text::{Bullet, TextBody};
use deckcraft_model::{Presentation, Shape, ShapeKind};

/// The font families the slides' text is drawn in (theme fonts resolved, bullet fonts included)
/// that aren't installed or bundled, sorted, without duplicates (case-insensitive).
pub fn missing_fonts(p: &Presentation) -> Vec<String> {
    let db = FontDb::global();
    let mut out: Vec<String> = Vec::new();
    for family in used_fonts(p) {
        if !db.has_family(&family)
            && deckcraft_text::document::embedded_face(p, &family, "Regular").is_none()
            && !out.iter().any(|f| f.eq_ignore_ascii_case(&family))
        {
            out.push(family);
        }
    }
    out.sort_by_key(|f| f.to_lowercase());
    out
}

/// The families the slides' text runs and bullets are drawn in.
fn used_fonts(p: &Presentation) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |f: String| {
        let f = f.trim().to_string();
        if !f.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&f)) {
            out.push(f);
        }
    };
    for slide in &p.slides {
        let Some(ctx) = Ctx::for_slide(p, slide) else { continue };
        deckcraft_model::walk(&slide.shapes, &mut |s, _| {
            if let Some(body) = &s.text {
                body_fonts(&ctx, s, body, &mut add);
            }
            if let ShapeKind::Table(t) = &s.kind {
                for cell in t.rows.iter().flat_map(|r| &r.cells) {
                    body_fonts(&ctx, s, &cell.text, &mut add);
                }
            }
        });
    }
    out
}

fn body_fonts(ctx: &Ctx, s: &Shape, body: &TextBody, add: &mut dyn FnMut(String)) {
    for para in &body.paragraphs {
        let has_text = para.runs.iter().any(|r| !r.text.trim().is_empty());
        if !has_text {
            continue;
        }
        for run in para.runs.iter().filter(|r| !r.text.trim().is_empty()) {
            let r = resolve::run(ctx, s, para, &run.props);
            add(resolve::font_family(ctx, &r));
            for f in [&r.font_ea, &r.font_cs, &r.font_sym].into_iter().flatten() {
                add(ctx.master.theme.font(f));
            }
        }
        let pp = resolve::para(ctx, s, para);
        if matches!(pp.bullet, Some(Bullet::Char { .. }))
            && let Some(f) = pp.bullet_font.as_deref()
        {
            add(ctx.master.theme.font(f));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_save_embeds_fonts_and_keeps_the_live_model_unchanged() {
        let mut p = Presentation::default();
        std::sync::Arc::make_mut(&mut p.masters[0]).theme.fonts.minor.latin = "Ubuntu".into();
        let q = deckcraft_format::load(&crate::cmd::file::save_bytes(&p, "deckcraft").unwrap()).unwrap();
        assert!(p.embedded_font_faces.is_empty());
        let font = q.embedded_font_faces.iter().find(|f| f.family == "Ubuntu").unwrap();
        assert!(!q.media(font.media).unwrap().data.is_empty());
        assert!(deckcraft_text::document::embedded_face(&q, "Ubuntu", "Regular").is_some());
        p.auto_embed_fonts = false;
        let q = deckcraft_format::load(&crate::cmd::file::save_bytes(&p, "deckcraft").unwrap()).unwrap();
        assert!(!q.auto_embed_fonts);
        assert!(q.embedded_font_faces.is_empty());
    }
}
