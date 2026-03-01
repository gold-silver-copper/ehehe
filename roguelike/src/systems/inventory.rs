use bevy::prelude::*;

use crate::components::{
    Ammo, Health, Inventory, Item, ItemKind, Name, Player, Position, Renderable,
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
    mut ammo_query: Query<&mut Ammo, With<Player>>,
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
            ItemKind::HealingPotion { amount } => {
                if let Ok(mut hp) = health_query.single_mut() {
                    let healed = (*amount).min(hp.max - hp.current);
                    hp.current = (hp.current + amount).min(hp.max);
                    combat_log.push(format!("Used {item_name}, healed {healed} HP"));
                }
                inv.items.remove(intent.item_index);
                commands.entity(item_entity).despawn();
            }
            ItemKind::Explosive { damage: _, radius: _ } => {
                // Explosives trigger a blast effect — for now just log and consume.
                combat_log.push(format!("Used {item_name}!"));
                inv.items.remove(intent.item_index);
                commands.entity(item_entity).despawn();
            }
            ItemKind::Armor { defense } => {
                combat_log.push(format!("Equipped {item_name} (+{defense} def)"));
                // Equip handled elsewhere; for now just log.
            }
            ItemKind::Weapon { attack } => {
                combat_log.push(format!("Equipped {item_name} (+{attack} atk)"));
            }
            ItemKind::Magazine { ammo: mag_ammo } => {
                // Reload from this magazine: set player ammo to magazine's ammo count.
                if let Ok(mut player_ammo) = ammo_query.single_mut() {
                    let loaded = (*mag_ammo).min(player_ammo.max);
                    let leftover = *mag_ammo - loaded;
                    // Save current partial magazine to inventory if it has ammo.
                    if player_ammo.current > 0 {
                        let partial_mag = commands.spawn((
                            Item,
                            Name(format!("Magazine ({})", player_ammo.current)),
                            Renderable {
                                symbol: "m".into(),
                                fg: RatColor::Rgb(180, 180, 60),
                                bg: RatColor::Black,
                            },
                            ItemKind::Magazine { ammo: player_ammo.current },
                        )).id();
                        // Add partial magazine to inventory if space.
                        if inv.items.len() < 9 {
                            inv.items.push(partial_mag);
                        }
                    }
                    // If the magazine had more ammo than max capacity, save leftover.
                    if leftover > 0 && inv.items.len() < 9 {
                        let leftover_mag = commands.spawn((
                            Item,
                            Name(format!("Magazine ({})", leftover)),
                            Renderable {
                                symbol: "m".into(),
                                fg: RatColor::Rgb(180, 180, 60),
                                bg: RatColor::Black,
                            },
                            ItemKind::Magazine { ammo: leftover },
                        )).id();
                        inv.items.push(leftover_mag);
                    }
                    player_ammo.current = loaded;
                    combat_log.push(format!("Loaded {item_name} ({loaded} rounds)"));
                }
                inv.items.remove(intent.item_index);
                commands.entity(item_entity).despawn();
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
        name: "Medkit",
        symbol: "+",
        fg: RatColor::Rgb(255, 50, 50),
        kind: ItemKind::HealingPotion { amount: 10 },
        weight: 0.25,
    },
    LootEntry {
        name: "Frag Grenade",
        symbol: "*",
        fg: RatColor::Rgb(255, 165, 0),
        kind: ItemKind::Explosive { damage: 8, radius: 2 },
        weight: 0.15,
    },
    LootEntry {
        name: "Magazine (30)",
        symbol: "m",
        fg: RatColor::Rgb(180, 180, 60),
        kind: ItemKind::Magazine { ammo: 30 },
        weight: 0.25,
    },
    LootEntry {
        name: "Body Armor",
        symbol: "[",
        fg: RatColor::Rgb(100, 130, 100),
        kind: ItemKind::Armor { defense: 1 },
        weight: 0.15,
    },
    LootEntry {
        name: "Combat Rifle",
        symbol: "/",
        fg: RatColor::Rgb(180, 180, 200),
        kind: ItemKind::Weapon { attack: 2 },
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
    // Fallback: spawn a medkit.
    commands.spawn((
        Position { x, y },
        Item,
        Name("Medkit".into()),
        Renderable {
            symbol: "+".into(),
            fg: RatColor::Rgb(255, 50, 50),
            bg: RatColor::Black,
        },
        ItemKind::HealingPotion { amount: 10 },
    ));
}

/// Processes reload: swaps the current magazine with one from inventory.
/// Triggered by the reload_pending flag in InputState.
/// The current partial magazine (if it has ammo) is saved back to inventory.
pub fn reload_system(
    mut commands: Commands,
    mut player_query: Query<(&mut Ammo, &mut Inventory), With<Player>>,
    item_kind_query: Query<(&ItemKind, Option<&Name>)>,
    mut combat_log: ResMut<CombatLog>,
    mut input_state: ResMut<InputState>,
) {
    if !input_state.reload_pending {
        return;
    }
    input_state.reload_pending = false;

    let Ok((mut ammo, mut inv)) = player_query.single_mut() else {
        return;
    };

    // Find the first Magazine in inventory.
    let mag_index = inv.items.iter().position(|&ent| {
        item_kind_query
            .get(ent)
            .ok()
            .map_or(false, |(k, _)| matches!(k, ItemKind::Magazine { .. }))
    });

    let Some(idx) = mag_index else {
        combat_log.push("No magazines in inventory!".into());
        return;
    };

    let mag_entity = inv.items[idx];
    let Ok((kind, _name)) = item_kind_query.get(mag_entity) else {
        return;
    };

    let mag_ammo = match kind {
        ItemKind::Magazine { ammo: a } => *a,
        _ => return,
    };

    // Save current partial magazine to inventory if it has ammo.
    if ammo.current > 0 {
        let partial_mag = commands.spawn((
            Item,
            Name(format!("Magazine ({})", ammo.current)),
            Renderable {
                symbol: "m".into(),
                fg: RatColor::Rgb(180, 180, 60),
                bg: RatColor::Black,
            },
            ItemKind::Magazine { ammo: ammo.current },
        )).id();
        // Replace the used magazine slot with the partial one.
        inv.items[idx] = partial_mag;
    } else {
        inv.items.remove(idx);
    }

    // Load the new magazine, preserving any leftover ammo.
    let loaded = mag_ammo.min(ammo.max);
    let leftover = mag_ammo - loaded;
    ammo.current = loaded;

    if leftover > 0 && inv.items.len() < 9 {
        let leftover_mag = commands.spawn((
            Item,
            Name(format!("Magazine ({})", leftover)),
            Renderable {
                symbol: "m".into(),
                fg: RatColor::Rgb(180, 180, 60),
                bg: RatColor::Black,
            },
            ItemKind::Magazine { ammo: leftover },
        )).id();
        inv.items.push(leftover_mag);
    }

    combat_log.push(format!("Reloaded! ({loaded} rounds)"));

    // Despawn the consumed magazine entity.
    commands.entity(mag_entity).despawn();
}

/// Auto-pickup system: automatically picks up magazines and grenades when the
/// player walks over them. Runs after movement.
pub fn auto_pickup_system(
    mut commands: Commands,
    player_query: Query<&Position, With<Player>>,
    items_query: Query<(Entity, &Position, &ItemKind, Option<&Name>), With<Item>>,
    spatial: Res<SpatialIndex>,
    mut inventory_query: Query<&mut Inventory, With<Player>>,
    mut ammo_query: Query<&mut Ammo, With<Player>>,
    mut combat_log: ResMut<CombatLog>,
) {
    let Ok(player_pos) = player_query.single() else {
        return;
    };
    let player_vec = player_pos.as_grid_vec();

    let entities_here = spatial.entities_at(&player_vec);

    for &ent in entities_here {
        let Ok((item_entity, _pos, item_kind, item_name)) = items_query.get(ent) else {
            continue;
        };

        match item_kind {
            ItemKind::Magazine { ammo: mag_ammo } => {
                // Auto-pickup: add ammo directly or store in inventory.
                let name_str = item_name.map_or("Magazine", |n| n.0.as_str()).to_string();
                if let Ok(mut player_ammo) = ammo_query.single_mut() {
                    if player_ammo.current < player_ammo.max {
                        // Add directly to active ammo.
                        let added = (*mag_ammo).min(player_ammo.max - player_ammo.current);
                        player_ammo.current += added;
                        combat_log.push(format!("Picked up {name_str} (+{added} ammo)"));
                        commands.entity(item_entity).despawn();
                        continue;
                    }
                }
                // Ammo full — store in inventory.
                if let Ok(mut inv) = inventory_query.single_mut() {
                    if inv.items.len() < 9 {
                        commands.entity(item_entity).remove::<Position>();
                        inv.items.push(item_entity);
                        combat_log.push(format!("Picked up {name_str}"));
                    }
                }
            }
            ItemKind::Explosive { .. } => {
                // Auto-pickup grenades into inventory.
                let name_str = item_name.map_or("Grenade", |n| n.0.as_str()).to_string();
                if let Ok(mut inv) = inventory_query.single_mut() {
                    if inv.items.len() < 9 {
                        commands.entity(item_entity).remove::<Position>();
                        inv.items.push(item_entity);
                        combat_log.push(format!("Picked up {name_str}"));
                    }
                }
            }
            _ => {} // Other items require manual pickup with 'G'.
        }
    }
}
