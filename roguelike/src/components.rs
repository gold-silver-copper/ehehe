use std::collections::HashSet;

use bevy::prelude::*;

use crate::typedefs::{CoordinateUnit, MyPoint, RatColor};

/// World-grid position for any entity.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: CoordinateUnit,
    pub y: CoordinateUnit,
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
