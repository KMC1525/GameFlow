# Changelog

## 0.4.0

### Changed
- **New look, aligned with OpenFlow**: soft graphite grays (no pure black) with a neutral light accent instead of blue, flat panels separated by thin lines, rounded cards and modals, pill-shaped buttons, a rounded floating prompt box, softer chat bubbles, uppercase letter-spaced brand and OpenFlow-style scrollbars, focus rings and toasts.
- The five themes keep their ids (your saved choice still applies) but use the new palettes; *Papel* is now shown as **Claro** and follows OpenFlow's light theme. The logo and favicon follow the accent color.
- The Settings → Appearance text and the README describe the new look.

## 0.3.0

### Added
- **Text adventures** — a new genre (*Aventura de Texto* / `text_adventure`): classic interactive fiction that runs on the web.
  - DOM-based engine with a Portuguese/English parser (look, go, take, drop, examine, read, use, open, talk, give, inventory, hint, save, load, restart, shortcuts), clickable items/people/exits, command history, autosave with *Continue*, room illustrations, sound, adjustable font size.
  - Procedural generator (9 themes, 7–11 rooms by difficulty) with a guaranteed puzzle chain and a built-in walkthrough; `docs/walkthrough.md` documents the solution and map for the author.
  - World validator (references, locks, reachability fixpoint) used by the Tester agent, so hand-edited and AI-edited worlds are checked on every run.
  - With a text model, the Writer polishes names/descriptions/dialogue/intro/ending while the structure, item/people names, the key trade and the clue are protected.
  - `js/custom.js` hooks: `GF.on('command' | 'enter' | 'event' | 'start')`.
  - Tests replay the walkthrough in the real engine (Node) for 36 generated worlds (pt/en, 3 difficulties), and the browser smoke test beats an adventure by typing its walkthrough.

### Changed
- The Architect keeps an existing world instead of regenerating it unless a new map is requested.
- The Tester checks for the terminal element (instead of a `<canvas>`) on text adventures.

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
