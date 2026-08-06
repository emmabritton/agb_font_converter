use crate::renderer::TextRenderer;
use crate::{TextAlign, TextFormat, TextOverflow};
use agb::display::tiled::RegularBackground;
use agb::fixnum::Vector2D;
use alloc::vec::Vec;
use gba_agb_font_eb::AgbFont;

/// Wraps [`TextRenderer`] to reveal text one chunk at a time, like a typewriter
///
/// Call [`update`](TypewriterRenderer::update) once per frame and
/// [`draw`](TypewriterRenderer::draw) whenever you want to flush newly revealed
/// characters to the background
///
/// ```ignore
///# use agb::fixnum::vec2;
///
/// let format = TextFormat::new(TextOverflow::Wrap(200, true), TextAlign::Left, (0, 0));
/// let mut tw = TypewriterRenderer::new(1, 2, &format); // 1 char every 2 frames, wrap at 200px
/// tw.load_text(b"Hello, world!", vec2(8, 16));
///
/// loop {
///     tw.update();
///     tw.draw(&FONT, &mut bg, 15);
///     if tw.is_complete() { break; }
///     vblank.wait_for_vblank();
/// }
/// ```
#[derive(Debug)]
pub struct TypewriterRenderer {
    renderer: TextRenderer,
    text: Vec<u8>,
    chars_revealed: usize,
    last_drawn: usize,
    frame_counter: u8,
    frames_per_update: u8,
    chars_per_update: u8,
    start: Vector2D<i32>,
    /// pixel position of the next character to be drawn (Left alignment only)
    cursor: Vector2D<i32>,
    overflow: TextOverflow,
    alignment: TextAlign,
    /// pixels used on the current line (Left alignment only)
    line_width: i32,
    text_wrapped: bool,
    /// Extra frames to hold after revealing any byte in
    /// [`punctuation_chars`](TypewriterRenderer::punctuation_chars) (default 0 = no pause)
    pub punctuation_pause_frames: u8,
    /// Bytes that trigger the punctuation pause (default `b".!?"`)
    pub punctuation_chars: &'static [u8],
    pause_remaining: u8,
}

impl TypewriterRenderer {
    /// * `chars_per_update` – characters revealed each time the timer fires
    /// * `frames_per_update` – frames between each reveal (1 = every frame)
    ///
    /// Both rates are clamped to at least 1; a zero `chars_per_update` would
    /// never reveal anything and the text would never complete.
    pub fn new(chars_per_update: u8, frames_per_update: u8, format: &TextFormat) -> Self {
        Self {
            renderer: TextRenderer::default(),
            text: Vec::new(),
            chars_revealed: 0,
            last_drawn: 0,
            frame_counter: 0,
            frames_per_update: frames_per_update.max(1),
            chars_per_update: chars_per_update.max(1),
            start: Vector2D::default(),
            cursor: Vector2D::default(),
            overflow: format.overflow,
            alignment: format.align,
            line_width: 0,
            text_wrapped: false,
            punctuation_pause_frames: 0,
            punctuation_chars: b".!?",
            pause_remaining: 0,
        }
    }

    /// Load new text and reset all state. Does **not** clear the background
    /// call [`clear`](TypewriterRenderer::clear) first if needed
    pub fn load_text(&mut self, text: &[u8], pos: Vector2D<i32>) {
        self.text.clear();
        self.text.extend_from_slice(text);
        self.chars_revealed = 0;
        self.last_drawn = 0;
        self.frame_counter = 0;
        self.start = pos;
        self.cursor = pos;
        self.line_width = 0;
        self.text_wrapped = false;
        self.pause_remaining = 0;
    }

    /// Advance the frame counter by one and reveal any characters that are due.
    /// Call once per frame; has no effect once all characters are revealed
    ///
    /// Returns the number of bytes newly revealed this call (0 when idle, paused
    /// or complete), useful for playing a blip sound per revealed character
    ///
    /// When [`punctuation_pause_frames`](TypewriterRenderer::punctuation_pause_frames)
    /// is non-zero, revealing stops just after a byte from
    /// [`punctuation_chars`](TypewriterRenderer::punctuation_chars) and holds for
    /// that many extra frames
    pub fn update(&mut self) -> usize {
        if self.chars_revealed >= self.text.len() {
            return 0;
        }
        if self.pause_remaining > 0 {
            self.pause_remaining -= 1;
            return 0;
        }
        self.frame_counter += 1;
        if self.frame_counter < self.frames_per_update {
            return 0;
        }
        self.frame_counter = 0;
        let mut revealed = 0;
        while revealed < self.chars_per_update as usize && self.chars_revealed < self.text.len() {
            let c = self.text[self.chars_revealed];
            self.chars_revealed += 1;
            revealed += 1;
            if self.punctuation_pause_frames > 0 && self.punctuation_chars.contains(&c) {
                self.pause_remaining = self.punctuation_pause_frames;
                break;
            }
        }
        revealed
    }

    /// Blit any characters that have been revealed since the last draw call.
    /// Cheap to call even when nothing has changed
    ///
    /// For [`TextAlign::Left`] only newly revealed characters are blitted.
    /// For [`TextAlign::Center`] and [`TextAlign::Right`] the full revealed
    /// portion is redrawn each call so that line offsets stay correct
    pub fn draw<T: AgbFont>(
        &mut self,
        font: &T,
        background: &mut RegularBackground,
        palette_id: u8,
    ) {
        if self.last_drawn >= self.chars_revealed {
            return;
        }
        let spacing = self.renderer.letter_spacing;
        if !self.text_wrapped {
            if let TextOverflow::Wrap(w, true) = self.overflow {
                font.word_wrap_in_place(&mut self.text, w as u32, spacing);
            }
            self.text_wrapped = true;
        }
        if self.alignment == TextAlign::Left {
            let new_chars = &self.text[self.last_drawn..self.chars_revealed];
            let (cx, cy, _) = self.renderer.int_draw_text(
                new_chars,
                font,
                background,
                self.cursor,
                self.overflow,
                TextAlign::Left,
                palette_id,
                (0, 0),
                self.line_width,
                self.start.x,
            );
            self.cursor.x += cx;
            self.cursor.y += cy;
            self.line_width += cx;
        } else {
            let column_w = match self.alignment {
                TextAlign::Center(w) | TextAlign::Right(w) => w,
                TextAlign::Left => unreachable!(),
            };
            let size_wrap = match self.overflow {
                TextOverflow::Wrap(w, _) => Some(w),
                _ => None,
            };
            let (_, h) = font.size_of(
                &self.text,
                size_wrap.map(|w| w as u32),
                spacing,
                self.renderer.line_spacing,
            );
            self.renderer.int_draw_text(
                &self.text[..self.chars_revealed],
                font,
                background,
                self.start,
                self.overflow,
                self.alignment,
                palette_id,
                (column_w as i32, h as i32),
                0,
                self.start.x,
            );
        }
        self.last_drawn = self.chars_revealed;
    }

    /// Immediately reveal all characters. The next [`draw`](TypewriterRenderer::draw)
    /// call will flush them all to the background
    pub fn complete(&mut self) {
        self.chars_revealed = self.text.len();
        self.pause_remaining = 0;
    }

    pub fn is_complete(&self) -> bool {
        self.chars_revealed >= self.text.len()
    }

    /// Pixel position where the next character will be drawn, e.g. for placing a
    /// blinking "continue" arrow after the text
    ///
    /// Maintained for [`TextAlign::Left`] only; with centered or right-aligned
    /// text the revealed portion is redrawn as a block each call and this stays
    /// at the start position
    pub fn cursor(&self) -> Vector2D<i32> {
        self.cursor
    }

    /// Number of bytes revealed so far.
    pub fn chars_revealed(&self) -> usize {
        self.chars_revealed
    }

    /// Clear the pixel region occupied by the current text.
    /// Call before [`load_text`](TypewriterRenderer::load_text) to erase the
    /// previous message
    pub fn clear<T: AgbFont>(&mut self, font: &T) {
        if self.text.is_empty() {
            return;
        }
        let spacing = self.renderer.letter_spacing;
        let line_spacing = self.renderer.line_spacing;
        let (w, h) = match self.overflow {
            TextOverflow::Nothing => {
                let (tw, th) = font.size_of(&self.text, None, spacing, line_spacing);
                (tw as i32, th as i32)
            }
            TextOverflow::Wrap(ow, _) => {
                let (_, th) = font.size_of(&self.text, Some(ow as u32), spacing, line_spacing);
                (ow as i32, th as i32)
            }
            TextOverflow::Cutoff(ow) => {
                let (_, th) = font.size_of(&self.text, None, spacing, line_spacing);
                (ow as i32, th as i32)
            }
        };
        let w = match self.alignment {
            TextAlign::Left => w,
            TextAlign::Center(cw) | TextAlign::Right(cw) => w.max(cw as i32),
        };
        // Styled (italic/bold) ink may overhang the measured width on every arm,
        // including past a Wrap/Cutoff column's last glyph
        let w = w + font.right_overhang() as i32;
        self.renderer.clear_pixel_rect(self.start, w, h);
    }

    /// Access the inner [`TextRenderer`] directly, e.g. to call
    /// [`clear_pixel_rect`](TextRenderer::clear_pixel_rect) with custom bounds
    pub fn renderer_mut(&mut self) -> &mut TextRenderer {
        &mut self.renderer
    }
}
