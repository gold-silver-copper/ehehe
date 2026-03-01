use bevy::prelude::*;

use crate::components::{AiState, Energy, Player, Position, Speed, Viewshed, ACTION_COST};
use crate::events::MoveIntent;
use crate::grid_vec::GridVec;

/// AI system: runs during `WorldTurn` for every entity with an `AiState`.
///
/// **Behaviour**:
/// - **Idle**: if the player is within the entity's Viewshed, switch to `Chasing`.
/// - **Chasing**: move one step toward the player using greedy best-first
///   (minimise Chebyshev distance). This is O(1) per entity per turn —
///   no pathfinding overhead for simple melee enemies.
///
/// Emits `MoveIntent` just like the player's input system, so the same
/// movement/collision/bump-to-attack pipeline resolves NPC actions. This is
/// the core ECS composability guarantee: AI and player share identical
/// intent→action→consequence data flow.
pub fn ai_system(
    mut ai_query: Query<
        (Entity, &Position, &mut AiState, Option<&Viewshed>, &mut Energy),
        Without<Player>,
    >,
    player_query: Query<&Position, With<Player>>,
    mut move_intents: MessageWriter<MoveIntent>,
) {
    let Ok(player_pos) = player_query.single() else {
        return;
    };
    let player_vec = player_pos.as_grid_vec();

    for (entity, pos, mut ai, viewshed, mut energy) in &mut ai_query {
        // Only act if enough energy has accumulated.
        if energy.0 < ACTION_COST {
            continue;
        }

        let my_pos = pos.as_grid_vec();

        match *ai {
            AiState::Idle => {
                // Check if player is visible — no energy cost for looking.
                if let Some(vs) = viewshed {
                    if vs.visible_tiles.contains(&player_vec) {
                        *ai = AiState::Chasing;
                    }
                }
            }
            AiState::Chasing => {
                // Greedy best-first: pick the cardinal/diagonal neighbour
                // that minimises Chebyshev distance to the player.
                let delta = player_vec - my_pos;
                let step = GridVec::new(delta.x.signum(), delta.y.signum());

                if step != GridVec::ZERO {
                    move_intents.write(MoveIntent {
                        entity,
                        dx: step.x,
                        dy: step.y,
                    });
                    // Only deduct energy when an action is actually emitted.
                    energy.0 -= ACTION_COST;
                }
            }
        }
    }
}

/// Accumulates energy for all actors each world tick.
///
/// Energy accumulation follows the standard roguelike scheduling formula:
///   energy += speed
///
/// An entity with Speed(100) gains exactly `ACTION_COST` per tick (acts every
/// tick). Speed(50) → acts every 2 ticks. Speed(200) → acts twice per tick
/// (if the system processes multiple actions per tick).
///
/// This is a discrete-event scheduler that provides exact long-run fairness:
///   actions_over_N_ticks = ⌊N × speed / ACTION_COST⌋
pub fn energy_accumulate_system(mut query: Query<(&Speed, &mut Energy)>) {
    for (speed, mut energy) in &mut query {
        energy.0 += speed.0;
    }
}
