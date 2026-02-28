use crate::graphic_trait::GraphicElement;
use crate::typeenums::{Floor, Furniture};
use crate::typedefs::{GraphicTriple, MyPoint, RatColor};

/// A single cell in the game map grid.
#[derive(Clone, Debug)]
pub struct Voxel {
    pub floor: Option<Floor>,
    pub furniture: Option<Furniture>,
    pub voxel_pos: MyPoint,
}

impl Voxel {
    /// Converts this voxel to a GraphicTriple based on visibility.
    /// Layers: floor → furniture. Unseen tiles are dimmed.
    pub fn to_graphic(&self, visible: bool) -> GraphicTriple {
        let floor = match &self.floor {
            Some(fl) => fl.to_graphic_triple(),
            None => (" ".into(), RatColor::Black, RatColor::Black),
        };

        let plus_furn: GraphicTriple = match &self.furniture {
            Some(furn) => (furn.symbol(), furn.fg_color(), floor.2.clone()),
            None => floor,
        };

        if visible {
            plus_furn
        } else {
            let mut dimmed = plus_furn;
            dimmed.1 = dim(dimmed.1, 0.3);
            dimmed.2 = dim(dimmed.2, 0.5);
            dimmed
        }
    }
}

/// Dims a color by a factor. Clamps RGB values to 0..=127.
pub fn dim(color: RatColor, factor: f32) -> RatColor {
    match color {
        RatColor::Rgb(r, g, b) => RatColor::Rgb(
            ((r as f32 * factor).clamp(0.0, 127.0)) as u8,
            ((g as f32 * factor).clamp(0.0, 127.0)) as u8,
            ((b as f32 * factor).clamp(0.0, 127.0)) as u8,
        ),
        _ => RatColor::Gray,
    }
}
