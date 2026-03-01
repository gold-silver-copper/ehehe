use bevy::prelude::*;

use crate::components::{
    Health, Inventory, Item, ItemKind, Name, Player, Position, Renderable,
};
use crate::events::{PickupItemIntent, UseItemIntent};
use crate::resources::{CombatLog, InputState, SpatialIndex};
use crate::typedefs::RatColor;

/// Processes pickup intents: player picks up an item on the ground at their position.
pub fn pickup_system(
    mut intents: MessageReader<PickupItemIntent>,
    mut commands: Commands,
    player_query: Query<&Position, With<Player>>,
    items_query: Query<(Entity, &Position, Option<&Name>), With<Item>>,
    spatial: Res<SpatialIndex>,
    mut inventory_query: Query<&mut Inventory, With<Player>>,
    mut combat_log: ResMut<CombatLog>,
) {
    for intent in intents.read() {
        let Ok(player_pos) = player_query.get(intent.picker) else {
            continue;
        };
        let player_vec = player_pos.as_grid_vec();

        // Find items at the player's position using the spatial index.
        let entities_here = spatial.entities_at(&player_vec);
        let mut picked_up = false;

        for &ent in entities_here {
            if items_query.get(ent).is_ok() {
                let item_name = items_query
                    .get(ent)
                    .ok()
                    .and_then(|(_, _, n)| n)
                    .map_or("item", |n| n.0.as_str())
                    .to_string();

                // Add to inventory.
                if let Ok(mut inv) = inventory_query.single_mut() {
                    if inv.items.len() < 9 {
                        // Remove position so it's no longer on the map.
                        commands.entity(ent).remove::<Position>();
                        inv.items.push(ent);
                        combat_log.push(format!("Picked up {item_name}"));
                        picked_up = true;
                        break; // Pick up one item at a time.
                    } else {
                        combat_log.push("Inventory full!".into());
                    }
                }
            }
        }

        if !picked_up {
            combat_log.push("Nothing to pick up here.".into());
        }
    }
}

/// Processes use-item intents: consumes an item from the player's inventory.
pub fn use_item_system(
    mut intents: MessageReader<UseItemIntent>,
    mut commands: Commands,
    mut inventory_query: Query<&mut Inventory, With<Player>>,
    mut health_query: Query<&mut Health, With<Player>>,
    item_kind_query: Query<(&ItemKind, Option<&Name>)>,
    mut combat_log: ResMut<CombatLog>,
) {
    for intent in intents.read() {
        let Ok(mut inv) = inventory_query.single_mut() else {
            continue;
        };

        let Some(&item_entity) = inv.items.get(intent.item_index) else {
            combat_log.push("No item in that slot.".into());
            continue;
        };

        let Ok((kind, name)) = item_kind_query.get(item_entity) else {
            combat_log.push("Invalid item.".into());
            continue;
        };

        let item_name = name.map_or("item", |n| n.0.as_str()).to_string();

        match kind {
            ItemKind::Whiskey { heal } => {
                if let Ok(mut hp) = health_query.single_mut() {
                    let healed = (*heal).min(hp.max - hp.current);
                    hp.current = (hp.current + heal).min(hp.max);
                    combat_log.push(format!("Used {item_name}, healed {healed} HP"));
                }
                inv.items.remove(intent.item_index);
                commands.entity(item_entity).despawn();
            }
            ItemKind::Grenade { .. } => {
                combat_log.push(format!("Used {item_name}!"));
                inv.items.remove(intent.item_index);
                commands.entity(item_entity).despawn();
            }
            ItemKind::Hat { defense } => {
                combat_log.push(format!("Equipped {item_name} (+{defense} def)"));
            }
            ItemKind::Gun { .. } => {
                combat_log.push(format!("Equipped {item_name}"));
            }
            ItemKind::Knife { .. } => {
                combat_log.push("Readied knife".into());
            }
            ItemKind::Tomahawk { .. } => {
                combat_log.push("Readied tomahawk".into());
            }
        }
    }
}

/// Loot table entries for item drops.
struct LootEntry {
    name: &'static str,
    symbol: &'static str,
    fg: RatColor,
    kind: ItemKind,
    weight: f64,
}

const LOOT_TABLE: &[LootEntry] = &[
    LootEntry {
        name: "Whiskey Bottle",
        symbol: "w",
        fg: RatColor::Rgb(180, 120, 60),
        kind: ItemKind::Whiskey { heal: 10 },
        weight: 0.25,
    },
    LootEntry {
        name: "Dynamite Stick",
        symbol: "*",
        fg: RatColor::Rgb(255, 165, 0),
        kind: ItemKind::Grenade { damage: 8, radius: 2 },
        weight: 0.15,
    },
    LootEntry {
        name: "Bowie Knife",
        symbol: "/",
        fg: RatColor::Rgb(192, 192, 210),
        kind: ItemKind::Knife { attack: 4 },
        weight: 0.20,
    },
    LootEntry {
        name: "Tomahawk",
        symbol: "t",
        fg: RatColor::Rgb(160, 120, 80),
        kind: ItemKind::Tomahawk { attack: 5 },
        weight: 0.20,
    },
    LootEntry {
        name: "Cowboy Hat",
        symbol: "^",
        fg: RatColor::Rgb(210, 180, 140),
        kind: ItemKind::Hat { defense: 1 },
        weight: 0.20,
    },
];

/// Spawns a random loot item at the given position using deterministic noise.
/// Called by the death system when a monster with a LootTable dies.
pub fn spawn_loot(commands: &mut Commands, x: i32, y: i32, roll: f64) {
    // Select item based on weighted roll.
    let mut cumulative = 0.0;
    for entry in LOOT_TABLE {
        cumulative += entry.weight;
        if roll < cumulative {
            commands.spawn((
                Position { x, y },
                Item,
                Name(entry.name.into()),
                Renderable {
                    symbol: entry.symbol.into(),
                    fg: entry.fg,
                    bg: RatColor::Black,
                },
                entry.kind.clone(),
            ));
            return;
        }
    }
    // Fallback: spawn a whiskey bottle.
    commands.spawn((
        Position { x, y },
        Item,
        Name("Whiskey Bottle".into()),
        Renderable {
            symbol: "w".into(),
            fg: RatColor::Rgb(180, 120, 60),
            bg: RatColor::Black,
        },
        ItemKind::Whiskey { heal: 10 },
    ));
}

/// Reload system placeholder. Real reloading will use per-gun loaded rounds.
pub fn reload_system(
    _commands: Commands,
    player_query: Query<&Inventory, With<Player>>,
    item_kind_query: Query<(&ItemKind, Option<&Name>)>,
    mut combat_log: ResMut<CombatLog>,
    mut input_state: ResMut<InputState>,
) {
    if !input_state.reload_pending {
        return;
    }
    input_state.reload_pending = false;

    let Ok(inv) = player_query.single() else {
        return;
    };

    // Find the first Gun in inventory.
    let _gun_index = inv.items.iter().position(|&ent| {
        item_kind_query
            .get(ent)
            .ok()
            .map_or(false, |(k, _)| matches!(k, ItemKind::Gun { .. }))
    });

    combat_log.push("Reload not yet implemented in field - use inventory mode".into());
}

/// Auto-pickup system: automatically picks up any item when the player walks
/// over it. Runs after movement.
pub fn auto_pickup_system(
    mut commands: Commands,
    player_query: Query<&Position, With<Player>>,
    items_query: Query<(Entity, &Position, Option<&Name>), With<Item>>,
    spatial: Res<SpatialIndex>,
    mut inventory_query: Query<&mut Inventory, With<Player>>,
    mut combat_log: ResMut<CombatLog>,
) {
    let Ok(player_pos) = player_query.single() else {
        return;
    };
    let player_vec = player_pos.as_grid_vec();

    let entities_here = spatial.entities_at(&player_vec);

    for &ent in entities_here {
        let Ok((item_entity, _pos, item_name)) = items_query.get(ent) else {
            continue;
        };

        let name_str = item_name.map_or("item", |n| n.0.as_str()).to_string();
        if let Ok(mut inv) = inventory_query.single_mut() {
            if inv.items.len() < 9 {
                commands.entity(item_entity).remove::<Position>();
                inv.items.push(item_entity);
                combat_log.push(format!("Picked up {name_str}"));
            }
        }
    }
}
