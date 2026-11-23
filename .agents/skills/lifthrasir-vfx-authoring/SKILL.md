---
name: lifthrasir-vfx-authoring
description: Use when creating, redoing or reviewing a skill/status/map visual effect in Lifthrasir, such as an authored `*.strfx.ron`, an `effects.ron` catalog entry (`visuals`, `skill_units`, `shake`, `sound`), or a complaint that an effect "looks bad", "renders as bars/stripes" or "has no visual". Covers finding the retail reference first, previewing pak textures/sprites, the strfx format, textures that work and fail on flat quads, the fx_capture screenshot loop, and the tests to run.
---

# Authoring VFX in Lifthrasir

Effects are judged by looking at them. Never ship an authored effect you have
not captured and viewed. The loop: **find the reference -> pick assets ->
author -> capture -> view -> tune -> wire -> test.**

Helper scripts live in `scripts/` next to this file. They use only the Python
stdlib plus macOS `sips`, because Pillow and ImageMagick are not installed. Run
them from the repo root.

## 0. Never print raw Korean file names

Tool output containing Korean pak/GRF paths has repeatedly restarted the agent
session. Korean *inside* `effects.ron` lines prints fine, but file listings do
not. Use `scripts/pak_preview.py find`, which escapes names, or pipe through
`LC_ALL=C sed 's/[^ -~]/?/g'`. Plain `zipinfo` mangles the names anyway.

## 1. Find what the retail client draws

Prefer retail art over authoring. In order:

1. **Native STR** (`Str("x.str")`) or a **retail SPR/ACT** (`Sprite("이팩트/name")`,
   a stem under `data/sprite/`; it loops on repeating units).
2. **Authored strfx** (`Str("x.strfx.ron")`), built only from retail textures.
3. `Shader("key")` or `Bespoke("key")`, only when an entry already fits.

Where to look:

- roBrowserLegacy `src/DB/Skills/SkillEffect.js` maps skill to effect id
  (`effectId`, `groundEffectId`, `beforeHitEffectId`...). `SkillUnit.js` maps
  ground units, and `src/DB/Effects/EffectTable.js` turns an id into its file
  (`type: 'STR' | 'SPR' | '3D' | 'FUNC'`). `curl` the raw files from
  `raw.githubusercontent.com/MrAntares/roBrowserLegacy/master/...` and `rg`
  them locally; WebFetch truncates EffectTable.
- Paths there are EUC-KR escapes. Decode them with
  `printf '\xb5\xa5...' | iconv -f EUC-KR -t UTF-8`, but only into a variable
  or file you then escape, per section 0.
- rAthena `doc/effect_list.md` gives EF id names and descriptions.
- `FUNC` entries are hardcoded client code, such as a camera quake
  (`shake: Some((amplitude, duration_secs))` in the catalog), so the art must be
  authored.
- A `'3D'` entry whose `file` is a `유저인터페이스/item/*.bmp` is a thrown item
  icon. Reproduce it as a spinning `Blend` quad of that BMP.

Record the reference (or its absence) in the strfx header comment and
in the catalog comment. Existing files follow this convention.

## 2. Preview candidate assets

```bash
S=.agents/skills/lifthrasir-vfx-authoring/scripts
python3 $S/pak_preview.py find effect/ smoke          # all substrings must match
python3 $S/pak_preview.py show /tmp/sw.png \
  data/texture/effect/glow11.bmp "data/sprite/이팩트/firewall.spr"
```

Then `read` the PNG. `show` takes `.bmp/.tga/.png` (one tile) and `.spr`
(every frame, upright, magenta keyed). Tiles sit on a dark background, which
is how additive black-background textures look in game. To see which
textures existing effects already use:

```bash
grep -ohE 'ro://data/texture/effect/[a-z0-9_./]+' assets/data/effects/*.ron | sort | uniq -c | sort -rn | head -40
```

(In rg, `-h` means `--help`, so `rg -oh` prints the help text. Use
`rg -o --no-filename` or the grep above.)

### Textures on flat quads

| Works as a billboard | Fails on a flat quad |
|---|---|
| `glow11.bmp`: round soft flash, tint it any colour | `lens_r/g/b/w.bmp`: 32x128 beam strips, draw as vertical bars |
| `fire_smoke_a.bmp`: orange fire cloud | `ring_*.tga`: cylinder wall strips, draw as a fence of streaks |
| `poison_smoke.bmp`: green cloud (acid, poison) | `water_slplash_*` at large sizes: streaky. Small (<=150 px) is ok |
| `dust_blast.bmp`: dusty smoke puff | `fire_smoke.bmp`: **cyan**, not orange |
| `pok1.tga`: spark/ember/droplet dot | `alpha_down.tga`, `alpha_center.tga`: gradients, not shapes |
| `fire_0001..0004.bmp`: flame flipbook | |
| `유저인터페이스/item/*.bmp`: item icons, use `Blend` (magenta keyed) | |

Effect BMPs have black backgrounds, so use `blend: Add`. Only magenta-keyed
art (item icons) and real-alpha TGAs should use `Blend`.

## 3. The strfx format

Schema: `crates/lifthrasir-data/src/effects/authored.rs`. Files go in
`assets/data/effects/<name>.strfx.ron` and are referenced as
`Str("<name>.strfx.ron")`.

```ron
(
    fps: 30,
    layers: [
        (
            textures: ["ro://data/texture/effect/glow11.bmp"],  // full ro:// paths
            blend: Add,                                         // Add | Blend | Multiply
            keys: [
                ( at: 5,  offset: (319.0, 238.0), quad: (20.0, 20.0),   color: (170, 255, 140, 0) ),
                ( at: 7,  offset: (319.0, 238.0), quad: (120.0, 120.0), color: (190, 255, 160, 255) ),
                ( at: 19, offset: (319.0, 238.0), quad: (165.0, 165.0), color: (80, 180, 70, 0), angle: 0.0, texture_index: 0 ),
            ],
        ),
    ],
)
```

- `offset` is in STR pixels. `(319, 291)` is the anchor, and world position is
  `(offset - (319, 291)) * 0.2`. **y grows down**, so a smaller y is higher.
- A character is about 100 px tall (20 world units). The feet are y 291, the
  chest about 238, the head about 195.
- Keys lerp linearly. `texture_index` is held, not lerped, which makes it a
  flipbook. `angle` is raw STR units (1024 per turn) and lerps, so 0 to 1024
  over a few frames is a spin.
- Validation requires strictly ascending `at`, exactly one of `quad`/`xy`, and
  `texture_index` in range. A layer is invisible outside its first..last key,
  so fade alpha in and out at 0.
- Layers are coplanar billboards at the anchor. Depth comes from size, offset
  and timing, not z. A ground glow is a flattened quad (about 3:1, w:h) at y 291.
- **Repeating** effects (fields, auras) must loop seamlessly: each layer's
  first key equals its last key.

Composition that worked (Demonstration, Acid Terror, Acid Demonstration):

- Keep a single-target burst around 130-170 px. Bursts above about 200 px
  with alpha near 255 wash out the character.
- Peak alpha 200-240 for clouds and flashes, decaying to about 100 by mid-life.
- Stagger the beats: projectile (frames 0-9), flash (+2), cloud (+3), debris
  arcs (3 keys: out, up, falling and fading), lingering ground vapour last.
- Use 3-4 `pok1` arcs with different end points instead of one big spray.

## 4. Capture and view

`fx_capture` opens a window, so **get the user's approval first** (asking for
a visual redo counts). Build it once (~2 min, run in the background):

```bash
cargo build --release -p lifthrasir-runtime --example fx_capture
target/release/examples/fx_capture ro://effects/acid_terror.strfx.ron /tmp/fx/at 0.1,0.2,0.3,0.5,0.9
python3 .agents/skills/lifthrasir-vfx-authoring/scripts/contact_sheet.py /tmp/fx/at.png /tmp/fx/at_0*.png
```

Then `read` the sheet. Times are seconds after load, and frame N is at N/30 s.
Pick times around each beat. The purple slab is a character stand-in. Always
capture the old effect first as a baseline. `shader:<key>` plays a `shader_fx`
entry. Sprites (`Sprite(...)`) cannot be captured, so preview their frames
with `pak_preview.py show`.

Look for bars and streaks (a strip texture), teal smoke (`fire_smoke.bmp`),
a burst that hides the body, empty frames between beats, and layers popping
in at full alpha.

## 5. Wire the catalog

`assets/data/ron/effects.ron`, whose header documents every section:

- `skills.<id>` is the one-shot cast/impact. `placement: Target | Ground | Caster`,
  plus `sound: Some("effect/x.wav")` and an optional `shake`.
- `skill_units.<id>` is the persistent ground-unit visual (must be
  `repeating: true`). `ground_anchor: Cell | Footprint | Group` selects
  visible cells, every server cell, or one at the group centre.
- Give a skill its own file rather than reusing another skill's when the two
  should differ. Delete strfx files nothing references any more
  (`rg -n '<name>' --glob '!target'`).
- Update nearby comments that your change makes wrong, such as "no reference
  exists".

## 6. Test

```bash
cargo test --locked -p lifthrasir-client                                  # catalog parse/validation
cargo test --locked -p lifthrasir-client --test effect_assets -- --ignored # every catalog path exists in the pak/loose data
```

The second one is read-only. Run it after adding a texture, sprite or sound
reference. Mask its output (section 0) if it lists missing paths.
