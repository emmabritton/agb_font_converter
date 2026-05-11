use crate::parsing::{extract_glyph_data, open_image};
use crate::{GLYPH_COUNT_FULL, GLYPH_COUNT_SMALL, SHEET_COLS};
use image::GenericImageView;
use std::fs::File;
use std::io::BufWriter;
use std::io::{Read, Write};
use std::path::PathBuf;

pub fn run_update(bin_path: &PathBuf, img_path: &PathBuf) {
    let mut bytes = {
        let mut f =
            File::open(bin_path).unwrap_or_else(|e| panic!("Failed to open {bin_path:?}: {e}"));
        let mut b = Vec::new();
        f.read_to_end(&mut b).expect("failed to read file");
        b
    };

    if bytes.len() < 3 {
        panic!("File too short to be a valid font binary");
    }

    let mode_byte = bytes[0];
    let cell_width = bytes[1];
    let cell_height = bytes[2];

    let (glyph_count, widths_len, pixel_data_offset) = match mode_byte {
        1 => (GLYPH_COUNT_FULL, 256usize, 260usize),
        0 => (GLYPH_COUNT_SMALL, 95usize, 100usize),
        _ => panic!("Unknown mode_byte {mode_byte}; expected 0 (small) or 1 (full)"),
    };

    if bytes.len() < pixel_data_offset {
        panic!(
            "File too short; expected at least {pixel_data_offset} bytes before pixel data, got {}",
            bytes.len()
        );
    }

    let img = open_image(img_path);
    let (_, img_height) = img.dimensions();

    let img_cell_rows = img_height / cell_height as u32;
    let img_mode = if img_cell_rows as usize * SHEET_COLS >= GLYPH_COUNT_FULL {
        1u8
    } else {
        0u8
    };
    assert_eq!(
        img_mode,
        mode_byte,
        "Image produces a {} font but binary is a {} font",
        if img_mode == 1 {
            "full (256-glyph)"
        } else {
            "small (95-glyph)"
        },
        if mode_byte == 1 {
            "full (256-glyph)"
        } else {
            "small (95-glyph)"
        }
    );

    let data = extract_glyph_data(&img, cell_width, cell_height, glyph_count);

    let row_u32s = (cell_width as usize + 7) >> 3;
    let glyph_size = row_u32s * cell_height as usize;
    let expected_pixel_bytes = glyph_size * glyph_count * 4;
    let expected_total = pixel_data_offset + expected_pixel_bytes;

    if bytes.len() != expected_total {
        eprintln!(
            "warning: binary size {}, expected {expected_total}; resizing",
            bytes.len()
        );
        bytes.resize(expected_total, 0);
    }

    // Preserve header (3 bytes) + char_widths (widths_len bytes) + padding.
    // Only overwrite the pixel data region starting at pixel_data_offset.
    let mut pos = pixel_data_offset;
    for d in &data {
        let le = d.to_le_bytes();
        bytes[pos..pos + 4].copy_from_slice(&le);
        pos += 4;
    }

    let mut out = BufWriter::new(
        File::create(bin_path).unwrap_or_else(|e| panic!("Failed to write {bin_path:?}: {e}")),
    );
    out.write_all(&bytes).expect("failed to write bytes");
    out.flush().expect("failed to flush");

    println!(
        "Updated pixel data in {bin_path:?} ({glyph_count} glyphs, {widths_len} widths preserved)"
    );
}
