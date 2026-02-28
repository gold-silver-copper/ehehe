use crate::typeenums::{Floor, Furniture};
use crate::typedefs::{GraphicTriple, RatColor};
use crate::voxel::dim;

/// Trait for game elements that can be rendered as a GraphicTriple.
pub trait GraphicElement {
    fn symbol(&self) -> String;
    fn fg_color(&self) -> RatColor;
    fn bg_color(&self) -> RatColor;

    fn to_graphic_triple(&self) -> GraphicTriple {
        (self.symbol(), self.fg_color(), self.bg_color())
    }
}

impl GraphicElement for Floor {
    fn symbol(&self) -> String {
        match self {
            Floor::Gravel => ".".into(),
            Floor::Dirt => " ".into(),
            Floor::Grass => "\"".into(),
            Floor::Sand => ".".into(),
        }
    }

    fn fg_color(&self) -> RatColor {
        match self {
            Floor::Sand => RatColor::Rgb(234, 208, 168),
            Floor::Dirt => RatColor::Rgb(107, 84, 40),
            Floor::Gravel => RatColor::Rgb(97, 84, 65),
            Floor::Grass => RatColor::Rgb(19, 109, 21),
        }
    }

    fn bg_color(&self) -> RatColor {
        dim(self.fg_color(), 0.8)
    }
}

impl GraphicElement for Furniture {
    fn symbol(&self) -> String {
        match self {
            Furniture::Wall => "#".into(),
            Furniture::Tree => "♣".into(),
        }
    }

    fn fg_color(&self) -> RatColor {
        match self {
            Furniture::Wall => RatColor::Rgb(139, 105, 20),
            Furniture::Tree => RatColor::Rgb(205, 170, 125),
        }
    }

    fn bg_color(&self) -> RatColor {
        dim(self.fg_color(), 0.8)
    }
}
