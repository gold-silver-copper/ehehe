use std::collections::HashSet;

use bevy::prelude::*;

use crate::grid_vec::GridVec;
use crate::typedefs::{CoordinateUnit, MyPoint, RatColor};

/// World-grid position for any entity.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: CoordinateUnit,
    pub y: CoordinateUnit,
}

impl Position {
    /// Convert to a `GridVec` for vector arithmetic and distance calculations.
    #[inline]
    pub fn as_grid_vec(self) -> GridVec {
        GridVec::new(self.x, self.y)
    }
}

impl From<GridVec> for Position {
    #[inline]
    fn from(v: GridVec) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<Position> for GridVec {
    #[inline]
    fn from(p: Position) -> Self {
        GridVec::new(p.x, p.y)
    }
}

/// Marker component: tags the player-controlled entity.
#[derive(Component, Debug)]
pub struct Player;

/// Visual representation used when rendering an entity on the grid.
#[derive(Component, Clone, Debug)]
pub struct Renderable {
    pub symbol: String,
    pub fg: RatColor,
    pub bg: RatColor,
}

/// Marker component: the camera will follow entities that have this.
#[derive(Component, Debug)]
pub struct CameraFollow;

/// Marker component: entity occupies its tile and blocks movement.
#[derive(Component, Debug)]
pub struct BlocksMovement;

/// Field-of-view component. Attached to entities that "see" the world.
/// `visible_tiles` is recomputed by `visibility_system` when dirty.
/// `revealed_tiles` accumulates all tiles ever seen (fog of war memory).
#[derive(Component, Debug)]
pub struct Viewshed {
    /// Maximum sight range (in tiles).
    pub range: CoordinateUnit,
    /// Set of world-grid coordinates currently visible.
    pub visible_tiles: HashSet<MyPoint>,
    /// Set of world-grid coordinates that have been seen at least once.
    /// Used for fog-of-war: revealed tiles are drawn dimmed when not visible.
    pub revealed_tiles: HashSet<MyPoint>,
    /// Whether the viewshed needs recalculation (dirty flag).
    pub dirty: bool,
}

/// Health pool for any entity that can take damage or be healed.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: CoordinateUnit,
    pub max: CoordinateUnit,
}

/// Combat statistics used by the combat system to resolve attacks.
/// Damage dealt = max(0, attacker.attack − defender.defense).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CombatStats {
    pub attack: CoordinateUnit,
    pub defense: CoordinateUnit,
}

/// Display name for any entity. Used in combat messages, UI, and logs.
#[derive(Component, Clone, Debug)]
pub struct Name(pub String);

/// Movement speed: determines how much energy an entity gains each world tick.
///
/// In the energy-based turn model, an entity acts when its accumulated energy
/// reaches `ACTION_COST`. Higher speed → more energy per tick → more frequent
/// actions. A speed of 100 is the "normal" baseline (one action per tick).
///
/// The energy model is a discrete event scheduler:
///   turns_between_actions = ⌈ACTION_COST / speed⌉
///
/// This is the standard roguelike scheduling algorithm used by Angband, DCSS,
/// and Cogmind. It avoids floating-point entirely and provides exact fairness.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Speed(pub CoordinateUnit);

/// Accumulated action energy. When `energy >= ACTION_COST`, the entity may act.
///
/// After acting, energy is reduced by `ACTION_COST`. Excess energy carries
/// over, ensuring long-run fairness: over N ticks, an entity with speed S
/// takes exactly ⌊N × S / ACTION_COST⌋ actions.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Energy(pub CoordinateUnit);

/// The energy threshold required to perform one action.
/// Entities accumulate energy each tick equal to their `Speed` value.
/// When energy ≥ ACTION_COST, they may act and energy is reduced by ACTION_COST.
pub const ACTION_COST: CoordinateUnit = 100;

/// AI behaviour state for non-player entities.
///
/// The AI system reads this to decide what action to emit:
/// - `Idle`: stand still, wait for the player to enter sight range.
/// - `Chasing`: move toward the last known player position.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum AiState {
    /// Entity is stationary — has not seen the player yet.
    Idle,
    /// Entity is actively pursuing the player.
    Chasing,
}

/// Marker component: tags entities hostile to the player.
/// Used by bump-to-attack: moving into a hostile entity's tile triggers combat.
#[derive(Component, Debug)]
pub struct Hostile;
