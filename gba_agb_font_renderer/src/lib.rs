#![no_std]
#![cfg_attr(test, no_main)]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

pub mod multi_typewriter;
pub mod renderer;
pub mod sprite_renderer;
pub mod typewriter;

extern crate alloc;

/// Declares a font `static` from an image sheet at compile time.
///
/// Re-exported so depending on this crate is enough, the macro resolves the font types
/// through the [`gba_agb_font_eb`] re-export below when the calling crate has no direct
/// dependency on it.
pub use gba_agb_font_loader::include_agb_font;

/// The font format crate, re-exported so [`include_agb_font!`] can name its types.
pub use gba_agb_font_eb;

pub mod prelude {
    pub use crate::TextAlign;
    pub use crate::TextFormat;
    pub use crate::TextOverflow;
    pub use crate::include_agb_font;
    pub use crate::multi_typewriter::*;
    pub use crate::renderer::*;
    pub use crate::sprite_renderer::*;
    pub use crate::typewriter::*;
    pub use gba_agb_font_eb::prelude::*;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TextFormat {
    pub overflow: TextOverflow,
    pub align: TextAlign,
    /// Width and height in pixels of a rectangle cleared at the draw position
    /// before drawing; `(0, 0)` disables it
    ///
    /// Sized from roman measures (`size_of`, previous `draw_text` returns); with a
    /// styled (bold) font the renderer widens the clear by the font's
    /// [`right_overhang`](gba_agb_font_eb::AgbFont::right_overhang) so overhanging
    /// ink is erased too
    pub clear: (u16, u16),
}

impl TextFormat {
    pub const fn new(overflow: TextOverflow, align: TextAlign, clear: (u16, u16)) -> Self {
        Self {
            overflow,
            align,
            clear,
        }
    }

    pub const fn wrapped(px: u16) -> Self {
        Self::new(TextOverflow::Wrap(px, true), TextAlign::Left, (0, 0))
    }

    pub const fn hard_wrapped(px: u16) -> Self {
        Self::new(TextOverflow::Wrap(px, false), TextAlign::Left, (0, 0))
    }

    pub const fn cutoff(px: u16) -> Self {
        Self::new(TextOverflow::Cutoff(px), TextAlign::Left, (0, 0))
    }

    pub const fn centered(column_px: u16) -> Self {
        Self::new(TextOverflow::Nothing, TextAlign::Center(column_px), (0, 0))
    }

    pub const fn right_aligned(column_px: u16) -> Self {
        Self::new(TextOverflow::Nothing, TextAlign::Right(column_px), (0, 0))
    }

    pub const fn with_clear(mut self, width: u16, height: u16) -> Self {
        self.clear = (width, height);
        self
    }

    pub const fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub const fn with_overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}

impl Default for TextFormat {
    #[inline]
    fn default() -> Self {
        Self {
            clear: (0, 0),
            align: TextAlign::default(),
            overflow: TextOverflow::default(),
        }
    }
}

/// Controls what happens when rendered text reaches the horizontal limit
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum TextOverflow {
    /// No overflow handling; text renders until the string ends
    #[default]
    Nothing,
    /// Stop rendering glyphs once the cursor reaches this pixel offset from the draw origin
    Cutoff(u16),
    /// Wrap to a new line once the cursor would exceed this pixel width
    /// If the bool is `true`, wraps at whitespace (words longer than the width still break)
    /// If `false`, breaks at the character boundary
    Wrap(u16, bool),
}

/// Horizontal alignment for a block of rendered text
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum TextAlign {
    /// Align to the left edge; no column width needed
    #[default]
    Left,
    /// Center text within a column of the given pixel width
    Center(u16),
    /// Right-align text within a column of the given pixel width
    Right(u16),
}

#[allow(clippy::empty_loop)]
#[cfg(test)]
#[agb::entry]
fn main(mut _gba: agb::Gba) -> ! {
    loop {}
}

#[unsafe(link_section = ".iwram")]
#[instruction_set(arm::a32)]
fn blit_pixel_row_arm(target: &mut u32, src: u32) {
    if src == 0 {
        return;
    }

    // Mask Generation Logic:
    // We want a bitmask where every 4-bit nibble in 'src' that is non-zero
    // is set to 0xF in the mask

    let hi = src & 0x8888_8888;
    let lo = src & 0x7777_7777;

    // (lo + 0x7777_7777) will carry into the 4th bit of each nibble if lo > 0
    // We then OR that with the original hi bit to catch all non-zero nibbles
    let set_nybbles = (hi | ((lo.wrapping_add(0x7777_7777)) & 0x8888_8888)) >> 3;

    // Spread the 1-bit flags to 4-bit masks (0x1 -> 0xF)
    let mask = set_nybbles * 0xF;

    // Apply the mask: Clear target nibbles where src has data, then OR src
    *target = (*target & !mask) | src;
}
