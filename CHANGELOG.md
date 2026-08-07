# Unreleased

- Add a `bold` argument to `include_agb_font!` (bare = strength 1, or `= N`), a pack-time
  transform smearing each pixel over its `N` left neighbours. Advance widths stay roman, so
  the thickened ink overhangs the next letter like natural kerning. The packed cell is
  widened by `bold` pixels at pack time, so glyphs drawn flush against the right edge of
  their cell thicken losslessly with no sheet padding. (An `italic` shear was tried and
  removed: splitting rows into thirds by cell height slants glyphs that don't fill the
  cell wrongly.)
- Store the max styled overhang (`bold`) in the previously unused header padding
  byte (offset 259 full / 98 small; old fonts read as 0) and expose it as
  `AgbFont::right_overhang()`. The renderers widen clear rects and sprite allocations by it
  so overhanging ink is neither cut off nor left behind.
- Add `recolor = { from = to, ... }` argument to `include_agb_font!`: a pack-time remap of
  the sheet's grey bands (band = `luma >> 4`), applied simultaneously so swaps don't chain.
  Band 0 (the transparent background) cannot be recoloured; remapping to 0 makes pixels
  transparent without changing advances.

**Breaking**

- `create_bytes` takes new `bold` and `recolor` parameters.
- `AgbFont` has a new required method `right_overhang()`; external implementors must add it.

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