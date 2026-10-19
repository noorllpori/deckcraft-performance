//! Image relationships always carry portable embedded bytes. Never mutate the source document.
use std::sync::Arc;

use deckcraft_model::MediaItem;

pub struct Picture {
    pub bytes: Arc<Vec<u8>>,
    pub mime: &'static str,
    pub ext: &'static str,
}

/// Export the last accepted in-memory preview, matching what the user edited. A link with no
/// cached bytes may be read locally; exporting never fetches a remote URL or changes the model.
fn source(item: &MediaItem) -> Result<Arc<Vec<u8>>, &'static str> {
    if !item.data.is_empty() {
        return Ok(item.data.clone());
    }
    let link = item.link.as_deref().ok_or("图片数据缺失")?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Read;
        let path = if link.to_ascii_lowercase().starts_with("file:") {
            url::Url::parse(link).ok().and_then(|u| u.to_file_path().ok()).ok_or("外部图片路径无效")?
        } else if link.contains("://") {
            return Err("外部图片尚未载入，请先刷新或嵌入图片");
        } else {
            std::path::PathBuf::from(link)
        };
        // Relative paths need the document base directory, which is not available here.
        if !path.is_absolute() {
            return Err("相对路径图片未载入，请先刷新或嵌入图片");
        }
        let file = std::fs::File::open(&path).map_err(|_| "外部图片无法读取")?;
        let before = file.metadata().map_err(|_| "外部图片无法读取")?;
        const LIMIT: u64 = 512 << 20;
        if !before.is_file() || before.len() > LIMIT {
            return Err("图片超过导出安全上限或不是普通文件");
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(before.len() as usize).map_err(|_| "图片读取内存不足")?;
        file.take(LIMIT + 1).read_to_end(&mut bytes).map_err(|_| "外部图片读取失败")?;
        let after = std::fs::metadata(path).map_err(|_| "外部图片无法读取")?;
        if bytes.len() as u64 > LIMIT
            || bytes.len() as u64 != before.len()
            || before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
        {
            return Err("外部图片正在写入，请稍后重新导出");
        }
        Ok(Arc::new(bytes))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = link;
        Err("外部图片未载入，请先嵌入图片")
    }
}

pub fn picture(item: &MediaItem) -> Result<Picture, &'static str> {
    let bytes = source(item)?;
    let decoded = deckcraft_render::decode_image(&bytes).ok_or("格式不支持、图片损坏或超过解码上限")?;
    // Office supports these vectors directly. Rasterising them through our renderer bakes
    // font substitutions and incomplete GDI rendering into the exchange file (e.g. tables).
    if bytes.get(0..4) == Some(&[1, 0, 0, 0]) && bytes.get(40..44) == Some(b" EMF") {
        return Ok(Picture { bytes, mime: "image/x-emf", ext: "emf" });
    }
    if bytes.starts_with(&[0xd7, 0xcd, 0xc6, 0x9a]) || (matches!(bytes.get(0..2), Some([1, 0] | [2, 0])) && bytes.get(2..4) == Some(&[9, 0])) {
        return Ok(Picture { bytes, mime: "image/x-wmf", ext: "wmf" });
    }
    // PNG/JPEG/GIF have broad Office support; retain their original bytes and GIF frames.
    let direct = match image::guess_format(&bytes).ok() {
        Some(image::ImageFormat::Png) => Some(("image/png", "png")),
        Some(image::ImageFormat::Jpeg) => Some(("image/jpeg", "jpeg")),
        Some(image::ImageFormat::Gif) => Some(("image/gif", "gif")),
        _ => None,
    };
    if let Some((mime, ext)) = direct {
        return Ok(Picture { bytes, mime, ext });
    }
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, decoded.width(), decoded.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // Tiled fills depend on physical image size. Keep the decoder's natural size when
        // changing formats, otherwise a high-DPI source would change the exported tile scale.
        if let Some((nw, nh)) = deckcraft_render::image_natural_size(&bytes) {
            let ppm = |pixels: u32, natural: f64| (f64::from(pixels) / natural * 96.0 / 0.0254).round().clamp(1.0, u32::MAX as f64) as u32;
            if nw > 0.0 && nh > 0.0 && nw.is_finite() && nh.is_finite() {
                encoder.set_pixel_dims(Some(png::PixelDimensions {
                    xppu: ppm(decoded.width(), nw),
                    yppu: ppm(decoded.height(), nh),
                    unit: png::Unit::Meter,
                }));
            }
        }
        let mut writer = encoder.write_header().map_err(|_| "图片转换为 PNG 失败")?;
        writer.write_image_data(decoded.as_raw()).map_err(|_| "图片转换为 PNG 失败")?;
        writer.finish().map_err(|_| "图片转换为 PNG 失败")?;
    }
    Ok(Picture { bytes: Arc::new(output), mime: "image/png", ext: "png" })
}
