//! On-demand, background font inspection. No full-deck scan on each UI frame.
use crate::{SlideApp, theme::Tokens};
use deckcraft_fonts::embedding::Permission;
use deckcraft_text::document::Usage;

#[derive(Default)]
pub struct State {
    pub open: bool,
    pub refresh: bool,
    key: Option<(u64, u64)>,
    rows: Vec<Usage>,
    pending: Option<std::sync::mpsc::Receiver<((u64, u64), Vec<Usage>)>>,
    error: Option<String>,
    search: String,
    issues_only: bool,
    selected: Option<usize>,
}

pub fn show(app: &mut SlideApp, ctx: &egui::Context) {
    if !app.font_inspector.open {
        return;
    }
    let Some(doc) = app.session.active() else {
        app.font_inspector.open = false;
        return;
    };
    let key = (doc.uid, doc.revision);
    let current_page = doc.selection.slide + 1;
    let page_count = doc.doc.slides.len();
    let mut enabled = doc.doc.auto_embed_fonts;
    let state = &mut app.font_inspector;
    if state.key.is_some_and(|k| k.0 != key.0) {
        state.rows.clear();
        state.key = None;
        state.selected = None;
        state.refresh = true;
    }
    if let Some(rx) = &state.pending {
        match rx.try_recv() {
            Ok((loaded, rows)) => {
                if loaded.0 == key.0 {
                    state.key = Some(loaded);
                    state.rows = rows;
                }
                state.pending = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.pending = None;
                state.key = Some(key);
                state.error = Some("Font inspection failed. Try refreshing.".into());
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
    }
    if (state.refresh || state.key.is_none()) && state.pending.is_none() {
        state.refresh = false;
        state.error = None;
        let p = doc.doc.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = std::sync::mpsc::channel();
            match std::thread::Builder::new().name("font-inspector".into()).spawn(move || {
                let rows = deckcraft_text::document::inspect(&p);
                let _ = tx.send((key, rows));
            }) {
                Ok(_) => state.pending = Some(rx),
                Err(e) => {
                    state.error = Some(e.to_string());
                    state.key = Some(key);
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            state.rows = deckcraft_text::document::inspect(&p);
            state.key = Some(key);
        }
    }
    let zh = crate::i18n::language(ctx) == crate::i18n::Language::SimplifiedChinese;
    let label = |en: &'static str, cn: &'static str| if zh { cn } else { en };
    let t = Tokens::get(ctx);
    let mut open = state.open;
    let mut changed = false;
    let mut jump = None;
    let fresh = state.key == Some(key);
    egui::Window::new(label("Font Inspector", "字体检查器"))
        .id(egui::Id::new("font_inspector_v2"))
        .open(&mut open)
        .default_width(760.0)
        .default_height(590.0)
        .min_width(620.0)
        .min_height(520.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            let missing = state.rows.iter().filter(|r| !r.available).count();
            let embedded = state.rows.iter().filter(|r| r.embedded).count();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label("Document fonts", "文稿字体")).font(crate::theme::bold(21.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(state.pending.is_none(), action(label("Refresh", "重新检查"), t)).clicked() {
                        state.refresh = true;
                    }
                    if state.pending.is_some() {
                        ui.spinner();
                    }
                });
            });
            ui.horizontal(|ui| {
                badge(ui, &format!("{} {}", state.rows.len(), label("font styles", "种字体样式")), false, t);
                badge(ui, &format!("{} {}", missing, label("missing", "项缺失")), missing > 0, t);
                badge(ui, &format!("{} {}", embedded, label("embedded", "项已嵌入")), false, t);
            });
            egui::Frame::new().fill(t.field_bg).corner_radius(6).inner_margin(10.0).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                changed = ui.checkbox(&mut enabled, label("Embed fonts automatically on save", "保存时自动打包字体")).changed();
                ui.label(
                    egui::RichText::new(label(
                        "Includes editable fonts in .deckcraft / .pptx; file size may increase.",
                        "将允许编辑嵌入的字体存入 .deckcraft / .pptx，文件体积可能增加。",
                    ))
                    .small()
                    .color(t.text_dim),
                )
                .on_hover_text(label(
                    "Embedding follows the font permission flags; the font licence still applies.",
                    "按字体文件的嵌入权限处理；字体许可证仍然适用。",
                ));
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut state.search).desired_width(240.0).hint_text(label("Search font name…", "搜索字体名称…")));
                ui.checkbox(&mut state.issues_only, label("Needs attention only", "仅看需注意项"));
            });
            if let Some(error) = &state.error {
                ui.colored_label(t.accent, format!("{} {error}", label("Inspection failed:", "检查失败：")));
            } else if state.pending.is_some() {
                ui.label(egui::RichText::new(label("Checking document fonts…", "正在检查文稿字体…")).color(t.text_dim));
            } else if !fresh {
                ui.colored_label(t.accent, label("Document changed. Refresh to enable page navigation.", "文稿已修改，重新检查后可点击页码跳转。"));
            }
            let query = state.search.trim().to_lowercase();
            let visible: Vec<usize> = state
                .rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row.family.to_lowercase().contains(&query) && (!state.issues_only || needs_attention(row)))
                .map(|(index, _)| index)
                .collect();
            if !state.selected.is_some_and(|index| visible.contains(&index)) {
                state.selected = visible.first().copied();
            }
            // A stable viewport avoids resize feedback as the selected font's details change.
            let list_height = (ctx.content_rect().height() * 0.28).clamp(140.0, 280.0);
            egui::Frame::new().stroke(egui::Stroke::new(1.0, t.border)).corner_radius(6).inner_margin(8.0).show(ui, |ui| {
                let width = ui.available_width();
                let name_width = (width - 240.0).max(220.0);
                let (header, _) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
                for (x, text) in [
                    (10.0, label("Font / style", "字体 / 样式")),
                    (name_width + 18.0, label("Availability", "可用状态")),
                    (name_width + 150.0, label("Usage", "使用范围")),
                ] {
                    ui.painter().text(
                        header.min + egui::vec2(x, 10.0),
                        egui::Align2::LEFT_CENTER,
                        text,
                        egui::FontId::proportional(12.0),
                        t.text_dim,
                    );
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("font_inspector_list")
                    .max_height(list_height)
                    .min_scrolled_height(list_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if visible.is_empty() {
                            ui.label(label("No matching fonts", "没有符合条件的字体"));
                        }
                        for index in visible {
                            let Some(row) = state.rows.get(index) else { continue };
                            let selected = state.selected == Some(index);
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 40.0), egui::Sense::click());
                            if selected || response.hovered() {
                                ui.painter().rect_filled(rect, 4.0, if selected { t.selected } else { t.hover });
                            }
                            if selected {
                                ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())), 1.0, t.accent);
                            }
                            ui.painter().text(
                                rect.min + egui::vec2(10.0, 12.0),
                                egui::Align2::LEFT_CENTER,
                                clipped_name(&row.family, name_width),
                                crate::theme::bold(14.0),
                                t.text,
                            );
                            ui.painter().text(
                                rect.min + egui::vec2(10.0, 29.0),
                                egui::Align2::LEFT_CENTER,
                                style_name(&row.style, zh),
                                egui::FontId::proportional(11.0),
                                t.text_dim,
                            );
                            let status = if !row.available {
                                label("Missing", "缺失字体")
                            } else if row.embedded {
                                label("Embedded", "已嵌入")
                            } else {
                                label("Installed", "本机可用")
                            };
                            ui.painter().text(
                                rect.min + egui::vec2(name_width + 18.0, 20.0),
                                egui::Align2::LEFT_CENTER,
                                status,
                                egui::FontId::proportional(13.0),
                                if !row.available { t.accent } else { t.text_dim },
                            );
                            let pages = if row.pages.is_empty() {
                                label("Theme defaults", "主题默认").to_owned()
                            } else {
                                format!("{} {}", row.pages.len(), label("pages", "页"))
                            };
                            ui.painter().text(
                                rect.min + egui::vec2(name_width + 150.0, 20.0),
                                egui::Align2::LEFT_CENTER,
                                pages,
                                egui::FontId::proportional(13.0),
                                t.text_dim,
                            );
                            if response.on_hover_text(&row.family).clicked() {
                                state.selected = Some(index);
                            }
                        }
                    });
            });
            if let Some(row) = state.selected.and_then(|index| state.rows.get(index)) {
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&row.family);
                    ui.label(egui::RichText::new(style_name(&row.style, zh)).color(t.text_dim));
                });
                ui.label(egui::RichText::new(embed_reason(row, enabled, zh)).color(if needs_attention(row) { t.accent } else { t.text_dim }));
                ui.label(egui::RichText::new(format!("{} {}", label("Rendered with:", "当前显示使用："), row.replacement)).small().color(t.text_dim));
                if row.stored_unreadable || row.style_substituted || !row.missing_chars.is_empty() {
                    let mut warnings = Vec::new();
                    if row.stored_unreadable {
                        warnings.push(label("Stored font cannot be rendered", "原有嵌入数据保留，但无法解析显示").to_owned());
                    }
                    if row.style_substituted {
                        warnings.push(label("Simulated weight / slant", "粗体或斜体使用模拟样式").to_owned());
                    }
                    if !row.missing_chars.is_empty() {
                        warnings.push(format!("{} {}", label("Fallback glyphs:", "字形回退："), row.missing_chars));
                    }
                    ui.colored_label(t.accent, warnings.join(" · "));
                }
                ui.horizontal(|ui| {
                    ui.strong(label("Used on pages", "使用页面"));
                    ui.label(egui::RichText::new(label("Click to jump and close inspector", "点击页码跳转并收起检查器")).small().color(t.text_dim));
                });
                egui::ScrollArea::vertical()
                    .id_salt("font_inspector_pages")
                    .max_height(88.0)
                    .min_scrolled_height(60.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if row.pages.is_empty() {
                            ui.label(
                                egui::RichText::new(label("Theme / master / layout defaults", "主题 / 母版 / 版式默认字体，未直接用于页面"))
                                    .color(t.text_dim),
                            );
                        }
                        ui.horizontal_wrapped(|ui| {
                            for &page in &row.pages {
                                let active = page == current_page;
                                let button =
                                    egui::Button::new(egui::RichText::new(page.to_string()).color(if active { t.accent_text } else { t.text }))
                                        .fill(if active { t.accent } else { t.field_bg })
                                        .stroke(egui::Stroke::new(1.0, if active { t.accent } else { t.border }))
                                        .min_size(egui::vec2(36.0, 28.0))
                                        .corner_radius(4);
                                if ui
                                    .add_enabled(fresh && page > 0 && page <= page_count, button)
                                    .on_hover_text(format!("{} {page}", label("Go to slide", "跳转到幻灯片")))
                                    .clicked()
                                {
                                    jump = page.checked_sub(1);
                                }
                            }
                        });
                    });
            }
        });
    state.open = open;
    if changed && let Err(e) = app.run("file.fontEmbedding", serde_json::json!({"enabled": enabled})) {
        app.set_status(e);
    }
    if let Some(index) = jump {
        match app.run("slide.go", serde_json::json!({"index": index})) {
            Ok(_) => {
                app.font_inspector.open = false;
                if let Err(e) = app.run("view.normal", serde_json::json!({})) {
                    app.set_status(e);
                }
            }
            Err(e) => app.set_status(e),
        }
    }
}

fn action(text: &str, t: Tokens) -> egui::Button<'_> {
    egui::Button::new(text).fill(t.field_bg).stroke(egui::Stroke::new(1.0, t.border)).min_size(egui::vec2(88.0, 30.0)).corner_radius(5)
}

fn badge(ui: &mut egui::Ui, text: &str, attention: bool, t: Tokens) {
    egui::Frame::new().fill(t.field_bg).corner_radius(4).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.label(egui::RichText::new(text).small().color(if attention { t.accent } else { t.text_dim }));
    });
}

fn needs_attention(row: &Usage) -> bool {
    !row.available
        || row.stored_unreadable
        || row.style_substituted
        || !row.missing_chars.is_empty()
        || !matches!(row.permission, Permission::Editable)
}

fn style_name(style: &str, zh: bool) -> &str {
    if !zh {
        return style;
    }
    match style {
        "Regular" => "常规",
        "Bold" => "粗体",
        "Italic" => "斜体",
        "Bold Italic" => "粗斜体",
        _ => style,
    }
}

fn clipped_name(name: &str, width: f32) -> String {
    // Conservative character budget; never allow long family names to cover the status column.
    let limit = ((width - 26.0) / 14.0).max(1.0) as usize;
    let mut chars = name.chars();
    let result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() { format!("{result}…") } else { result }
}

fn embed_reason(row: &Usage, enabled: bool, zh: bool) -> &'static str {
    let label = |en, cn| if zh { cn } else { en };
    match row.permission {
        Permission::Budget => label("Not embedded: 256 MiB save budget exceeded", "暂不打包 · 超过单次保存 256 MiB 字体预算"),
        Permission::Editable if row.embedded => label("Already included in this document", "已打包 · 文稿中已包含此字体"),
        Permission::Editable if row.stored_unreadable => label("Original embedded data will be retained", "保留数据 · 保存时保留原有嵌入字体"),
        Permission::Editable if enabled => label("Will be embedded on next save", "待打包 · 下次保存时自动嵌入"),
        Permission::Editable => label("Automatic embedding is off", "未打包 · 自动打包已关闭"),
        Permission::Restricted => label("Not embedded: font prohibits embedding", "无法打包 · 字体禁止嵌入"),
        Permission::PreviewOnly => label("Not embedded: preview / print permission only", "无法打包 · 仅允许预览或打印，不允许编辑嵌入"),
        Permission::BitmapOnly => label("Not embedded: bitmap embedding only", "无法打包 · 仅允许位图嵌入"),
        Permission::Unknown => label("Not embedded: font missing or permissions unknown", "无法打包 · 字体缺失或嵌入权限未知"),
        Permission::Unsupported => {
            label("Not embedded: unsupported format, variable font or size limit", "暂不打包 · 格式暂不支持、可变字体或超过大小限制")
        }
    }
}
