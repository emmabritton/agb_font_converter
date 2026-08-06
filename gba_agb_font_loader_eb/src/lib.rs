use gba_agb_font_creation_internals::{DEFAULT_COLS, DEFAULT_ROWS_FULL, DEFAULT_ROWS_SMALL};
use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::quote;
use syn::{
    LitChar, LitInt, LitStr, Token, braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

struct Args {
    vis: syn::Visibility,
    name: syn::Ident,
    path: LitStr,
    grid: Option<(u32, u32)>,
    monospace: Option<Option<u8>>,
    width_overrides: Vec<(u8, u8)>,
    bold: u8,
    recolor: Vec<(u8, u8)>,
    debug: bool,
}

impl Args {
    fn grid(&self) -> (u32, u32) {
        self.grid.unwrap_or((DEFAULT_COLS, DEFAULT_ROWS_SMALL))
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis: syn::Visibility = input.parse()?;
        let name: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let path: LitStr = input.parse()?;

        let mut grid = None;
        let mut monospace = None;
        let mut width_overrides = Vec::new();
        let mut bold = 0u8;
        let mut recolor = Vec::new();
        let mut debug = false;

        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            if input.peek(LitInt) {
                let v: LitInt = input.parse()?;
                return Err(syn::Error::new(
                    v.span(),
                    "cell dimensions are no longer passed as bare numbers; omit them for a \
                     16x6 small-font sheet, or write `full` or `size(cols, rows)`",
                ));
            }
            let ident: syn::Ident = input.parse()?;
            match ident.to_string().as_str() {
                "full" => {
                    if grid.is_some() {
                        return Err(syn::Error::new(
                            ident.span(),
                            "`full` and `size` both set the grid; use one",
                        ));
                    }
                    grid = Some((DEFAULT_COLS, DEFAULT_ROWS_FULL));
                }
                "size" => {
                    if grid.is_some() {
                        return Err(syn::Error::new(
                            ident.span(),
                            "`full` and `size` both set the grid; use one",
                        ));
                    }
                    let content;
                    parenthesized!(content in input);
                    let cols: LitInt = content.parse()?;
                    let cols = cols.base10_parse::<u32>()?;
                    content.parse::<Token![,]>()?;
                    let rows: LitInt = content.parse()?;
                    let rows = rows.base10_parse::<u32>()?;
                    if !content.is_empty() {
                        return Err(content.error("expected `size(cols, rows)`"));
                    }
                    grid = Some((cols, rows));
                }
                "monospace" => {
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let v: LitInt = input.parse()?;
                        monospace = Some(Some(v.base10_parse::<u8>()?));
                    } else {
                        monospace = Some(None);
                    }
                }
                "widths" => {
                    input.parse::<Token![=]>()?;
                    let content;
                    braced!(content in input);
                    while !content.is_empty() {
                        let cp: u8 = if content.peek(LitChar) {
                            let c: LitChar = content.parse()?;
                            let ch = c.value();
                            if !ch.is_ascii() {
                                return Err(syn::Error::new(
                                    c.span(),
                                    "only ASCII characters are supported",
                                ));
                            }
                            ch as u8
                        } else {
                            let v: LitInt = content.parse()?;
                            v.base10_parse::<u8>()?
                        };
                        content.parse::<Token![=]>()?;
                        let w: LitInt = content.parse()?;
                        width_overrides.push((cp, w.base10_parse::<u8>()?));
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                "recolor" => {
                    input.parse::<Token![=]>()?;
                    let content;
                    braced!(content in input);
                    while !content.is_empty() {
                        let from_lit: LitInt = content.parse()?;
                        let from = from_lit.base10_parse::<u8>()?;
                        if from == 0 {
                            return Err(syn::Error::new(
                                from_lit.span(),
                                "band 0 is the transparent background and cannot be recolored",
                            ));
                        }
                        if from > 15 {
                            return Err(syn::Error::new(
                                from_lit.span(),
                                "recolor bands are 0-15 (luma >> 4)",
                            ));
                        }
                        content.parse::<Token![=]>()?;
                        let to_lit: LitInt = content.parse()?;
                        let to = to_lit.base10_parse::<u8>()?;
                        if to > 15 {
                            return Err(syn::Error::new(
                                to_lit.span(),
                                "recolor bands are 0-15 (luma >> 4)",
                            ));
                        }
                        recolor.push((from, to));
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                "bold" => {
                    bold = if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let v: LitInt = input.parse()?;
                        v.base10_parse::<u8>()?
                    } else {
                        1
                    };
                }
                "debug" => {
                    debug = true;
                }
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!(
                            "unknown argument `{other}`; expected `full`, `size`, `monospace`, \
                             `widths`, `bold`, `recolor` or `debug`"
                        ),
                    ));
                }
            }
        }

        if !input.is_empty() {
            return Err(syn::Error::new(Span::call_site(), "unexpected tokens"));
        }

        Ok(Args {
            vis,
            name,
            path,
            grid,
            monospace,
            width_overrides,
            bold,
            recolor,
            debug,
        })
    }
}

/// The path the expansion uses to reach the font types.
///
/// A crate that depends on `gba_agb_font_eb` directly gets that; one that only depends on
/// `gba_agb_font_renderer` reaches the same types through its re-export, so the macro works
/// without adding a second dependency. Renamed dependencies resolve to their local name.
fn font_crate_path() -> proc_macro2::TokenStream {
    fn found(name: &str) -> Option<FoundCrate> {
        crate_name(name).ok()
    }
    fn ident(name: &str) -> syn::Ident {
        syn::Ident::new(name, Span::call_site())
    }

    if let Some(found) = found("gba_agb_font_eb") {
        return match found {
            FoundCrate::Itself => quote! { crate },
            FoundCrate::Name(name) => {
                let name = ident(&name);
                quote! { ::#name }
            }
        };
    }

    if let Some(found) = found("gba_agb_font_renderer") {
        return match found {
            FoundCrate::Itself => quote! { crate::gba_agb_font_eb },
            FoundCrate::Name(name) => {
                let name = ident(&name);
                quote! { ::#name::gba_agb_font_eb }
            }
        };
    }

    // Neither is a dependency. Emit the plain path so the error names the missing crate.
    quote! { ::gba_agb_font_eb }
}

/// Renders a code point as a `widths = { … }` key: a char literal when printable,
/// otherwise a hex code point.
fn width_key(cp: u8) -> String {
    match cp {
        b'\'' => String::from("'\\''"),
        b'\\' => String::from("'\\\\'"),
        0x20..=0x7e => format!("'{}'", cp as char),
        _ => format!("0x{cp:02x}"),
    }
}

/// Prints the computed advance widths to stdout at expansion time, formatted so the
/// block can be pasted straight back into the macro as a `widths = { … }` argument.
fn print_debug(args: &Args, path: &std::path::Path, bytes: &[u8], glyph_count: usize) {
    let mode_byte = bytes[0];
    let widths = &bytes[3..3 + glyph_count];

    let mut notes = Vec::new();
    match args.monospace {
        Some(Some(px)) => notes.push(format!("monospace = {px}")),
        Some(None) => notes.push(String::from("monospace (widest glyph)")),
        None => {}
    }
    if !args.width_overrides.is_empty() {
        notes.push(format!("{} width override(s)", args.width_overrides.len()));
    }
    if args.bold != 0 {
        notes.push(format!(
            "bold = {}, right overhang {}px",
            args.bold, args.bold
        ));
    }
    if !args.recolor.is_empty() {
        notes.push(format!("recolor {} band(s)", args.recolor.len()));
    }
    let notes = if notes.is_empty() {
        String::new()
    } else {
        format!(", {}", notes.join(", "))
    };

    let (cols, rows) = args.grid();
    let mode_desc = if mode_byte == 0 { "small" } else { "full" };
    println!("include_agb_font!({}), {}", args.name, path.display());
    println!(
        "  {mode_desc} font, {glyph_count} glyphs, grid {cols}x{rows}, cell {}x{}, {} bytes{notes}",
        bytes[1],
        bytes[2],
        bytes.len()
    );

    let first_cp: u8 = if mode_byte == 0 { 32 } else { 0 };
    let entries: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(i, w)| format!("{} = {w},", width_key(first_cp + i as u8)))
        .collect();
    let col_width = entries.iter().map(String::len).max().unwrap_or(0);

    println!("  widths = {{");
    for row in entries.chunks(8) {
        let mut line = String::from("   ");
        for entry in row {
            line.push_str(&format!(" {entry:col_width$}"));
        }
        println!("{}", line.trim_end());
    }
    println!("  }}");
}

/// Declares a `static` font constant from an image file, resolved at compile time.
///
/// Produces a [`PrintableFont`] (95 ASCII glyphs) or [`FullFont`] (256 Latin-1 glyphs)
/// depending on the sheet grid.
///
/// ```ignore
/// include_agb_font!(FONT, "font.png");
/// include_agb_font!(pub FONT, "font.png", full);
/// include_agb_font!(pub(crate) FONT, "font.png", size(16, 6));
/// include_agb_font!(pub FONT, "font.png", monospace);
/// include_agb_font!(pub FONT, "font.png", full, monospace = 8);
/// include_agb_font!(pub FONT, "font.png", widths = { 'A' = 5, ' ' = 3, 65 = 4 });
/// include_agb_font!(pub FONT, "font.png", monospace = 8, widths = { 'A' = 5 });
/// include_agb_font!(pub FONT, "font.png", bold);
/// include_agb_font!(pub FONT, "font.png", bold = 2);
/// include_agb_font!(pub FONT, "font.png", recolor = { 15 = 14, 14 = 15 });
/// include_agb_font!(pub FONT, "font.png", debug);
/// ```
///
/// The path is relative to `CARGO_MANIFEST_DIR` of the crate calling the macro.
/// When `monospace` and `widths` are both given, `widths` overrides win.
///
/// # Cell size
///
/// Cell dimensions are not passed in; they are the image dimensions divided by the sheet
/// grid, which is 16 columns wide. `full` makes it 16 rows (256 cells, one per Latin-1 code
/// point); with neither argument it is 6 rows (96 cells, 95 of them used by the ASCII
/// glyphs). `size(cols, rows)` gives the grid explicitly for a sheet laid out any other way
///, `size(16, 6)` and `size(16, 16)` are the two defaults spelled out.
///
/// The image must divide exactly into the grid; a sheet with slack pixels around the cells
/// is a compile error rather than a silently misaligned font.
///
/// `debug` prints the computed advance widths at compile time as a paste-ready
/// `widths = { … }` block. The output only appears when the calling crate is actually
/// recompiled, so touch the source or `cargo clean -p <crate>` if it doesn't show.
///
/// # Drawing glyphs in the cell
///
/// Leading blank columns are used to separate letters; trailing blank columns are ignored
/// except in determining the glyph's width; vertical positioning within the cell is used
/// as is.
///
/// Pixels with alpha below 128 **or luminance below 16** convert to palette index 0, which
/// the GBA treats as transparent, glyphs drawn in near-black disappear. A sheet whose every
/// glyph converts to empty is rejected with a compile error.
///
/// A glyph's advance width is the rightmost inked column plus one, and the renderer advances
/// by exactly that, so a glyph drawn against the left edge of its cell sits flush against the
/// next one, leave column 0 empty for a 1px gap. Padding on the right cannot add spacing, it
/// only makes the glyph narrower. Vertically, every row of the cell is kept as drawn with no
/// trimming or baseline detection, which is how ascenders and descenders are expressed.
///
/// # Bold
///
/// `bold` thickens the glyphs at pack time; bare it means strength 1, an integer sets the
/// strength (`= 0` is off, same as omitting it). `bold = N` smears each pixel over its `N`
/// left neighbours (taking the brightest).
///
/// **Advance widths stay roman**: they are measured before styling, so the thickened ink
/// overhangs into the following letter like natural kerning, and text metrics match the
/// unstyled sheet exactly. The maximum overhang, `bold` pixels, is stored in the font; the
/// renderers read it back to widen clear rectangles and sprite allocations so the extra
/// ink is never cut off or left behind. Measuring APIs are not widened, so with
/// `Center`/`Right` alignment or wrapping the last glyph on a line may ink up to that many
/// pixels past the measured edge.
///
/// The packed cell is automatically widened by `bold` pixels so the smeared ink always
/// fits, no matter how the sheet is drawn; sheets need no right padding of their own.
/// The widened cell must still fit the format's 255px limit (a compile error otherwise).
///
/// # Recolor
///
/// `recolor = { from = to, … }` remaps the sheet's grey bands at pack time. A band is a
/// palette index 0–15, i.e. the luminance range `from*16 ..= from*16 + 15` (`luma >> 4`).
/// The map is applied simultaneously, so `{ 15 = 14, 14 = 15 }` swaps two bands rather
/// than chaining; unmapped bands pass through. Band 0 is the transparent background and
/// cannot be recolored (a compile error); remapping *to* 0 makes those pixels transparent
/// without changing advance widths, which are always scanned from the sheet as drawn.
/// Recolor runs after `bold`.
///
/// [`PrintableFont`]: ::gba_agb_font_eb::printable::PrintableFont
/// [`FullFont`]: ::gba_agb_font_eb::full::FullFont
#[proc_macro]
pub fn include_agb_font(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as Args);

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| String::from("."));
    let full_path = std::path::Path::new(&manifest_dir).join(args.path.value());
    let full_path_buf = full_path.to_path_buf();

    let (cols, rows) = args.grid();
    let img = gba_agb_font_creation_internals::parsing::open_image(&full_path_buf);
    let bytes = gba_agb_font_creation_internals::create::create_bytes(
        cols,
        rows,
        &img,
        args.monospace,
        &args.width_overrides,
        args.bold,
        &args.recolor,
    );

    let mode_byte = bytes[0];

    if args.debug {
        let glyph_count = if mode_byte == 0 {
            gba_agb_font_creation_internals::GLYPH_COUNT_SMALL
        } else {
            gba_agb_font_creation_internals::GLYPH_COUNT_FULL
        };
        print_debug(&args, &full_path, &bytes, glyph_count);
    }

    let len = bytes.len();
    // A single byte-string token; one literal per byte makes rustc chew through
    // tens of thousands of tokens per font.
    let byte_lit = proc_macro2::Literal::byte_string(&bytes);
    let path_str = full_path.to_string_lossy().into_owned();

    let vis = &args.vis;
    let name = &args.name;
    let font_crate = font_crate_path();

    let static_item = if mode_byte == 0 {
        quote! {
            #vis static #name: #font_crate::printable::PrintableFont = {
                const _: &[u8] = ::core::include_bytes!(#path_str);
                #[repr(C, align(4))]
                struct AlignedFont([u8; #len]);
                static FONT_BYTES: AlignedFont = AlignedFont(*#byte_lit);
                // SAFETY: `AlignedFont` is `#[repr(C, align(4))]`, so the bytes are 4-byte aligned.
                unsafe { #font_crate::printable::PrintableFont::from_static_bytes(&FONT_BYTES.0) }
            };
        }
    } else {
        quote! {
            #vis static #name: #font_crate::full::FullFont = {
                const _: &[u8] = ::core::include_bytes!(#path_str);
                #[repr(C, align(4))]
                struct AlignedFont([u8; #len]);
                static FONT_BYTES: AlignedFont = AlignedFont(*#byte_lit);
                // SAFETY: `AlignedFont` is `#[repr(C, align(4))]`, so the bytes are 4-byte aligned.
                unsafe { #font_crate::full::FullFont::from_static_bytes(&FONT_BYTES.0) }
            };
        }
    };

    static_item.into()
}
