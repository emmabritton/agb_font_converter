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

/// Applies the naive bold smear to packed glyph data in place.
///
/// Each output pixel becomes the max palette index over itself and its `bold` left
/// neighbours, so ink extends `bold` px rightwards. Ink pushed past `cell_width` is
/// clipped; `create_bytes` widens the packed cell by `bold` first (see [`widen_cells`]),
/// so end-to-end nothing is lost.
pub fn apply_bold(data: &mut [u32], cell_width: u8, cell_height: u8, glyph_count: usize, bold: u8) {
    if bold == 0 {
        return;
    }

    let cell_width = cell_width as usize;
    let cell_height = cell_height as usize;
    let row_u32s = (cell_width + 7) >> 3;
    let glyph_size = row_u32s * cell_height;

    let mut src = vec![0u8; cell_width];
    for glyph_idx in 0..glyph_count {
        for row in 0..cell_height {
            let row_base = glyph_idx * glyph_size + row * row_u32s;
            for (x, px) in src.iter_mut().enumerate() {
                *px = ((data[row_base + (x >> 3)] >> ((x & 7) * 4)) & 0xF) as u8;
            }

            data[row_base..row_base + row_u32s].fill(0);
            for x in 0..cell_width {
                // Max over the pixel and its smeared left neighbours, never a
                // bitwise OR: OR-ing two luma indices fabricates brighter values
                let mut idx = 0u8;
                for smear in 0..=bold as usize {
                    if let Some(sx) = x.checked_sub(smear) {
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

/// Repacks glyph data from `old_width`-px cells into `new_width`-px cells, the new
/// columns transparent. The rightmost inked column can only sit at `old_width - 1`, so
/// widening by the bold strength before [`apply_bold`] guarantees the smeared ink
/// always fits, no matter how the sheet is drawn
pub fn widen_cells(
    data: Vec<u32>,
    old_width: u8,
    new_width: u8,
    cell_height: u8,
    glyph_count: usize,
) -> Vec<u32> {
    debug_assert!(new_width >= old_width);
    let old_row_u32s = (old_width as usize + 7) >> 3;
    let new_row_u32s = (new_width as usize + 7) >> 3;
    if new_row_u32s == old_row_u32s {
        // The extra pixels live in the high nibbles of the existing words, which
        // extract_glyph_data already left zero
        return data;
    }

    let rows = cell_height as usize * glyph_count;
    let mut out = vec![0u32; new_row_u32s * rows];
    for row in 0..rows {
        out[row * new_row_u32s..row * new_row_u32s + old_row_u32s]
            .copy_from_slice(&data[row * old_row_u32s..(row + 1) * old_row_u32s]);
    }
    out
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
    use super::{apply_bold, apply_recolour, widen_cells};

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
    fn zero_bold_is_a_no_op() {
        let mut data = buf(8, 6);
        set_px(&mut data, 8, 3, 1, 0x7);
        let before = data.clone();
        apply_bold(&mut data, 8, 6, 1, 0);
        assert_eq!(data, before);
    }

    #[test]
    fn bold_takes_max_not_or() {
        let mut data = buf(8, 3);
        set_px(&mut data, 8, 2, 2, 0x9);
        set_px(&mut data, 8, 3, 2, 0x6);
        apply_bold(&mut data, 8, 3, 1, 1);
        assert_eq!(get_px(&data, 8, 2, 2), 0x9);
        assert_eq!(get_px(&data, 8, 3, 2), 0x9, "an OR would fabricate 0xF");
        assert_eq!(get_px(&data, 8, 4, 2), 0x6);
        assert_eq!(get_px(&data, 8, 5, 2), 0);
    }

    #[test]
    fn smear_pushed_past_the_cell_edge_clips() {
        let mut data = buf(8, 2);
        set_px(&mut data, 8, 7, 0, 0xF);
        apply_bold(&mut data, 8, 2, 1, 2);
        assert_eq!(get_px(&data, 8, 7, 0), 0xF);
        // The smear past the edge must vanish, not wrap into the next row
        assert_eq!(get_px(&data, 8, 0, 1), 0);
        assert_eq!(get_px(&data, 8, 1, 1), 0);
    }

    #[test]
    fn widen_within_the_same_word_reuses_the_buffer() {
        let mut data = buf(8, 2);
        set_px(&mut data, 8, 7, 1, 0xF);
        let widened = widen_cells(data.clone(), 8, 8, 2, 1);
        assert_eq!(widened, data);
    }

    #[test]
    fn widen_across_a_word_boundary_repacks_rows() {
        // Two 8x3 glyphs into 10x3 cells: row_u32s goes 1 -> 2
        let mut data = buf(8, 6);
        set_px(&mut data, 8, 7, 0, 0xA);
        set_px(&mut data, 8, 0, 5, 0xB);
        let widened = widen_cells(data, 8, 10, 3, 2);
        assert_eq!(widened.len(), 12);
        assert_eq!(get_px(&widened, 10, 7, 0), 0xA);
        assert_eq!(get_px(&widened, 10, 0, 5), 0xB);
        for x in 8..10 {
            for y in 0..6 {
                assert_eq!(get_px(&widened, 10, x, y), 0, "new columns must be blank");
            }
        }
        // The widened cell now has room for the smear that clipped before
        let mut widened = widened;
        apply_bold(&mut widened, 10, 3, 2, 2);
        assert_eq!(get_px(&widened, 10, 9, 0), 0xA);
    }

    #[test]
    fn smear_crosses_a_u32_boundary() {
        let mut data = buf(16, 1);
        set_px(&mut data, 16, 7, 0, 0xA);
        apply_bold(&mut data, 16, 1, 1, 2);
        assert_eq!(get_px(&data, 16, 7, 0), 0xA);
        assert_eq!(get_px(&data, 16, 8, 0), 0xA);
        assert_eq!(get_px(&data, 16, 9, 0), 0xA);
        assert_eq!(get_px(&data, 16, 10, 0), 0);
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
        apply_bold(&mut data, 8, 6, 2, 1);
        assert!(data[..6].iter().all(|&w| w == 0), "glyph 0 stays empty");
        assert_eq!(get_px(&data[6..], 8, 0, 0), 0xF);
        assert_eq!(get_px(&data[6..], 8, 1, 0), 0xF);
        assert_eq!(get_px(&data[6..], 8, 2, 0), 0);
    }
}
