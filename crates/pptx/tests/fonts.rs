//! Font parts must survive import/export and remain document-scoped.
use deckcraft_fonts::{FontDb, embedding};
use deckcraft_model::{EmbeddedFont, Presentation};

#[test]
fn embedded_font_parts_roundtrip_without_installing_fonts() {
    let db = FontDb::with_font_dirs(vec![]);
    let face = db.face("Ubuntu", "Regular");
    let mut p = Presentation { auto_embed_fonts: false, ..Default::default() };
    let media = p.add_media("ubuntu.ttf", "font/ttf", face.data().to_vec());
    p.embedded_font_faces.push(EmbeddedFont { family: "Document private font".into(), style: "Regular".into(), media });
    let bytes = deckcraft_pptx::export(&p).unwrap();
    let q = deckcraft_pptx::import(&bytes).unwrap();
    let entry = q.embedded_font_faces.first().unwrap();
    assert_eq!(entry.family, "Document private font");
    assert_eq!(embedding::sfnt(&q.media(entry.media).unwrap().data), Some(face.data()));
    assert!(deckcraft_text::document::embedded_face(&q, &entry.family, "Regular").is_some());
    assert!(!db.has_family(&entry.family));
    assert!(deckcraft_text::document::embedded_face(&Presentation::default(), &entry.family, "Regular").is_none());
    let again = deckcraft_pptx::import(&deckcraft_pptx::export(&q).unwrap()).unwrap();
    assert_eq!(again.embedded_font_faces.len(), 1);
    assert_eq!(again.media(again.embedded_font_faces[0].media).unwrap().data, q.media(entry.media).unwrap().data);
    let zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(zip.file_names().any(|n| n == "ppt/fonts/font1.fntdata"));
}
