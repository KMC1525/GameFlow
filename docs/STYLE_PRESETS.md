# Style presets

A style preset is a small JSON file (`*.gfstyle.json`) that describes a reusable visual style. GameFlow can export presets from its library, import files made by other people, apply a preset to a new or existing project, and capture the style of a project as a new preset.

## File format (version 1)

```json
{
  "format": "gameflow-style-preset",
  "version": 1,
  "name": "Neon Noir",
  "description": "Dark city nights with glowing magenta and cyan.",
  "author": "Your name",
  "art_style": "pixel art, neon cyberpunk, dark background",
  "prompt_suffix": "high contrast, glowing edges, limited palette",
  "negative_prompt": "blurry, photo, text, watermark",
  "palette": {
    "bg": "#0d0820",
    "bg2": "#241447",
    "ground": "#2f1b5c",
    "primary": "#00e5ff",
    "secondary": "#ff2d95",
    "accent": "#f5ff3b",
    "danger": "#ff3d3d",
    "text": "#f2e9ff"
  }
}
```

| Field | Required | Meaning |
|---|---|---|
| `format` | yes | Must be exactly `"gameflow-style-preset"`. Files without it are rejected. |
| `version` | yes | `1`. Files from a newer version are rejected with a hint to run `gameflow update`. |
| `name` | yes | Shown in the library (max 60 characters). The library derives the preset `id` from it. |
| `art_style` | yes | Style sentence (max 200). Used in image prompts. If it mentions "pixel", the game draws with crisp pixels; otherwise sprites are smoothed. |
| `palette` | yes | All 8 colors: `bg` (sky/background), `bg2`, `ground`, `primary` (hero/UI), `secondary`, `accent` (coins, highlights), `danger` (enemies), `text`. Written as `#rrggbb` (`#rgb` is accepted and expanded). |
| `description`, `author` | no | Free text (max 300 / 60). |
| `prompt_suffix` | no | Appended to every image prompt (max 300). |
| `negative_prompt` | no | Sent to image models that support it: `-n` for stable-diffusion.cpp, `{negative}` in custom commands. |

Unknown fields are ignored. `id`, `builtin` and `created_at` found in a file are ignored too: the library assigns a fresh, unique id on import (`neon-noir`, `neon-noir-2`, …), so an import can never overwrite another preset or a built-in one. Files larger than 256 KB are rejected.

## What applying a preset does

- **New game**: the Director uses the preset palette and art style instead of the theme defaults or the colors of attached images, and the style is saved into `game.json` (`palette`, `art_style`, and a `style` block with the preset name and prompt hints). The model's suggestions can never override the preset.
- **Existing game** (*Apply to project*): runs in edit mode — `game.json`, `style.css` and `index.html` get the new colors, and every generated image is redone with the new palette and prompt hints. Files you supplied yourself (marked *use in game*) are never overwritten, and your code, maps and story stay as they are.
- **Without an image model** the built-in artist redraws sprites, tiles, backgrounds and portraits in pixel art using the preset palette.

## Using presets

| Where | How |
|---|---|
| Interface | Settings → *Estilos de arte*: import (button or drag & drop), export, apply, delete, *save the style of the current project*. The prompt bar has a `Style:` selector for the next generation. |
| Terminal | `gameflow preset list`, `gameflow preset import look.gfstyle.json`, `gameflow preset export neon-noir --out look.gfstyle.json`, `gameflow preset delete ID`, `gameflow generate "…" --preset neon-noir` |
| API | `GET /api/presets`, `POST /api/presets` (body = file contents), `GET /api/presets/{id}/export`, `DELETE /api/presets/{id}`, `POST /api/projects/{id}/preset-save`, and `"preset": "<id>"` in `POST /api/projects/{id}/generate` |

Presets live in the `presets/` folder next to GameFlow's settings (see `gameflow paths`); each one is a `<id>.gfstyle.json` file you can also copy by hand.
