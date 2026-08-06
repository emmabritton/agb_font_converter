use crate::renderer::TextRenderer;
use crate::{TextAlign, TextFormat, TextOverflow};
use agb::display::tiled::RegularBackground;
use agb::fixnum::Vector2D;
use alloc::vec::Vec;
use gba_agb_font_eb::AgbFont;
use gba_agb_font_eb::full::FullFont;
use gba_agb_font_eb::printable::PrintableFont;

#[derive(Debug, Clone)]
pub enum SlotFont {
    Printable(&'static PrintableFont),
    Full(&'static FullFont),
}

impl From<&'static PrintableFont> for SlotFont {
    #[inline(always)]
    fn from(font: &'static PrintableFont) -> Self {
        Self::Printable(font)
    }
}

impl From<&'static FullFont> for SlotFont {
    #[inline(always)]
    fn from(font: &'static FullFont) -> Self {
        Self::Full(font)
    }
}

impl AgbFont for SlotFont {
    #[inline]
    fn char_widths(&self) -> &[u8] {
        match self {
            Self::Printable(f) => f.char_widths(),
            Self::Full(f) => f.char_widths(),
        }
    }

    #[inline]
    fn char_offset(&self) -> usize {
        match self {
            Self::Printable(f) => f.char_offset(),
            Self::Full(f) => f.char_offset(),
        }
    }

    #[inline]
    fn data(&self) -> &[u32] {
        match self {
            Self::Printable(f) => f.data(),
            Self::Full(f) => f.data(),
        }
    }

    #[inline]
    fn glyph_height(&self) -> u32 {
        match self {
            Self::Printable(f) => f.glyph_height(),
            Self::Full(f) => f.glyph_height(),
        }
    }

    #[inline]
    fn glyph_size(&self) -> usize {
        match self {
            Self::Printable(f) => f.glyph_size(),
            Self::Full(f) => f.glyph_size(),
        }
    }

    #[inline]
    fn row_u32s(&self) -> usize {
        match self {
            Self::Printable(f) => f.row_u32s(),
            Self::Full(f) => f.row_u32s(),
        }
    }

    #[inline]
    fn right_overhang(&self) -> u8 {
        match self {
            Self::Printable(f) => f.right_overhang(),
            Self::Full(f) => f.right_overhang(),
        }
    }
}

#[derive(Debug)]
struct TypewriterSlot {
    font: SlotFont,
    text: Vec<u8>,
    chars_revealed: usize,
    last_drawn: usize,
    frame_counter: u8,
    frames_per_update: u8,
    chars_per_update: u8,
    start: Vector2D<i32>,
    cursor: Vector2D<i32>,
    overflow: TextOverflow,
    alignment: TextAlign,
    line_width: i32,
    text_wrapped: bool,
    pause_remaining: u8,
}

impl TypewriterSlot {
    fn new(
        font: SlotFont,
        text: &[u8],
        pos: Vector2D<i32>,
        chars_per_update: u8,
        frames_per_update: u8,
        format: &TextFormat,
    ) -> Self {
        let mut v = Vec::with_capacity(text.len());
        v.extend_from_slice(text);
        Self {
            font,
            text: v,
            chars_revealed: 0,
            last_drawn: 0,
            frame_counter: 0,
            frames_per_update,
            chars_per_update,
            start: pos,
            cursor: pos,
            overflow: format.overflow,
            alignment: format.align,
            line_width: 0,
            text_wrapped: false,
            pause_remaining: 0,
        }
    }

    fn update(&mut self, punct_pause_frames: u8, punct_chars: &[u8]) -> usize {
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
            if punct_pause_frames > 0 && punct_chars.contains(&c) {
                self.pause_remaining = punct_pause_frames;
                break;
            }
        }
        revealed
    }
}

/// Like [`TypewriterRenderer`](crate::typewriter::TypewriterRenderer) but manages
/// multiple independent text streams over a single shared tile pool.
///
/// Each call to [`add_text`](MultiTypewriterRenderer::add_text) returns a slot
/// index. Call [`update`](MultiTypewriterRenderer::update) once per frame and
/// [`draw`](MultiTypewriterRenderer::draw) to flush all slots. Use the `*_slot`
/// variants to operate on a single slot.
///
/// ```ignore
///# use agb::fixnum::vec2;
///
/// let name_format = TextFormat::default();
/// let dialog_format = TextFormat::new(TextOverflow::Wrap(200, true), TextAlign::Left, (0, 0));
/// let mut mtw = MultiTypewriterRenderer::new(2, 1); // 1 char every 2 frames by default
/// mtw.add_text(SlotFont::from(FONT), b"Hero", vec2(8, 8), &name_format);
/// let dialog = mtw.add_text(SlotFont::from(OTHER_FONT), b"Hello, world!", vec2(8, 24), &dialog_format);
///
/// loop {
///     mtw.update();
///     mtw.draw(&mut bg, 15);
///     if mtw.is_complete() { break; }
///     vblank.wait_for_vblank();
/// }
/// mtw.clear_all();
/// ```
#[derive(Debug)]
pub struct MultiTypewriterRenderer {
    renderer: TextRenderer,
    slots: Vec<TypewriterSlot>,
    frames_per_update: u8,
    chars_per_update: u8,
    /// Extra frames a slot holds after revealing any byte in
    /// [`punctuation_chars`](MultiTypewriterRenderer::punctuation_chars)
    /// (default 0 = no pause). Shared by every slot
    pub punctuation_pause_frames: u8,
    /// Bytes that trigger the punctuation pause (default `b".!?"`)
    pub punctuation_chars: &'static [u8],
}

impl Default for MultiTypewriterRenderer {
    /// One character revealed every frame
    fn default() -> Self {
        Self::new(1, 1)
    }
}

impl MultiTypewriterRenderer {
    /// Create a renderer with the given default `frames_per_update` and `chars_per_update` applied to [`add_text`](MultiTypewriterRenderer::add_text) calls.
    ///
    /// Both rates are clamped to at least 1; a zero `chars_per_update` would never
    /// reveal anything and the slots would never complete
    pub fn new(frames_per_update: u8, chars_per_update: u8) -> Self {
        Self {
            renderer: TextRenderer::default(),
            slots: Vec::new(),
            frames_per_update: frames_per_update.max(1),
            chars_per_update: chars_per_update.max(1),
            punctuation_pause_frames: 0,
            punctuation_chars: b".!?",
        }
    }

    /// Add a new text stream. Returns its slot index for use with `*_slot` methods
    /// The update rates are clamped to at least 1, as in [`new`](MultiTypewriterRenderer::new)
    pub fn add_text_custom(
        &mut self,
        font: SlotFont,
        text: &[u8],
        pos: Vector2D<i32>,
        chars_per_update: u8,
        frames_per_update: u8,
        format: &TextFormat,
    ) -> usize {
        self.slots.push(TypewriterSlot::new(
            font,
            text,
            pos,
            chars_per_update.max(1),
            frames_per_update.max(1),
            format,
        ));
        self.slots.len() - 1
    }

    /// Add a new text stream. Returns its slot index for use with `*_slot` methods
    /// Uses default `chars_per_update` and `frames_per_update`
    pub fn add_text(
        &mut self,
        font: SlotFont,
        text: &[u8],
        pos: Vector2D<i32>,
        format: &TextFormat,
    ) -> usize {
        self.add_text_custom(
            font,
            text,
            pos,
            self.chars_per_update,
            self.frames_per_update,
            format,
        )
    }

    /// Advance all slots by one frame
    ///
    /// Returns the total number of bytes newly revealed across all slots, useful
    /// for playing a blip sound when any character appears
    pub fn update(&mut self) -> usize {
        let mut revealed = 0;
        for slot in &mut self.slots {
            revealed += slot.update(self.punctuation_pause_frames, self.punctuation_chars);
        }
        revealed
    }

    /// Advance a single slot by one frame. Returns the number of bytes it revealed
    pub fn update_slot(&mut self, idx: usize) -> usize {
        let (frames, chars) = (self.punctuation_pause_frames, self.punctuation_chars);
        self.slots[idx].update(frames, chars)
    }

    /// Draw newly revealed characters for all slots
    pub fn draw(&mut self, background: &mut RegularBackground, palette_id: u8) {
        for i in 0..self.slots.len() {
            self.draw_slot(i, background, palette_id);
        }
    }
    /// Draw newly revealed characters for a single slot
    pub fn draw_slot(&mut self, idx: usize, background: &mut RegularBackground, palette_id: u8) {
        let spacing = self.renderer.letter_spacing;
        if !self.slots[idx].text_wrapped {
            if let TextOverflow::Wrap(w, true) = self.slots[idx].overflow {
                let slot = &mut self.slots[idx];
                slot.font
                    .word_wrap_in_place(&mut slot.text, w as u32, spacing);
            }
            self.slots[idx].text_wrapped = true;
        }
        let slot = &self.slots[idx];
        if slot.last_drawn >= slot.chars_revealed {
            return;
        }
        let alignment = slot.alignment;
        let overflow = slot.overflow;
        let start = slot.start;
        let cursor = slot.cursor;
        let chars_revealed = slot.chars_revealed;
        let last_drawn = slot.last_drawn;
        let line_w = slot.line_width;

        if alignment == TextAlign::Left {
            let (cx, cy, _) = {
                let (renderer, slots) = (&mut self.renderer, &self.slots);
                renderer.int_draw_text(
                    &slots[idx].text[last_drawn..chars_revealed],
                    &slots[idx].font,
                    background,
                    cursor,
                    overflow,
                    TextAlign::Left,
                    palette_id,
                    (0, 0),
                    line_w,
                    start.x,
                )
            };
            let slot = &mut self.slots[idx];
            slot.cursor.x += cx;
            slot.cursor.y += cy;
            slot.line_width += cx;
        } else {
            let column_w = match alignment {
                TextAlign::Center(w) | TextAlign::Right(w) => w,
                TextAlign::Left => unreachable!(),
            };
            let size_wrap = match overflow {
                TextOverflow::Wrap(w, _) => Some(w as u32),
                _ => None,
            };
            let (_, h) = self.slots[idx].font.size_of(
                &self.slots[idx].text,
                size_wrap,
                spacing,
                self.renderer.line_spacing,
            );
            let (renderer, slots) = (&mut self.renderer, &self.slots);
            renderer.int_draw_text(
                &slots[idx].text[..chars_revealed],
                &slots[idx].font,
                background,
                start,
                overflow,
                alignment,
                palette_id,
                (column_w as i32, h as i32),
                0,
                start.x,
            );
        }
        self.slots[idx].last_drawn = chars_revealed;
    }

    /// Immediately reveal all characters in every slot
    pub fn complete_all(&mut self) {
        for slot in &mut self.slots {
            slot.chars_revealed = slot.text.len();
            slot.pause_remaining = 0;
        }
    }

    /// Immediately reveal all characters in a single slot
    pub fn complete_slot(&mut self, idx: usize) {
        let slot = &mut self.slots[idx];
        slot.chars_revealed = slot.text.len();
        slot.pause_remaining = 0;
    }

    /// Pixel position where the given slot will draw its next character
    /// Maintained for [`TextAlign::Left`] slots only, as in
    /// [`TypewriterRenderer::cursor`](crate::typewriter::TypewriterRenderer::cursor)
    pub fn slot_cursor(&self, idx: usize) -> Vector2D<i32> {
        self.slots[idx].cursor
    }

    /// Number of bytes the given slot has revealed so far
    pub fn slot_chars_revealed(&self, idx: usize) -> usize {
        self.slots[idx].chars_revealed
    }

    pub fn is_complete(&self) -> bool {
        self.slots.iter().all(|s| s.chars_revealed >= s.text.len())
    }

    /// Returns `true` when the given slot has fully revealed its text
    pub fn is_slot_complete(&self, idx: usize) -> bool {
        let slot = &self.slots[idx];
        slot.chars_revealed >= slot.text.len()
    }

    /// Number of active slots
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Clear the pixel region of `slot`, honouring its overflow and alignment
    fn clear_slot_pixels(renderer: &mut TextRenderer, slot: &TypewriterSlot) {
        if slot.text.is_empty() {
            return;
        }
        let spacing = renderer.letter_spacing;
        let line_spacing = renderer.line_spacing;
        let (w, h) = match slot.overflow {
            TextOverflow::Nothing => {
                let (tw, th) = slot.font.size_of(&slot.text, None, spacing, line_spacing);
                (tw as i32, th as i32)
            }
            TextOverflow::Wrap(ow, _) => {
                let (_, th) = slot
                    .font
                    .size_of(&slot.text, Some(ow as u32), spacing, line_spacing);
                (ow as i32, th as i32)
            }
            TextOverflow::Cutoff(ow) => {
                let (_, th) = slot.font.size_of(&slot.text, None, spacing, line_spacing);
                (ow as i32, th as i32)
            }
        };
        let w = match slot.alignment {
            TextAlign::Left => w,
            TextAlign::Center(cw) | TextAlign::Right(cw) => w.max(cw as i32),
        };
        // Styled (bold) ink may overhang the measured width
        let w = w + slot.font.right_overhang() as i32;
        renderer.clear_pixel_rect(slot.start, w, h);
    }

    /// Clear a single slot's pixel region and empty its text
    ///
    /// The slot stays in place as an inert, complete entry so that the indices of
    /// other slots remain valid
    pub fn clear_slot(&mut self, idx: usize) {
        Self::clear_slot_pixels(&mut self.renderer, &self.slots[idx]);
        let slot = &mut self.slots[idx];
        slot.text.clear();
        slot.chars_revealed = 0;
        slot.last_drawn = 0;
        slot.pause_remaining = 0;
    }

    /// Clear every slot's pixel region and remove all slots
    pub fn clear_all(&mut self) {
        for i in 0..self.slots.len() {
            Self::clear_slot_pixels(&mut self.renderer, &self.slots[i]);
        }
        self.slots.clear();
    }

    /// Access the shared inner [`TextRenderer`] directly
    pub fn renderer_mut(&mut self) -> &mut TextRenderer {
        &mut self.renderer
    }
}
