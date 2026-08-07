use crate::{TextAlign, TextFormat, TextOverflow, blit_pixel_row_arm};
use agb::display::object::{DynamicSprite16, Object, PaletteVramSingle, Size, SpriteVram};
use agb::display::{GraphicsFrame, Priority};
use agb::fixnum::{Vector2D, vec2};
use agb::interrupt::VBlank;
use alloc::vec::Vec;
use gba_agb_font_eb::AgbFont;

/// Renders text into GBA hardware sprites (OBJ), packing multiple characters
/// per sprite to minimise OAM slot usage.
///
/// The GBA's largest sprite is 64x64: fonts taller than 64px cannot be drawn
/// this way, and a single glyph (plus its bold overhang) wider than the widest
/// sprite for its height (32px for glyphs up to 16px tall, 64px above that) is
/// truncated. Both are `debug_assert`ed in
/// [`draw_text`](SpriteTextRenderer::draw_text).
///
/// Call [`draw_text`](SpriteTextRenderer::draw_text) to rasterise text into
/// [`SpriteVram`] allocations, then call [`show`](SpriteTextRenderer::show)
/// every frame to submit the objects to the current [`GraphicsFrame`].
/// Call [`clear`](SpriteTextRenderer::clear) before loading new text.
///
/// ```ignore
/// use agb::display::object::PaletteVramSingle;
///
/// let palette = PaletteVramSingle::new(&MY_PALETTE);
/// let mut spr = SpriteTextRenderer::new(palette);
/// spr.draw_text(b"Hello!", &FONT, vec2(8, 8), &TextFormat::default());
///
/// loop {
///     let mut frame = gfx.frame();
///     spr.show(&mut frame);
///     frame.commit();
/// }
/// ```
#[derive(Debug)]
pub struct SpriteTextRenderer {
    pub palette: PaletteVramSingle,
    pub priority: Priority,
    /// Extra pixels inserted between adjacent characters (default 1)
    ///
    /// Behaves exactly as [`TextRenderer::letter_spacing`](crate::renderer::TextRenderer::letter_spacing):
    /// the gap sits between letters only, so alignment and wrapping stay correct
    /// Note that wider spacing packs fewer characters into each sprite
    pub letter_spacing: i8,
    /// Extra pixels inserted between adjacent lines (default 0)
    ///
    /// Behaves exactly as [`TextRenderer::line_spacing`](crate::renderer::TextRenderer::line_spacing):
    /// the gap sits between lines only; negative values tighten, with each line
    /// still advancing at least 1px
    pub line_spacing: i8,
    objects: Vec<(Vector2D<i32>, SpriteVram)>,
}

impl SpriteTextRenderer {
    pub fn new(palette: PaletteVramSingle) -> Self {
        Self {
            palette,
            priority: Priority::P0,
            letter_spacing: 1,
            line_spacing: 0,
            objects: Vec::new(),
        }
    }

    /// Replace the text after the vsync to avoid tearing or palette issues
    ///
    /// Returns the same `(cursor_dx, cursor_dy, longest_line_px)` tuple as
    /// [`draw_text`](SpriteTextRenderer::draw_text).
    pub fn replace_text<T: AgbFont>(
        &mut self,
        text: &[u8],
        font: &T,
        pos: Vector2D<i32>,
        format: &TextFormat,
    ) -> (i32, i32, i32) {
        self.clear();
        VBlank::get().wait_for_vblank();
        self.draw_text(text, font, pos, format)
    }

    /// Rasterise `text` into sprites starting at `pos`, appending to any
    /// previously rendered sprites. Call [`clear`](Self::clear) first to
    /// replace existing text.
    ///
    /// # Returns
    /// `(cursor_dx, cursor_dy, longest_line_px)`, the cursor position after the last
    /// character relative to `pos` (x past the last glyph, y at the top of the last
    /// line), and the width of the longest line, the same contract as
    /// [`TextRenderer::draw_text`](crate::renderer::TextRenderer::draw_text).
    pub fn draw_text<T: AgbFont>(
        &mut self,
        text: &[u8],
        font: &T,
        pos: Vector2D<i32>,
        format: &TextFormat,
    ) -> (i32, i32, i32) {
        let glyph_h = font.glyph_height();
        debug_assert!(
            glyph_h <= 64,
            "glyphs taller than 64px cannot fit a GBA sprite"
        );
        let sprite_h = sprite_height_for(glyph_h);
        let valid_widths = valid_widths_for_height(sprite_h);
        let max_sprite_w = *valid_widths.last().unwrap();

        let (wrap_px, word_wrap) = match format.overflow {
            TextOverflow::Wrap(w, ww) => (Some(w as u32), ww),
            _ => (None, false),
        };
        let cutoff_abs = match format.overflow {
            TextOverflow::Cutoff(w) => Some(pos.x + w as i32),
            _ => None,
        };

        let mut cursor_y = pos.y;
        let mut first_line = true;
        let mut longest: i32 = 0;
        let mut last_cx = pos.x;
        let spacing = self.letter_spacing;
        // Styled (bold) ink may extend past the last glyph's advance, so
        // sprite packing keeps that many columns free for it
        let overhang = font.right_overhang() as i32;

        for (line, line_w) in font.lines(text, wrap_px, word_wrap, spacing) {
            if !first_line {
                cursor_y += font.line_advance(self.line_spacing) as i32;
            }
            first_line = false;

            let x_off = match format.align {
                TextAlign::Left => 0,
                TextAlign::Center(col_w) => ((col_w as i32 - line_w as i32) >> 1).max(0),
                TextAlign::Right(col_w) => (col_w as i32 - line_w as i32).max(0),
            };
            let line_start_x = pos.x + x_off;

            let mut sprite_screen_x = line_start_x;
            let mut sprite_content_w: i32 = 0;
            let mut sprite_chars: Vec<(u8, u32)> = Vec::new();
            let mut screen_x = line_start_x;

            for (n, &c) in line.iter().enumerate() {
                let cw = font.char_width(c) as i32;
                // The first character on a line carries no leading gap.
                let gap = if n == 0 {
                    0
                } else {
                    font.letter_gap(c, spacing)
                };

                if cutoff_abs.is_some_and(|cx| screen_x + gap >= cx) {
                    break;
                }

                // Where this glyph sits inside the current sprite; a glyph that opens
                // a sprite starts at 0 and its gap moves the whole sprite instead.
                let offset = if sprite_chars.is_empty() {
                    0
                } else {
                    sprite_content_w + gap
                };

                if offset + cw + overhang > max_sprite_w as i32 {
                    if !sprite_chars.is_empty() {
                        self.flush_sprite(
                            &sprite_chars,
                            sprite_content_w as u32,
                            sprite_screen_x,
                            cursor_y,
                            font,
                            sprite_h,
                            valid_widths,
                        );
                    }
                    sprite_screen_x = screen_x + gap;
                    sprite_chars.clear();
                    sprite_chars.push((c, 0));
                    sprite_content_w = cw;
                } else {
                    sprite_chars.push((c, offset as u32));
                    sprite_content_w = offset + cw;
                }

                screen_x += gap + cw;
            }

            if !sprite_chars.is_empty() {
                self.flush_sprite(
                    &sprite_chars,
                    sprite_content_w as u32,
                    sprite_screen_x,
                    cursor_y,
                    font,
                    sprite_h,
                    valid_widths,
                );
            }
            longest = longest.max(line_w as i32);
            last_cx = screen_x;
        }

        (last_cx - pos.x, cursor_y - pos.y, longest)
    }

    #[allow(clippy::too_many_arguments)]
    fn flush_sprite<T: AgbFont>(
        &mut self,
        chars: &[(u8, u32)],
        content_w: u32,
        screen_x: i32,
        screen_y: i32,
        font: &T,
        sprite_h: u32,
        valid_widths: &[u32],
    ) {
        // The last glyph's styled ink may overhang its advance; reserve room so
        // the sprite's own width doesn't truncate it
        debug_assert!(
            content_w + font.right_overhang() as u32 <= *valid_widths.last().unwrap(),
            "glyph wider than the largest sprite for this font height; ink is truncated"
        );
        let sprite_w = pick_min_width(
            (content_w + font.right_overhang() as u32).max(1),
            valid_widths,
        );
        let size = Size::from_width_height(sprite_w as usize, sprite_h as usize);
        let mut dyn_sprite = DynamicSprite16::new(size);
        let w_tiles = (sprite_w >> 3) as usize;
        {
            let data = dyn_sprite.data_mut();
            for &(c, ox) in chars {
                blit_glyph_to_sprite(data, w_tiles, font.glyph(c), font, ox as usize);
            }
        }
        let vram = dyn_sprite.to_vram(self.palette.clone());
        self.objects.push((vec2(screen_x, screen_y), vram));
    }

    pub fn show(&self, frame: &mut GraphicsFrame<'_>) {
        self.show_at(frame, vec2(0, 0));
    }

    pub fn show_at(&self, frame: &mut GraphicsFrame<'_>, offset: Vector2D<i32>) {
        for (pos, vram) in &self.objects {
            let mut obj = Object::new(vram.clone());
            obj.set_pos(*pos + offset);
            obj.set_priority(self.priority);
            obj.show(frame);
        }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    /// Number of OAM slots this renderer will consume per frame
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }
}

// --- Size helpers -----------------------------------------------------------

fn sprite_height_for(glyph_h: u32) -> u32 {
    match glyph_h {
        0..=8 => 8,
        9..=16 => 16,
        17..=32 => 32,
        _ => 64,
    }
}

fn valid_widths_for_height(sprite_h: u32) -> &'static [u32] {
    match sprite_h {
        8 | 16 => &[8, 16, 32],
        32 => &[8, 16, 32, 64],
        _ => &[32, 64],
    }
}

fn pick_min_width(content_px: u32, valid: &[u32]) -> u32 {
    valid
        .iter()
        .copied()
        .find(|&w| w >= content_px)
        .unwrap_or_else(|| *valid.last().unwrap())
}

// --- Pixel blit -------------------------------------------------------------

/// Blit one glyph into `data` (the raw 4bpp byte buffer from
/// [`DynamicSprite16::data_mut`]) at horizontal pixel offset `sprite_x`
///
/// Sprite data is stored as a grid of 8x8 tiles (row-major). Each tile is
/// 32 bytes (8 rows x 4 bytes = 8 rows x 8 nibble-packed pixels)
///
/// Glyph data has the same 4bpp nibble-packed format as background tiles:
/// pixel 0 in the lowest nibble, pixel 7 in the highest
fn blit_glyph_to_sprite<T: AgbFont>(
    data: &mut [u8],
    sprite_w_tiles: usize,
    glyph: &[u32],
    font: &T,
    sprite_x: usize,
) {
    let height = font.glyph_height() as usize;
    let row_u32s = font.row_u32s();

    for row in 0..height {
        let ty = row >> 3;
        let row_in_tile = row & 7;

        for chunk in 0..row_u32s {
            let pixel_data = glyph[row * row_u32s + chunk];
            if pixel_data == 0 {
                continue;
            }

            let cx = sprite_x + chunk * 8;
            let tx_left = cx >> 3;
            if tx_left >= sprite_w_tiles {
                continue;
            }
            let shift = (cx & 7) as u32;
            let shift_bits = shift << 2;

            let base = (ty * sprite_w_tiles + tx_left) * 32 + row_in_tile * 4;
            let mut left_val = u32::from_le_bytes(data[base..base + 4].try_into().unwrap());
            blit_pixel_row_arm(&mut left_val, pixel_data << shift_bits);
            data[base..base + 4].copy_from_slice(&left_val.to_le_bytes());

            if shift > 0 {
                let tx_right = tx_left + 1;
                if tx_right < sprite_w_tiles {
                    let base_r = (ty * sprite_w_tiles + tx_right) * 32 + row_in_tile * 4;
                    let mut right_val =
                        u32::from_le_bytes(data[base_r..base_r + 4].try_into().unwrap());
                    blit_pixel_row_arm(&mut right_val, pixel_data >> (32 - shift_bits));
                    data[base_r..base_r + 4].copy_from_slice(&right_val.to_le_bytes());
                }
            }
        }
    }
}
