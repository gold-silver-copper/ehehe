use bevy::prelude::*;

use crate::resources::TurnState;

/// Advances the turn state from `PlayerTurn` → `WorldTurn`.
/// Runs only during `TurnState::PlayerTurn` after all player-phase systems.
pub fn end_player_turn(mut next_state: ResMut<NextState<TurnState>>) {
    next_state.set(TurnState::WorldTurn);
}

/// Advances the turn state from `WorldTurn` → `AwaitingInput`.
/// Runs only during `TurnState::WorldTurn` after all world-phase systems.
pub fn end_world_turn(mut next_state: ResMut<NextState<TurnState>>) {
    next_state.set(TurnState::AwaitingInput);
}
