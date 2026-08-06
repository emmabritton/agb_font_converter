use crate::parsing::{apply_bold, apply_recolour, extract_glyph_data, widen_cells};
use crate::{GLYPH_COUNT_FULL, GLYPH_COUNT_SMALL};
use image::{DynamicImage, GenericImageView};

/// A sheet's cell grid, resolved against the image it describes
///
/// The grid is the only thing that ties pixels to glyphs: cell dimensions are the image
/// dimensions divided by the grid, and the font mode follows from the cell count
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SheetGrid {
    pub cols: u32,
    pub rows: u32,
    pub cell_width: u8,
    pub cell_height: u8,
    /// 95 for a small font, 256 for a full one
    pub glyph_count: usize,
    /// 0 for a small font, 1 for a full one
    pub mode_byte: u8,
}

impl SheetGrid {
    pub fn resolve(img: &DynamicImage, cols: u32, rows: u32) -> Self {
        assert!(cols > 0 && rows > 0, "Sheet grid must be at least 1x1");

        let (img_width, img_height) = img.dimensions();
        assert!(
            img_width % cols == 0,
            "Image width {img_width} does not divide into {cols} columns; \
             crop the sheet or pass the real grid as `size(cols, rows)`"
        );
        assert!(
            img_height % rows == 0,
            "Image height {img_height} does not divide into {rows} rows; \
             crop the sheet or pass the real grid as `size(cols, rows)`"
        );

        let cell_width = img_width / cols;
        let cell_height = img_height / rows;
        assert!(
            cell_width > 0 && cell_height > 0,
            "Grid {cols}x{rows} leaves no pixels per cell in a {img_width}x{img_height} image"
        );
        assert!(
            cell_width <= u8::MAX as u32 && cell_height <= u8::MAX as u32,
            "Cell size {cell_width}x{cell_height} exceeds the 255px the font format can store"
        );

        let cells = cols as usize * rows as usize;
        let (glyph_count, mode_byte) = if cells >= GLYPH_COUNT_FULL {
            (GLYPH_COUNT_FULL, 1)
        } else {
            (GLYPH_COUNT_SMALL, 0)
        };
        assert!(
            cells >= glyph_count,
            "Grid {cols}x{rows} is {cells} cells, but a small font needs {GLYPH_COUNT_SMALL}"
        );

        Self {
            cols,
            rows,
            cell_width: cell_width as u8,
            cell_height: cell_height as u8,
            glyph_count,
            mode_byte,
        }
    }
}

/// `bold` is pack-time styling: each pixel is smeared over its `bold` left neighbours
/// (0 means off). Advance widths are always scanned before styling, so bold fonts keep
/// their roman metrics and the extra ink overhangs the next letter like kerning.
pub fn create_bytes(
    cols: u32,
    rows: u32,
    img: &DynamicImage,
    monospace: Option<Option<u8>>,
    width_overrides: &[(u8, u8)],
    bold: u8,
    recolor: &[(u8, u8)],
) -> Vec<u8> {
    let SheetGrid {
        cell_width,
        cell_height,
        glyph_count,
        mode_byte,
        ..
    } = SheetGrid::resolve(img, cols, rows);

    // Max pixels the smeared ink can extend past the roman advance width; stored in the
    // header and read back by the renderers to widen clears and sprite allocations
    let overhang = bold as u32;
    let packed_width = cell_width as u32 + overhang;
    assert!(
        packed_width <= u8::MAX as u32,
        "bold = {bold} widens the {cell_width}px cell by {overhang}px, past the 255px the \
         font format can store; reduce the strength"
    );
    let packed_width = packed_width as u8;

    for &(from, to) in recolor {
        assert!(
            from != 0,
            "recolor = {{ 0 = {to} }} would fill every transparent pixel; band 0 is the \
             background and cannot be recoloured"
        );
        assert!(
            from <= 15 && to <= 15,
            "recolor = {{ {from} = {to} }} is out of range; bands are 0-15 (luma >> 4)"
        );
    }

    let data = extract_glyph_data(img, cell_width, cell_height, cols as usize, glyph_count);
    assert!(
        data.iter().any(|&word| word != 0),
        "Every glyph in the sheet is empty. Note that opaque pixels with luminance below 16 \
         (near-black) convert to palette index 0, which is transparent on the GBA, draw \
         glyphs in a brighter colour"
    );

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
            cp as usize
        } else {
            assert!(
                (32..=126).contains(&cp),
                "Width override for code point 0x{cp:02x} is outside the small font's range \
                 (0x20-0x7E); use a `full` sheet or remove the override"
            );
            (cp - 32) as usize
        };
        char_widths[idx] = w;
    }

    // Bold runs after the width scan on purpose: advances stay roman. The packed cell
    // is widened by the overhang first, so the smeared ink can never be pushed past the
    // cell edge, sheets need no right padding of their own
    let mut data = widen_cells(data, cell_width, packed_width, cell_height, glyph_count);
    apply_bold(&mut data, packed_width, cell_height, glyph_count, bold);

    // Recolour runs last: bold's smear takes the max of indices, so a downward
    // remap applied earlier would wrongly lose to un-remapped ink
    if !recolor.is_empty() {
        let mut table: [u8; 16] = core::array::from_fn(|i| i as u8);
        for &(from, to) in recolor {
            table[from as usize] = to;
        }
        apply_recolour(&mut data, &table);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&[mode_byte, packed_width, cell_height]);
    out.extend_from_slice(&char_widths);
    out.push(overhang as u8);
    if mode_byte == 0 {
        out.push(0);
    }
    for d in &data {
        out.extend_from_slice(&d.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    /// A 16x6 sheet of 4x6 cells with a white bar at cell-local x=1, so every roman
    /// glyph has advance width 2 and the bold smear has room to move ink right
    fn small_sheet() -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 36, |x, _| {
            if x % 4 == 1 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        }))
    }

    /// Same bar pattern on a 16x16 grid for a full font
    fn full_sheet() -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 96, |x, _| {
            if x % 4 == 1 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        }))
    }

    #[test]
    fn overhang_byte_lands_in_the_small_font_header() {
        let roman = create_bytes(16, 6, &small_sheet(), None, &[], 0, &[]);
        let styled = create_bytes(16, 6, &small_sheet(), None, &[], 3, &[]);
        assert_eq!(roman[98], 0);
        assert_eq!(roman[99], 0);
        assert_eq!(styled[98], 3);
        assert_eq!(styled[99], 0, "second padding byte stays reserved");
    }

    #[test]
    fn overhang_byte_lands_in_the_full_font_header() {
        let styled = create_bytes(16, 16, &full_sheet(), None, &[], 2, &[]);
        assert_eq!(styled[0], 1, "sheet should resolve as a full font");
        assert_eq!(styled[259], 2);
    }

    #[test]
    fn bold_fonts_keep_roman_advances() {
        let roman = create_bytes(16, 6, &small_sheet(), None, &[], 0, &[]);
        let styled = create_bytes(16, 6, &small_sheet(), None, &[], 1, &[]);
        assert_eq!(roman[3..98], styled[3..98]);
        assert!(roman[3..98].iter().all(|&w| w == 2));
        assert_ne!(roman[100..], styled[100..], "bold should alter pixel data");
    }

    /// White bar at cell-local x=3, the rightmost column of the 4px cell, so every
    /// glyph fills its cell and the bold smear would clip without the widening
    fn edge_sheet() -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 36, |x, _| {
            if x % 4 == 3 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        }))
    }

    #[test]
    fn bold_widens_the_cell_instead_of_clipping() {
        let styled = create_bytes(16, 6, &edge_sheet(), None, &[], 2, &[]);
        assert_eq!(styled[1], 6, "4px cell + 2px overhang");
        assert_eq!(styled[3], 4, "advance stays roman");

        // 6px cells still pack one u32 per row; the 1px bar smears to 3px per row,
        // ending past the sheet's cell edge
        let ink: u32 = styled[100..]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .map(|w| (0..8).filter(|px| (w >> (px * 4)) & 0xF != 0).count() as u32)
            .sum();
        assert_eq!(ink, 95 * 6 * 3, "widening must not lose any ink");

        // Glyph 0, top row: the bar's smear reaches x=5, past the sheet's cell edge
        let top_row = u32::from_le_bytes(styled[100..104].try_into().unwrap());
        assert_eq!((top_row >> 20) & 0xF, 0xF);
    }

    #[test]
    fn widening_grows_the_packed_rows_when_needed() {
        // 4px cell + 5px overhang = 9px: row_u32s goes 1 -> 2
        let styled = create_bytes(16, 6, &small_sheet(), None, &[], 5, &[]);
        assert_eq!(styled[1], 9);
        assert_eq!(styled.len(), 100 + 2 * 6 * 95 * 4);
    }

    #[test]
    #[should_panic(expected = "255px")]
    fn bold_overflowing_the_format_width_is_rejected() {
        create_bytes(16, 6, &small_sheet(), None, &[], 252, &[]);
    }

    #[test]
    fn recolour_remaps_pixel_data_only() {
        let plain = create_bytes(16, 6, &small_sheet(), None, &[], 0, &[]);
        let recoloured = create_bytes(16, 6, &small_sheet(), None, &[], 0, &[(15, 13)]);
        assert_eq!(plain[..100], recoloured[..100], "header must be untouched");
        // The white bar at cell-local x=1 is band 15: the high nibble of the
        // first data byte (glyph 0, row 0, little-endian u32)
        assert_eq!(plain[100], 0xF0);
        assert_eq!(recoloured[100], 0xD0);
    }

    #[test]
    #[should_panic(expected = "background")]
    fn recolouring_the_transparent_band_is_rejected() {
        create_bytes(16, 6, &small_sheet(), None, &[], 0, &[(0, 15)]);
    }
}
