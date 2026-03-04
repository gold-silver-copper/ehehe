use bevy::prelude::*;

use crate::components::{CrimeType, Faction, Player, Position, Viewshed};
use crate::events::CrimeEvent;
use crate::resources::{CombatLog, StarLevel};

/// Distance within which an NPC must be to witness a crime.
const WITNESS_RANGE: i32 = 10;

/// Processes crime events and updates the wanted level.
///
/// A crime raises the wanted level only if it is witnessed by a law NPC
/// (Sheriff faction) or civilian NPC within range. Unwitnessed crimes in
/// the wilderness do not raise the wanted level.
///
/// This system integrates with the existing `StarLevel` resource and
/// `star_level_system` for decay and sheriff spawning.
pub fn crime_system(
    mut crime_events: MessageReader<CrimeEvent>,
    mut star_level: ResMut<StarLevel>,
    mut combat_log: ResMut<CombatLog>,
    npc_query: Query<(&Position, &Viewshed, Option<&Faction>), Without<Player>>,
) {
    for event in crime_events.read() {
        let crime_pos = event.position;

        // Check if any NPC witnessed the crime
        let mut witnessed = false;
        let mut law_witnessed = false;

        for (npc_pos, viewshed, faction) in &npc_query {
            let dist = npc_pos.as_grid_vec().chebyshev_distance(crime_pos);
            if dist > WITNESS_RANGE {
                continue;
            }

            // NPC must have LOS to the crime position
            if viewshed.visible_tiles.contains(&crime_pos) {
                witnessed = true;
                if matches!(faction, Some(Faction::Sheriff)) {
                    law_witnessed = true;
                }
            }
        }

        if witnessed {
            let increase = event.crime.wanted_increase();
            // Law witnesses escalate faster
            let actual_increase = if law_witnessed { increase + 1 } else { increase };
            star_level.level = (star_level.level + actual_increase).min(5);
            star_level.unseen_turns = 0;

            let crime_name = match event.crime {
                CrimeType::Assault => "assault",
                CrimeType::Murder => "murder",
                CrimeType::Arson => "arson",
                CrimeType::Theft => "theft",
                CrimeType::ShootingInTown => "shooting in town",
            };
            combat_log.push(format!(
                "Crime witnessed: {crime_name}! Wanted level: {}★",
                star_level.level
            ));
        }
    }
}
