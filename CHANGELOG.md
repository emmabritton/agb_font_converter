# Unreleased

- Add `bold [= N]` and `recolor { from = to*}` to `include_agb_font!()`

# v0.25.1

- Re-export `include_agb_font!` and the `gba_agb_font_eb` crate, so depending on the renderer
  alone is enough to declare and render a font.
- Add `letter_spacing` and `line_spacing` to `TextRenderer` and `SpriteTextRenderer`

**Breaking**

- Some methods, such as `size_of`

# v0.25.0

- No changes, but for simplicity as the renderer is tied to AGBs version, these libraries will match AGBs version number

# v0.24.0

- No changes, but for simplicity as the renderer is tied to AGBs version, these libraries will match AGBs version number

# v0.6.0

- Add macro to create font from image file directly

# v0.4.0

- Add editing (get and set individual character widths)
- Add updating (replace font images without changing widths/sizes)

# v0.3.0

- Add support for forcing monospace

# v0.2.0

- Add support for aseprite
- Change file format so font can be all 256 chars or just the printable 95

# v0.1.0

- Initial version