use bevy::prelude::*;

use crate::components::{CombatStats, Health, HellGate, Hostile, LootTable, Name, Position};
use crate::events::{AttackIntent, DamageEvent};
use crate::noise::value_noise;
use crate::resources::{CombatLog, GameState, KillCount, MapSeed};
use crate::systems::inventory::spawn_loot;

/// Resolves attack intents into damage events.
///
/// Damage = max(0, attacker.attack − target.defense).
/// Emits a `DamageEvent` for each successful hit and logs combat messages.
pub fn combat_system(
    mut intents: MessageReader<AttackIntent>,
    mut damage_events: MessageWriter<DamageEvent>,
    stats_query: Query<(&CombatStats, Option<&Name>)>,
    mut combat_log: ResMut<CombatLog>,
) {
    for intent in intents.read() {
        let Ok((attacker_stats, attacker_name)) = stats_query.get(intent.attacker) else {
            continue;
        };
        let Ok((target_stats, target_name)) = stats_query.get(intent.target) else {
            continue;
        };

        let damage = (attacker_stats.attack - target_stats.defense).max(0);
        let a_name = attacker_name.map_or("???", |n| &n.0);
        let t_name = target_name.map_or("???", |n| &n.0);

        if damage > 0 {
            combat_log.push(format!("{a_name} hits {t_name} for {damage} damage"));
            damage_events.write(DamageEvent {
                target: intent.target,
                amount: damage,
            });
        } else {
            combat_log.push(format!("{a_name} attacks {t_name} but deals no damage"));
        }
    }
}

/// Applies damage events to entity health pools.
pub fn apply_damage_system(
    mut events: MessageReader<DamageEvent>,
    mut health_query: Query<&mut Health>,
) {
    for event in events.read() {
        if let Ok(mut health) = health_query.get_mut(event.target) {
            health.current = (health.current - event.amount).max(0);
        }
    }
}

/// Despawns entities whose health has reached zero.
/// Logs a death message, increments the kill counter for hostile entities,
/// spawns loot from entities with a LootTable, and removes the entity
/// from the world. If the Hell Gate is destroyed, transitions to the Victory state.
pub fn death_system(
    mut commands: Commands,
    query: Query<(Entity, &Health, Option<&Name>, Option<&Hostile>, Option<&HellGate>, Option<&Position>, Option<&LootTable>)>,
    mut combat_log: ResMut<CombatLog>,
    mut kill_count: ResMut<KillCount>,
    mut next_game_state: ResMut<NextState<GameState>>,
    seed: Res<MapSeed>,
) {
    for (entity, health, name, hostile, hell_gate, pos, loot_table) in &query {
        if health.current <= 0 {
            let label = name.map_or("Something", |n| &n.0);
            combat_log.push(format!("{label} has been slain!"));
            if hostile.is_some() {
                kill_count.0 += 1;
            }
            if hell_gate.is_some() {
                combat_log.push("The Gate of Hell crumbles! You are victorious!".into());
                next_game_state.set(GameState::Victory);
            }

            // Loot drop: if the entity has a LootTable, roll for item drop.
            if let (Some(lt), Some(p)) = (loot_table, pos) {
                let drop_roll = value_noise(p.x.wrapping_add(kill_count.0 as i32), p.y, seed.0.wrapping_add(55555));
                if drop_roll < lt.drop_chance {
                    let item_roll = value_noise(p.y, p.x.wrapping_add(kill_count.0 as i32), seed.0.wrapping_add(77777));
                    spawn_loot(&mut commands, p.x, p.y, item_roll);
                    combat_log.push(format!("{label} dropped an item!"));
                }
            }

            commands.entity(entity).despawn();
        }
    }
}
