<p align="center">
  <img width="300" height="300" src="https://iili.io/KbXkCeR.md.png">
</p>

[![](https://dcbadge.limes.pink/api/server/https://discord.gg/xGq832HYey)](https://discord.gg/xGq832HYey)

# Lifthrasir

A Ragnarok Online client built with Rust and Bevy. This is a learning project, and I want room to try new ideas while keeping the feel of the original game. A perfect 1:1 remake isn't the goal.

You’ll need Rust and your own legitimate Ragnarok Online GRF files. Game data is proprietary and isn't included here.

## Getting started

Clone the repo, put your GRF files in `assets/`, configure `assets/loader.toml`, then follow the [development guide](docs/development.md) to convert the data and build a pak. Once that’s done, run:

```bash
cargo run -p lifthrasir --features dev
```

For an overview of the crates and how they fit together, see [Architecture](docs/architecture.md).

## Server compatibility

The client currently works with [Aesir](https://github.com/EndurnyrProject/aesir). The network boundary is protocol-neutral, so other server adapters are possible, but rAthena compatibility isn't a project goal.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for project guidance.

## Thanks

This project borrows code and ideas from:

- [RagnarokRebuildTcp](https://github.com/Doddler/RagnarokRebuildTcp)
- [BrowEdit3](https://github.com/Borf/BrowEdit3)
- [GRFEditor](https://github.com/Tokeiburu/GRFEditor)
- [Korangar](https://github.com/vE5li/korangar)

## Legal notice

This project is not affiliated with, endorsed by, or connected to Gravity Co., Ltd. or any official Ragnarok Online server. Users must provide their own legitimate game data files and comply with all applicable terms of service.
