use asefile::AsepriteFile;
use clap::{arg, command, value_parser};
use image::{DynamicImage, GenericImageView, Rgba};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

const GLYPH_COUNT: usize = 256;
const SHEET_COLS: usize = 16;

fn pixel_to_index(pixel: Rgba<u8>) -> u8 {
    if pixel[3] < 128 {
        return 0;
    }
    let luma = (pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000;
    // luma 0-15 → index 0 (transparent), 16-31 → 1, ..., 240-255 → 15
    (luma >> 4) as u8
}

fn main() {
    let matches = command!()
        .arg(
            arg!(-w --width [PX] "Cell width")
                .required(true)
                .value_parser(value_parser!(u8)),
        )
        .arg(
            arg!(-h --height [PX] "Cell height")
                .required(true)
                .value_parser(value_parser!(u8)),
        )
        .arg(
            arg!([FILE] "PNG/Aseprite Image")
                .required(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            arg!(-o --output [FILE] "Output file")
                .required(false)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            clap::Arg::new("help")
                .long("help")
                .action(clap::ArgAction::Help)
                .help("Print help"),
        )
        .disable_help_flag(true)
        .get_matches();

    let cell_width: u8 = *matches.get_one("width").expect("no width");
    let cell_height: u8 = *matches.get_one("height").expect("no height");
    let png_path: &PathBuf = matches.get_one("FILE").expect("no input");
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

    let img = if png_path
        .extension()
        .map(|s| s.to_str() == Some("aseprite"))
        .unwrap_or(false)
    {
        let ase = AsepriteFile::read_file(png_path).expect("Aseprite file could not be read");
        DynamicImage::ImageRgba8(ase.frame(0).image())
    } else {
        image::open(png_path).unwrap_or_else(|e| panic!("Failed to open {:?}: {}", png_path, e))
    };

    let (img_width, img_height) = img.dimensions();
    let sheet_rows = GLYPH_COUNT.div_ceil(SHEET_COLS);

    assert!(
        img_width >= cell_width as u32 * SHEET_COLS as u32,
        "Image width {} too small for {} columns of {} px cells",
        img_width,
        SHEET_COLS,
        cell_width
    );
    assert!(
        img_height >= cell_height as u32 * sheet_rows as u32,
        "Image height {} too small for {} rows of {} px cells",
        img_height,
        sheet_rows,
        cell_height
    );

    let row_u32s = (cell_width as usize + 7) >> 3;
    let glyph_size = row_u32s * cell_height as usize;
    let total_u32s = glyph_size * GLYPH_COUNT;

    let mut data: Vec<u32> = vec![0u32; total_u32s];

    for glyph_idx in 0..GLYPH_COUNT {
        let sheet_col = glyph_idx % SHEET_COLS;
        let sheet_row = glyph_idx / SHEET_COLS;
        let base_x = sheet_col * cell_width as usize;
        let base_y = sheet_row * cell_height as usize;
        let glyph_base = glyph_idx * glyph_size;

        for row in 0..cell_height as usize {
            for word in 0..row_u32s {
                let mut val: u32 = 0;
                for px in 0..8usize {
                    let x = base_x + word * 8 + px;
                    if x < base_x + cell_width as usize {
                        let pixel = img.get_pixel(x as u32, (base_y + row) as u32);
                        let idx = pixel_to_index(pixel) as u32;
                        val |= idx << (px * 4);
                    }
                }
                data[glyph_base + row * row_u32s + word] = val;
            }
        }
    }

    let mut char_widths = [cell_width; GLYPH_COUNT];
    #[allow(clippy::needless_range_loop)]
    for glyph_idx in 0..GLYPH_COUNT {
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

    let mut out = BufWriter::new(File::create(&output_path).expect("failed to create output file"));
    out.write_all(&[cell_width, cell_height]).unwrap();
    out.write_all(&char_widths).unwrap();
    out.write_all(&[0, 0]).unwrap();
    for d in &data {
        out.write_all(&d.to_le_bytes()).unwrap();
    }
    out.flush().unwrap();

    println!("Written to {output_path:?}");
}
