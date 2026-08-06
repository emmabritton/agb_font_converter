/// Binary layout: 1 mode + 1 glyph_width + 1 glyph_height + 95 char_widths + 1 right_overhang + 1 padding = 100 bytes
pub const HEADER_PRINTABLE: usize = 95 + 1 + 1 + 1 + 2;
/// Printable ASCII: 0x20 (space) through 0x7E (tilde), 95 characters
pub const GLYPH_COUNT_PRINTABLE: usize = 95;
pub const PRINTABLE_ASCII_START: u8 = 0x20;
pub const PRINTABLE_ASCII_END: u8 = 0x7E;

/// 4bpp font for gba covering the 95 printable ASCII characters (0x20–0x7E)
/// Font data location (ROM/IWRAM/EWRAM) is determined by the `printable_font!` macro
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintableFont {
    pub char_widths: [u8; GLYPH_COUNT_PRINTABLE],
    pub data: &'static [u32],
    pub glyph_height: u32,
    pub glyph_size: usize,
    pub row_u32s: usize,
    pub right_overhang: u8,
}

impl PrintableFont {
    /// Parse a mode-0 binary font blob. Panics if the data has the wrong mode byte or its
    /// length does not match the glyph dimensions in the header
    ///
    /// # Safety
    ///
    /// `bytes` must be 4-byte aligned. The `printable_font!` and `include_agb_font!` macros
    /// guarantee this via `#[repr(C, align(4))]`; direct callers must uphold it
    pub const unsafe fn from_static_bytes(bytes: &'static [u8]) -> Self {
        assert!(bytes.len() >= HEADER_PRINTABLE, "font bytes too short");
        let mode = bytes[0];
        assert!(
            mode == 0,
            "invalid font mode (must be 0 for printable font)"
        );

        let glyph_width = bytes[1];
        let glyph_height = bytes[2] as u32;
        let row_u32s = (glyph_width as usize + 7) >> 3;

        assert!(
            bytes.len() >= 3 + GLYPH_COUNT_PRINTABLE,
            "font bytes too short for printable-95 mode"
        );
        let mut char_widths = [0u8; GLYPH_COUNT_PRINTABLE];
        let mut i = 0usize;
        while i < GLYPH_COUNT_PRINTABLE {
            char_widths[i] = bytes[3 + i];
            i += 1;
        }

        // Old fonts emitted a zero padding byte here, so they parse as overhang 0
        let right_overhang = bytes[HEADER_PRINTABLE - 2];

        let glyph_size = row_u32s * glyph_height as usize;
        let data_len = bytes.len() - HEADER_PRINTABLE;
        assert!(
            data_len == glyph_size * GLYPH_COUNT_PRINTABLE * 4,
            "font pixel data length does not match the glyph dimensions in the header"
        );
        let data: &'static [u32] = unsafe {
            core::slice::from_raw_parts(
                bytes.as_ptr().add(HEADER_PRINTABLE) as *const u32,
                data_len / 4,
            )
        };
        Self {
            glyph_height,
            glyph_size,
            row_u32s,
            char_widths,
            data,
            right_overhang,
        }
    }
}

/// Construct a static [`PrintableFont`] from a byte array literal, with optional `iwram` or `ewram` placement
#[macro_export]
macro_rules! printable_font {
    ($bytes:expr) => {{
        #[repr(C, align(4))]
        struct AlignedFont([u8; { $bytes.len() }]);
        static FONT_BYTES: AlignedFont = AlignedFont(*$bytes);
        // SAFETY: `AlignedFont` is `#[repr(C, align(4))]`, so the bytes are 4-byte aligned.
        unsafe { $crate::printable::PrintableFont::from_static_bytes(&FONT_BYTES.0) }
    }};
    ($bytes:expr, iwram) => {{
        #[repr(C, align(4))]
        struct AlignedFont([u8; { $bytes.len() }]);
        #[unsafe(link_section = ".iwram")]
        static FONT_BYTES: AlignedFont = AlignedFont(*$bytes);
        // SAFETY: `AlignedFont` is `#[repr(C, align(4))]`, so the bytes are 4-byte aligned.
        unsafe { $crate::printable::PrintableFont::from_static_bytes(&FONT_BYTES.0) }
    }};
    ($bytes:expr, ewram) => {{
        #[repr(C, align(4))]
        struct AlignedFont([u8; { $bytes.len() }]);
        #[unsafe(link_section = ".ewram")]
        static FONT_BYTES: AlignedFont = AlignedFont(*$bytes);
        // SAFETY: `AlignedFont` is `#[repr(C, align(4))]`, so the bytes are 4-byte aligned.
        unsafe { $crate::printable::PrintableFont::from_static_bytes(&FONT_BYTES.0) }
    }};
}
