use crate::SHEET_COLS;
use asefile::AsepriteFile;
use image::{DynamicImage, GenericImageView, Rgba};
use std::path::PathBuf;

fn pixel_to_index(pixel: Rgba<u8>) -> u8 {
    if pixel[3] < 128 {
        return 0;
    }
    let luma = (pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000;
    (luma >> 4) as u8
}

pub fn parse_char_arg(s: &str) -> Result<u8, String> {
    let s = s.trim();
    if s.len() == 1 {
        let c = s.chars().next().unwrap();
        if c.is_ascii() {
            return Ok(c as u8);
        }
    }
    if s == "space" {
        return Ok(b' ');
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16).map_err(|e| format!("invalid hex: {e}"));
    }
    s.parse::<u8>()
        .map_err(|e| format!("invalid code point: {e}"))
}

pub fn open_image(path: &PathBuf) -> DynamicImage {
    if path
        .extension()
        .map(|s| s.to_str() == Some("aseprite"))
        .unwrap_or(false)
    {
        let ase = AsepriteFile::read_file(path).expect("Aseprite file could not be read");
        DynamicImage::ImageRgba8(ase.frame(0).image())
    } else {
        image::open(path).unwrap_or_else(|e| panic!("Failed to open {:?}: {}", path, e))
    }
}

pub fn extract_glyph_data(
    img: &DynamicImage,
    cell_width: u8,
    cell_height: u8,
    glyph_count: usize,
) -> Vec<u32> {
    let (img_width, img_height) = img.dimensions();
    let sheet_rows = glyph_count.div_ceil(SHEET_COLS);
    assert!(
        img_width >= cell_width as u32 * SHEET_COLS as u32,
        "Image width {} too small for {} columns of {} px cells",
        img_width,
        SHEET_COLS,
        cell_width
    );
    assert!(
        img_height >= cell_height as u32 * sheet_rows as u32,
        "Image height {} too small for {} rows of {} px cells",
        img_height,
        sheet_rows,
        cell_height
    );

    let row_u32s = (cell_width as usize + 7) >> 3;
    let glyph_size = row_u32s * cell_height as usize;
    let mut data = vec![0u32; glyph_size * glyph_count];

    for glyph_idx in 0..glyph_count {
        let sheet_col = glyph_idx % SHEET_COLS;
        let sheet_row = glyph_idx / SHEET_COLS;
        let base_x = sheet_col * cell_width as usize;
        let base_y = sheet_row * cell_height as usize;
        let glyph_base = glyph_idx * glyph_size;

        for row in 0..cell_height as usize {
            for word in 0..row_u32s {
                let mut val: u32 = 0;
                for px in 0..8usize {
                    let x = base_x + word * 8 + px;
                    if x < base_x + cell_width as usize {
                        let pixel = img.get_pixel(x as u32, (base_y + row) as u32);
                        let idx = pixel_to_index(pixel) as u32;
                        val |= idx << (px * 4);
                    }
                }
                data[glyph_base + row * row_u32s + word] = val;
            }
        }
    }

    data
}
