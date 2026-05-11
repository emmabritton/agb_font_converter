use crate::{GLYPH_COUNT_FULL, GLYPH_COUNT_SMALL};
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::PathBuf;

pub fn run_edit(bin_path: &PathBuf, gets: &[u8], sets: &[(u8, u8)]) {
    let mut bytes = {
        let mut f =
            File::open(bin_path).unwrap_or_else(|e| panic!("Failed to open {bin_path:?}: {e}"));
        let mut b = Vec::new();
        f.read_to_end(&mut b).expect("failed to read file");
        b
    };

    if bytes.len() < 3 {
        panic!("File too short to be a valid font binary");
    }

    let mode_byte = bytes[0];
    let cell_width = bytes[1];
    let cell_height = bytes[2];

    let (glyph_count, widths_start) = match mode_byte {
        1 => (GLYPH_COUNT_FULL, 3usize),
        0 => (GLYPH_COUNT_SMALL, 3usize),
        _ => panic!("Unknown mode_byte {mode_byte}; expected 0 (small) or 1 (full)"),
    };
    let widths_end = widths_start + glyph_count;

    if bytes.len() < widths_end {
        panic!("File too short; expected at least {widths_end} bytes");
    }

    if gets.is_empty() && sets.is_empty() {
        let font_desc = if mode_byte == 1 {
            "full (256 glyphs)"
        } else {
            "small (95 glyphs)"
        };
        println!(
            "{} — {font_desc} — cell {cell_width}×{cell_height}",
            bin_path.display()
        );
        for i in 0..glyph_count {
            let cp = index_to_code_point(i, mode_byte);
            let w = bytes[widths_start + i];
            println!("  {:3}  {}  width={}", cp, char_display(cp), w);
        }
        return;
    }

    for &cp in gets {
        match code_point_to_index(cp, mode_byte) {
            Ok(idx) => println!(
                "{} ({}): width = {}",
                char_display(cp),
                cp,
                bytes[widths_start + idx]
            ),
            Err(e) => eprintln!("--get {}: {e}", char_display(cp)),
        }
    }

    let mut modified = false;
    for &(cp, new_width) in sets {
        if new_width > cell_width {
            eprintln!(
                "warning: width {new_width} for {} exceeds cell_width {cell_width}",
                char_display(cp)
            );
        }
        match code_point_to_index(cp, mode_byte) {
            Ok(idx) => {
                let old = bytes[widths_start + idx];
                bytes[widths_start + idx] = new_width;
                println!("{} ({}): {} → {new_width}", char_display(cp), cp, old);
                modified = true;
            }
            Err(e) => eprintln!("--set {}: {e}", char_display(cp)),
        }
    }

    if modified {
        let mut out = BufWriter::new(
            File::create(bin_path).unwrap_or_else(|e| panic!("Failed to write {bin_path:?}: {e}")),
        );
        out.write_all(&bytes).expect("failed to write bytes");
        out.flush().expect("failed to flush");
        println!("Saved.");
    }
}

fn index_to_code_point(idx: usize, mode_byte: u8) -> u8 {
    if mode_byte == 1 {
        idx as u8
    } else {
        (idx + 32) as u8
    }
}

fn char_display(cp: u8) -> String {
    if (32..127).contains(&cp) {
        format!("'{}'", cp as char)
    } else {
        format!("0x{cp:02x}")
    }
}

fn code_point_to_index(cp: u8, mode_byte: u8) -> Result<usize, String> {
    if mode_byte == 1 {
        Ok(cp as usize)
    } else if (32..=126).contains(&cp) {
        Ok((cp - 32) as usize)
    } else {
        Err(format!(
            "code point {cp} is outside the small font range (32–126)"
        ))
    }
}
