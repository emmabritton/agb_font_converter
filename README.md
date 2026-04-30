# AGB Font Converter

Converts Aseprite/PNG to my [AGB Font format](https://github.com/emmabritton/gba_agb_font_renderer)

## Usage

```
./converter -w 8 -h 8 image.aseprite 
```

The mode is detected automatically from the image dimensions:

### 256-char mode

Image contains all 256 ASCII glyphs arranged as a 16x16 grid of cells (16 columns, 16 rows), ordered by code point.

See `examples/full_font.aseprite`

### 95-char mode

Image contains all 95 visible ASCII glyphs (codes 32–126) arranged as a 16x6 grid (16 columns, 6 rows), in sequential code-point order starting from space:

See `examples/alphanum_font.aseprite`

| Cell indices | Characters     |
|--------------|----------------|
| 0            | space (32)     |
| 1–15         | `!` – `/`      |
| 16–25        | `0`–`9`        |
| 26–41        | `:` – `Z`      |
| 42–68        | `[` – `z`      |
| 69–94        | `{` – `~`      |

## Image format

Image can use up to 15 grays. Alpha < 50% is treated as transparent (palette index 0).

Color to palette index mapping:

| Luma    | Palette index          |
|---------|------------------------|
| 0–15    | 0 (transparent in GBA) |
| 16–31   | 1                      |
| 32–47   | 2                      |
| 48–63   | 3                      |
| 64–79   | 4                      |
| 80–95   | 5                      |
| 96–111  | 6                      |
| 112–127 | 7                      |
| 128–143 | 8                      |
| 144–159 | 9                      |
| 160–175 | 10                     |
| 176–191 | 11                     |
| 192–207 | 12                     |
| 208–223 | 13                     |
| 224–239 | 14                     |
| 240–255 | 15                     |
