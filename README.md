<div align="center">

# PumpkinMC-AI

![CI](https://github.com/gringoestrangeiro/pumpkinmc-ai/actions/workflows/rust.yml/badge.svg)
[![Discord](https://img.shields.io/discord/1268592337445978193.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/wT8XjrjKkf)
[![License: GPL](https://img.shields.io/badge/License-GPLv3-yellow.svg)](https://opensource.org/licenses/gpl-3-0)

</div>

> [!IMPORTANT]
> **PumpkinMC-AI is an unofficial fork of [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) and is not affiliated with or endorsed by the PumpkinMC organization.**
> It keeps the original licenses (GPL-3.0 for the server, MIT OR Apache-2.0 for the plugin API crates), preserves the full git history, and credits the Pumpkin contributors in [NOTICE](NOTICE).
> This fork accepts AI-written code and pull requests under the rules in [AGENTS.md](AGENTS.md) and [CONTRIBUTING.md](CONTRIBUTING.md#ai-assisted-contributions).

PumpkinMC-AI tracks [Pumpkin](https://pumpkinmc.org/), a Minecraft server built entirely in Rust, offering a fast, efficient,
and customizable experience. It prioritizes performance and player enjoyment while adhering to the core mechanics of the game.
<div align="center">

![Pumpkin Chunk Loading](./assets/pumpkin-chunk-loading.webp)

</div>

## Goals

- **Performance**: Leveraging multi-threading for maximum speed and efficiency.
- **Compatibility**: Supports the latest Java & Bedrock Minecraft server version while adhering to Vanilla game mechanics.
- **Security**: Prioritizes security by preventing known security exploits.
- **Flexibility**: Highly configurable, with the ability to disable unnecessary features.
- **Extensibility**: Provides a foundation for plugin development.

> [!IMPORTANT]
> Pumpkin is currently under heavy development.
>
> [See what needs to be done before the 1.0.0 Release](https://github.com/Pumpkin-MC/Pumpkin/issues/449)

## Features

- [x] Configuration (toml)
- [Tracking: Protocol](https://github.com/Pumpkin-MC/Pumpkin/issues/1401)
  - [x] Server Status/Ping
  - [x] Encryption
  - [x] Packet Compression
  - [x] Java Edition
  - [x] Bedrock Edition (W.I.P)
  - ...
- [Tracking: World](https://github.com/Pumpkin-MC/Pumpkin/issues/1403)
  - [x] Player Tab-list
  - [x] Scoreboard
  - [x] World Loading
  - [x] World Time
  - [x] World Borders
  - [x] World Saving
  - [x] Lighting
  - [x] Entity Spawning
  - [x] Bossbar
  - [x] Chunk Loading (Vanilla, Linear, Pump)
  - [Chunk Generation](https://github.com/Pumpkin-MC/Pumpkin/issues/36)
  - [x] Chunk Saving (Vanilla, Linear, Pump)
  - [Redstone](https://github.com/Pumpkin-MC/Pumpkin/issues/1402)
  - [x] Liquid Physics
  - ...
- [Tracking: Player](https://github.com/Pumpkin-MC/Pumpkin/issues/1405)
  - [x] Skins
  - [x] Teleport
  - [x] Movement
  - [x] Animation
  - [x] Inventory
  - [Combat](https://github.com/Pumpkin-MC/Pumpkin/issues/1404)
  - [x] Experience
  - [x] Hunger
  - [X] Off Hand
  - [X] Advancements (W.I.P)
  - [x] Eating
  - ...
- Entities
  - [x] Non-Living (Minecart, Eggs...) (W.I.P)
  - [x] Entity Effects
  - [x] Players
  - [x] Mobs (W.I.P)
  - [x] Animals (W.I.P)
  - [Entity AI](https://github.com/Pumpkin-MC/Pumpkin/issues/1406)
  - [x] Boss (W.I.P)
  - [x] Villagers (W.I.P)
  - [X] Entity Saving
- Server
  - [Plugins](https://github.com/Pumpkin-MC/Pumpkin/issues/1407)
  - [x] Query
  - [x] [Minecraft Server Management Protocol (MSMP)](https://minecraft.wiki/w/Minecraft_Server_Management_Protocol)
  - [x] Inventories
  - [x] Particles
  - [x] Chat
  - [Commands](https://github.com/Pumpkin-MC/Pumpkin/issues/15)
  - [x] Permissions
  - [x] Translations
- Proxy
  - [x] [BungeeCord](https://github.com/SpigotMC/BungeeCord)
  - [x] [BungeeGuard](https://github.com/lucko/BungeeGuard)
  - [x] [Velocity](https://github.com/PaperMC/Velocity)

<!-- Check out our [Github Project](https://github.com/orgs/Pumpkin-MC/projects/3) to see current progress. -->

## Parity

Tracker-derived completion against Java Edition 26.3. This is a coverage score, not a behavioral audit; see [PARITY.md](PARITY.md) for the methodology, per-category detail, and known gaps.

| Category | Done | Total | Parity |
| :--- | ---: | ---: | ---: |
| Blocks | 163 | 196 | 83.2% |
| Entities | 59 | 160 | 36.9% |
| Commands | 92 | 94 | 97.9% |
| Items | 51 | 55 | 92.7% |
| Redstone | 24 | 27 | 88.9% |
| World | 13 | 16 | 81.2% |
| Combat | 13 | 14 | 92.9% |
| **Total** | **415** | **562** | **73.8%** |

Integrated-server-only commands such as `/publish` and `/unpublish` are excluded from the command total. The upstream public tracker reports about 71.5% on the same in-scope basis.

## How to run

See the upstream [Quick Start](https://docs.pumpkinmc.org/#quick-start) guide to get PumpkinMC-AI running.

## Contributions

Contributions are welcome, including AI-written ones under the rules in [CONTRIBUTING.md](CONTRIBUTING.md).

## Docs

Upstream Pumpkin's documentation can be found at <https://pumpkinmc.org/>

## Communication

Consider joining [our Discord server](https://discord.gg/wT8XjrjKkf) to stay up-to-date on events, updates, and connect with other members.

## Syncing with upstream

This repository keeps the full Pumpkin history but is not attached to GitHub's fork network, so add the upstream remote manually:

```bash
git remote add upstream https://github.com/Pumpkin-MC/Pumpkin.git
git fetch upstream
git merge upstream/master
```

## Funding

If you want to fund me and help the project, check out the [Donation Page](https://pumpkinmc.org/donate/).

## License & Attribution

* **Pumpkin Server**: Licensed under the [GNU General Public License v3.0 (GPLv3)](LICENSE).
* **Plugin API (`pumpkin-plugin-api` & `pumpkin-plugin-wit`)**: Dual-licensed under [MIT](crates/pumpkin-plugin-api/LICENSE-MIT) OR [Apache-2.0](crates/pumpkin-plugin-api/LICENSE-APACHE) for maximum flexibility when writing plugins.
* **This fork**: unofficial and not affiliated with the PumpkinMC organization. See [NOTICE](NOTICE).
* **Third-Party Assets & Data**: Bedrock mappings, protocol conversion data, and Minecraft assets are subject to their respective licenses and attribution terms. See [assets/NOTICE.md](assets/NOTICE.md) for full details.
