use crate::typedefs::{CoordinateUnit, MyPoint, RenderPacket, SPAWN_X, SPAWN_Y, create_2d_array};
use crate::typeenums::{BlockingFurniture, Floor, Furniture, WalkableFurniture};
use crate::voxel::Voxel;

const NOISE_RANGE: u32 = 100;
const BLOCKING_FURNITURE_THRESHOLD: u32 = 8;
const WALKABLE_FURNITURE_THRESHOLD: u32 = 19;
const SPAWN_CLEAR_RADIUS: CoordinateUnit = 2;
// Hash constants for lightweight 2D coordinate mixing (deterministic pseudo-random layout).
const NOISE_X_PRIME: u64 = 73_856_093;
const NOISE_Y_PRIME: u64 = 19_349_663;
const NOISE_MIXER: u64 = 1_274_126_177;
const NOISE_FINALIZER: u32 = 2_654_435_761;

/// The game map: a simple 2D grid of voxels.
pub struct GameMap {
    pub width: CoordinateUnit,
    pub height: CoordinateUnit,
    pub voxels: Vec<Vec<Voxel>>,
}

impl GameMap {
    /// Creates a new game map filled with a simple pattern of floor and furniture tiles.
    pub fn new(width: CoordinateUnit, height: CoordinateUnit) -> Self {
        let mut voxels = Vec::with_capacity(height as usize);
        for y in 0..height {
            let mut row = Vec::with_capacity(width as usize);
            for x in 0..width {
                let noise = procedural_noise(x, y);
                let floor = Floor::from_noise(noise);
                let is_spawn_clear =
                    (x - SPAWN_X).abs() <= SPAWN_CLEAR_RADIUS && (y - SPAWN_Y).abs() <= SPAWN_CLEAR_RADIUS;

                let furniture = if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                    Some(Furniture::Blocking(BlockingFurniture::Wall))
                } else if is_spawn_clear {
                    None
                } else if noise % NOISE_RANGE < BLOCKING_FURNITURE_THRESHOLD {
                    Some(Furniture::Blocking(match noise % 3 {
                        0 => BlockingFurniture::OakTree,
                        1 => BlockingFurniture::PineTree,
                        _ => BlockingFurniture::BirchTree,
                    }))
                } else if noise % NOISE_RANGE < WALKABLE_FURNITURE_THRESHOLD {
                    Some(Furniture::Walkable(match noise % 4 {
                        0 => WalkableFurniture::Shrub,
                        1 => WalkableFurniture::Fern,
                        2 => WalkableFurniture::TallGrass,
                        _ => WalkableFurniture::Wildflower,
                    }))
                } else {
                    None
                };

                row.push(Voxel {
                    floor: Some(floor),
                    furniture,
                    voxel_pos: (x, y),
                });
            }
            voxels.push(row);
        }

        GameMap {
            width,
            height,
            voxels,
        }
    }

    /// Get a reference to the voxel at the given map coordinate.
    pub fn get_voxel_at(&self, point: &MyPoint) -> Option<&Voxel> {
        let (x, y) = *point;
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            Some(&self.voxels[y as usize][x as usize])
        } else {
            None
        }
    }

    /// Returns true when a map coordinate is in bounds and not blocked by impassable furniture.
    pub fn can_move_to(&self, point: &MyPoint) -> bool {
        self.get_voxel_at(point).is_some_and(|voxel| {
            !voxel
                .furniture
                .as_ref()
                .is_some_and(Furniture::blocks_movement)
        })
    }

    /// Creates a RenderPacket (2D grid of GraphicTriples) for display,
    /// centered on the given position with the given render dimensions.
    pub fn create_render_packet(
        &self,
        center: &MyPoint,
        render_width: u16,
        render_height: u16,
    ) -> RenderPacket {
        let w_radius = render_width as CoordinateUnit / 2;
        let h_radius = render_height as CoordinateUnit / 2;

        let bottom_left = (center.0 - w_radius, center.1 - h_radius);

        let mut grid = create_2d_array(render_width as usize, render_height as usize);

        for ry in 0..render_height as CoordinateUnit {
            for rx in 0..render_width as CoordinateUnit {
                let world_x = bottom_left.0 + rx;
                let world_y = bottom_left.1 + ry;

                if let Some(voxel) = self.get_voxel_at(&(world_x, world_y)) {
                    grid[ry as usize][rx as usize] = voxel.to_graphic(true);
                }
            }
        }

        grid
    }
}

/// Hash-based deterministic pseudo-noise used for procedural tile variation.
fn procedural_noise(x: CoordinateUnit, y: CoordinateUnit) -> u32 {
    let mut n = (x as u64).wrapping_mul(NOISE_X_PRIME) ^ (y as u64).wrapping_mul(NOISE_Y_PRIME);
    n = (n ^ (n >> 13)).wrapping_mul(NOISE_MIXER);
    (n as u32).wrapping_mul(NOISE_FINALIZER)
}

impl Default for GameMap {
    fn default() -> Self {
        GameMap::new(80, 50)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forest_has_diverse_tiles_and_open_ground() {
        let map = GameMap::new(120, 80);
        let mut blocking = 0;
        let mut walkable_plants = 0;
        let mut empty_ground = 0;
        let mut floor_variants = std::collections::HashSet::new();

        for row in &map.voxels {
            for voxel in row {
                if let Some(floor) = &voxel.floor {
                    floor_variants.insert(std::mem::discriminant(floor));
                }
                match &voxel.furniture {
                    Some(Furniture::Blocking(_)) => blocking += 1,
                    Some(Furniture::Walkable(_)) => walkable_plants += 1,
                    None => empty_ground += 1,
                }
            }
        }

        assert!(floor_variants.len() >= 4);
        assert!(blocking > 0);
        assert!(walkable_plants > 0);
        assert!(empty_ground > blocking);
    }

    #[test]
    fn movement_rules_match_furniture_blocking() {
        let map = GameMap::new(120, 80);
        assert!(map.can_move_to(&(SPAWN_X, SPAWN_Y)));
        assert!(!map.can_move_to(&(0, 0)));

        let walkable_plant = map
            .voxels
            .iter()
            .flatten()
            .find(|v| matches!(v.furniture, Some(Furniture::Walkable(_))))
            .map(|v| v.voxel_pos)
            .expect("expected at least one walkable plant");

        let tree = map
            .voxels
            .iter()
            .flatten()
            .find(|v| {
                matches!(
                    v.furniture,
                    Some(Furniture::Blocking(
                        BlockingFurniture::OakTree
                            | BlockingFurniture::PineTree
                            | BlockingFurniture::BirchTree
                    ))
                )
            })
            .map(|v| v.voxel_pos)
            .expect("expected at least one tree");

        assert!(map.can_move_to(&walkable_plant));
        assert!(!map.can_move_to(&tree));
    }

    #[test]
    fn procedural_noise_is_deterministic() {
        assert_eq!(procedural_noise(10, 20), procedural_noise(10, 20));
        assert_ne!(procedural_noise(10, 20), procedural_noise(11, 20));
    }
}
