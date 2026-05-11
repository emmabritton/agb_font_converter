use clap::{Arg, ArgAction, Command, arg, command, value_parser};
use gba_agb_font_creation_internals::create::create;
use gba_agb_font_creation_internals::edit::run_edit;
use gba_agb_font_creation_internals::parsing::parse_char_arg;
use gba_agb_font_creation_internals::update::run_update;
use std::path::PathBuf;

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

    let monospace = if matches.contains_id("monospace") {
        if let Some(value) = matches.get_one::<u8>("monospace") {
            Some(Some(*value))
        } else {
            Some(None)
        }
    } else {
        None
    };

    create(cell_width, cell_height, png_path, output, monospace);
}
