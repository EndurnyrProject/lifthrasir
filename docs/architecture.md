# Architecture

The workspace is split between executable apps in `apps/` and reusable libraries in `crates/`.

## Workspace

- `lifthrasir-data`, `lifthrasir-pak`, and `lifthrasir-tooling` provide Bevy-free schemas, archive I/O, and offline imports. The converter CLI presents structured reports; the library does not own terminal output or protobuf generation.
- `lifthrasir-assets` and `lifthrasir-runtime` handle content loading, catalogs, and reusable actor, effect, world, and audio presentation without client or network policy.
- `lifthrasir-ui` contains generic Feathers primitives. Client screens and gameplay live in `lifthrasir-client-ui` and `lifthrasir-client`.
- `net-contract` defines protocol-neutral messages, while `net-aesir` implements the selected transport and codec. `ro-formats` provides Ragnarok Online format readers.
- `apps/lifthrasir` composes the client. `apps/grf-utils` handles GRF inspection and pak operations, and `apps/ro-to-lifthrasir-cli` dispatches offline conversion and protobuf generation.

## Network boundary

`net-contract` owns the Bevy message contract: inbound server events, outbound client commands, and the neutral data types they use. It knows nothing about a wire protocol. An adapter such as `net-aesir` translates between those messages and its transport and codec.

The client and client UI depend on `net-contract`, not on a transport or codec. To add another protocol, implement an adapter crate against the contract and compose its plugin in the binary. The client, UI, and contract can remain unchanged.

## Map boundary

Maps are converted offline to glTF; the runtime does not parse GND, RSW, RSM, or RSM2. A converted map includes its terrain, lighting, emitters, prop references, water parameters and baked water tile mask, plus the original GAT bytes. Runtime GAT decoding remains necessary for grounding, raycasts, and pathfinding.

A missing or invalid map glTF is a hard failure. Props must also be converted to glTF. When changing the map format, extend the converter schema and glTF writer, and bump `lif::MAP_FORMAT_VERSION`; a prop model change bumps `lif::MODEL_FORMAT_VERSION`. The two versions are independent, so a map bump does not invalidate cached models. Ground textures are pooled in `ground/tex/` and referenced by absolute `ro://` URI; prop textures live in `models/tex/`.

## Shared plugins

The binary composes the contract, content, client, network adapter, and UI groups. Shared content/runtime/UI crates do not depend on the client or network. For plugin setup and dependency-boundary details, see `AGENTS.md`.
