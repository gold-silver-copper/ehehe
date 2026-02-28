use crate::typedefs::{GraphicTriple, RatColor};
use crate::typeenums::{BlockingFurniture, Floor, Furniture, WalkableFurniture};
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
            Floor::GrassDeep => "\"".into(),
            Floor::Grass => "\"".into(),
            Floor::GrassLight => "\"".into(),
            Floor::Moss => ",".into(),
            Floor::LeafLitter => ".".into(),
        }
    }

    fn fg_color(&self) -> RatColor {
        match self {
            Floor::GrassDeep => RatColor::Rgb(17, 95, 24),
            Floor::Grass => RatColor::Rgb(20, 106, 27),
            Floor::GrassLight => RatColor::Rgb(22, 113, 29),
            Floor::Moss => RatColor::Rgb(28, 97, 32),
            Floor::LeafLitter => RatColor::Rgb(66, 74, 35),
        }
    }

    fn bg_color(&self) -> RatColor {
        dim(self.fg_color(), 0.8)
    }
}

impl GraphicElement for Furniture {
    fn symbol(&self) -> String {
        match self {
            Furniture::Blocking(BlockingFurniture::Wall) => "#".into(),
            Furniture::Blocking(BlockingFurniture::OakTree) => "♣".into(),
            Furniture::Blocking(BlockingFurniture::PineTree) => "♠".into(),
            Furniture::Blocking(BlockingFurniture::BirchTree) => "♤".into(),
            Furniture::Walkable(WalkableFurniture::Shrub) => "%".into(),
            Furniture::Walkable(WalkableFurniture::Fern) => "&".into(),
            Furniture::Walkable(WalkableFurniture::TallGrass) => ";".into(),
            Furniture::Walkable(WalkableFurniture::Wildflower) => "*".into(),
        }
    }

    fn fg_color(&self) -> RatColor {
        match self {
            Furniture::Blocking(BlockingFurniture::Wall) => RatColor::Rgb(139, 105, 20),
            Furniture::Blocking(BlockingFurniture::OakTree) => RatColor::Rgb(34, 120, 30),
            Furniture::Blocking(BlockingFurniture::PineTree) => RatColor::Rgb(28, 102, 36),
            Furniture::Blocking(BlockingFurniture::BirchTree) => RatColor::Rgb(170, 198, 128),
            Furniture::Walkable(WalkableFurniture::Shrub) => RatColor::Rgb(54, 132, 42),
            Furniture::Walkable(WalkableFurniture::Fern) => RatColor::Rgb(40, 122, 52),
            Furniture::Walkable(WalkableFurniture::TallGrass) => RatColor::Rgb(71, 145, 62),
            Furniture::Walkable(WalkableFurniture::Wildflower) => RatColor::Rgb(201, 172, 89),
        }
    }

    fn bg_color(&self) -> RatColor {
        dim(self.fg_color(), 0.8)
    }
}
