---
name: lifthrasir-grf-utils
description: Use when exploring or extracting Ragnarok Online GRF archives in the Lifthrasir repo — listing files, finding a sprite/texture/map/model path, extracting SPR/ACT/GND/GAT/RSW/RSM/BMP assets, or inspecting GRF metadata. Covers the grf-utils CLI, the data.grf/en.grf locations, and the backslash + Korean (EUC-KR) filename gotchas.
---

# grf-utils (Lifthrasir GRF explorer)

CLI in `grf-utils/` that lists, extracts, and inspects GRF archives via game-engine's `GrfFile` parser. Use it to find and pull assets (SPR, ACT, GND, GAT, RSW, RSM, BMP, Lua) out of the game data.

## Archives in this repo

- `assets/data.grf` — main RO archive (~215k files, ~16 GB uncompressed)
- `assets/en.grf` — English overlay

## Running

Prefer the prebuilt binary if present, else build it:

```bash
./target/debug/grf-utils <cmd> assets/data.grf       # if already built
cargo run -p grf-utils -- <cmd> assets/data.grf      # otherwise
```

`<cmd>` is `list`, `info`, or `extract`. Run with `--help` for flags.

## Commands

| Command | Use |
|---------|-----|
| `info assets/data.grf` | counts, compressed/uncompressed size, encrypted file count |
| `list assets/data.grf` | dump every filename + size (no filter — pipe to `rg`) |
| `extract assets/data.grf [FILES...] -o out` | extract named files, or ALL if none given, into `out/` (default `output/`) |

## Finding a file (list has no filter)

`list` prints all ~215k paths. Filter with ripgrep — match works with `\` or `/`:

```bash
./target/debug/grf-utils list assets/data.grf | rg -i 'poring.*\.spr'
./target/debug/grf-utils list assets/data.grf | rg -i 'prontera' | rg '\.gat'
```

## Extracting

```bash
# forward OR backslashes both work; lookup is case-insensitive (ASCII)
./target/debug/grf-utils extract assets/data.grf "data/sprite/poring.spr" -o out
./target/debug/grf-utils extract assets/data.grf  # extracts EVERYTHING (16 GB) — don't, unless you mean it
```

## Filename gotchas

- **Paths use backslashes**: stored as `data\sprite\...`. `extract` normalizes `/`→`\` for lookup, so either works; when matching `list` output yourself, account for `\`.
- **Korean, EUC-KR encoded**: many job/robe paths are Korean (e.g. `data\sprite\로브\...`). `list` prints them decoded as UTF-8. Copy-paste the exact string when extracting.
- **Lookup is case-insensitive** (ASCII only) — Korean must match exactly.
- **Encrypted entries** (~41k, `file_type & 0x06`): DES-decrypted on extract; directory entries (`file_type & 0x01 == 0`) return nothing.

## Don't

- Don't `extract` with no file list unless you want all 16 GB.
- Don't grep `list` for a casing-variant Korean path expecting a hit — Korean is case-sensitive.
