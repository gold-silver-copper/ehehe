use bevy::prelude::*;

use crate::components::{Position, Viewshed};
use crate::events::MoveIntent;
use crate::resources::GameMapResource;

/// Processes `MoveIntent` events: checks the target tile on the `GameMap` for
/// walkability, then updates the entity's `Position` if the move is valid.
/// Also marks the entity's `Viewshed` as dirty so FOV is recalculated.
pub fn movement_system(
    mut intents: MessageReader<MoveIntent>,
    game_map: Res<GameMapResource>,
    mut movers: Query<(&mut Position, Option<&mut Viewshed>)>,
) {
    for intent in intents.read() {
        let Ok((mut pos, viewshed)) = movers.get_mut(intent.entity) else {
            continue;
        };

        let target_x = pos.x + intent.dx;
        let target_y = pos.y + intent.dy;

        // Check if the target tile is walkable (no blocking furniture)
        if let Some(voxel) = game_map.0.get_voxel_at(&(target_x, target_y))
            && voxel.furniture.is_none()
        {
            pos.x = target_x;
            pos.y = target_y;
            // Mark viewshed dirty so visibility is recalculated
            if let Some(mut vs) = viewshed {
                vs.dirty = true;
            }
        }
    }
}
