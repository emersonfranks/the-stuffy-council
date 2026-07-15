# Characters

Each character lives in its own TOML file in this directory. The filename
(minus `.toml`) is the character's stable `id`.

Two kinds of characters live here:

- **`kind = "stuffy"`** (default) — actual stuffed animals. Rotate through
  the daily story cast.
- **`kind = "human"`** — Lennon and Dad. Always present as narrative
  frame in every story regardless of the daily cast.

## Schema

```toml
# Identity (all required)
name = "Woofy"
species = "gray-and-white plush wolf (Avocatt)"
title = "President of the Universe (self-declared)"
role  = "leads a crew of roughly six Avocatt stuffies; considers himself the Supreme Leader of the Universe"
speech_style = "Makes pseudo-humming sounds aloud; Dad interprets them as thought-bubble dialogue..."

# Kind (optional; default "stuffy")
kind = "stuffy"        # or "human"

# Faction / council (all optional)
faction      = "Avocatts"       # freeform label, e.g. "Avocatts", "TeeTurtles", "The OG"
faction_role = "leader"         # "leader" or "member"
on_council   = true             # default true; Ruff Ruff is famously false

# Visuals (optional)
image = "woofy.png"                       # file under /static/stuffies/ — art spec in ../docs/character-art.md
color_palette = ["dove-gray", "cloud-white", "pink-ear"]

# Personality (all optional except speech_style)
traits      = ["grandiose", "twitchy about respect"]
fears       = ["being addressed by anything less than his full title"]
loves       = ["his AK collection", "being addressed as Supreme Leader"]
catchphrase = "*imperious hum*"

# Relationships to other characters by id (optional)
[[relationships]]
with = "ruff-ruff"
bond = "sworn rival over who's the real leader; escalates fast, forgives afterward"

# Freeform notes the story generator can weave in (optional)
lore = """
Woofy leads about six Avocatt stuffies. Each of them refers to him as
Supreme Leader of the Universe...
"""
```

## Editing

- **Filenames are stable IDs.** Rename with care; historical `stories.cast_json`
  rows will contain the old id.
- **Keep `traits` and `speech_style` short and *distinctive.*** Long, generic
  descriptions bleed together in the model output.
- **Keep `lore` under ~6 short sentences.** It's fed straight into the prompt.
- **Every `relationships[].with` MUST reference another character's id.**
  The loader fails hard on dangling references.
- **Character art (the `image` file) is generated to a fixed spec.** Prompts,
  the house style, and per-character identity locks live in
  [../docs/character-art.md](../docs/character-art.md).

The Rust side validates these files on startup — malformed TOML, missing
required fields (`name`, `species`, `title`, `role`, `speech_style`), or
dangling relationships will fail the boot loudly.
