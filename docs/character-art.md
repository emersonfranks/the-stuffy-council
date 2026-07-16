# Character art — the Stuffy Council art bible

Single source for generating on-brand character assets. Route B
(stylized illustration from each plush's reference photo). Consistency
comes from reusing the fixed blocks below **verbatim** and changing only
the one-line `Shot:` per request.

Reference photos of the real plushes are NOT committed (they live with
the family). Attach the relevant photo to every generation request — text
alone will not hold likeness.

## Asset spec (what a file must satisfy to drop in)

| Property    | Value                                                        | Why |
| ----------- | ------------------------------------------------------------ | --- |
| Files       | `woofy.png`, `bar-bar.png`, `ruff-ruff.png` in `static/stuffies/` | Must equal the `image =` value in each `cast/*.toml` (stable ids) |
| Aspect      | Square 1:1                                                   | `.sc-portrait` is `aspect-ratio: 1` with `object-fit: contain` |
| Master size | 1024×1024, delivered at 512×512                             | Covers 2× of every use (mini 56 / spot 72 / md 96) with headroom |
| Background  | Transparent                                                  | The portrait frame supplies its own faction tint; a cutout floats on it |
| Framing     | Subject centered, ~10% transparent padding all sides        | `contain` preserves the full cutout while the frame supplies faction tint |
| Format      | Transparent PNG-24                                           | Matches the `image` fields; sizes are small so PNG is fine |
| Palette     | Stay in each character's `cast/*.toml` `color_palette`; read well on its frame tint | Cohesion with the site |

Frame tint per character comes from `Character::accent()` in
[../src/cast.rs](../src/cast.rs): Woofy → mint, Bar Bar → lavender,
Ruff Ruff → peach.

## Operational rules

- **PNGs are binary.** The `*.png binary` rule in
  [../.gitattributes](../.gitattributes) prevents line-ending normalization;
  keep it when adding or moving art.
- **No weapons in art, ever.** If a scene earns Woofy's rare threat cue,
  typeset only a small `chk-chk` sound overlay; never draw, name, or explain a
  weapon. Convey his authority with posture, his crew, a crown / sash / medal,
  or another scene-appropriate signal. Baked into the global negatives below.
- **Templates render only complete canonical assets.** A character gets a real
  portrait when `image` equals `<stable-id>.png` and that file exists under
  `static/stuffies/`; missing or noncanonical declarations use the silhouette
  fallback (`.sc-portrait__ph`).
- **Humans get no portrait.** Human characters (`kind = "human"`) are real
  people; they keep the silhouette placeholder and carry no `image` field.

## Dialogue in story-scene art

Portrait assets contain no text. Story-scene illustrations apply this canon
as a deterministic layout overlay after image generation; do not ask the
image model to draw letters. This is a visual convention only: prose stories
use free indirect discourse and never narrate that a thought bubble appeared
or "read" something.

- Ruff Ruff is the only stuffy with literal voiced English dialogue. His words
  use a speech bubble.
- Every other stuffy makes its native sounds aloud. When a sound cue improves
  character or timing, put it near the stuffy (for example, Woofy's hum or Bar
  Bar's tonal name). Use Dad's interpreted-English thought bubble only when it
  earns scarce panel space; do not annotate every action or exchange. The
  thought bubble points to the stuffy and does not place Dad in the scene.
- When Dad is physically present, he may translate the sound in his own speech
  bubble instead of using the stuffy's thought bubble.
- Scene-generation output must reserve bubble space and return typed overlay
  records separately. Each record contains `kind` (`native_sound`, `thought`,
  or `speech`), the attributed character id, text, an anchor point, and a
  reserved placement rectangle. The application typesets these records so
  wording stays legible, the correct bubble form survives generation, and
  character identity is not coupled to model-rendered text.

## Style key (paste first, unchanged, every request)

```
STYLE — Stuffy Council house style:
Die-cut sticker / soft-vinyl-toy illustration of a PLUSH TOY. Thick, clean,
unified rounded outline; soft cel shading with gentle top-left studio light;
smooth matte plush surfaces (illustrated, not photoreal fur); wholesome,
storybook-cute, collectible-card energy. Cute but not pink-heavy; a little
artistic and sporty. Warm, friendly, kid-safe. Flat-ish with soft gradients,
no photorealism.
```

## Global negatives (paste last, unchanged)

```
NEGATIVE — no text, no logos, no watermark, no signature, no border, no
frame, no drop shadow baked onto an opaque background, no realistic photo
texture, no human hands, no extra characters, no clutter, no weapons, no
guns, not scary, not gory.
```

## Prompt formula (only the `Shot:` line changes)

```
<STYLE KEY>
<CHARACTER IDENTITY LOCK — the character's block below, verbatim>
Shot: <view> · <pose / action> · <expression / mood>
Output: square 1:1, subject centered, ~10% padding, transparent background
<GLOBAL NEGATIVES>
```

Consistency rules:

1. Attach the character's reference photo as an image/style reference on
   every request.
2. Keep the style key and the identity lock byte-for-byte identical across
   requests. Change only the `Shot:` line.
3. First asset per character is a turnaround sheet (`<id>--sheet.png`: front,
   3/4, side, back). Approve it, then generate individual poses from it.
4. Pin the seed once a look is approved; vary only the shot. Same model,
   same settings.
5. The site portrait (`<id>.png`) is the canonical default pose; everything
   else is a named variant.

## Character identity locks

### Bar Bar — `bar-bar.png` · frame tint lavender

```
CHARACTER — Bar Bar (formal name Granola Bar), a small round REVERSIBLE
plush lion: body shaped like a fuzzy ball wrapped in a full two-tone orange
mane (marigold outer, deeper flame streaks), small rounded ears peeking over
the mane, short stubby paws and tiny feet at the base, three whisker-dots per
cheek. TWO canonical faces (he is a mood-flip plush):
  • HAPPY (default / site): round black dot eyes, small orange triangle nose,
    closed cat-smile mouth, content — tough-but-cute.
  • ANGRY (variant only): one raised sharp brow, one squint, wide roaring
    mouth with a small fang — comic-angry, never scary.
Colors: marigold, amber, flame. Personality: small-but-scrappy, confident,
secretly musical.
```

### Woofy — `woofy.png` · frame tint mint

```
CHARACTER — Woofy, a chunky, huggable plush husky-wolf in a cute sitting
build. Dove-gray cap / back / outer-ears / tail; cloud-white muzzle, chest,
belly, front legs and paws (classic husky mask). Upright triangle ears with
pink insides; small black oval nose; round solid-black friendly dot eyes;
pink paw pads; big fluffy curled gray-and-white tail. Bearing: grandiose,
imperious "President of the Universe." Regal prop options (on brand): tiny
gold crown, presidential sash, or little medal — NEVER weapons.
```

### Ruff Ruff — `ruff-ruff.png` · frame tint peach

The Well Loved design was selected during family review on 2026-07-16 and is
the canonical 512×512 transparent portrait at `static/stuffies/ruff-ruff.png`.

```
CHARACTER — Ruff Ruff, a well-loved shaggy plush dog with a soft curly-pile
coat, gently worn and aged. Oatmeal / cream body with a faint gray-green
tinge from years of love (well-loved-gray). Long floppy ears hanging beside
the head, smooth satin cream inner-ear lining with a faint star print.
Asymmetric stitched face: one notched round black eye on viewer-left, one
small X-shaped stitched eye on viewer-right; worn suede oval brown nose;
rounded muzzle. Smooth satin paw-pad undersides (cream, faint stars). Chunky
huggable lovey proportions. Occasional prop: a little wooden spoon he wields
like a doctor's instrument only in a doctor-play or absurd medical-tangent
scene. Bearing: self-important doctor, devoted, a bit random — gentle, never
sad.
```

## Variant naming

The canonical site portrait keeps the bare id (`woofy.png`). Extra views get
a `--suffix` so ids stay predictable:

- `woofy--sheet.png` (turnaround reference)
- `woofy--full-body.png`, `woofy--wave.png`
- `bar-bar--angry.png`, `bar-bar--singing.png`
- `ruff-ruff--doctor.png`

## Worked example — Woofy, full-body hero wave

```
<STYLE KEY>
CHARACTER — Woofy, a chunky, huggable plush husky-wolf … (full block above)
Shot: front 3/4 full-body · standing tall, chest out, one paw raised in a
regal wave, wearing a small gold presidential sash · proud, chin-up smile
Output: square 1:1, subject centered, ~10% padding, transparent background
<GLOBAL NEGATIVES>
```
