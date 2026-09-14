# RO GR2 reader

Adapted from [nostalro-client](https://github.com/nmeylan/nostalro-client),
revision `5d5bd9039dc46ae8fe5a29a9aebd49cbc5e2a8c6`,
`lib/formats/src/gr2`. Copyright 2026 Nicolas Meylan, Apache-2.0; see `LICENSE`.
The CLI sampler is adapted from that revision's `lib/game/src/gr2_model.rs`.

This module is behind the opt-in `gr2` feature. Only offline tooling enables it;
the game loads converted glTF, not GR2. It is an RO reader, not a general Granny SDK.

Lifthrasir changes: crate-local error and module paths; checked sector/root/fixup
ranges, bounded decoded output (64 MiB), array counts (one million), type nesting
(64), texture dimensions (4096 per axis), range-stream failures, required model
references and vertex layouts. Empty arrays may contain unused, unrelocated
pointer values, so their count takes precedence. Missing weight streams are
preserved explicitly in `Gr2VertexData::has_bone_weights`: those meshes bind rigidly
to the first named bone binding, not skeleton joint zero.

## Local compatibility check

The configured retail GRFs on 2026-09-14 contain six models and fifteen associated
animation files, all version 6, compression types 0 (stored) and 1 (Oodle0).
All parsed, their textures decoded, and every associated animation sampled at
120 Hz including its endpoint. Curves are constant (degree 0) or quadratic
(degree 2); no unrepresentable joint shear was found in these samples.

| Model | Available clips |
| --- | --- |
| aguardian90_8 | idle, walk, attack, hit, dead |
| empelium90_0 | idle |
| guildflag90_1 | idle, attack |
| kguardian90_7 | idle, walk, attack, hit, dead |
| sguardian90_9 | idle, walk, attack, hit, dead |
| treasurebox_2 | idle, hit, dead |

Guild flag: 2 meshes, 296 vertices, 41 joints, 5 materials and 2 Bink-encoded
textures (256×256 banner and a raw 16×16 emblem). Idle lasts 5.666667 seconds.
Its initial placement is identity. Sampled source bounds at time zero are
approximately `(-8.3865, -8.2567, -0.8428)` to `(8.3865, 6.1579, 41.5211)`;
the source is Z-up; the converter stands it up with -90° about X into glTF
Y-up space, which is also the runtime world. Maximum observed idle vertex displacement is 13.3028 units.

Run parser tests with `cargo test -p ro-formats --features gr2 gr2`.
The CLI's ignored `retail_gr2_corpus_decodes_and_samples` test requires the GRFs
listed in `assets/convert.toml`. No retail assets are included as fixtures.
