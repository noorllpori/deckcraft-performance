//! ECMA-376 PresentationML embeddedFontLst and font part relationships.
use super::{Exp, Out};
use crate::xml::{A, W};
use deckcraft_fonts::embedding;
use deckcraft_model::{EmbeddedFont, MediaId};
use std::collections::{BTreeMap, HashMap};

pub fn write(w: &mut W, x: &mut Exp, o: &mut Out) {
    let mut entries = x.p.embedded_font_faces.clone();
    entries.extend(x.p.embedded_fonts.iter().map(|(family, media)| EmbeddedFont { family: family.clone(), style: "Regular".into(), media: *media }));
    let mut groups: BTreeMap<String, (String, BTreeMap<usize, String>)> = BTreeMap::new();
    let mut parts: HashMap<MediaId, String> = HashMap::new();
    for entry in entries {
        let slot = match entry.style.as_str() {
            "Regular" => 0,
            "Bold" => 1,
            "Italic" => 2,
            "Bold Italic" => 3,
            _ => continue,
        };
        let Some(media) = x.p.media(entry.media).filter(|m| m.link.is_none() && !m.data.is_empty()) else { continue };
        let rid = if let Some(rid) = parts.get(&entry.media) {
            rid.clone()
        } else {
            let data = if media.content_type == "application/x-fontdata" {
                // Already embedded parts are preserved byte-for-byte, including compressed EOT
                // variants this renderer cannot use. No installed fonts are accessed here.
                media.data.as_ref().clone()
            } else {
                let Some(raw) = embedding::sfnt(&media.data) else { continue };
                let Some(eot) = embedding::to_eot(raw) else { continue };
                eot
            };
            let name = format!("ppt/fonts/font{}.fntdata", parts.len() + 1);
            x.pkg.binary(&name, "application/x-fontdata", data);
            let rid = o.rels.add("font", &crate::opc::relative(&o.name, &name), false);
            parts.insert(entry.media, rid.clone());
            rid
        };
        groups.entry(entry.family.to_lowercase()).or_insert_with(|| (entry.family, BTreeMap::new())).1.entry(slot).or_insert(rid);
    }
    if groups.is_empty() {
        return;
    }
    w.open0("p:embeddedFontLst");
    for (family, slots) in groups.into_values() {
        w.open0("p:embeddedFont");
        w.empty("p:font", A::new().a("typeface", family));
        for (slot, rid) in slots {
            if let Some(tag) = ["p:regular", "p:bold", "p:italic", "p:boldItalic"].get(slot) {
                w.empty(tag, A::new().a("r:id", rid));
            }
        }
        w.close("p:embeddedFont");
    }
    w.close("p:embeddedFontLst");
}
