//! Visible fallbacks shared by the canvas and portable PPTX export.
use std::sync::Arc;

use deckcraft_color::Rgba;
use deckcraft_geom::{Size, Xfrm};
use deckcraft_model::text::{Align, Anchor, AutoFit, TextBody};
use deckcraft_model::{ColorRef, Fill, Geom, Line, Presentation, Shape, Slide};

/// A regular editable shape, without dependencies on the failed object's style or media.
pub fn compatibility_notice_shape(source: &Shape, reason: &str) -> Shape {
    let mut text = TextBody::from_text(&format!("内容无法显示\n{reason}"));
    text.body.anchor = Some(Anchor::Middle);
    text.body.autofit = Some(AutoFit::Shrink { font_scale: 1.0, line_reduction: 0.0 });
    text.body.inset_l = Some(4.0);
    text.body.inset_r = Some(4.0);
    text.body.inset_t = Some(2.0);
    text.body.inset_b = Some(2.0);
    let height = source.xfrm.map_or(100.0, |x| x.h);
    let size = (height / 7.0).clamp(5.0, 18.0);
    for p in &mut text.paragraphs {
        p.props.align = Some(Align::Center);
        for r in &mut p.runs {
            r.props.size = Some(size);
            r.props.fill = Some(Fill::solid(ColorRef::rgb(Rgba::rgb(110, 65, 15))));
        }
    }
    Shape {
        id: source.id,
        name: source.name.clone(),
        xfrm: source.xfrm,
        ph: source.ph.clone(),
        hidden: source.hidden,
        locked: source.locked,
        descr: format!("兼容性提示：{reason}"),
        geom: Geom::preset("rect"),
        fill: Some(Fill::solid(ColorRef::rgb(Rgba::rgb(255, 242, 212)))),
        line: Some(Line::solid(ColorRef::rgb(Rgba::rgb(191, 126, 30)), 1.0)),
        text: Some(text),
        ..Default::default()
    }
}

/// Self-contained rendering: never borrow the thread-local slide renderer recursively.
pub fn compatibility_notice_image(reason: &str) -> crate::Image {
    let mut p = Presentation { slide_size: Size::new(480.0, 240.0), ..Default::default() };
    let source = Shape { xfrm: Some(Xfrm::new(0.5, 0.5, 479.0, 239.0)), ..Default::default() };
    let slide = Slide { shapes: vec![compatibility_notice_shape(&source, reason)], show_master_shapes: false, ..Default::default() };
    p.slides = vec![Arc::new(slide.clone())];
    crate::Renderer::default().slide(&p, &slide, 0, &crate::RenderOpts::default())
}
