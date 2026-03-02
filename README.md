# Dead Man's Hand

**An 1850s cowboy shootout roguelike**

```
            -*-  DEAD MAN'S HAND  -*-

  You're a cowboy drinking in a saloon
  when bandits raid your town!
```

A turn-based roguelike set in the American frontier. Navigate a
procedurally generated Western town, fight outlaws, vaqueros, lawmen,
and wildlife, and destroy the Outlaw Hideout (Ω) to win.

## Features

- **Procedural Western town** — desert terrain, saloons, stables, sheriff's
  offices, and more, all generated from deterministic noise.
- **Cap-and-ball revolvers** — period-accurate .31, .36, and .44 caliber
  firearms with per-gun loaded-round tracking and manual reloading.
- **Faction warfare** — Outlaws, Lawmen, Vaqueros, and Wildlife fight each
  other as well as the player. NPCs pathfind with A\*, patrol, scavenge
  items, and level up from their own kills.
- **Procedural NPC names** — every humanoid enemy gets a unique 1850s-themed
  name (e.g., "Dusty" Silas Crowley, Ezekiel Boone, Cornelius Shaw).
- **Throwable weapons** — knives, tomahawks, dynamite, and molotov cocktails
  with area-of-effect fire spread.
- **Energy-based turn scheduling** — faster entities act more often; excess
  energy carries over for exact long-run fairness.
- **Directional field-of-view** — both the player and enemies have
  cone-based vision tied to facing direction.
- **Sound indicators** — off-screen audible events (gunshots, explosions)
  appear as yellow `!` on the map in fog-of-war areas.
- **Combat log filtering** — only events visible to the player are shown,
  with projectile ownership attribution (e.g., "Silas Crowley's bullet hits
  Player").
- **Fire propagation** — molotovs and explosions ignite flammable furniture;
  fire spreads to adjacent objects and burns out over time.
- **Collectible supply system** — caps, black powder, lead bullets, bandages,
  and dollars.
- **Leveling and progression** — gain EXP from kills, level up to increase
  attack, defense, HP, and stamina.

## Controls

| Key            | Action                     |
| -------------- | -------------------------- |
| W A S D        | Move                       |
| I J K L        | Aim cursor                 |
| Space          | Fire toward cursor         |
| F              | Roundhouse kick (AoE)      |
| E              | Pick up item               |
| B              | Open inventory             |
| 1–9            | Quick-use inventory slot    |
| R              | Reload gun                 |
| .              | Wait a turn                |
| ?              | Toggle help overlay        |
| Esc            | Pause / menu               |

## Building & Running

Requires **Rust 1.85+** (nightly features are used).

```bash
cd roguelike
cargo run --release
```

The game renders in your terminal using [ratatui](https://ratatui.rs/) and
runs on any platform with a compatible terminal emulator.

## Running Tests

```bash
cd roguelike
cargo test
```

## Architecture

The game is built on [Bevy ECS](https://bevyengine.org/) with a custom
terminal renderer via `bevy_ratatui`. See
[`roguelike/ECS_ARCHITECTURE.md`](roguelike/ECS_ARCHITECTURE.md) for a
detailed breakdown of systems, components, resources, and data flow.

## License

This project is provided as-is for educational and entertainment purposes.
