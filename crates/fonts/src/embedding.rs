//! OpenType document embedding permissions and uncompressed EOT (W3C EOT submission,
//! https://www.w3.org/submissions/EOT/). This never installs or redistributes fonts with the app.
use skrifa::MetadataProvider;
use skrifa::raw::types::Tag;
use skrifa::string::StringId;

pub const MAX_FONT_BYTES: usize = 40 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Editable,
    Restricted,
    PreviewOnly,
    BitmapOnly,
    Unknown,
    Unsupported,
    Budget,
}

/// OS/2 fsType governs document embedding, not installation or application redistribution.
pub fn permission(data: &[u8], index: u32) -> Permission {
    let Ok(f) = skrifa::FontRef::from_index(data, index) else { return Permission::Unsupported };
    let Some(os2) = f.table_data(Tag::new(b"OS/2")) else { return Permission::Unknown };
    let Some(bits) = be16(os2.as_bytes(), 8) else { return Permission::Unknown };
    if bits & !0x030e != 0 {
        return Permission::Unknown;
    }
    if bits & 0x0200 != 0 {
        return Permission::BitmapOnly;
    }
    if bits & 0x0002 != 0 {
        return Permission::Restricted;
    }
    if bits & 0x0004 != 0 && bits & 0x0008 == 0 {
        return Permission::PreviewOnly;
    }
    // Variable instances need flattening, and CFF needs a different Office embedding path.
    if data.len() > MAX_FONT_BYTES || f.table_data(Tag::new(b"fvar")).is_some() || f.table_data(Tag::new(b"glyf")).is_none() {
        return Permission::Unsupported;
    }
    Permission::Editable
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        for (dst, src) in word.iter_mut().zip(chunk) {
            *dst = *src;
        }
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

/// Extract one complete TrueType face from a collection. No glyph subsetting: all editable
/// characters survive and fsType's no-subsetting flag is respected.
pub fn standalone(data: &[u8], index: u32) -> Option<Vec<u8>> {
    if permission(data, index) != Permission::Editable {
        return None;
    }
    if !data.starts_with(b"ttcf") {
        return (index == 0).then(|| data.to_vec());
    }
    let count = be32(data, 8)?;
    if index >= count || count > 256 {
        return None;
    }
    let offset = be32(data, 12usize.checked_add((index as usize).checked_mul(4)?)?)? as usize;
    let n = be16(data, offset.checked_add(4)?)? as usize;
    if n == 0 || n > 512 {
        return None;
    }
    let header_len = 12 + n * 16;
    let mut out = data.get(offset..offset.checked_add(header_len)?)?.to_vec();
    let mut head_at = None;
    for i in 0..n {
        let rec = 12 + i * 16;
        let tag = out.get(rec..rec + 4)?.to_vec();
        let start = be32(&out, rec + 8)? as usize;
        let len = be32(&out, rec + 12)? as usize;
        let table = data.get(start..start.checked_add(len)?)?;
        if out.len().checked_add(len)?.checked_add(3)? > MAX_FONT_BYTES {
            return None;
        }
        let target = out.len();
        out.extend_from_slice(table);
        if tag == b"head" {
            out.get_mut(target + 8..target + 12)?.fill(0);
            head_at = Some(target);
        }
        let sum = checksum(out.get(target..target + len)?);
        out.get_mut(rec + 4..rec + 8)?.copy_from_slice(&sum.to_be_bytes());
        out.get_mut(rec + 8..rec + 12)?.copy_from_slice(&(target as u32).to_be_bytes());
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    let adjustment = 0xb1b0_afbau32.wrapping_sub(checksum(&out));
    let head = head_at?;
    out.get_mut(head + 8..head + 12)?.copy_from_slice(&adjustment.to_be_bytes());
    Some(out)
}

/// Build the EOT wrapper used by PresentationML embedded-font relationships.
pub fn to_eot(data: &[u8]) -> Option<Vec<u8>> {
    if permission(data, 0) != Permission::Editable || data.starts_with(b"ttcf") {
        return None;
    }
    let f = skrifa::FontRef::new(data).ok()?;
    let os = f.table_data(Tag::new(b"OS/2"))?;
    let os = os.as_bytes();
    let head = f.table_data(Tag::new(b"head"))?;
    let mut out = vec![0; 4];
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x0002_0001u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // uncompressed, no XOR
    out.extend_from_slice(os.get(32..42)?);
    out.push(1); // DEFAULT_CHARSET
    out.push(u8::from(be16(os, 62)? & 1 != 0));
    out.extend_from_slice(&u32::from(be16(os, 4)?).to_le_bytes());
    out.extend_from_slice(&be16(os, 8)?.to_le_bytes());
    out.extend_from_slice(&0x504cu16.to_le_bytes());
    for offset in [42, 46, 50, 54, 78, 82] {
        out.extend_from_slice(&be32(os, offset).unwrap_or(0).to_le_bytes());
    }
    out.extend_from_slice(&be32(head.as_bytes(), 8)?.to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    for id in [StringId::FAMILY_NAME, StringId::SUBFAMILY_NAME, StringId::VERSION_STRING, StringId::FULL_NAME] {
        let name = f.localized_strings(id).english_or_first()?.to_string();
        let bytes: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let len = u16::try_from(bytes.len()).ok()?;
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&bytes);
    }
    out.extend_from_slice(&[0; 4]); // padding, empty root string
    out.extend_from_slice(data);
    let size = u32::try_from(out.len()).ok()?;
    out.get_mut(..4)?.copy_from_slice(&size.to_le_bytes());
    Some(out)
}

/// Read raw SFNT or uncompressed, unrestricted EOT. Other EOT variants are retained for
/// round-trip, but are not passed to the font parser or silently treated as usable.
pub fn sfnt(data: &[u8]) -> Option<&[u8]> {
    if data.len() > MAX_FONT_BYTES + (1 << 20) {
        return None;
    }
    if data.starts_with(&[0, 1, 0, 0]) || data.starts_with(b"OTTO") || data.starts_with(b"ttcf") {
        return skrifa::FontRef::from_index(data, 0).ok().map(|_| data);
    }
    if le32(data, 0)? as usize != data.len() || data.get(34..36)? != [0x4c, 0x50] || le32(data, 12)? != 0 {
        return None;
    }
    let version = le32(data, 8)?;
    if !matches!(version, 0x0001_0000 | 0x0002_0001) {
        return None;
    }
    let mut at = 80usize;
    for _ in 0..4 {
        if data.get(at..at + 2)? != [0, 0] {
            return None;
        }
        let len = u16::from_le_bytes(data.get(at + 2..at + 4)?.try_into().ok()?) as usize;
        if !len.is_multiple_of(2) {
            return None;
        }
        at = at.checked_add(4)?.checked_add(len)?;
    }
    if version == 0x0002_0001 {
        // Root restrictions cannot be satisfied by a local document renderer.
        if data.get(at..at + 4)? != [0; 4] {
            return None;
        }
        at += 4;
    }
    let len = le32(data, 4)? as usize;
    if at.checked_add(len)? != data.len() {
        return None;
    }
    let font = data.get(at..)?;
    skrifa::FontRef::new(font).ok().map(|_| font)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eot_is_bounded_and_roundtrips_full_font() {
        let font = epaint_default_fonts::UBUNTU_LIGHT;
        let eot = to_eot(font).unwrap();
        assert_eq!(sfnt(&eot), Some(font));
        for n in [0, 4, 36, 80, eot.len() - 1] {
            assert!(sfnt(&eot[..n]).is_none());
        }
        let mut compressed = eot;
        compressed[12] = 4;
        assert!(sfnt(&compressed).is_none());
    }

    #[test]
    fn collection_extraction_keeps_tables_and_correct_checksum() {
        let font = epaint_default_fonts::UBUNTU_LIGHT;
        let mut ttc = b"ttcf".to_vec();
        ttc.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        ttc.extend_from_slice(&1u32.to_be_bytes());
        ttc.extend_from_slice(&16u32.to_be_bytes());
        ttc.extend_from_slice(font);
        for i in 0..be16(font, 4).unwrap() as usize {
            let rec = 16 + 12 + i * 16;
            let offset = be32(&ttc, rec + 8).unwrap() + 16;
            ttc[rec + 8..rec + 12].copy_from_slice(&offset.to_be_bytes());
        }
        let extracted = standalone(&ttc, 0).unwrap();
        assert_eq!(checksum(&extracted), 0xb1b0_afba);
        assert!(to_eot(&extracted).is_some());
        assert!(standalone(&ttc, 1).is_none());
        let original = skrifa::FontRef::new(font).unwrap();
        let restored = skrifa::FontRef::new(&extracted).unwrap();
        assert_eq!(original.charmap().map('A'), restored.charmap().map('A'));
    }

    #[test]
    fn restricted_preview_and_bitmap_fonts_are_not_packaged() {
        let mut font = epaint_default_fonts::UBUNTU_LIGHT.to_vec();
        let n = be16(&font, 4).unwrap() as usize;
        let rec = (0..n).map(|i| 12 + i * 16).find(|i| &font[*i..*i + 4] == b"OS/2").unwrap();
        let at = be32(&font, rec + 8).unwrap() as usize + 8;
        for (bits, expected) in [(2u16, Permission::Restricted), (4, Permission::PreviewOnly), (512, Permission::BitmapOnly)] {
            font[at..at + 2].copy_from_slice(&bits.to_be_bytes());
            assert_eq!(permission(&font, 0), expected);
            assert!(standalone(&font, 0).is_none());
            assert!(to_eot(&font).is_none());
        }
        font[at..at + 2].copy_from_slice(&0x0108u16.to_be_bytes());
        assert_eq!(standalone(&font, 0).unwrap(), font); // no subset allowed: all glyphs retained
    }
}
