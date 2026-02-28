use bevy::prelude::*;

use crate::components::{CombatStats, Health};
use crate::events::{AttackIntent, DamageEvent};

/// Resolves attack intents into damage events.
///
/// Damage = max(0, attacker.attack − target.defense).
/// Emits a `DamageEvent` for each successful hit.
pub fn combat_system(
    mut intents: MessageReader<AttackIntent>,
    mut damage_events: MessageWriter<DamageEvent>,
    stats_query: Query<&CombatStats>,
) {
    for intent in intents.read() {
        let Ok(attacker_stats) = stats_query.get(intent.attacker) else {
            continue;
        };
        let Ok(target_stats) = stats_query.get(intent.target) else {
            continue;
        };

        let damage = (attacker_stats.attack - target_stats.defense).max(0);
        if damage > 0 {
            damage_events.write(DamageEvent {
                target: intent.target,
                amount: damage,
            });
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
