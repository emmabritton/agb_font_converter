# AGB Font Converter

Converts Aseprite/PNG font sheets to the [AGB Font format](https://github.com/emmabritton/gba_agb_font_renderer).

## Usage

```
font_converter [OPTIONS] --width <PX> --height <PX> <FILE>
font_converter update <BIN> <IMAGE>
font_converter edit <FILE> [--get <CHAR>] [--set <CHAR=WIDTH>]
```

---

## Convert (default mode)

Reads a font sheet image and writes a `.bin` file.

```sh
font_converter -w 8 -h 8 font.aseprite
font_converter -w 8 -h 8 font.png -o out.bin
font_converter -w 8 -h 8 font.png -m        # monospace: use widest glyph
font_converter -w 8 -h 8 font.png -m 8      # monospace: force width to 8px
```

| Flag                       | Description                                                                                                |
|----------------------------|------------------------------------------------------------------------------------------------------------|
| `-w`, `--width <PX>`       | Cell width in pixels                                                                                       |
| `-h`, `--height <PX>`      | Cell height in pixels                                                                                      |
| `-o`, `--output <FILE>`    | Output path (default: same directory as input, `.bin` extension)                                           |
| `-m`, `--monospace [<PX>]` | Force all glyph widths equal. Without a value uses the widest glyph; with a value forces that exact width. |

The output path defaults to the input filename with a `.bin` extension, written to the same directory.

---

## Update subcommand

Replaces the pixel data in an existing `.bin` file from a new image, preserving all stored character widths.

```sh
font_converter update font.bin new_sheet.png
font_converter update font.bin new_sheet.aseprite
```

The new image must produce the same font mode (full/small) as the existing binary.

---

## Edit subcommand

Inspects or patches character widths in an existing `.bin` file without touching pixel data.

```sh
font_converter edit font.bin                     # list all character widths
font_converter edit font.bin -g A                # get width for 'A'
font_converter edit font.bin --get space         # get width for ' ', long form
font_converter edit font.bin -s A=6              # set width for 'A' to 6
font_converter edit font.bin --set A=6           # same, long form
font_converter edit font.bin -g A -g B -s 32=4   # multiple ops in one call
```

Characters can be specified as:
- A single ASCII character: `A`, `!`
- A decimal code point: `65`, `32`
- A hex code point: `0x41`, `0x20`
- The word `space`

---

## Font modes

The mode is auto-detected from image dimensions (`(image_height / cell_height) * 16`):

### Full — 256 glyphs

Image is a 16×16 grid of cells, one glyph per Latin-1 code point (0–255).

See `examples/full_font.aseprite`.

### Small — 95 glyphs

Image is a 16×6 grid of cells covering ASCII 32–126 (space through `~`), in code-point order.

See `examples/alphanum_font.aseprite`.

| Cell indices | Characters |
|--------------|------------|
| 0            | space (32) |
| 1–15         | `!` – `/`  |
| 16–25        | `0`–`9`    |
| 26–41        | `:` – `Z`  |
| 42–68        | `[` – `z`  |
| 69–94        | `{` – `~`  |

---

## Image format

PNG and Aseprite files are supported (frame 0 is used for Aseprite). Images can use up to 15 shades of gray. Alpha < 50% is treated as transparent (palette index 0).

| Grey    | Palette index   |
|---------|-----------------|
| 0–15    | 0 (transparent) |
| 16–31   | 1               |
| 32–47   | 2               |
| 48–63   | 3               |
| 64–79   | 4               |
| 80–95   | 5               |
| 96–111  | 6               |
| 112–127 | 7               |
| 128–143 | 8               |
| 144–159 | 9               |
| 160–175 | 10              |
| 176–191 | 11              |
| 192–207 | 12              |
| 208–223 | 13              |
| 224–239 | 14              |
| 240–255 | 15              |

---

## Glyph widths

Each glyph's advance width is computed automatically: the rightmost non-transparent pixel column determines the width. Empty glyphs default to width 1. Use `-m` to force all glyphs to the same width, or `edit --set` to adjust individual glyphs after conversion.
