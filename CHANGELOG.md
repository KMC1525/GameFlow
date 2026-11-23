# Changelog

## 0.2.0

### Added
- **Style presets**: reusable visual styles (8-color palette, art-style description, image-prompt hints).
  - Import and export portable `.gfstyle.json` files (Settings → *Estilos de arte*, drag & drop, or `gameflow preset import|export`).
  - Pick a style for the next generation (`Style:` selector in the prompt bar, `gameflow generate --preset`) or apply it to an existing project (palette, `style.css` and generated art are redone; your own files are kept).
  - Save the style of any project to your library.
  - Five built-in presets: Neon Noir, Cozy Pastel, Handheld Green, Storybook Watercolor, Ember Dusk.
  - Strict validation of imported files (format marker, version, size, `#rrggbb` colors); ids are always assigned by the library.
  - Negative prompts reach stable-diffusion.cpp (`-n`) and custom commands (`{negative}`).
- English documentation: `README.md`, `docs/MODELS.md`, `docs/STYLE_PRESETS.md` (Portuguese versions: `README.pt-BR.md`, `docs/MODELOS.md`).
- `GAMEFLOW_HOME` now also holds the `presets/` folder.

### Changed
- `game.json` has a new optional `style` block (older projects keep working).

## 0.1.0

Initial release: Rust backend, non-linear agent pipeline, local model runtimes, Hugging Face integration, 8 game genres, IDE interface, terminal installers and self-update.
