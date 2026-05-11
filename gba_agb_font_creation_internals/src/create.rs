use crate::parsing::{extract_glyph_data, open_image};
use crate::{GLYPH_COUNT_FULL, GLYPH_COUNT_SMALL, SHEET_COLS};
use image::{DynamicImage, GenericImageView};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

pub fn create_bytes(
    cell_width: u8,
    cell_height: u8,
    img: &DynamicImage,
    monospace: Option<Option<u8>>,
    width_overrides: &[(u8, u8)],
) -> Vec<u8> {
    let (_, img_height) = img.dimensions();
    let cell_rows = img_height / cell_height as u32;
    let (glyph_count, mode_byte): (usize, u8) =
        if cell_rows as usize * SHEET_COLS >= GLYPH_COUNT_FULL {
            (GLYPH_COUNT_FULL, 1)
        } else {
            (GLYPH_COUNT_SMALL, 0)
        };

    let data = extract_glyph_data(img, cell_width, cell_height, glyph_count);

    let row_u32s = (cell_width as usize + 7) >> 3;
    let glyph_size = row_u32s * cell_height as usize;

    let mut char_widths = vec![cell_width; glyph_count];
    #[allow(clippy::needless_range_loop)]
    for glyph_idx in 0..glyph_count {
        let glyph_base = glyph_idx * glyph_size;
        let mut max_set_px = 0usize;
        for row in 0..cell_height as usize {
            for word in 0..row_u32s {
                let val = data[glyph_base + row * row_u32s + word];
                for px in 0..8usize {
                    let x = word * 8 + px;
                    if x < cell_width as usize && (val >> (px * 4)) & 0xF != 0 && x + 1 > max_set_px
                    {
                        max_set_px = x + 1;
                    }
                }
            }
        }
        char_widths[glyph_idx] = if max_set_px == 0 { 1 } else { max_set_px as u8 };
    }

    if let Some(flag) = monospace {
        let mono_width = flag.unwrap_or_else(|| *char_widths.iter().max().unwrap_or(&cell_width));
        char_widths.fill(mono_width);
    }

    for &(cp, w) in width_overrides {
        let idx = if mode_byte == 1 {
            Some(cp as usize)
        } else if (32..=126).contains(&cp) {
            Some((cp - 32) as usize)
        } else {
            None
        };
        if let Some(i) = idx
            && i < glyph_count
        {
            char_widths[i] = w;
        }
    }

    let mut out = Vec::new();
    out.extend_from_slice(&[mode_byte, cell_width, cell_height]);
    out.extend_from_slice(&char_widths);
    out.push(0);
    if mode_byte == 0 {
        out.push(0);
    }
    for d in &data {
        out.extend_from_slice(&d.to_le_bytes());
    }
    out
}

pub fn create(
    cell_width: u8,
    cell_height: u8,
    png_path: &PathBuf,
    output: Option<&PathBuf>,
    monospace: Option<Option<u8>>,
) {
    let output_path: PathBuf = if let Some(output) = output {
        if output.is_dir() {
            panic!("Output {output:?} is a directory");
        } else {
            output.clone()
        }
    } else {
        let file_name = png_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| String::from("font"));
        let file_name = format!("{}.bin", file_name);
        if let Some(parent) = png_path.parent() {
            PathBuf::from(parent).join(file_name)
        } else {
            PathBuf::from(file_name)
        }
    };

    let img = open_image(png_path);
    let (_, img_height) = img.dimensions();
    let glyph_count = {
        let cell_rows = img_height / cell_height as u32;
        if cell_rows as usize * SHEET_COLS >= GLYPH_COUNT_FULL {
            GLYPH_COUNT_FULL
        } else {
            GLYPH_COUNT_SMALL
        }
    };

    let bytes = create_bytes(cell_width, cell_height, &img, monospace, &[]);

    let mut out = BufWriter::new(File::create(&output_path).expect("failed to create output file"));
    out.write_all(&bytes).unwrap();
    out.flush().unwrap();

    println!("Written to {output_path:?} ({} glyphs)", glyph_count);
}
