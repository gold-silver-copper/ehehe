use bevy::prelude::*;

use crate::components::{BlocksMovement, Position, Viewshed};
use crate::events::MoveIntent;
use crate::resources::{GameMapResource, SpatialIndex};

/// Processes `MoveIntent` events: checks the target tile on the `GameMap` for
/// walkability *and* the `SpatialIndex` for entities that block movement,
/// then updates the entity's `Position` if the move is valid.
/// Also marks the entity's `Viewshed` as dirty so FOV is recalculated.
pub fn movement_system(
    mut intents: MessageReader<MoveIntent>,
    game_map: Res<GameMapResource>,
    spatial: Res<SpatialIndex>,
    blockers: Query<(), With<BlocksMovement>>,
    mut movers: Query<(&mut Position, Option<&mut Viewshed>)>,
) {
    for intent in intents.read() {
        let Ok((mut pos, viewshed)) = movers.get_mut(intent.entity) else {
            continue;
        };

        let target_x = pos.x + intent.dx;
        let target_y = pos.y + intent.dy;
        let target_point = (target_x, target_y);

        // 1. Check map tile walkability (no blocking furniture).
        let tile_passable = game_map
            .0
            .get_voxel_at(&target_point)
            .is_some_and(|v| v.furniture.is_none());

        // 2. Check spatial index for blocking entities at the target.
        let entity_blocked = spatial.entities_at(&target_point).iter().any(|&e| {
            e != intent.entity && blockers.contains(e)
        });

        if tile_passable && !entity_blocked {
            pos.x = target_x;
            pos.y = target_y;
            // Mark viewshed dirty so visibility is recalculated.
            if let Some(mut vs) = viewshed {
                vs.dirty = true;
            }
        }
    }
}
