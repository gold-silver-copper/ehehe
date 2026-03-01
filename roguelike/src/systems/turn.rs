use bevy::prelude::*;

use crate::components::{Mana, Player};
use crate::resources::{TurnCounter, TurnState};

/// Mana regenerated per world turn.
const MANA_REGEN_PER_TURN: i32 = 2;

/// Advances the turn state from `PlayerTurn` → `WorldTurn`.
/// Runs only during `TurnState::PlayerTurn` after all player-phase systems.
pub fn end_player_turn(mut next_state: ResMut<NextState<TurnState>>) {
    next_state.set(TurnState::WorldTurn);
}

/// Advances the turn state from `WorldTurn` → `AwaitingInput`.
/// Increments the turn counter each world turn, which drives wave spawning.
/// Also regenerates player mana each turn.
/// Runs only during `TurnState::WorldTurn` after all world-phase systems.
pub fn end_world_turn(
    mut next_state: ResMut<NextState<TurnState>>,
    mut turn_counter: ResMut<TurnCounter>,
    mut mana_query: Query<&mut Mana, With<Player>>,
) {
    turn_counter.0 += 1;

    // Regenerate player mana.
    if let Ok(mut mana) = mana_query.single_mut() {
        mana.current = (mana.current + MANA_REGEN_PER_TURN).min(mana.max);
    }

    next_state.set(TurnState::AwaitingInput);
}
