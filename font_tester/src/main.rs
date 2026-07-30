#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

extern crate alloc;

use agb::display::tiled::{RegularBackground, RegularBackgroundSize, TileFormat};
use agb::display::{GraphicsFrame, Priority, Rgb15};
use agb::fixnum::vec2;
use agb::input::{Button, ButtonController};
use alloc::format;
use alloc::string::String;
// `include_agb_font!` is re-exported by the prelude, so the loader needs no direct
// dependency here.
use gba_agb_font_renderer::prelude::*;

include_agb_font!(SIMPLE, "../examples/font_simple_idx0.aseprite");
include_agb_font!(
    SIMPLE_SHADOW,
    "../examples/font_simple_idx0shadow1.aseprite"
);
include_agb_font!(SEGMENT, "../examples/font_8segment_idx0.aseprite");
include_agb_font!(FANTASY, "../examples/font_fantasy_idx0shadow1.aseprite");
include_agb_font!(
    BALLOON,
    "../examples/font_balloon_idx0shadow1outline2.aseprite"
);
include_agb_font!(MONO_0, "../examples/mono_3x5_idx0.aseprite");
include_agb_font!(MONO_1, "../examples/mono_3x5_idx1.aseprite");
include_agb_font!(FULL, "../examples/full_font.aseprite", full);

include_agb_font!(
    SIMPLE_MONO,
    "../examples/font_simple_idx0.aseprite",
    monospace
);
include_agb_font!(
    SEGMENT_MONO,
    "../examples/font_8segment_idx0.aseprite",
    monospace
);
include_agb_font!(MONO_1_MONO, "../examples/mono_3x5_idx1.aseprite", monospace);

static UI_FONT: &PrintableFont = &SIMPLE;

static FONTS: &[(&[u8], SlotFont)] = &[
    (b"font_simple_idx0", SlotFont::Printable(&SIMPLE)),
    (
        b"font_simple_idx0shadow1",
        SlotFont::Printable(&SIMPLE_SHADOW),
    ),
    (b"font_8segment_idx0", SlotFont::Printable(&SEGMENT)),
    (b"font_fantasy_idx0shadow1", SlotFont::Printable(&FANTASY)),
    (
        b"font_balloon_idx0shadow1outline2",
        SlotFont::Printable(&BALLOON),
    ),
    (b"mono_3x5_idx0", SlotFont::Printable(&MONO_0)),
    (b"mono_3x5_idx1", SlotFont::Printable(&MONO_1)),
    (b"full_font", SlotFont::Full(&FULL)),
    (
        b"font_simple_idx0 (monospace)",
        SlotFont::Printable(&SIMPLE_MONO),
    ),
    (
        b"font_8segment_idx0 (monospace)",
        SlotFont::Printable(&SEGMENT_MONO),
    ),
    (
        b"mono_3x5_idx1 (monospace)",
        SlotFont::Printable(&MONO_1_MONO),
    ),
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

fn log_metrics() {
    agb::println!("font_tester: {} fonts, {} samples", FONTS.len(), TEXT.len());
    for (name, font) in FONTS {
        let height = font.glyph_height();
        let (w, h) = font.size_of(TEXT, Some(220), 0, 0);
        let name = String::from_utf8_lossy(name);
        agb::println!("  {name} text: {w}x{h}px (glyph h={height})");
    }

    for (name, font) in FONTS {
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

    for (name, font) in FONTS {
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

        if button_controller.is_just_pressed(Button::Right) && self.text_renderer.letter_spacing < 4 {
            self.text_renderer.letter_spacing += 1;
            self.dirty = true;
        } else if button_controller.is_just_pressed(Button::Left)
            && self.text_renderer.letter_spacing > -2
        {
            self.text_renderer.letter_spacing -= 1;
            self.dirty = true;
        }
        if button_controller.is_just_pressed(Button::Up) && self.text_renderer.line_spacing < 4 {
            self.text_renderer.line_spacing += 1;
            self.dirty = true;
        } else if button_controller.is_just_pressed(Button::Down)
            && self.text_renderer.line_spacing > -2
        {
            self.text_renderer.line_spacing -= 1;
            self.dirty = true;
        }

        if self.dirty {
            self.text_renderer.reset(false);
            self.text_renderer.draw_text(
                TEXT,
                &FONTS[self.font].1,
                &mut self.bg_text,
                vec2(6, 14),
                &FORMAT,
            );

            self.text_renderer.draw_text(
                FONTS[self.font].0,
                UI_FONT,
                &mut self.bg_text,
                vec2(0, 1),
                &FORMAT_INFO,
            );

            let spacing_info = format!(
                "ls {} lh {}",
                self.text_renderer.letter_spacing, self.text_renderer.line_spacing
            );
            self.text_renderer.draw_text(
                spacing_info.as_bytes(),
                UI_FONT,
                &mut self.bg_text,
                vec2(1, 1),
                &TextFormat::default(),
            );
            self.dirty = false;
        }
    }
}
