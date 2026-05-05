mod edit;
mod parsing;
mod update;

use crate::edit::run_edit;
use crate::parsing::{extract_glyph_data, open_image, parse_char_arg};
use crate::update::run_update;
use clap::{Arg, ArgAction, Command, arg, command, value_parser};
use image::GenericImageView;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

const SHEET_COLS: usize = 16;
const GLYPH_COUNT_FULL: usize = 256;
const GLYPH_COUNT_SMALL: usize = 95; // ASCII 32-126 (space to ~)

fn main() {
    let matches = command!()
        .subcommand_required(false)
        .subcommand(
            Command::new("update")
                .about("Replace pixel data in a compiled font binary from a new image, preserving all character widths")
                .arg(
                    arg!(<BIN> "Binary font file (.bin) to update in-place")
                        .value_parser(value_parser!(PathBuf)),
                )
                .arg(
                    arg!(<IMAGE> "PNG or Aseprite font sheet to read pixel data from")
                        .value_parser(value_parser!(PathBuf)),
                )
                .arg(
                    Arg::new("help")
                        .long("help")
                        .action(clap::ArgAction::Help)
                        .help("Print help"),
                ),
        )
        .subcommand(
            Command::new("edit")
                .about("Check or patch character widths in a compiled font binary")
                .arg(
                    arg!(<FILE> "Binary font file (.bin)")
                        .value_parser(value_parser!(PathBuf)),
                )
                .arg(
                    Arg::new("get")
                        .long("get")
                        .short('g')
                        .value_name("CHAR")
                        .action(ArgAction::Append)
                        .help("Print width for CHAR (ASCII char, decimal, or 0x hex)"),
                )
                .arg(
                    Arg::new("set")
                        .long("set")
                        .short('s')
                        .value_name("CHAR=WIDTH")
                        .action(ArgAction::Append)
                        .help("Set width for CHAR to WIDTH (e.g. A=8, 65=8, 0x41=8)"),
                )
                .arg(
                    Arg::new("help")
                        .long("help")
                        .action(clap::ArgAction::Help)
                        .help("Print help"),
                ),
        )
        .arg(
            arg!(-w --width [PX] "Cell width")
                .value_parser(value_parser!(u8)),
        )
        .arg(
            arg!(-h --height [PX] "Cell height")
                .value_parser(value_parser!(u8)),
        )
        .arg(
            arg!([FILE] "PNG/Aseprite Image")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            arg!(-o --output [FILE] "Output file")
                .required(false)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("monospace")
                .short('m')
                .long("monospace")
                .num_args(0..=1)
                .value_name("PX")
                .value_parser(value_parser!(u8))
                .help("Force uniform width. Without a value uses the widest glyph; with a value (e.g. -m 8) uses that width for all glyphs"),
        )
        .subcommand_negates_reqs(true)
        .arg(
            Arg::new("help")
                .long("help")
                .action(clap::ArgAction::Help)
                .help("Print help"),
        )
        .disable_help_flag(true)
        .get_matches();

    if let Some(update_matches) = matches.subcommand_matches("update") {
        let bin_path: &PathBuf = update_matches.get_one("BIN").expect("no bin file");
        let img_path: &PathBuf = update_matches.get_one("IMAGE").expect("no image file");
        run_update(bin_path, img_path);
        return;
    }

    if let Some(edit_matches) = matches.subcommand_matches("edit") {
        let bin_path: &PathBuf = edit_matches.get_one("FILE").expect("no file");

        let gets: Vec<u8> = edit_matches
            .get_many::<String>("get")
            .unwrap_or_default()
            .map(|s| parse_char_arg(s).unwrap_or_else(|e| panic!("--get {s:?}: {e}")))
            .collect();

        let sets: Vec<(u8, u8)> = edit_matches
            .get_many::<String>("set")
            .unwrap_or_default()
            .map(|s| {
                let (char_part, width_part) = s
                    .split_once('=')
                    .unwrap_or_else(|| panic!("--set {s:?}: expected CHAR=WIDTH format"));
                let cp = parse_char_arg(char_part).unwrap_or_else(|e| panic!("--set {s:?}: {e}"));
                let width = width_part
                    .parse::<u8>()
                    .unwrap_or_else(|e| panic!("--set {s:?}: invalid width: {e}"));
                (cp, width)
            })
            .collect();

        run_edit(bin_path, &gets, &sets);
        return;
    }

    let cell_width: u8 = matches.get_one::<u8>("width").copied().unwrap_or_else(|| {
        eprintln!("error: -w/--width is required for convert mode");
        std::process::exit(1);
    });
    let cell_height: u8 = matches.get_one::<u8>("height").copied().unwrap_or_else(|| {
        eprintln!("error: -h/--height is required for convert mode");
        std::process::exit(1);
    });
    let png_path: &PathBuf = matches.get_one("FILE").unwrap_or_else(|| {
        eprintln!("error: input FILE is required for convert mode");
        std::process::exit(1);
    });
    let output: Option<&PathBuf> = matches.get_one("output");

    let output_path: PathBuf = if let Some(output) = output {
        if output.is_dir() {
            panic!("Output {output:?} is a directory");
        } else {
            output.clone()
        }
    } else {
        let file_name = png_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| String::from("font"));
        let file_name = format!("{}.bin", file_name);
        if let Some(parent) = png_path.parent() {
            PathBuf::from(parent).join(file_name)
        } else {
            PathBuf::from(file_name)
        }
    };

    let img = open_image(png_path);

    let (_, img_height) = img.dimensions();

    let cell_rows = img_height / cell_height as u32;
    let (glyph_count, mode_byte): (usize, u8) =
        if cell_rows as usize * SHEET_COLS >= GLYPH_COUNT_FULL {
            (GLYPH_COUNT_FULL, 1)
        } else {
            (GLYPH_COUNT_SMALL, 0)
        };

    let data = extract_glyph_data(&img, cell_width, cell_height, glyph_count);

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

    if matches.contains_id("monospace") {
        let mono_width = matches
            .get_one::<u8>("monospace")
            .copied()
            .unwrap_or_else(|| *char_widths.iter().max().unwrap_or(&cell_width));
        char_widths.fill(mono_width);
    }

    let mut out = BufWriter::new(File::create(&output_path).expect("failed to create output file"));
    out.write_all(&[mode_byte, cell_width, cell_height])
        .unwrap();
    out.write_all(&char_widths).unwrap();
    out.write_all(&[0]).unwrap();
    if mode_byte == 0 {
        out.write_all(&[0]).unwrap();
    }
    for d in &data {
        out.write_all(&d.to_le_bytes()).unwrap();
    }
    out.flush().unwrap();

    println!("Written to {output_path:?} ({} glyphs)", glyph_count);
}
