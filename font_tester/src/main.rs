#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

extern crate alloc;

use agb::display::tiled::{RegularBackground, RegularBackgroundSize, TileFormat};
use agb::display::{GraphicsFrame, Priority, Rgb, Rgb15};
use agb::fixnum::vec2;
use agb::input::{Button, ButtonController};
use alloc::string::String;
// `include_agb_font!` is re-exported by the prelude, so the loader needs no direct
// dependency here.
use gba_agb_font_renderer::prelude::*;

include_agb_font!(
    LIMITED_BLOCKY,
    "../examples/limited_blocky_3x5_text15.aseprite"
);
include_agb_font!(BLOCKY, "../examples/blocky_3x7_text15.aseprite");
include_agb_font!(VHS, "../examples/vhs_10x16_text15.aseprite");
include_agb_font!(SEGMENT, "../examples/segment_8x8_text15_shadow14.aseprite");
include_agb_font!(
    BALLOON,
    "../examples/balloon_14x14_text15_shadow14_outline13.aseprite"
);
include_agb_font!(WESTERN, "../examples/western_13x14_text15.aseprite");
include_agb_font!(SERIF_7, "../examples/serif_7x10_text15_shadow14.aseprite");
include_agb_font!(SERIF_9, "../examples/serif_9x10_text15.aseprite");
include_agb_font!(SERIF_11, "../examples/serif_11x10_text15.aseprite");
include_agb_font!(
    GOTHIC,
    "../examples/gothic_16x14_text15.aseprite",
    widths = { 'q' = 5 }
);
include_agb_font!(PLAIN, "../examples/plain_8x8_text15.aseprite");
include_agb_font!(SIMPLE_8, "../examples/simple_8x8_text15_shadow14.aseprite");
include_agb_font!(SIMPLE_12, "../examples/simple_12x14_text15.aseprite");
include_agb_font!(
    SIMPLE_16,
    "../examples/simple_16x16_text15_shadow14.aseprite"
);
include_agb_font!(RETRO, "../examples/retro_8x9_text15.aseprite");
include_agb_font!(SCRIPT, "../examples/script_12x15_text15.aseprite");
include_agb_font!(DOT, "../examples/dot_16x16_text15.aseprite");
include_agb_font!(FANTASY, "../examples/fantasy_8x10_text15_shadow14.aseprite");
include_agb_font!(CHROME, "../examples/chrome_effect.aseprite");
include_agb_font!(FULL, "../examples/full_font.aseprite", full);

include_agb_font!(
    PLAIN_MONO,
    "../examples/plain_8x8_text15.aseprite",
    monospace
);
include_agb_font!(
    SEGMENT_MONO,
    "../examples/segment_8x8_text15_shadow14.aseprite",
    monospace
);
include_agb_font!(
    LIMITED_BLOCKY_MONO,
    "../examples/limited_blocky_3x5_text15.aseprite",
    monospace
);
include_agb_font!(PLAIN_STYLED, "../examples/plain_8x8_text15.aseprite", bold);
include_agb_font!(
    PLAIN_RECOLOURED,
    "../examples/plain_8x8_text15.aseprite",
    recolor = { 15 = 13 }
);

static UI_FONT: &PrintableFont = &PLAIN;

static FONTS: &[(&[u8], SlotFont, bool, u8)] = &[
    (
        b"limited_blocky_3x5_text15",
        SlotFont::Printable(&LIMITED_BLOCKY),
        false,
        1,
    ),
    (b"blocky_3x7_text15", SlotFont::Printable(&BLOCKY), false, 1),
    (b"vhs_10x16_text15", SlotFont::Printable(&VHS), true, 1),
    (
        b"segment_8x8_text15_shadow14",
        SlotFont::Printable(&SEGMENT),
        false,
        1,
    ),
    (
        b"balloon_14x14_text15_shadow14_outline13",
        SlotFont::Printable(&BALLOON),
        true,
        1,
    ),
    (
        b"western_13x14_text15",
        SlotFont::Printable(&WESTERN),
        true,
        1,
    ),
    (
        b"serif_7x10_text15_shadow14",
        SlotFont::Printable(&SERIF_7),
        false,
        1,
    ),
    (
        b"serif_9x10_text15",
        SlotFont::Printable(&SERIF_9),
        false,
        1,
    ),
    (
        b"serif_11x10_text15",
        SlotFont::Printable(&SERIF_11),
        false,
        1,
    ),
    (
        b"gothic_16x14_text15",
        SlotFont::Printable(&GOTHIC),
        true,
        1,
    ),
    (b"plain_8x8_text15", SlotFont::Printable(&PLAIN), false, 1),
    (
        b"simple_8x8_text15_shadow14",
        SlotFont::Printable(&SIMPLE_8),
        false,
        1,
    ),
    (
        b"simple_12x14_text15",
        SlotFont::Printable(&SIMPLE_12),
        true,
        1,
    ),
    (
        b"simple_16x16_text15_shadow14",
        SlotFont::Printable(&SIMPLE_16),
        true,
        1,
    ),
    (b"retro_8x9_text15", SlotFont::Printable(&RETRO), false, 1),
    (
        b"script_12x15_text15",
        SlotFont::Printable(&SCRIPT),
        true,
        1,
    ),
    (b"dot_16x16_text15", SlotFont::Printable(&DOT), true, 4),
    (
        b"fantasy_8x10_text15_shadow14",
        SlotFont::Printable(&FANTASY),
        true,
        1,
    ),
    (b"chrome_effect", SlotFont::Printable(&CHROME), true, 1),
    (b"full_font", SlotFont::Full(&FULL), false, 1),
];

static TEXT: &[u8] = br##" !"#$%&'()*+,-./
0123456789:;<=>?
@ABCDEFGHIJKLMNO
PQRSTUVWXYZ[\]^_
`abcdefghijklmno
pqrstuvwxyz{|}~

The quick brown fox jumps over the lazy dog.

Example Number: 45.6 (sym!)

Hyphen-aware word-wrapping over-engineered anti-disestablishmentarianism
"##;

static TALL_TEXT: &[u8] = br##" !"#$%&'()*+,-./
0123456789:;<=>?
@ABCDEFGHIJKLMNO
PQRSTUVWXYZ[\]^_
`abcdefghijklmno
pqrstuvwxyz{|}~

The quick brown fox jumps over the lazy dog.
"##;

fn log_metrics() {
    agb::println!("font_tester: {} fonts, {} samples", FONTS.len(), TEXT.len());
    for (name, font, _, _) in FONTS {
        let height = font.glyph_height();
        let (w, h) = font.size_of(TEXT, Some(220), 0, 0);
        let name = String::from_utf8_lossy(name);
        agb::println!("  {name} text: {w}x{h}px (glyph h={height})");
    }

    for (name, font, _, _) in FONTS {
        let name = String::from_utf8_lossy(name);
        let (one_tight, _) = font.size_of(b"M", None, -1, 0);
        let (one, _) = font.size_of(b"M", None, 0, 0);
        let (one_wide, _) = font.size_of(b"M", None, 3, 0);
        assert_eq!(one, one_tight, "{name}: 1 char changed with -1 spacing");
        assert_eq!(one, one_wide, "{name}: 1 char changed with +3 spacing");

        let (base, _) = font.size_of(b"MMMM", None, 0, 0);
        for spacing in 1..=4i8 {
            let (wide, _) = font.size_of(b"MMMM", None, spacing, 0);
            assert_eq!(
                wide,
                base + 3 * spacing as u32,
                "{name}: 4 chars at spacing {spacing}"
            );
        }
        let (tight, _) = font.size_of(b"MMMM", None, -1, 0);
        assert!(tight < base, "{name}: -1 spacing did not tighten");
        assert!(tight >= 4, "{name}: -1 spacing collapsed below 1px/char");
    }

    for (name, font, _, _) in FONTS {
        let name = String::from_utf8_lossy(name);
        let (_, one) = font.size_of(b"M", None, 0, 0);
        let (_, one_wide) = font.size_of(b"M", None, 0, 3);
        assert_eq!(one, one_wide, "{name}: 1 line changed with +3 line spacing");

        let (_, base) = font.size_of(b"M\nM\nM", None, 0, 0);
        for ls in 1..=4i8 {
            let (_, tall) = font.size_of(b"M\nM\nM", None, 0, ls);
            assert_eq!(
                tall,
                base + 2 * ls as u32,
                "{name}: 3 lines at line spacing {ls}"
            );
        }
        let (_, tight) = font.size_of(b"M\nM\nM", None, 0, -1);
        assert!(tight < base, "{name}: -1 line spacing did not tighten");
        assert!(
            tight >= 3,
            "{name}: -1 line spacing collapsed below 1px/line"
        );
    }

    // Bare `monospace` must give every glyph the widest scanned roman advance
    for (name, mono, roman) in [
        ("plain_8x8 monospace", &PLAIN_MONO, &PLAIN),
        ("segment_8x8 monospace", &SEGMENT_MONO, &SEGMENT),
        (
            "limited_blocky_3x5 monospace",
            &LIMITED_BLOCKY_MONO,
            &LIMITED_BLOCKY,
        ),
    ] {
        let widest = *roman.char_widths.iter().max().unwrap();
        assert!(
            mono.char_widths.iter().all(|&w| w == widest),
            "{name}: expected every advance to be {widest}"
        );
        assert_eq!(
            mono.right_overhang(),
            roman.right_overhang(),
            "{name}: monospace changed the overhang"
        );
    }

    // Styling must not touch metrics: advances stay roman, only the stored
    // overhang differs
    assert_eq!(
        PLAIN.char_widths, PLAIN_STYLED.char_widths,
        "styled font changed roman advances"
    );
    assert_eq!(
        PLAIN_STYLED.right_overhang(),
        1,
        "bare bold should store overhang of 1"
    );
    assert_eq!(PLAIN.right_overhang(), 0, "roman font gained an overhang");

    // Recolour must not touch metrics or overhang, only pixel data
    assert_eq!(
        PLAIN.char_widths, PLAIN_RECOLOURED.char_widths,
        "recoloured font changed advances"
    );
    assert_eq!(
        PLAIN_RECOLOURED.right_overhang(),
        0,
        "recolour should not store an overhang"
    );

    agb::println!("font_tester: metrics ok");
}

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    log_metrics();

    let mut gfx = gba.graphics.get();
    let mut input = ButtonController::new();

    gfx.set_background_palette_colour(0, 0, Rgb15(0x30e3));
    gfx.set_background_palette_colour(15, 15, Rgb15::WHITE);
    gfx.set_background_palette_colour(15, 14, Rgb15::BLACK);
    gfx.set_background_palette_colour(15, 13, Rgb15(0x7ffa));
    gfx.set_background_palette_colour(15, 11, Rgb15::from(Rgb::new(140, 0, 0)));
    gfx.set_background_palette_colour(15, 7, Rgb15::from(Rgb::new(0, 140, 0)));
    gfx.set_background_palette_colour(15, 9, Rgb15::from(Rgb::new(0, 0, 140)));

    let mut state = App::new();

    loop {
        input.update();
        let mut frame = gfx.frame();

        state.update(&input);
        state.show(&mut frame);

        frame.commit();
    }
}

static FORMAT: TextFormat = TextFormat {
    overflow: TextOverflow::Wrap(220, true),
    align: TextAlign::Left,
    clear: (1, 1),
};

static FORMAT_INFO: TextFormat = TextFormat {
    overflow: TextOverflow::Nothing,
    align: TextAlign::Right(240),
    clear: (1, 1),
};

struct App {
    text_renderer: TextRenderer,
    bg: RegularBackground,
    bg_text: RegularBackground,
    dirty: bool,
    font: usize,
}

impl App {
    fn new() -> Self {
        Self {
            bg: RegularBackground::new(
                Priority::P3,
                RegularBackgroundSize::Background32x32,
                TileFormat::FourBpp,
            ),
            bg_text: RegularBackground::new(
                Priority::P3,
                RegularBackgroundSize::Background32x32,
                TileFormat::FourBpp,
            ),
            text_renderer: TextRenderer::default(),
            dirty: true,
            font: 0,
        }
    }
}

impl App {
    pub fn show(&self, frame: &mut GraphicsFrame) {
        self.bg.show(frame);
        self.bg_text.show(frame);
    }

    pub fn update(&mut self, button_controller: &ButtonController) {
        if button_controller.is_just_pressed(Button::L) && self.font > 0 {
            self.font -= 1;
            self.dirty = true;
        } else if button_controller.is_just_pressed(Button::R) && self.font < FONTS.len() - 1 {
            self.font += 1;
            self.dirty = true;
        }

        if self.dirty {
            self.text_renderer.letter_spacing = FONTS[self.font].3 as i8;
            self.text_renderer.reset(false);
            self.text_renderer.draw_text(
                if FONTS[self.font].2 { TALL_TEXT } else { TEXT },
                &FONTS[self.font].1,
                &mut self.bg_text,
                vec2(4, 10),
                &FORMAT,
            );

            self.text_renderer.draw_text(
                FONTS[self.font].0,
                UI_FONT,
                &mut self.bg_text,
                vec2(0, 1),
                &FORMAT_INFO,
            );

            self.dirty = false;
        }
    }
}
