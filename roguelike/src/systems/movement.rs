use bevy::prelude::*;

use crate::components::{BlocksMovement, Hostile, Player, Position, Viewshed};
use crate::events::{AttackIntent, MoveIntent};
use crate::grid_vec::GridVec;
use crate::resources::{GameMapResource, SpatialIndex};

/// Processes `MoveIntent` events: checks the target tile on the `GameMap` for
/// walkability *and* the `SpatialIndex` for entities that block movement.
///
/// **Bump-to-attack**: if the target tile contains an entity the mover would
/// attack, emits an `AttackIntent` instead of moving. For the player, this
/// means walking into a `Hostile` entity. For `Hostile` entities, this means
/// walking into the `Player`. This is the standard roguelike mechanic where
/// walking into an enemy initiates melee combat.
///
/// Also marks the entity's `Viewshed` as dirty so FOV is recalculated.
pub fn movement_system(
    mut intents: MessageReader<MoveIntent>,
    game_map: Res<GameMapResource>,
    spatial: Res<SpatialIndex>,
    blockers: Query<(), With<BlocksMovement>>,
    hostiles: Query<(), With<Hostile>>,
    players: Query<(), With<Player>>,
    mut attack_intents: MessageWriter<AttackIntent>,
    mut movers: Query<(&mut Position, Option<&mut Viewshed>)>,
) {
    for intent in intents.read() {
        let Ok((mut pos, viewshed)) = movers.get_mut(intent.entity) else {
            continue;
        };

        let target = pos.as_grid_vec() + GridVec::new(intent.dx, intent.dy);

        // ── Bump-to-attack ──────────────────────────────────────
        // Check if a hostile entity occupies the target tile (player attacks monster).
        let hostile_at_target = spatial.entities_at(&target).iter().find(|&&e| {
            e != intent.entity && hostiles.contains(e)
        });
        if let Some(&target_entity) = hostile_at_target {
            attack_intents.write(AttackIntent {
                attacker: intent.entity,
                target: target_entity,
            });
            continue;
        }

        // Check if the player occupies the target tile (monster attacks player).
        let player_at_target = spatial.entities_at(&target).iter().find(|&&e| {
            e != intent.entity && players.contains(e)
        });
        if let Some(&target_entity) = player_at_target {
            if hostiles.contains(intent.entity) {
                attack_intents.write(AttackIntent {
                    attacker: intent.entity,
                    target: target_entity,
                });
                continue;
            }
        }

        // 1. Check map tile walkability (no blocking furniture).
        let tile_passable = game_map.0.is_passable(&target);

        // 2. Check spatial index for blocking entities at the target.
        let entity_blocked = spatial.entities_at(&target).iter().any(|&e| {
            e != intent.entity && blockers.contains(e)
        });

        if tile_passable && !entity_blocked {
            pos.x = target.x;
            pos.y = target.y;
            // Mark viewshed dirty so visibility is recalculated.
            if let Some(mut vs) = viewshed {
                vs.dirty = true;
            }
        }
    }
}
