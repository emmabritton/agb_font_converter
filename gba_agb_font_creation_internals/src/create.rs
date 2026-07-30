use crate::parsing::extract_glyph_data;
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

pub fn create_bytes(
    cols: u32,
    rows: u32,
    img: &DynamicImage,
    monospace: Option<Option<u8>>,
    width_overrides: &[(u8, u8)],
) -> Vec<u8> {
    let SheetGrid {
        cell_width,
        cell_height,
        glyph_count,
        mode_byte,
        ..
    } = SheetGrid::resolve(img, cols, rows);

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
