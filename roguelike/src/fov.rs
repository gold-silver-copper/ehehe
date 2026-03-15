// Albert Ford's symmetric shadowcasting: https://www.albertford.com/shadowcasting/
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fraction {
    pub numerator: i32,
    pub denominator: i32,
}

impl Fraction {
    pub const fn new(numerator: i32, denominator: i32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    fn cmp_fraction(&self, other: &Self) -> std::cmp::Ordering {
        let left = self.numerator as i64 * other.denominator as i64;
        let right = other.numerator as i64 * self.denominator as i64;
        left.cmp(&right)
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp_fraction(other))
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_fraction(other)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Row {
    depth: i32,
    start_slope: Fraction,
    end_slope: Fraction,
}

impl Row {
    fn new(depth: i32, start_slope: Fraction, end_slope: Fraction) -> Self {
        Self {
            depth,
            start_slope,
            end_slope,
        }
    }

    fn next(self) -> Self {
        Self::new(self.depth + 1, self.start_slope, self.end_slope)
    }

    fn slope(col: i32, depth: i32) -> Fraction {
        Fraction::new(2 * col - 1, 2 * depth)
    }

    fn round_ties_up(fraction: Fraction) -> i32 {
        (2 * fraction.numerator + fraction.denominator) / (2 * fraction.denominator)
    }

    fn round_ties_down(fraction: Fraction) -> i32 {
        (2 * fraction.numerator - fraction.denominator) / (2 * fraction.denominator)
    }

    fn col_range(&self) -> Option<(i32, i32)> {
        let start = Self::round_ties_up(Fraction::new(
            self.depth * self.start_slope.numerator,
            self.start_slope.denominator,
        ));
        let end = Self::round_ties_down(Fraction::new(
            self.depth * self.end_slope.numerator,
            self.end_slope.denominator,
        ));
        (start <= end).then_some((start, end))
    }

    fn is_symmetric(&self, col: i32) -> bool {
        col as i64 * self.start_slope.denominator as i64
            >= self.depth as i64 * self.start_slope.numerator as i64
            && col as i64 * self.end_slope.denominator as i64
                <= self.depth as i64 * self.end_slope.numerator as i64
    }
}

fn transform_octant((x, y): (i32, i32), octant: i32) -> (i32, i32) {
    match octant {
        0 => (x, y),
        1 => (y, x),
        2 => (-y, x),
        3 => (-x, y),
        4 => (-x, -y),
        5 => (-y, -x),
        6 => (y, -x),
        7 => (x, -y),
        _ => unreachable!("octant must be in [0, 8)"),
    }
}

pub fn compute_fov(
    origin: (i32, i32),
    radius: i32,
    is_opaque: impl Fn(i32, i32) -> bool,
) -> HashSet<(i32, i32)> {
    let mut visible = HashSet::new();
    visible.insert(origin);
    for octant in 0..8 {
        scan_octant(
            origin,
            radius,
            octant,
            &is_opaque,
            &mut visible,
            Row::new(1, Fraction::new(-1, 1), Fraction::new(1, 1)),
        );
    }
    visible
}

fn scan_octant(
    origin: (i32, i32),
    radius: i32,
    octant: i32,
    is_opaque: &impl Fn(i32, i32) -> bool,
    visible: &mut HashSet<(i32, i32)>,
    row: Row,
) {
    if row.depth > radius {
        return;
    }

    let Some((start_col, end_col)) = row.col_range() else {
        return;
    };

    let radius_sq = radius * radius;
    let mut previous_opaque: Option<bool> = None;
    let mut start_slope = row.start_slope;

    for col in start_col..=end_col {
        let (dx, dy) = transform_octant((col, row.depth), octant);
        let x = origin.0 + dx;
        let y = origin.1 + dy;

        let opaque = is_opaque(x, y);
        let symmetric = row.is_symmetric(col);
        if (opaque || symmetric) && (dx * dx + dy * dy <= radius_sq) {
            visible.insert((x, y));
        }

        if let Some(prev_opaque) = previous_opaque {
            if prev_opaque && !opaque {
                start_slope = Row::slope(col, row.depth);
            } else if !prev_opaque && opaque {
                scan_octant(
                    origin,
                    radius,
                    octant,
                    is_opaque,
                    visible,
                    Row::new(row.next().depth, start_slope, Row::slope(col, row.depth)),
                );
            }
        }

        previous_opaque = Some(opaque);
    }

    if previous_opaque == Some(false) {
        scan_octant(
            origin,
            radius,
            octant,
            is_opaque,
            visible,
            Row::new(row.next().depth, start_slope, row.end_slope),
        );
    }
}

pub fn has_los(from: (i32, i32), to: (i32, i32), is_opaque: impl Fn(i32, i32) -> bool) -> bool {
    let (mut x0, mut y0) = from;
    let (x1, y1) = to;
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    loop {
        if (x0, y0) == to {
            return true;
        }
        if (x0, y0) != from && is_opaque(x0, y0) {
            return false;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 < dx {
            err += dx;
            y0 += sy;
        }
    }
}

pub fn print_ascii_fov_demo() {
    let mut map = vec![vec![false; 20]; 20];
    for x in 2..18 {
        map[5][x] = true;
    }
    for row in map.iter_mut().take(18).skip(8) {
        row[11] = true;
    }
    let player = (10, 10);
    let visible = compute_fov(player, 8, |x, y| {
        if x < 0 || y < 0 || x >= 20 || y >= 20 {
            true
        } else {
            map[y as usize][x as usize]
        }
    });

    for y in (0..20).rev() {
        let mut line = String::with_capacity(20);
        for x in 0..20 {
            let ch = if (x, y) == player {
                '@'
            } else if map[y as usize][x as usize] {
                '#'
            } else if visible.contains(&(x, y)) {
                '.'
            } else {
                ' '
            };
            line.push(ch);
        }
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_visible() {
        let visible = compute_fov((5, 5), 5, |_, _| false);
        assert!(visible.contains(&(5, 5)));
    }

    #[test]
    fn visibility_is_symmetric_on_open_map() {
        let a = (5, 5);
        let b = (8, 7);
        let av = compute_fov(a, 10, |_, _| false);
        let bv = compute_fov(b, 10, |_, _| false);
        assert_eq!(av.contains(&b), bv.contains(&a));
    }

    #[test]
    fn wall_that_blocks_is_visible() {
        let visible = compute_fov((5, 5), 10, |x, y| (x, y) == (7, 5));
        assert!(visible.contains(&(7, 5)));
        assert!(!visible.contains(&(8, 5)));
    }

    #[test]
    fn los_respects_blockers() {
        assert!(has_los((0, 0), (5, 0), |_, _| false));
        assert!(!has_los((0, 0), (5, 0), |x, y| (x, y) == (3, 0)));
    }

    #[test]
    fn ascii_harness_smoke_test() {
        print_ascii_fov_demo();
    }
}
