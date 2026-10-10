//! Windows bitmap decoding: DIBs, `TBitmap`/`TIcon` property streams and ImageList streams.

#[derive(Debug, Clone)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    /// Row-major, top-down, 4 bytes per pixel.
    pub pixels: Vec<u8>,
}

impl Rgba {
    pub fn new(width: usize, height: usize) -> Rgba {
        Rgba { width, height, pixels: vec![0; width * height * 4] }
    }
    pub fn get(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.width + x) * 4;
        self.pixels[i..i + 4].try_into().unwrap()
    }
    pub fn set(&mut self, x: usize, y: usize, px: [u8; 4]) {
        let i = (y * self.width + x) * 4;
        self.pixels[i..i + 4].copy_from_slice(&px);
    }
    pub fn crop(&self, x0: usize, y0: usize, w: usize, h: usize) -> Rgba {
        let mut out = Rgba::new(w, h);
        for y in 0..h {
            for x in 0..w {
                if x0 + x < self.width && y0 + y < self.height {
                    out.set(x, y, self.get(x0 + x, y0 + y));
                }
            }
        }
        out
    }
}

fn u16_at(b: &[u8], o: usize) -> Result<u16, String> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "bitmap truncated".into())
}
fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| "bitmap truncated".into())
}

/// Decode a DIB (BITMAPINFOHEADER + optional palette + pixels) starting at `b[0]`.
/// `pixels_at` is where pixel data starts (relative to `b`), or None to follow the palette.
/// `height_override` is used for icons, whose header height covers XOR+AND masks.
/// Returns the image and the number of bytes consumed.
pub fn decode_dib(b: &[u8], pixels_at: Option<usize>, height_override: Option<i32>) -> Result<(Rgba, usize), String> {
    let hdr = u32_at(b, 0)? as usize;
    let w = u32_at(b, 4)? as i32;
    let h_raw = height_override.unwrap_or(u32_at(b, 8)? as i32);
    let bpp = u16_at(b, 14)? as usize;
    let comp = u32_at(b, 16)?;
    if comp != 0 && comp != 3 {
        return Err(format!("unsupported bitmap compression {comp}"));
    }
    let mut clr_used = u32_at(b, 32)? as usize;
    if clr_used == 0 && bpp <= 8 {
        clr_used = 1 << bpp;
    }
    let pal_at = hdr + if comp == 3 { 12 } else { 0 };
    let palette: Vec<[u8; 4]> = (0..clr_used)
        .map(|i| {
            let o = pal_at + 4 * i;
            b.get(o..o + 4).map(|p| [p[2], p[1], p[0], 255]).ok_or_else(|| "palette truncated".to_string())
        })
        .collect::<Result<_, _>>()?;
    let data_at = pixels_at.unwrap_or(pal_at + 4 * clr_used);
    let (width, height) = (w.unsigned_abs() as usize, h_raw.unsigned_abs() as usize);
    let stride = (width * bpp + 31) / 32 * 4;
    let top_down = h_raw < 0;
    let mut img = Rgba::new(width, height);
    for row in 0..height {
        let src = data_at + row * stride;
        let line = b.get(src..src + stride).ok_or("bitmap pixel data truncated")?;
        let y = if top_down { row } else { height - 1 - row };
        for x in 0..width {
            let px = match bpp {
                1 => palette[((line[x / 8] >> (7 - x % 8)) & 1) as usize],
                4 => palette[((line[x / 2] >> if x % 2 == 0 { 4 } else { 0 }) & 0xf) as usize],
                8 => palette[line[x] as usize],
                16 => {
                    let v = u16::from_le_bytes([line[2 * x], line[2 * x + 1]]);
                    let (r, g, bl) = ((v >> 10) & 31, (v >> 5) & 31, v & 31);
                    [(r << 3) as u8, (g << 3) as u8, (bl << 3) as u8, 255]
                }
                24 => [line[3 * x + 2], line[3 * x + 1], line[3 * x], 255],
                32 => [line[4 * x + 2], line[4 * x + 1], line[4 * x], 255],
                _ => return Err(format!("unsupported bit depth {bpp}")),
            };
            img.set(x, y, px);
        }
    }
    Ok((img, data_at + stride * height))
}

/// Decode a BMP file ("BM" + BITMAPFILEHEADER). The `bfSize` field is ignored because
/// ImageList streams store a bogus value there.
pub fn decode_bmp_file(b: &[u8]) -> Result<(Rgba, usize), String> {
    if b.get(0..2) != Some(b"BM") {
        return Err("missing BM signature".into());
    }
    let off_bits = u32_at(b, 10)? as usize;
    let (img, end) = decode_dib(&b[14..], Some(off_bits - 14), None)?;
    Ok((img, end + 14))
}

/// `Picture.Data` property: shortstring class name, then the graphic's own stream.
pub fn decode_picture(data: &[u8]) -> Result<Rgba, String> {
    let n = *data.first().ok_or("empty picture")? as usize;
    let class = std::str::from_utf8(&data[1..1 + n]).map_err(|e| e.to_string())?;
    let rest = &data[1 + n..];
    match class {
        // TBitmap.ReadData: Int32 size, then a BMP file.
        "TBitmap" => Ok(decode_bmp_file(&rest[4..])?.0),
        "TIcon" => decode_ico(rest),
        _ => Err(format!("unsupported picture class {class}")),
    }
}

/// First image of an .ico file (XOR bitmap + AND mask).
pub fn decode_ico(b: &[u8]) -> Result<Rgba, String> {
    let count = u16_at(b, 4)? as usize;
    if count == 0 {
        return Err("icon has no images".into());
    }
    let off = u32_at(b, 6 + 12)? as usize;
    let dib = &b[off..];
    let full_h = u32_at(dib, 8)? as i32;
    let h = full_h / 2;
    let (mut img, end) = decode_dib(dib, None, Some(h))?;
    let w = img.width;
    let mask_stride = (w + 31) / 32 * 4;
    for row in 0..h as usize {
        let y = h as usize - 1 - row;
        for x in 0..w {
            let byte = dib.get(end + row * mask_stride + x / 8).copied().unwrap_or(0);
            if (byte >> (7 - x % 8)) & 1 == 1 {
                let mut p = img.get(x, y);
                p[3] = 0;
                img.set(x, y, p);
            }
        }
    }
    Ok(img)
}

/// Decoded ImageList: one RGBA image per index.
#[derive(Debug, Clone)]
pub struct ImageList {
    pub width: usize,
    pub height: usize,
    pub images: Vec<Rgba>,
}

/// Win32 `ImageList_Write` stream ("IL" header, image bitmap, optional mask bitmap),
/// as stored in a Delphi TImageList's `Bitmap` property.
pub fn decode_imagelist(b: &[u8]) -> Result<ImageList, String> {
    if b.get(0..2) != Some(b"IL") {
        return Err("missing IL signature".into());
    }
    let count = u16_at(b, 4)? as usize;
    let cx = u16_at(b, 10)? as usize;
    let cy = u16_at(b, 12)? as usize;
    let flags = u16_at(b, 18)?;
    let (sheet, used) = decode_bmp_file(&b[28..])?;
    let mask = if flags & 1 != 0 && b.get(28 + used..30 + used) == Some(b"BM") {
        Some(decode_bmp_file(&b[28 + used..])?.0)
    } else {
        None
    };
    let cols = (sheet.width / cx).max(1);
    let images = (0..count)
        .map(|i| {
            let (x0, y0) = ((i % cols) * cx, (i / cols) * cy);
            let mut tile = sheet.crop(x0, y0, cx, cy);
            if let Some(m) = &mask {
                for y in 0..cy {
                    for x in 0..cx {
                        // Mask: white = transparent.
                        let mp = m.get(x0 + x, y0 + y);
                        let mut p = tile.get(x, y);
                        p[3] = 255 - mp[0];
                        tile.set(x, y, p);
                    }
                }
            }
            tile
        })
        .collect();
    Ok(ImageList { width: cx, height: cy, images })
}
