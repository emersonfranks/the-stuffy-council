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
| Aspect      | Square 1:1                                                   | `.sc-portrait` is `aspect-ratio: 1` with `object-fit: cover` |
| Master size | 1024×1024, delivered at 512×512                             | Covers 2× of every use (mini 56 / spot 72 / md 96) with headroom |
| Background  | Transparent                                                  | The portrait frame supplies its own faction tint; a cutout floats on it |
| Framing     | Subject centered, ~10% padding all sides                    | `cover` crops to square; edge detail gets clipped |
| Format      | Transparent PNG-24                                           | Matches the `image` fields; sizes are small so PNG is fine |
| Palette     | Stay in each character's `cast/*.toml` `color_palette`; read well on its frame tint | Cohesion with the site |

Frame tint per character comes from `Character::accent()` in
[../src/cast.rs](../src/cast.rs): Woofy → mint, Bar Bar → lavender,
Ruff Ruff → peach.

## Operational rules

- **PNGs are binary.** Before committing art, add a `.gitattributes` entry
  (`*.png binary`) so line-ending normalization cannot corrupt them
  (tracked as backlog #10). Do NOT commit art before that guard exists.
- **No weapons in art, ever.** Woofy's lore mentions a "(mostly ceremonial)
  AK collection"; this is a kids' site and the depiction has already tripped
  a safety review. Convey his authority with a crown / sash / medal, never a
  firearm. Baked into the global negatives below.
- **Templates do not render `image` yet.** The portraits currently show the
  silhouette placeholder (`.sc-portrait__ph`). Wiring `character.image` into
  the frame with a silhouette fallback is a separate change; dropping files
  into `static/stuffies/` does nothing until that lands.
- **Humans get no portrait.** Human characters (`kind = "human"`) are real
  people; they keep the silhouette placeholder and carry no `image` field.

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

```
CHARACTER — Ruff Ruff, a well-loved shaggy plush dog with a soft curly-pile
coat, gently worn and aged. Oatmeal / cream body with a faint gray-green
tinge from years of love (well-loved-gray). Long floppy ears hanging beside
the head, smooth satin cream inner-ear lining with a faint star print. Sweet
CLOSED stitched eyes (content, a touch sleepy); worn suede oval brown nose;
rounded muzzle. Smooth satin paw-pad undersides (cream, faint stars). Chunky
huggable lovey proportions. Signature prop: a little wooden spoon he wields
like a doctor's instrument. Bearing: self-important doctor, devoted, a bit
random — gentle, never sad.
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
