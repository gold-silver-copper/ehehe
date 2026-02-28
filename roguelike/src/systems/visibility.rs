use std::collections::HashSet;

use bevy::prelude::*;

use crate::components::{Position, Viewshed};
use crate::resources::GameMapResource;
use crate::typedefs::{CoordinateUnit, MyPoint};

/// Recomputes the `visible_tiles` set for every entity whose `Viewshed` is
/// dirty (e.g., because the entity moved). Uses Bresenham line casting to
/// determine line-of-sight against the game map.
pub fn visibility_system(
    game_map: Res<GameMapResource>,
    mut query: Query<(&Position, &mut Viewshed)>,
) {
    for (pos, mut viewshed) in &mut query {
        if !viewshed.dirty {
            continue;
        }

        viewshed.visible_tiles.clear();
        let origin = (pos.x, pos.y);
        let range = viewshed.range;

        // Cast rays to every point on the perimeter of a square bounding the range.
        // This gives uniform coverage in all directions.
        for dx in -range..=range {
            cast_ray(&game_map, &mut viewshed.visible_tiles, origin, range, (origin.0 + dx, origin.1 - range));
            cast_ray(&game_map, &mut viewshed.visible_tiles, origin, range, (origin.0 + dx, origin.1 + range));
        }
        for dy in -range..=range {
            cast_ray(&game_map, &mut viewshed.visible_tiles, origin, range, (origin.0 - range, origin.1 + dy));
            cast_ray(&game_map, &mut viewshed.visible_tiles, origin, range, (origin.0 + range, origin.1 + dy));
        }

        // The origin itself is always visible.
        viewshed.visible_tiles.insert(origin);
        viewshed.dirty = false;
    }
}

/// Casts a single Bresenham ray from `origin` to `target`, adding each
/// traversed tile to `visible`. Stops when hitting an opaque tile (one with
/// furniture) or exceeding the range.
fn cast_ray(
    game_map: &GameMapResource,
    visible: &mut HashSet<MyPoint>,
    origin: MyPoint,
    range: CoordinateUnit,
    target: MyPoint,
) {
    let (mut x, mut y) = origin;
    let (tx, ty) = target;

    let dx = (tx - x).abs();
    let dy = -(ty - y).abs();
    let sx: CoordinateUnit = if x < tx { 1 } else { -1 };
    let sy: CoordinateUnit = if y < ty { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        // Out of range?
        let dist_sq = (x - origin.0) * (x - origin.0) + (y - origin.1) * (y - origin.1);
        if dist_sq > range * range {
            break;
        }

        visible.insert((x, y));

        // Check if this tile blocks sight (furniture is opaque, except the origin)
        if (x, y) != origin {
            if let Some(voxel) = game_map.0.get_voxel_at(&(x, y)) {
                if voxel.furniture.is_some() {
                    break; // can see the wall/tree but not past it
                }
            } else {
                break; // off map
            }
        }

        if x == tx && y == ty {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}
