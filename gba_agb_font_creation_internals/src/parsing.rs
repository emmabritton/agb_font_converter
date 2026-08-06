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

pub fn open_image(path: &PathBuf) -> DynamicImage {
    let is_aseprite = path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("aseprite") || ext.eq_ignore_ascii_case("ase"));
    if is_aseprite {
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
    sheet_cols: usize,
    glyph_count: usize,
) -> Vec<u32> {
    let (img_width, img_height) = img.dimensions();
    let sheet_rows = glyph_count.div_ceil(sheet_cols);
    assert!(
        img_width >= cell_width as u32 * sheet_cols as u32,
        "Image width {} too small for {} columns of {} px cells",
        img_width,
        sheet_cols,
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

    let rgba = img.to_rgba8();

    for glyph_idx in 0..glyph_count {
        let sheet_col = glyph_idx % sheet_cols;
        let sheet_row = glyph_idx / sheet_cols;
        let base_x = sheet_col * cell_width as usize;
        let base_y = sheet_row * cell_height as usize;
        let glyph_base = glyph_idx * glyph_size;

        for row in 0..cell_height as usize {
            for word in 0..row_u32s {
                let mut val: u32 = 0;
                for px in 0..8usize {
                    let x = base_x + word * 8 + px;
                    if x < base_x + cell_width as usize {
                        let pixel = *rgba.get_pixel(x as u32, (base_y + row) as u32);
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

/// Applies the naive italic shear and bold smear to packed glyph data in place.
///
/// Rows `[0, h/3)` shift right by `2 * italic` px and rows `[h/3, 2*(h/3))` by `italic` px;
/// the remaining rows (the bottom band absorbs any remainder) stay put. Each output pixel
/// then becomes the max palette index over itself and its `bold` left neighbours. Ink pushed
/// past `cell_width` is clipped.
pub fn apply_style(
    data: &mut [u32],
    cell_width: u8,
    cell_height: u8,
    glyph_count: usize,
    italic: u8,
    bold: u8,
) {
    if italic == 0 && bold == 0 {
        return;
    }

    let cell_width = cell_width as usize;
    let cell_height = cell_height as usize;
    let row_u32s = (cell_width + 7) >> 3;
    let glyph_size = row_u32s * cell_height;
    let third = cell_height / 3;

    let mut src = vec![0u8; cell_width];
    for glyph_idx in 0..glyph_count {
        for row in 0..cell_height {
            let shift = if row < third {
                2 * italic as usize
            } else if row < 2 * third {
                italic as usize
            } else {
                0
            };

            let row_base = glyph_idx * glyph_size + row * row_u32s;
            for (x, px) in src.iter_mut().enumerate() {
                *px = ((data[row_base + (x >> 3)] >> ((x & 7) * 4)) & 0xF) as u8;
            }

            data[row_base..row_base + row_u32s].fill(0);
            for x in 0..cell_width {
                // Max over the shifted pixel and its smeared left neighbours, never a
                // bitwise OR: OR-ing two luma indices fabricates brighter values
                let mut idx = 0u8;
                for smear in 0..=bold as usize {
                    if let Some(sx) = x.checked_sub(shift + smear) {
                        idx = idx.max(src[sx]);
                    }
                }
                if idx != 0 {
                    data[row_base + (x >> 3)] |= (idx as u32) << ((x & 7) * 4);
                }
            }
        }
    }
}

/// Remaps every pixel's palette index through `table` (index N becomes `table[N]`)
///
/// The table is applied in a single pass, so a swapping table exchanges two bands
/// rather than chaining. Index 0 is the transparent background; `create_bytes`
/// rejects maps that recolour it
pub fn apply_recolour(data: &mut [u32], table: &[u8; 16]) {
    for word in data.iter_mut() {
        let mut out = 0u32;
        for px in 0..8 {
            let idx = ((*word >> (px * 4)) & 0xF) as usize;
            out |= (table[idx] as u32) << (px * 4);
        }
        *word = out;
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_recolour, apply_style};

    fn buf(cell_width: usize, cell_height: usize) -> Vec<u32> {
        vec![0u32; ((cell_width + 7) >> 3) * cell_height]
    }

    fn set_px(data: &mut [u32], cell_width: usize, x: usize, y: usize, idx: u8) {
        let row_u32s = (cell_width + 7) >> 3;
        data[y * row_u32s + (x >> 3)] |= (idx as u32) << ((x & 7) * 4);
    }

    fn get_px(data: &[u32], cell_width: usize, x: usize, y: usize) -> u8 {
        let row_u32s = (cell_width + 7) >> 3;
        ((data[y * row_u32s + (x >> 3)] >> ((x & 7) * 4)) & 0xF) as u8
    }

    #[test]
    fn zero_style_is_a_no_op() {
        let mut data = buf(8, 6);
        set_px(&mut data, 8, 3, 1, 0x7);
        let before = data.clone();
        apply_style(&mut data, 8, 6, 1, 0, 0);
        assert_eq!(data, before);
    }

    #[test]
    fn bands_shift_top_double_mid_single_bottom_absorbs_remainder() {
        for h in [6usize, 7, 8] {
            let third = h / 3;
            let mut data = buf(8, h);
            for y in 0..h {
                set_px(&mut data, 8, 0, y, 0xF);
            }
            apply_style(&mut data, 8, h as u8, 1, 1, 0);
            for y in 0..h {
                let expected_x = if y < third {
                    2
                } else if y < 2 * third {
                    1
                } else {
                    // h=8 pins the remainder rule: row 4 belongs to the bottom band
                    // (2*third = 4), not the middle one (2*h/3 would be 5)
                    0
                };
                for x in 0..8 {
                    let expected = if x == expected_x { 0xF } else { 0 };
                    assert_eq!(get_px(&data, 8, x, y), expected, "h={h} y={y} x={x}");
                }
            }
        }
    }

    #[test]
    fn bold_takes_max_not_or() {
        let mut data = buf(8, 3);
        // Bottom band (h=3 -> row 2) so the italic shift stays out of the way
        set_px(&mut data, 8, 2, 2, 0x9);
        set_px(&mut data, 8, 3, 2, 0x6);
        apply_style(&mut data, 8, 3, 1, 0, 1);
        assert_eq!(get_px(&data, 8, 2, 2), 0x9);
        assert_eq!(get_px(&data, 8, 3, 2), 0x9, "an OR would fabricate 0xF");
        assert_eq!(get_px(&data, 8, 4, 2), 0x6);
        assert_eq!(get_px(&data, 8, 5, 2), 0);
    }

    #[test]
    fn ink_pushed_past_the_cell_edge_clips() {
        let mut data = buf(8, 6);
        set_px(&mut data, 8, 7, 0, 0xF);
        apply_style(&mut data, 8, 6, 1, 1, 0);
        for x in 0..8 {
            assert_eq!(get_px(&data, 8, x, 0), 0, "x={x}");
        }
    }

    #[test]
    fn shift_crosses_a_u32_boundary() {
        let mut data = buf(16, 6);
        set_px(&mut data, 16, 7, 0, 0xA);
        apply_style(&mut data, 16, 6, 1, 4, 0);
        assert_eq!(get_px(&data, 16, 15, 0), 0xA);
        assert_eq!(get_px(&data, 16, 7, 0), 0);
    }

    fn identity_table() -> [u8; 16] {
        core::array::from_fn(|i| i as u8)
    }

    #[test]
    fn recolour_swaps_bands_simultaneously() {
        let mut data = buf(8, 1);
        set_px(&mut data, 8, 0, 0, 0xF);
        set_px(&mut data, 8, 1, 0, 0xE);
        set_px(&mut data, 8, 2, 0, 0x7);
        let mut table = identity_table();
        table[0xF] = 0xE;
        table[0xE] = 0xF;
        apply_recolour(&mut data, &table);
        assert_eq!(get_px(&data, 8, 0, 0), 0xE, "swapped, not chained");
        assert_eq!(get_px(&data, 8, 1, 0), 0xF);
        assert_eq!(get_px(&data, 8, 2, 0), 0x7, "unmapped band passes through");
    }

    #[test]
    fn recolour_to_zero_clears_pixels() {
        let mut data = buf(8, 1);
        set_px(&mut data, 8, 3, 0, 0xF);
        let mut table = identity_table();
        table[0xF] = 0;
        apply_recolour(&mut data, &table);
        assert_eq!(data, buf(8, 1));
    }

    #[test]
    fn identity_recolour_is_a_no_op() {
        let mut data = buf(8, 2);
        set_px(&mut data, 8, 4, 1, 0x9);
        let before = data.clone();
        apply_recolour(&mut data, &identity_table());
        assert_eq!(data, before);
    }

    #[test]
    fn glyphs_transform_independently() {
        let mut data = buf(8, 6);
        data.extend_from_slice(&buf(8, 6));
        set_px(&mut data[6..], 8, 0, 0, 0xF);
        apply_style(&mut data, 8, 6, 2, 1, 0);
        assert!(data[..6].iter().all(|&w| w == 0), "glyph 0 stays empty");
        assert_eq!(get_px(&data[6..], 8, 2, 0), 0xF);
    }
}
