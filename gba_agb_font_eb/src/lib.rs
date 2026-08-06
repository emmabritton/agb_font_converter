#![no_std]

use crate::full::FullFont;
use crate::printable::PrintableFont;

pub mod full;
pub mod printable;

pub mod prelude {
    pub use crate::AgbFont;
    pub use crate::Lines;
    pub use crate::full::FullFont;
    pub use crate::full_font;
    pub use crate::printable::PrintableFont;
    pub use crate::printable_font;
}

/// Iterator over visual lines of text, returned by [`AgbFont::lines`].
///
/// Each item is `(line_slice, line_width_px)`. Word-wrap boundaries consume the
/// space character; a break after a hyphen keeps the hyphen at the end of the
/// line and consumes nothing; hard-wrap and explicit `\n` boundaries do not
/// consume extra bytes.
pub struct Lines<'a, F: AgbFont + ?Sized> {
    font: &'a F,
    remaining: &'a [u8],
    wrap_px: Option<u32>,
    word_wrap: bool,
    spacing: i8,
}

impl<'a, F: AgbFont + ?Sized> Lines<'a, F> {
    /// Width of the unbreakable fragment starting at `start` and the index one past
    /// it. A fragment ends at a space or newline, or just *after* a hyphen, the
    /// next break opportunity. Every fragment character follows another character,
    /// so all of them carry a gap.
    fn fragment(&self, text: &[u8], start: usize) -> (u32, usize) {
        let end = text[start..]
            .iter()
            .position(|&x| x == b' ' || x == b'\n')
            .map_or(text.len(), |p| start + p);
        let end = text[start..end]
            .iter()
            .position(|&x| x == b'-')
            .map_or(end, |p| start + p + 1);
        let w = text[start..end]
            .iter()
            .map(|&x| self.font.char_advance(x, self.spacing))
            .sum();
        (w, end)
    }

    fn next_word_wrapped(&mut self, wrap_px: u32) -> (&'a [u8], u32) {
        let text = self.remaining;
        let mut line_w: u32 = 0;
        let mut i = 0;

        while i < text.len() {
            let c = text[i];
            if c == b'\n' {
                self.remaining = &text[i + 1..];
                return (&text[..i], line_w);
            }
            if c == b' ' {
                let space_w = self.font.char_step(c, self.spacing, i == 0);
                let (next_w, next_end) = self.fragment(text, i + 1);
                if next_end > i + 1 && line_w > 0 && line_w + space_w + next_w > wrap_px {
                    self.remaining = &text[i + 1..];
                    return (&text[..i], line_w);
                }
                line_w += space_w;
            } else if c == b'-' {
                let char_w = self.font.char_step(c, self.spacing, i == 0);
                if i > 0 && line_w + char_w > wrap_px {
                    self.remaining = &text[i..];
                    return (&text[..i], line_w);
                }
                line_w += char_w;
                // Break after the hyphen if what follows it would overflow; the
                // hyphen stays on this line and nothing is consumed.
                let (next_w, next_end) = self.fragment(text, i + 1);
                if next_end > i + 1 && line_w + next_w > wrap_px {
                    self.remaining = &text[i + 1..];
                    return (&text[..=i], line_w);
                }
            } else {
                let char_w = self.font.char_step(c, self.spacing, i == 0);
                if i > 0 && line_w + char_w > wrap_px {
                    self.remaining = &text[i..];
                    return (&text[..i], line_w);
                }
                line_w += char_w;
            }
            i += 1;
        }
        self.remaining = &[];
        (text, line_w)
    }
}

impl<'a, F: AgbFont + ?Sized> Iterator for Lines<'a, F> {
    type Item = (&'a [u8], u32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        if self.word_wrap
            && let Some(wrap) = self.wrap_px
        {
            return Some(self.next_word_wrapped(wrap));
        }
        let (width, consumed) = self
            .font
            .measure_line(self.remaining, self.wrap_px, self.spacing);
        let ends_newline = consumed > 0 && self.remaining[consumed - 1] == b'\n';
        let line = if ends_newline {
            &self.remaining[..consumed - 1]
        } else {
            &self.remaining[..consumed]
        };
        self.remaining = &self.remaining[consumed..];
        Some((line, width))
    }
}

/// 4bpp bitmap font for the GBA, loaded from a binary font blob
pub trait AgbFont {
    /// Advance widths in pixels for each character in the font's range
    fn char_widths(&self) -> &[u8];

    /// Advance width in pixels for character `c`
    ///
    /// Panics if `c` is outside the font's character range
    #[inline]
    fn char_width(&self, c: u8) -> u8 {
        let idx = c as usize;
        assert!(
            idx >= self.char_offset() && idx < self.char_offset() + self.char_widths().len(),
            "char {c} out of font bounds"
        );
        self.char_widths()[idx - self.char_offset()]
    }

    /// Advance width for `c` when it follows another character on the same line,
    /// i.e. [`char_width`](AgbFont::char_width) plus `spacing` pixels of gap
    ///
    /// Negative `spacing` tightens the text; the result is clamped to at least 1px
    /// so the cursor always moves forwards
    #[inline]
    fn char_advance(&self, c: u8, char_spacing: i8) -> u32 {
        let w = self.char_width(c) as i32;
        (w + char_spacing as i32).max(1) as u32
    }

    /// Pixels of gap inserted *before* `c` when it follows another character
    ///
    /// This is `char_advance - char_width`, so it is `spacing` except where the
    /// 1px minimum advance clamps a negative `spacing`. Renderers add this to the
    /// cursor before blitting a glyph; it is 0 for the first glyph on a line
    #[inline]
    fn letter_gap(&self, c: u8, char_spacing: i8) -> i32 {
        self.char_advance(c, char_spacing) as i32 - self.char_width(c) as i32
    }

    /// Advance for `c` at a given position in a line: the first character on a line
    /// gets no leading gap, every later one does
    ///
    /// All measuring and drawing goes through this so that a line's measured width
    /// always matches what the renderer draws
    #[inline]
    fn char_step(&self, c: u8, char_spacing: i8, first_on_line: bool) -> u32 {
        if first_on_line {
            self.char_width(c) as u32
        } else {
            self.char_advance(c, char_spacing)
        }
    }

    /// Vertical advance from the top of one line to the top of the next, i.e.
    /// [`glyph_height`](AgbFont::glyph_height) plus `line_spacing` pixels of gap
    ///
    /// Negative `line_spacing` tightens the text; the result is clamped to at
    /// least 1px so the cursor always moves downwards. Like letter spacing, the
    /// gap sits *between* lines only, a single line never gains one
    #[inline]
    fn line_advance(&self, line_spacing: i8) -> u32 {
        (self.glyph_height() as i32 + line_spacing as i32).max(1) as u32
    }

    /// Subtracted from a character's byte value to index into [`char_widths`](AgbFont::char_widths).
    fn char_offset(&self) -> usize;

    /// Raw 4bpp pixel data; 8 pixels packed per `u32`, row-major
    fn data(&self) -> &[u32];

    /// Height of every glyph in pixels
    fn glyph_height(&self) -> u32;

    /// Max pixels a styled (bold) glyph's ink may extend past its roman
    /// advance width; 0 for unstyled fonts
    ///
    /// Renderers add this to any bounding box derived from measured text, clear
    /// rectangles and sprite widths, so the overhanging ink is neither cut off nor
    /// left behind. The measuring APIs themselves stay roman
    fn right_overhang(&self) -> u8;

    /// Number of `u32`s per glyph
    fn glyph_size(&self) -> usize;

    /// Number of `u32`s per glyph row
    fn row_u32s(&self) -> usize;

    /// Return the pixel data for the glyph corresponding to `c`
    ///
    /// Panics if `c` is outside the font's character range, for a
    /// [`PrintableFont`], that is any byte below 0x20 or above 0x7E, including
    /// the continuation bytes of non-ASCII UTF-8 text
    fn glyph(&self, c: u8) -> &[u32] {
        let idx = c as usize;
        assert!(
            idx >= self.char_offset() && idx < self.char_offset() + self.char_widths().len(),
            "glyph {c} out of font bounds"
        );
        let idx = idx - self.char_offset();
        let offset = idx * self.glyph_size();
        &self.data()[offset..offset + self.glyph_size()]
    }

    /// Returns an iterator over visual lines of `text`
    ///
    /// Each item is `(line_slice, line_width_px)`. Set `word_wrap` to `true` to
    /// break at word (space) boundaries instead of character boundaries
    ///
    /// When `word_wrap` is `true` but `wrap_px` is `None`, word wrapping is silently
    /// skipped and the iterator falls back to character-level breaking (same as
    /// `word_wrap: false`)
    ///
    /// `spacing` is the extra gap in pixels between adjacent characters; the widths
    /// returned never include a trailing gap after a line's last character
    fn lines<'a>(
        &'a self,
        text: &'a [u8],
        wrap_px: Option<u32>,
        word_wrap: bool,
        char_spacing: i8,
    ) -> Lines<'a, Self>
    where
        Self: Sized,
    {
        Lines {
            font: self,
            remaining: text,
            wrap_px,
            word_wrap,
            spacing: char_spacing,
        }
    }

    /// Mutate `text` in place for word-aware wrapping, replacing spaces with `\n`
    /// where the next word would overflow `wrap_px`
    ///
    /// Words longer than `wrap_px` are **not** split here, because the slice length
    /// is fixed, inserting a mid-word `\n` would require shifting bytes.
    /// The caller's renderer must handle character-level hard breaks for overlong words.
    /// (`int_draw_text` does this; use [`lines`](AgbFont::lines) instead if you need
    /// a self-contained iterator that handles both cases.)
    ///
    /// Hyphen breaks are also impossible in place (they would need a byte *inserted*
    /// after the hyphen), so text wrapped this way, including by the typewriters,
    /// breaks at spaces only, unlike [`lines`](AgbFont::lines)
    fn word_wrap_in_place(&self, text: &mut [u8], wrap_px: u32, char_spacing: i8) {
        let mut line_w: u32 = 0;
        let mut i = 0;
        while i < text.len() {
            let c = text[i];
            if c == b'\n' {
                line_w = 0;
            } else if c == b' ' {
                let (space_w, next_w, next_end) =
                    self.word_lookahead(text, i, char_spacing, line_w == 0);
                if next_end > i + 1 && line_w > 0 && line_w + space_w + next_w > wrap_px {
                    text[i] = b'\n';
                    line_w = 0;
                } else {
                    line_w += space_w;
                }
            } else {
                line_w += self.char_step(c, char_spacing, line_w == 0);
            }
            i += 1;
        }
    }

    /// Widths of the space at `space_at` and of the word following it, plus the index
    /// one past that word. `first_on_line` suppresses the leading gap on the space
    #[inline]
    fn word_lookahead(
        &self,
        text: &[u8],
        space_at: usize,
        char_spacing: i8,
        first_on_line: bool,
    ) -> (u32, u32, usize) {
        let space_w = self.char_step(b' ', char_spacing, first_on_line);
        let next_start = space_at + 1;
        let next_end = text[next_start..]
            .iter()
            .position(|&x| x == b' ' || x == b'\n')
            .map_or(text.len(), |p| next_start + p);
        // Every character of the word follows the space, so all of them carry a gap.
        let next_w: u32 = text[next_start..next_end]
            .iter()
            .map(|&x| self.char_advance(x, char_spacing))
            .sum();
        (space_w, next_w, next_end)
    }

    /// Measures one line of text, returning `(line_width_px, bytes_consumed)`
    ///
    /// `bytes_consumed` includes the terminating `\n` if present, so
    /// `&text[bytes_consumed..]` always starts the next line. Wrapping
    /// never splits off a zero-width line: the first character always fits
    ///
    /// `spacing` adds a gap between adjacent characters. The returned width has no
    /// trailing gap after the last character, so it can be used for alignment directly.
    fn measure_line(&self, text: &[u8], wrap_at: Option<u32>, char_spacing: i8) -> (u32, usize) {
        let mut width: u32 = 0;
        for (i, &c) in text.iter().enumerate() {
            if c == b'\n' {
                return (width, i + 1);
            }
            let char_w = self.char_step(c, char_spacing, i == 0);
            if let Some(max) = wrap_at
                && i > 0
                && width + char_w > max
            {
                return (width, i);
            }
            width += char_w;
        }
        (width, text.len())
    }

    /// Returns the `(width, height)` in pixels required to render `text`, with optional line-wrapping
    ///
    /// `spacing` is the gap between adjacent characters and `line_spacing` the gap
    /// between adjacent lines; both sit *between* elements only, so n lines gain
    /// n-1 line gaps and a single line gains none
    fn size_of(
        &self,
        text: &[u8],
        wrap_at: Option<u32>,
        char_spacing: i8,
        line_spacing: i8,
    ) -> (u32, u32) {
        if text.is_empty() {
            return (0, 0);
        }
        let mut max_w: u32 = 0;
        let mut total_h: u32 = self.glyph_height();
        let mut remaining = text;
        loop {
            let (line_w, consumed) = self.measure_line(remaining, wrap_at, char_spacing);
            if line_w > max_w {
                max_w = line_w;
            }
            remaining = &remaining[consumed..];
            if remaining.is_empty() {
                break;
            }
            total_h += self.line_advance(line_spacing);
        }
        (max_w, total_h)
    }
}

macro_rules! impl_agb_font {
    ($font_class:ident, $offset: expr) => {
        impl AgbFont for $font_class {
            #[inline]
            fn char_widths(&self) -> &[u8] {
                &self.char_widths
            }

            #[inline]
            fn char_offset(&self) -> usize {
                $offset
            }

            #[inline]
            fn data(&self) -> &[u32] {
                &self.data
            }

            #[inline]
            fn glyph_height(&self) -> u32 {
                self.glyph_height
            }

            #[inline]
            fn right_overhang(&self) -> u8 {
                self.right_overhang
            }

            #[inline]
            fn glyph_size(&self) -> usize {
                self.glyph_size
            }

            #[inline]
            fn row_u32s(&self) -> usize {
                self.row_u32s
            }
        }
    };
}

impl_agb_font!(PrintableFont, 32);
impl_agb_font!(FullFont, 0);

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    /// Minimal valid small-font blob: 4x1 cells, so one u32 of pixel data per glyph
    const fn printable_blob(right_overhang: u8) -> [u8; 100 + 95 * 4] {
        let mut bytes = [0u8; 100 + 95 * 4];
        bytes[0] = 0;
        bytes[1] = 4;
        bytes[2] = 1;
        let mut i = 0;
        while i < 95 {
            bytes[3 + i] = 2;
            i += 1;
        }
        bytes[98] = right_overhang;
        bytes
    }

    /// Minimal valid full-font blob: 4x1 cells
    const fn full_blob(right_overhang: u8) -> [u8; 260 + 256 * 4] {
        let mut bytes = [0u8; 260 + 256 * 4];
        bytes[0] = 1;
        bytes[1] = 4;
        bytes[2] = 1;
        let mut i = 0;
        while i < 256 {
            bytes[3 + i] = 2;
            i += 1;
        }
        bytes[259] = right_overhang;
        bytes
    }

    const STYLED_PRINTABLE: [u8; 480] = printable_blob(5);
    const ROMAN_PRINTABLE: [u8; 480] = printable_blob(0);
    const STYLED_FULL: [u8; 1284] = full_blob(7);

    #[test]
    fn printable_font_reads_the_overhang_byte() {
        static FONT: PrintableFont = printable_font!(&STYLED_PRINTABLE);
        assert_eq!(FONT.right_overhang(), 5);
        assert_eq!(FONT.char_width(b'A'), 2, "overhang must not affect widths");
    }

    #[test]
    fn zero_padding_parses_as_no_overhang() {
        static FONT: PrintableFont = printable_font!(&ROMAN_PRINTABLE);
        assert_eq!(FONT.right_overhang(), 0);
    }

    #[test]
    fn full_font_reads_the_overhang_byte() {
        static FONT: FullFont = full_font!(&STYLED_FULL);
        assert_eq!(FONT.right_overhang(), 7);
        assert_eq!(FONT.char_width(0xE9), 2);
    }
}
