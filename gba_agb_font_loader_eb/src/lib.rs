use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    LitChar, LitInt, LitStr, Token, braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

struct Args {
    vis: syn::Visibility,
    name: syn::Ident,
    path: LitStr,
    cell_width: u8,
    cell_height: u8,
    monospace: Option<Option<u8>>,
    width_overrides: Vec<(u8, u8)>,
}

fn parse_int_lit(v: &LitInt) -> syn::Result<u8> {
    let s = v.to_string();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16)
            .map_err(|e| syn::Error::new(v.span(), format!("invalid hex literal: {e}")))
    } else {
        v.base10_parse::<u8>()
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis: syn::Visibility = input.parse()?;
        let name: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let path: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let cell_width = parse_int_lit(&input.parse::<LitInt>()?)?;
        input.parse::<Token![,]>()?;
        let cell_height = parse_int_lit(&input.parse::<LitInt>()?)?;

        let mut monospace = None;
        let mut width_overrides = Vec::new();

        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let ident: syn::Ident = input.parse()?;
            match ident.to_string().as_str() {
                "monospace" => {
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let v: LitInt = input.parse()?;
                        monospace = Some(Some(parse_int_lit(&v)?));
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
                            parse_int_lit(&v)?
                        };
                        content.parse::<Token![=]>()?;
                        let w: LitInt = content.parse()?;
                        width_overrides.push((cp, parse_int_lit(&w)?));
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("unknown argument `{other}`; expected `monospace` or `widths`"),
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
            cell_width,
            cell_height,
            monospace,
            width_overrides,
        })
    }
}

/// Declares a `static` font constant from an image file, resolved at compile time.
///
/// Produces a [`PrintableFont`] (95 ASCII glyphs) or [`FullFont`] (256 Latin-1 glyphs)
/// depending on the image dimensions.
///
/// ```ignore
/// include_agb_font!(FONT, "font.png", 8, 8);
/// include_agb_font!(pub FONT, "font.png", 8, 8);
/// include_agb_font!(pub(crate) FONT, "font.png", 8, 8, monospace);
/// include_agb_font!(pub FONT, "font.png", 8, 8, monospace = 8);
/// include_agb_font!(pub FONT, "font.png", 8, 8, widths = { 'A' = 5, ' ' = 3, 65 = 4 });
/// include_agb_font!(pub FONT, "font.png", 8, 8, monospace = 8, widths = { 'A' = 5 });
/// ```
///
/// The path is relative to `CARGO_MANIFEST_DIR` of the crate calling the macro.
/// When `monospace` and `widths` are both given, `widths` overrides win.
///
/// [`PrintableFont`]: ::gba_agb_font_eb::printable::PrintableFont
/// [`FullFont`]: ::gba_agb_font_eb::full::FullFont
#[proc_macro]
pub fn include_agb_font(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as Args);

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| String::from("."));
    let full_path = std::path::Path::new(&manifest_dir).join(args.path.value());
    let full_path_buf = full_path.to_path_buf();

    let img = gba_agb_font_creation_internals::parsing::open_image(&full_path_buf);
    let bytes = gba_agb_font_creation_internals::create::create_bytes(
        args.cell_width,
        args.cell_height,
        &img,
        args.monospace,
        &args.width_overrides,
    );

    let mode_byte = bytes[0];
    let len = bytes.len();
    let byte_lits = bytes.iter().copied().map(|b| quote! { #b });
    let path_str = full_path.to_string_lossy().into_owned();

    let vis = &args.vis;
    let name = &args.name;

    let static_item = if mode_byte == 0 {
        quote! {
            #vis static #name: ::gba_agb_font_eb::printable::PrintableFont = {
                const _: &[u8] = ::core::include_bytes!(#path_str);
                #[repr(C, align(4))]
                struct AlignedFont([u8; #len]);
                static FONT_BYTES: AlignedFont = AlignedFont([#(#byte_lits),*]);
                ::gba_agb_font_eb::printable::PrintableFont::from_static_bytes(&FONT_BYTES.0)
            };
        }
    } else {
        quote! {
            #vis static #name: ::gba_agb_font_eb::full::FullFont = {
                const _: &[u8] = ::core::include_bytes!(#path_str);
                #[repr(C, align(4))]
                struct AlignedFont([u8; #len]);
                static FONT_BYTES: AlignedFont = AlignedFont([#(#byte_lits),*]);
                ::gba_agb_font_eb::full::FullFont::from_static_bytes(&FONT_BYTES.0)
            };
        }
    };

    static_item.into()
}
