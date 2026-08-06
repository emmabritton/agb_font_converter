use crate::{TextAlign, TextFormat, TextOverflow, blit_pixel_row_arm};
use agb::display::tiled::{DynamicTile16, RegularBackground, TileEffect};
use agb::fixnum::{Vector2D, vec2};
use alloc::vec::Vec;
use gba_agb_font_eb::AgbFont;

/// Renders text glyphs to [`agb`] background tiles, tracking allocated [`DynamicTile16`]s
#[derive(Debug)]
pub struct TextRenderer {
    tiles: Vec<(i32, i32, DynamicTile16)>,
    last_idx_cache: Option<(i32, i32, usize)>,
    pub palette_id: u8,
    /// Extra pixels inserted between adjacent characters (default 1)
    ///
    /// The gap goes *between* letters only, never before the first or after the
    /// last character on a line, so alignment, wrapping and the widths returned by
    /// [`draw_text`](TextRenderer::draw_text) all stay correct. Change it at any
    /// time to space the same font differently; negative values tighten the text,
    /// with each character still advancing at least 1px
    pub letter_spacing: i8,
    /// Extra pixels inserted between adjacent lines (default 0)
    ///
    /// The vertical twin of [`letter_spacing`](TextRenderer::letter_spacing): the
    /// gap sits between lines only, so a single line is unaffected. Negative
    /// values tighten, with each line still advancing at least 1px
    pub line_spacing: i8,
    /// Reusable glyph staging buffer, so `draw_text` doesn't heap-allocate on
    /// every call; taken out for the duration of a draw and put back after.
    staged: Vec<(i32, i32, [u32; 8])>,
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self {
            tiles: Vec::with_capacity(64),
            last_idx_cache: None,
            palette_id: 15,
            letter_spacing: 1,
            line_spacing: 0,
            staged: Vec::with_capacity(48),
        }
    }
}

fn x_offset(line_w: u32, alignment: TextAlign) -> i32 {
    match alignment {
        TextAlign::Left => 0,
        TextAlign::Center(column_w) => {
            let gap = (column_w as u32).saturating_sub(line_w);
            (gap >> 1) as i32
        }
        TextAlign::Right(column_w) => (column_w as u32).saturating_sub(line_w) as i32,
    }
}

impl TextRenderer {
    /// Clear all pixels in tracked tiles. Pass `drop_tiles: true` to deallocate them instead
    pub fn reset(&mut self, drop_tiles: bool) {
        if drop_tiles {
            self.tiles.clear();
        } else {
            for (_, _, tile) in &mut self.tiles {
                tile.data_mut().fill(0);
            }
        }
        self.last_idx_cache = None;
    }

    /// Render `text` at `pos` on `background` with `format` using DynamicTiles
    ///
    /// # Returns
    /// `(cursor_dx, cursor_dy, longest_line_px)` which is the bottom right of the last character drawn, longest line contains the width of longest line (will match cursor_dx for single line)
    #[inline(always)]
    pub fn draw_text<T: AgbFont>(
        &mut self,
        text: &[u8],
        font: &T,
        background: &mut RegularBackground,
        pos: Vector2D<i32>,
        format: &TextFormat,
    ) -> (i32, i32, i32) {
        let (wrap_px, word_wrap) = match format.overflow {
            TextOverflow::Wrap(w, ww) => (Some(w as u32), ww),
            _ => (None, false),
        };
        let cutoff_x = match format.overflow {
            TextOverflow::Cutoff(w) => Some(pos.x + w as i32),
            _ => None,
        };

        let mut cursor_y = pos.y;
        let mut longest: i32 = 0;
        let mut last_cx = pos.x;
        let mut staged = core::mem::take(&mut self.staged);
        staged.clear();
        let mut left_cache: Option<(i32, i32, usize)> = None;
        let mut right_cache: Option<(i32, i32, usize)> = None;
        let mut first = true;
        let spacing = self.letter_spacing;

        for (line, line_w) in font.lines(text, wrap_px, word_wrap, spacing) {
            if !first {
                cursor_y += font.line_advance(self.line_spacing) as i32;
            }
            first = false;
            let mut cursor_x = pos.x + x_offset(line_w, format.align);
            for (n, &c) in line.iter().enumerate() {
                if n > 0 {
                    cursor_x += font.letter_gap(c, spacing);
                }
                if cutoff_x.is_none_or(|cx| cursor_x < cx) {
                    Self::blit_glyph_staged(
                        font.glyph(c),
                        font,
                        &mut staged,
                        cursor_x,
                        cursor_y,
                        &mut left_cache,
                        &mut right_cache,
                    );
                }
                cursor_x += font.char_width(c) as i32;
            }
            longest = longest.max(line_w as i32);
            last_cx = cursor_x;
        }

        // Styled (bold) ink may overhang the measured width, so an active
        // clear widens to cover it; (0, 0) stays a true no-op
        let clear_w = format.clear.0 as i32;
        let clear_size = if clear_w > 0 {
            (
                clear_w + font.right_overhang() as i32,
                format.clear.1 as i32,
            )
        } else {
            (clear_w, format.clear.1 as i32)
        };
        clear_rect_in_tiles(&mut self.tiles, pos, clear_size);

        for (tx, ty, data) in &staged {
            // Cells are staged over their full footprint, so narrow glyphs leave
            // all-zero tiles; skipping them keeps blank VRAM tiles unallocated
            // (safe: clears only ever touch tracked tiles)
            if data.iter().all(|&row| row == 0) {
                continue;
            }
            let idx = self.ensure_tile_idx(*tx, *ty, background, self.palette_id);
            let tile_data = self.tiles[idx].2.data_mut();
            for (dst, &src) in tile_data.iter_mut().zip(data.iter()) {
                blit_pixel_row_arm(dst, src);
            }
        }
        self.staged = staged;

        (last_cx - pos.x, cursor_y - pos.y, longest)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn int_draw_text<T: AgbFont>(
        &mut self,
        text: &[u8],
        font: &T,
        background: &mut RegularBackground,
        pos: Vector2D<i32>,
        overflow: TextOverflow,
        alignment: TextAlign,
        palette_id: u8,
        clear_size: (i32, i32),
        initial_line_w: i32,
        left_margin_x: i32,
    ) -> (i32, i32, i32) {
        let wrap_px = match overflow {
            TextOverflow::Wrap(w, _) => Some(w),
            _ => None,
        };
        let cutoff_x = match overflow {
            TextOverflow::Cutoff(w) => Some(left_margin_x + w as i32),
            _ => None,
        };
        let mut cursor_y = pos.y;
        let mut line_w: i32 = initial_line_w;
        let mut longest: i32 = 0;
        let spacing = self.letter_spacing;

        let mut staged = core::mem::take(&mut self.staged);
        staged.clear();

        let (first_w, _) = font.measure_line(text, wrap_px.map(|n| n as u32), spacing);
        let mut cursor_x = pos.x + x_offset(first_w, alignment);

        let mut left_cache: Option<(i32, i32, usize)> = None;
        let mut right_cache: Option<(i32, i32, usize)> = None;

        for (i, &c) in text.iter().enumerate() {
            if c == b'\n' {
                longest = longest.max(line_w);
                cursor_y += font.line_advance(self.line_spacing) as i32;
                line_w = 0;
                let (next_w, _) =
                    font.measure_line(&text[i + 1..], wrap_px.map(|n| n as u32), spacing);
                cursor_x = left_margin_x + x_offset(next_w, alignment);
            } else {
                let char_w = font.char_width(c) as i32;
                let mut gap = if line_w == 0 {
                    0
                } else {
                    font.letter_gap(c, spacing)
                };
                if let Some(wa) = wrap_px
                    && line_w + gap + char_w > wa as i32
                {
                    longest = longest.max(line_w);
                    cursor_y += font.line_advance(self.line_spacing) as i32;
                    line_w = 0;
                    gap = 0;
                    let (next_w, _) =
                        font.measure_line(&text[i..], wrap_px.map(|n| n as u32), spacing);
                    cursor_x = left_margin_x + x_offset(next_w, alignment);
                }

                cursor_x += gap;

                if cutoff_x.is_none_or(|cx| cursor_x < cx) {
                    Self::blit_glyph_staged(
                        font.glyph(c),
                        font,
                        &mut staged,
                        cursor_x,
                        cursor_y,
                        &mut left_cache,
                        &mut right_cache,
                    );
                }

                cursor_x += char_w;
                line_w += gap + char_w;
            }
        }
        longest = longest.max(line_w);

        // Same overhang widening as draw_text; this path also serves the
        // typewriters' full-line redraws
        let clear_size = if clear_size.0 > 0 {
            (clear_size.0 + font.right_overhang() as i32, clear_size.1)
        } else {
            clear_size
        };
        clear_rect_in_tiles(&mut self.tiles, pos, clear_size);

        for (tx, ty, data) in &staged {
            // Same blank-tile skip as draw_text
            if data.iter().all(|&row| row == 0) {
                continue;
            }
            let idx = self.ensure_tile_idx(*tx, *ty, background, palette_id);
            let tile_data = self.tiles[idx].2.data_mut();
            for (dst, &src) in tile_data.iter_mut().zip(data.iter()) {
                blit_pixel_row_arm(dst, src);
            }
        }
        self.staged = staged;

        (cursor_x - pos.x, cursor_y - pos.y, longest)
    }

    fn blit_glyph_staged<T: AgbFont>(
        glyph: &[u32],
        font: &T,
        staged: &mut Vec<(i32, i32, [u32; 8])>,
        pixel_x: i32,
        pixel_y: i32,
        left_cache: &mut Option<(i32, i32, usize)>,
        right_cache: &mut Option<(i32, i32, usize)>,
    ) {
        let row_u32s = font.row_u32s();
        let height = font.glyph_height() as i32;

        for chunk in 0..row_u32s {
            let px_left = pixel_x + ((chunk as i32) << 3);
            let tile_x = px_left >> 3;
            let x_shift = (px_left & 7) as u32;
            let shift_bits = x_shift << 2;

            let mut last_tile_y = -1i32;
            let mut left_idx = 0;
            let mut right_idx = None;

            for row in 0..height {
                let abs_y = pixel_y + row;
                let current_tile_y = abs_y >> 3;
                let row_in_tile = (abs_y & 7) as usize;

                if current_tile_y != last_tile_y {
                    left_idx = Self::ensure_staged_idx(staged, tile_x, current_tile_y, left_cache);
                    right_idx = if x_shift > 0 {
                        Some(Self::ensure_staged_idx(
                            staged,
                            tile_x + 1,
                            current_tile_y,
                            right_cache,
                        ))
                    } else {
                        None
                    };
                    last_tile_y = current_tile_y;
                }

                let pixel_data = glyph[(row as usize * row_u32s) + chunk];

                blit_pixel_row_arm(
                    &mut staged[left_idx].2[row_in_tile],
                    pixel_data << shift_bits,
                );

                if let Some(r_idx) = right_idx {
                    blit_pixel_row_arm(
                        &mut staged[r_idx].2[row_in_tile],
                        pixel_data >> (32 - shift_bits),
                    );
                }
            }
        }
    }

    fn ensure_staged_idx(
        staged: &mut Vec<(i32, i32, [u32; 8])>,
        tx: i32,
        ty: i32,
        cache: &mut Option<(i32, i32, usize)>,
    ) -> usize {
        if let Some((cx, cy, cidx)) = *cache
            && cx == tx
            && cy == ty
        {
            return cidx;
        }

        if let Some(pos) = staged.iter().rposition(|(x, y, _)| *x == tx && *y == ty) {
            *cache = Some((tx, ty, pos));
            return pos;
        }

        staged.push((tx, ty, [0u32; 8]));
        let pos = staged.len() - 1;
        *cache = Some((tx, ty, pos));
        pos
    }

    fn ensure_tile_idx(&mut self, tx: i32, ty: i32, bg: &mut RegularBackground, pal: u8) -> usize {
        if let Some((cx, cy, cidx)) = self.last_idx_cache
            && cx == tx
            && cy == ty
        {
            return cidx;
        }

        if let Some(pos) = self
            .tiles
            .iter()
            .rposition(|(x, y, _)| *x == tx && *y == ty)
        {
            self.last_idx_cache = Some((tx, ty, pos));
            return pos;
        }

        let tile = DynamicTile16::new().fill_with(0);
        bg.set_tile_dynamic16(vec2(tx, ty), &tile, TileEffect::default().palette(pal));
        self.tiles.push((tx, ty, tile));
        let pos = self.tiles.len() - 1;
        self.last_idx_cache = Some((tx, ty, pos));
        pos
    }

    /// Clear only the exact pixel columns within the given rect
    /// Only tiles already tracked by this renderer are modified; tiles outside
    /// the pool are left untouched
    pub fn clear_pixel_rect(&mut self, pos: Vector2D<i32>, width: i32, height: i32) {
        clear_rect_in_tiles(&mut self.tiles, pos, (width, height));
    }

    /// Number of VRAM tiles currently allocated by this renderer
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::include_agb_font;
    use agb::display::Priority;
    use agb::display::tiled::{RegularBackgroundSize, TileFormat};

    include_agb_font!(
        STYLED,
        "../examples/simple_8x8_text15_shadow14.aseprite",
        bold
    );

    /// Rightmost inked pixel column of `c`'s glyph, or `None` for an empty glyph
    fn rightmost_ink<T: AgbFont>(font: &T, c: u8) -> Option<usize> {
        let row_u32s = font.row_u32s();
        let mut rightmost = None;
        for (i, &word) in font.glyph(c).iter().enumerate() {
            for px in 0..8usize {
                if (word >> (px * 4)) & 0xF != 0 {
                    let x = (i % row_u32s) * 8 + px;
                    if rightmost.is_none_or(|m| x > m) {
                        rightmost = Some(x);
                    }
                }
            }
        }
        rightmost
    }

    fn any_ink(renderer: &mut TextRenderer) -> bool {
        renderer
            .tiles
            .iter_mut()
            .any(|(_, _, tile)| tile.data_mut().iter().any(|&row| row != 0))
    }

    #[test_case]
    fn clear_covers_styled_overhang(_gba: &mut agb::Gba) {
        assert_eq!(STYLED.right_overhang(), 1, "bare bold stores overhang 1");

        // Bold smears ink past the rightmost roman column, so the styled sheet
        // must have ink past its roman advance
        let ink_w = rightmost_ink(&STYLED, b'\'').unwrap() + 1;
        let roman_w = STYLED.char_width(b'\'') as usize;
        assert!(
            ink_w > roman_w,
            "expected styled ink past the roman advance ({ink_w} <= {roman_w})"
        );

        let mut bg = RegularBackground::new(
            Priority::P3,
            RegularBackgroundSize::Background32x32,
            TileFormat::FourBpp,
        );
        let mut renderer = TextRenderer::default();
        let (w, h) = STYLED.size_of(b"'", None, renderer.letter_spacing, renderer.line_spacing);

        renderer.draw_text(b"'", &STYLED, &mut bg, vec2(0, 0), &TextFormat::default());
        assert!(any_ink(&mut renderer), "glyph should have drawn pixels");

        // A clear sized from the roman measure must still erase the overhang
        renderer.draw_text(
            b"",
            &STYLED,
            &mut bg,
            vec2(0, 0),
            &TextFormat::default().with_clear(w as u16, h as u16),
        );
        assert!(
            !any_ink(&mut renderer),
            "roman-sized clear left styled overhang ink behind"
        );
    }
}

fn clear_rect_in_tiles(
    tiles: &mut [(i32, i32, DynamicTile16)],
    pos: Vector2D<i32>,
    size: (i32, i32),
) {
    if size.0 <= 0 || size.1 <= 0 {
        return;
    }
    let tile_x_start = pos.x >> 3;
    let tile_x_end = (pos.x + size.0 - 1) >> 3;
    let tile_y_start = pos.y >> 3;
    let tile_y_end = (pos.y + size.1 - 1) >> 3;
    let py_end = pos.y + size.1 - 1;
    let px_end = pos.x + size.0 - 1;
    for (tx, ty, tile) in tiles {
        if *tx < tile_x_start || *tx > tile_x_end || *ty < tile_y_start || *ty > tile_y_end {
            continue;
        }
        let tile_py0 = *ty << 3;
        let row_start = (pos.y - tile_py0).max(0) as usize;
        let row_end = (py_end - tile_py0).min(7) as usize;
        let tile_px0 = *tx << 3;
        let n_start = (pos.x - tile_px0).max(0) as u32;
        let n_end = (px_end - tile_px0).min(7) as u32;
        let n_count = n_end - n_start + 1;
        let mask = if n_count >= 8 {
            u32::MAX
        } else {
            ((1u32 << (n_count << 2)) - 1) << (n_start << 2)
        };
        for row in tile.data_mut().iter_mut().take(row_end + 1).skip(row_start) {
            *row &= !mask;
        }
    }
}
