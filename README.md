# AGB Font Converter

Converts Aseprite/PNG to my AGB Font format

## Usage

```
./converter -w 8 -h 8 image.aseprite 
```

Image should be 256 8x8 cells (16x16 cells)

Image can use up to 15 grays

Colour format: 

| RGB     | Palette index          |
|---------|------------------------|
| 0-15    | 0 (transparent in GBA) |
| 16-31   | 1                      |
| 32-47   | 2                      |
| 48-63   | 3                      |
| 64-79   | 4                      |
| 80-95   | 5                      |
| 96-111  | 6                      |
| 112-127 | 7                      |
| 128-143 | 8                      |
| 144-159 | 9                      |
| 160-175 | 10                     |
| 176-191 | 11                     |
| 192-207 | 12                     |
| 208-223 | 13                     |
| 224-239 | 14                     |
| 240-255 | 15                     |

Follows ASCII, so

| Cells  | Chars |
|--------|-------|
| 48-57  | 0-9   |
| 65-90  | A-Z   |
| 97-122 | a-z   |