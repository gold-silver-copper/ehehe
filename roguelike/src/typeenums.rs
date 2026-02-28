/// Floor tile types for the game map.
#[derive(Clone, Debug, PartialEq)]
pub enum Floor {
    GrassDeep,
    Grass,
    GrassLight,
    Moss,
    LeafLitter,
}

impl Floor {
    const NOISE_BUCKETS: u32 = 10;

    pub fn from_noise(noise: u32) -> Self {
        match noise % Self::NOISE_BUCKETS {
            0 => Floor::Moss,
            1 | 2 => Floor::GrassDeep,
            3..=7 => Floor::Grass,
            8 => Floor::GrassLight,
            _ => Floor::LeafLitter,
        }
    }
}

/// Furniture that blocks movement.
#[derive(Clone, Debug, PartialEq)]
pub enum BlockingFurniture {
    Wall,
    OakTree,
    PineTree,
    BirchTree,
}

/// Furniture that can be walked through.
#[derive(Clone, Debug, PartialEq)]
pub enum WalkableFurniture {
    Shrub,
    Fern,
    TallGrass,
    Wildflower,
}

/// Furniture (obstacles/structures) placed on tiles.
#[derive(Clone, Debug, PartialEq)]
pub enum Furniture {
    Blocking(BlockingFurniture),
    Walkable(WalkableFurniture),
}

impl Furniture {
    pub fn blocks_movement(&self) -> bool {
        matches!(self, Furniture::Blocking(_))
    }
}
