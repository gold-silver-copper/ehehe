use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use bevy::prelude::*;

use crate::components::{
    AiLookDir, AiMemory, AiPersonality, AiPursuitBoost, AiState, AiTarget, AimingStyle,
    BlocksMovement, Cursor, Energy, Faction, Health, Inventory, Item, ItemKind, PatrolOrigin,
    PlayerControlled, Position, SPELL_STAMINA_COST, Speed, Stamina, Viewshed,
};
use crate::events::{
    AttackIntent, MolotovCastIntent, MoveIntent, PickupItemIntent, RangedAttackIntent,
    SpellCastIntent, UseItemIntent,
};
use crate::grid_vec::GridVec;
use crate::resources::{GameMapResource, SpatialIndex, SpellParticles, TurnCounter};
use crate::typeenums::{Floor, Props};

mod cost {
    pub const BASE: i32 = 10;
    pub const FIRE: i32 = 70;
    pub const NEAR_FIRE: i32 = 28;
    pub const NEAR_CACTUS: i32 = 10;
    pub const SAND_CLOUD: i32 = 16;
    pub const COVER_PER_WALL: i32 = 2;
    pub const EXPOSED: i32 = 4;
}

const MAX_A_STAR_NODES: usize = 640;
const MAX_DIJKSTRA_NODES: usize = 640;

const MEMORY_DURATION: u32 = 28;
const TARGET_LOCK_TIMEOUT: u32 = 18;
const PURSUIT_AWARENESS_BOOST: i32 = 8;
const PURSUIT_BOOST_DECAY_TURNS: u32 = 3;
const PROXIMITY_OVERRIDE_RANGE: i32 = 4;
const ALLY_SHARE_RANGE: i32 = 24;
const PATROL_RADIUS: i32 = 18;
const LOOK_AROUND_BASE_INTERVAL: u32 = 10;
const LOOK_AROUND_DICE_RANGE: u32 = 5;
const MAX_SEARCH_SWEEPS: u8 = 2;
const FULL_ROTATION_STEPS: u8 = 8;
const STUCK_FLANK_TURNS: u8 = 2;
const ITEM_INTEREST_RANGE: i32 = 8;
const EXPLOSIVE_MIN_RANGE: i32 = 4;
const EXPLOSIVE_MAX_RANGE: i32 = 8;
const BOW_MAX_RANGE: i32 = 14;

const HASH_KNUTH: u64 = 2_654_435_761;
const HASH_MIX_A: u64 = 0xff51afd7ed558ccd;
const HASH_MIX_B: u64 = 0xc4ceb9fe1a85ec53;

#[derive(Clone, Copy)]
struct Threat {
    entity: Entity,
    pos: GridVec,
    score: i32,
}

#[derive(Clone, Copy)]
struct TrackedTarget {
    entity: Entity,
    pos: GridVec,
    visible: bool,
    locked: bool,
}

#[derive(Clone, Copy)]
struct WeaponProfile {
    min_range: i32,
    preferred_range: i32,
    max_range: i32,
}

#[derive(Clone, Copy)]
struct GunChoice {
    entity: Entity,
    profile: WeaponProfile,
}

#[derive(Clone, Copy)]
struct BowChoice {
    attack: i32,
    profile: WeaponProfile,
}

#[derive(Clone, Copy)]
struct ExplosiveChoice {
    index: usize,
    damage: i32,
    radius: i32,
}

#[derive(Clone, Copy)]
struct HealChoice {
    index: usize,
    amount: i32,
}

#[derive(Default)]
struct InventoryTactics {
    loaded_gun: Option<GunChoice>,
    reloadable_gun: Option<GunChoice>,
    bow: Option<BowChoice>,
    heal: Option<HealChoice>,
    grenade: Option<ExplosiveChoice>,
    molotov: Option<ExplosiveChoice>,
}

#[inline]
fn hash64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(HASH_MIX_A);
    x ^= x >> 33;
    x = x.wrapping_mul(HASH_MIX_B);
    x ^= x >> 33;
    x
}

#[inline]
fn turn_hash(entity: Entity, turn: u32, salt: u64) -> u64 {
    hash64(entity.to_bits() ^ ((turn as u64) << 32) ^ salt)
}

#[inline]
fn look_interval(entity: Entity) -> u32 {
    LOOK_AROUND_BASE_INTERVAL + (entity.to_bits() as u32 % LOOK_AROUND_DICE_RANGE.max(1))
}

#[inline]
fn has_visibility(viewshed: Option<&Viewshed>, pos: GridVec) -> bool {
    viewshed.is_some_and(|vs| vs.visible_tiles.contains(&pos))
}

#[inline]
fn is_walkable_tile(game_map: &GameMapResource, pos: GridVec) -> bool {
    game_map.0.is_passable(&pos) && !game_map.0.is_water(&pos)
}

fn base_tile_cost(pos: GridVec, game_map: &GameMapResource) -> Option<i32> {
    if !is_walkable_tile(game_map, pos) {
        return None;
    }

    let mut value = cost::BASE;

    if let Some(voxel) = game_map.0.get_voxel_at(&pos) {
        match voxel.floor {
            Some(Floor::Fire) => value += cost::FIRE,
            Some(Floor::SandCloud) => value += cost::SAND_CLOUD,
            _ => {}
        }
    }

    let mut wall_count = 0;
    let mut near_fire = false;
    let mut near_cactus = false;

    for neighbor in pos.all_neighbors() {
        if let Some(voxel) = game_map.0.get_voxel_at(&neighbor) {
            if matches!(voxel.floor, Some(Floor::Fire)) {
                near_fire = true;
            }
            if matches!(voxel.props, Some(Props::Cactus)) {
                near_cactus = true;
            }
            if voxel
                .props
                .as_ref()
                .is_some_and(|prop| prop.blocks_movement())
            {
                wall_count += 1;
            }
        }
    }

    if near_fire {
        value += cost::NEAR_FIRE;
    }
    if near_cactus {
        value += cost::NEAR_CACTUS;
    }

    if wall_count > 0 {
        value -= (wall_count * cost::COVER_PER_WALL).min(value - 1);
    } else {
        value += cost::EXPOSED;
    }

    Some(value)
}

fn chase_tile_cost(pos: GridVec, game_map: &GameMapResource) -> Option<i32> {
    if !is_walkable_tile(game_map, pos) {
        return None;
    }

    let mut value = cost::BASE;

    if let Some(voxel) = game_map.0.get_voxel_at(&pos) {
        match voxel.floor {
            Some(Floor::Fire) => value += cost::FIRE,
            Some(Floor::SandCloud) => value += cost::SAND_CLOUD,
            _ => {}
        }
    }

    for neighbor in pos.all_neighbors() {
        if let Some(voxel) = game_map.0.get_voxel_at(&neighbor) {
            if matches!(voxel.floor, Some(Floor::Fire)) {
                value += cost::NEAR_FIRE;
            }
            if matches!(voxel.props, Some(Props::Cactus)) {
                value += cost::NEAR_CACTUS;
            }
        }
    }

    Some(value)
}

fn tile_cost_for_ai(
    pos: GridVec,
    self_entity: Entity,
    game_map: &GameMapResource,
    spatial: &SpatialIndex,
    blockers: &Query<(), With<BlocksMovement>>,
) -> Option<i32> {
    if spatial
        .entities_at(&pos)
        .iter()
        .any(|&entity| entity != self_entity && blockers.contains(entity))
    {
        return None;
    }
    base_tile_cost(pos, game_map)
}

fn tile_cost_for_ai_chase(
    pos: GridVec,
    self_entity: Entity,
    game_map: &GameMapResource,
    spatial: &SpatialIndex,
    blockers: &Query<(), With<BlocksMovement>>,
) -> Option<i32> {
    if spatial
        .entities_at(&pos)
        .iter()
        .any(|&entity| entity != self_entity && blockers.contains(entity))
    {
        return None;
    }
    chase_tile_cost(pos, game_map)
}

fn a_star_first_step(
    start: GridVec,
    goal: GridVec,
    cost_fn: impl Fn(GridVec) -> Option<i32>,
) -> Option<GridVec> {
    if start == goal {
        return None;
    }

    if start.chebyshev_distance(goal) == 1 && cost_fn(goal).is_some() {
        return Some(goal - start);
    }

    let mut open: BinaryHeap<Reverse<(i32, i32, GridVec)>> = BinaryHeap::new();
    let mut came_from: HashMap<GridVec, GridVec> = HashMap::new();
    let mut g_score: HashMap<GridVec, i32> = HashMap::new();
    let mut closed: HashSet<GridVec> = HashSet::new();

    let start_h = start.chebyshev_distance(goal) * cost::BASE;
    g_score.insert(start, 0);
    open.push(Reverse((start_h, start_h, start)));

    let mut explored = 0usize;

    while let Some(Reverse((_, _, current))) = open.pop() {
        if current == goal {
            let mut step = current;
            while let Some(&prev) = came_from.get(&step) {
                if prev == start {
                    return Some(step - start);
                }
                step = prev;
            }
            return None;
        }

        if !closed.insert(current) {
            continue;
        }

        explored += 1;
        if explored >= MAX_A_STAR_NODES {
            break;
        }

        let current_g = g_score[&current];

        for dir in GridVec::DIRECTIONS_8 {
            let neighbor = current + dir;
            if closed.contains(&neighbor) {
                continue;
            }

            let edge_cost = if neighbor == goal {
                cost::BASE
            } else {
                match cost_fn(neighbor) {
                    Some(cost) => cost,
                    None => continue,
                }
            };

            let candidate_g = current_g + edge_cost;
            if candidate_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                came_from.insert(neighbor, current);
                g_score.insert(neighbor, candidate_g);
                let h = neighbor.chebyshev_distance(goal) * cost::BASE;
                open.push(Reverse((candidate_g + h, h, neighbor)));
            }
        }
    }

    None
}

pub fn a_star_first_step_pub(
    start: GridVec,
    goal: GridVec,
    is_walkable: impl Fn(GridVec) -> bool,
) -> Option<GridVec> {
    a_star_first_step(start, goal, |pos| {
        if is_walkable(pos) {
            Some(cost::BASE)
        } else {
            None
        }
    })
}

fn dijkstra_map(
    sources: &[GridVec],
    cost_fn: impl Fn(GridVec) -> Option<i32>,
) -> HashMap<GridVec, i32> {
    let mut dist = HashMap::with_capacity(MAX_DIJKSTRA_NODES);
    let mut open: BinaryHeap<Reverse<(i32, GridVec)>> = BinaryHeap::new();

    for &source in sources {
        dist.insert(source, 0);
        open.push(Reverse((0, source)));
    }

    let mut explored = 0usize;

    while let Some(Reverse((current_dist, current))) = open.pop() {
        if current_dist > *dist.get(&current).unwrap_or(&i32::MAX) {
            continue;
        }

        explored += 1;
        if explored >= MAX_DIJKSTRA_NODES {
            break;
        }

        for dir in GridVec::DIRECTIONS_8 {
            let neighbor = current + dir;
            let edge_cost = match cost_fn(neighbor) {
                Some(cost) => cost,
                None => continue,
            };
            let next_dist = current_dist + edge_cost;
            if next_dist < *dist.get(&neighbor).unwrap_or(&i32::MAX) {
                dist.insert(neighbor, next_dist);
                open.push(Reverse((next_dist, neighbor)));
            }
        }
    }

    dist
}

fn gun_profile(name: &str, capacity: i32) -> WeaponProfile {
    let is_long_gun = capacity == 1
        || name.contains("Rifle")
        || name.contains("Springfield")
        || name.contains("Enfield")
        || name.contains("Hawken");

    if is_long_gun {
        WeaponProfile {
            min_range: 3,
            preferred_range: 8,
            max_range: 18,
        }
    } else {
        WeaponProfile {
            min_range: 2,
            preferred_range: 5,
            max_range: 12,
        }
    }
}

fn adjust_profile_for_style(
    profile: WeaponProfile,
    style: Option<AimingStyle>,
    personality: AiPersonality,
) -> WeaponProfile {
    let aggression_bias = if personality.aggression >= 0.85 { 1 } else { 0 };

    match style {
        Some(AimingStyle::CarefulAim) => WeaponProfile {
            min_range: (profile.min_range + 1).max(2),
            preferred_range: profile.preferred_range + 1,
            max_range: profile.max_range,
        },
        Some(AimingStyle::SnapShot) => WeaponProfile {
            min_range: (profile.min_range - 1).max(1),
            preferred_range: (profile.preferred_range - aggression_bias).max(2),
            max_range: profile.max_range + 1,
        },
        Some(AimingStyle::Suppression) => WeaponProfile {
            min_range: (profile.min_range - 1).max(1),
            preferred_range: profile.preferred_range,
            max_range: profile.max_range + 3,
        },
        None => profile,
    }
}

fn analyze_inventory(
    inventory: Option<&Inventory>,
    item_kinds: &mut Query<&mut ItemKind>,
) -> InventoryTactics {
    let Some(inventory) = inventory else {
        return InventoryTactics::default();
    };

    let mut tactics = InventoryTactics::default();

    for (index, &item_entity) in inventory.items.iter().enumerate() {
        let Ok(kind) = item_kinds.get_mut(item_entity) else {
            continue;
        };

        match &*kind {
            ItemKind::Gun {
                loaded,
                capacity,
                name,
                ..
            } => {
                let choice = GunChoice {
                    entity: item_entity,
                    profile: gun_profile(name, *capacity),
                };
                if *loaded > 0 && tactics.loaded_gun.is_none() {
                    tactics.loaded_gun = Some(choice);
                }
                if *loaded < *capacity && tactics.reloadable_gun.is_none() {
                    tactics.reloadable_gun = Some(choice);
                }
            }
            ItemKind::Bow { attack, .. } => {
                if tactics.bow.is_none() {
                    tactics.bow = Some(BowChoice {
                        attack: *attack,
                        profile: WeaponProfile {
                            min_range: 3,
                            preferred_range: 7,
                            max_range: BOW_MAX_RANGE,
                        },
                    });
                }
            }
            ItemKind::Grenade { damage, radius, .. } => {
                if tactics.grenade.is_none() {
                    tactics.grenade = Some(ExplosiveChoice {
                        index,
                        damage: *damage,
                        radius: *radius,
                    });
                }
            }
            ItemKind::Molotov { damage, radius, .. } => {
                if tactics.molotov.is_none() {
                    tactics.molotov = Some(ExplosiveChoice {
                        index,
                        damage: *damage,
                        radius: *radius,
                    });
                }
            }
            ItemKind::Whiskey { heal, .. }
            | ItemKind::Beer { heal, .. }
            | ItemKind::Ale { heal, .. }
            | ItemKind::Stout { heal, .. }
            | ItemKind::Wine { heal, .. }
            | ItemKind::Rum { heal, .. } => {
                let current_best = tactics.heal.map(|choice| choice.amount).unwrap_or(i32::MIN);
                if *heal > current_best {
                    tactics.heal = Some(HealChoice {
                        index,
                        amount: *heal,
                    });
                }
            }
            _ => {}
        }
    }

    tactics
}

fn should_flee(health: Option<&Health>, has_heal: bool, personality: AiPersonality) -> bool {
    let Some(health) = health else {
        return false;
    };
    if has_heal {
        return false;
    }

    let base_threshold = 12;
    let courage_modifier = (personality.courage * 8.0).round() as i32;
    let hp_threshold = (base_threshold - courage_modifier).clamp(4, 14);
    health.current <= hp_threshold || health.fraction() <= 0.18
}

fn heal_threshold(personality: AiPersonality) -> f64 {
    (0.52 - personality.courage * 0.22).clamp(0.24, 0.52)
}

fn effective_awareness_range(
    base_range: i32,
    pursuit_boost: Option<&AiPursuitBoost>,
    current_turn: u32,
) -> i32 {
    let bonus = pursuit_boost
        .map(|boost| {
            let unseen_turns = current_turn.saturating_sub(boost.last_spotted_turn);
            let decay = (unseen_turns / PURSUIT_BOOST_DECAY_TURNS) as i32;
            (boost.extra_range - decay).max(0)
        })
        .unwrap_or(0);

    base_range + bonus
}

fn threat_score(distance: i32) -> i32 {
    (30 - distance).max(0) * 6
}

fn choose_aiming_style(entity: Entity, turn: u32, personality: AiPersonality) -> AimingStyle {
    if personality.aggression >= 0.9 {
        return AimingStyle::SnapShot;
    }

    match turn_hash(entity, turn, HASH_KNUTH) % 3 {
        0 => AimingStyle::CarefulAim,
        1 => AimingStyle::SnapShot,
        _ => AimingStyle::Suppression,
    }
}

fn ensure_aiming_style(
    commands: &mut Commands,
    entity: Entity,
    current: Option<AimingStyle>,
    turn: u32,
    personality: AiPersonality,
    target_changed: bool,
) -> AimingStyle {
    if target_changed || current.is_none() {
        let style = choose_aiming_style(entity, turn, personality);
        commands.entity(entity).insert(style);
        style
    } else {
        current.expect("checked above")
    }
}

fn set_cursor(
    commands: &mut Commands,
    entity: Entity,
    cursor: &mut Option<Mut<Cursor>>,
    pos: GridVec,
) {
    if let Some(cursor) = cursor.as_deref_mut() {
        cursor.pos = pos;
    } else {
        commands.entity(entity).insert(Cursor { pos });
    }
}

fn clear_engagement(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AiTarget>();
    commands.entity(entity).remove::<AimingStyle>();
    commands.entity(entity).remove::<Cursor>();
    commands.entity(entity).remove::<AiPursuitBoost>();
}

fn update_look_dir(
    dir: GridVec,
    ai_look_dir: &mut Option<Mut<AiLookDir>>,
    viewshed: &mut Option<Mut<Viewshed>>,
) {
    if dir.is_zero() {
        return;
    }

    if let Some(look) = ai_look_dir.as_deref_mut() {
        look.0 = dir.king_step();
        look.1 = 0;
        if let Some(viewshed) = viewshed.as_deref_mut() {
            viewshed.dirty = true;
        }
    }
}

fn rotate_look_dir(ai_look_dir: &mut Option<Mut<AiLookDir>>, viewshed: &mut Option<Mut<Viewshed>>) {
    if let Some(look) = ai_look_dir.as_deref_mut() {
        let current = look.0.king_step();
        let index = GridVec::DIRECTIONS_8
            .iter()
            .position(|&dir| dir == current)
            .map(|idx| (idx + 1) % GridVec::DIRECTIONS_8.len())
            .unwrap_or(0);
        look.0 = GridVec::DIRECTIONS_8[index];
        look.1 = look.1.saturating_sub(1);
        if let Some(viewshed) = viewshed.as_deref_mut() {
            viewshed.dirty = true;
        }
    }
}

fn begin_circular_rotation(
    ai_look_dir: &mut Option<Mut<AiLookDir>>,
    viewshed: &mut Option<Mut<Viewshed>>,
) {
    if let Some(look) = ai_look_dir.as_deref_mut() {
        look.1 = FULL_ROTATION_STEPS;
    }
    rotate_look_dir(ai_look_dir, viewshed);
}

fn is_rotating(ai_look_dir: &Option<Mut<AiLookDir>>) -> bool {
    ai_look_dir.as_ref().is_some_and(|look| look.1 > 0)
}

fn has_clear_line_of_sight(
    origin: GridVec,
    target: GridVec,
    game_map: &GameMapResource,
    sand_clouds: &HashSet<GridVec>,
) -> bool {
    if origin == target {
        return true;
    }

    let line = origin.bresenham_line(target);
    for &tile in line.iter().skip(1) {
        if tile == target {
            return true;
        }
        if sand_clouds.contains(&tile) {
            return false;
        }
        match game_map.0.get_voxel_at(&tile) {
            Some(voxel) => {
                if matches!(voxel.floor, Some(Floor::SandCloud)) {
                    return false;
                }
                if voxel
                    .props
                    .as_ref()
                    .is_some_and(|prop| prop.blocks_vision())
                {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

fn has_friendly_in_path(
    origin: GridVec,
    target: GridVec,
    my_faction: Option<Faction>,
    self_entity: Entity,
    spatial: &SpatialIndex,
    npc_overview: &Query<
        (Entity, &Position, Option<&Faction>, Option<&AiTarget>),
        Without<PlayerControlled>,
    >,
) -> bool {
    let Some(my_faction) = my_faction else {
        return false;
    };

    let line = origin.bresenham_line(target);
    for &tile in line.iter().skip(1) {
        if tile == target {
            return false;
        }
        for &entity in spatial.entities_at(&tile) {
            if entity == self_entity {
                continue;
            }
            if let Ok((_, _, Some(faction), _)) = npc_overview.get(entity) {
                if !factions_are_hostile(my_faction, *faction) {
                    return true;
                }
            }
        }
    }

    false
}

fn count_hostiles_near(
    center: GridVec,
    radius: i32,
    my_faction: Option<Faction>,
    player_info: Option<(Entity, GridVec)>,
    npc_overview: &Query<
        (Entity, &Position, Option<&Faction>, Option<&AiTarget>),
        Without<PlayerControlled>,
    >,
) -> usize {
    let mut count = 0usize;

    if let Some((_, player_pos)) = player_info {
        if center.chebyshev_distance(player_pos) <= radius {
            count += 1;
        }
    }

    if let Some(my_faction) = my_faction {
        for (_, position, faction, _) in npc_overview {
            if faction.is_some_and(|other| factions_are_hostile(my_faction, *other))
                && center.chebyshev_distance(position.as_grid_vec()) <= radius
            {
                count += 1;
            }
        }
    }

    count
}

fn blast_hits_friendlies(
    center: GridVec,
    radius: i32,
    my_pos: GridVec,
    my_faction: Option<Faction>,
    self_entity: Entity,
    npc_overview: &Query<
        (Entity, &Position, Option<&Faction>, Option<&AiTarget>),
        Without<PlayerControlled>,
    >,
) -> bool {
    if my_pos.chebyshev_distance(center) <= radius + 1 {
        return true;
    }

    let Some(my_faction) = my_faction else {
        return false;
    };

    for (entity, position, faction, _) in npc_overview {
        if entity == self_entity {
            continue;
        }
        if faction.is_some_and(|other| !factions_are_hostile(my_faction, *other))
            && center.chebyshev_distance(position.as_grid_vec()) <= radius
        {
            return true;
        }
    }

    false
}

fn nearest_fire_threat(my_pos: GridVec, game_map: &GameMapResource) -> Option<GridVec> {
    let mut fire_sum = GridVec::ZERO;
    let mut fire_count = 0i32;

    if game_map
        .0
        .get_voxel_at(&my_pos)
        .is_some_and(|voxel| matches!(voxel.floor, Some(Floor::Fire)))
    {
        fire_sum += my_pos;
        fire_count += 1;
    }

    for neighbor in my_pos.all_neighbors() {
        if game_map
            .0
            .get_voxel_at(&neighbor)
            .is_some_and(|voxel| matches!(voxel.floor, Some(Floor::Fire)))
        {
            fire_sum += neighbor;
            fire_count += 1;
        }
    }

    if fire_count == 0 {
        None
    } else {
        Some(GridVec::new(
            fire_sum.x / fire_count,
            fire_sum.y / fire_count,
        ))
    }
}

fn flee_direction(
    my_pos: GridVec,
    threat_pos: GridVec,
    entity: Entity,
    game_map: &GameMapResource,
    spatial: &SpatialIndex,
    blockers: &Query<(), With<BlocksMovement>>,
) -> Option<GridVec> {
    let threat_map = dijkstra_map(&[threat_pos], |pos| {
        tile_cost_for_ai(pos, entity, game_map, spatial, blockers)
    });

    let my_distance = *threat_map.get(&my_pos).unwrap_or(&0);
    let mut best_direction = None;
    let mut best_score = i32::MIN;

    for dir in GridVec::DIRECTIONS_8 {
        let neighbor = my_pos + dir;
        let tile_cost = match tile_cost_for_ai(neighbor, entity, game_map, spatial, blockers) {
            Some(cost) => cost,
            None => continue,
        };
        let neighbor_distance = *threat_map
            .get(&neighbor)
            .unwrap_or(&(my_distance + cost::BASE * 4));
        let score = neighbor_distance * 2 - tile_cost;
        if score > best_score {
            best_score = score;
            best_direction = Some(dir);
        }
    }

    best_direction
}

fn retreat_direction(
    my_pos: GridVec,
    threat_pos: GridVec,
    entity: Entity,
    game_map: &GameMapResource,
    spatial: &SpatialIndex,
    blockers: &Query<(), With<BlocksMovement>>,
    sand_clouds: &HashSet<GridVec>,
    preserve_los: bool,
) -> Option<GridVec> {
    let mut best_direction = None;
    let mut best_score = i32::MIN;

    for dir in GridVec::DIRECTIONS_8 {
        let candidate = my_pos + dir;
        let tile_cost = match tile_cost_for_ai(candidate, entity, game_map, spatial, blockers) {
            Some(cost) => cost,
            None => continue,
        };
        if preserve_los && !has_clear_line_of_sight(candidate, threat_pos, game_map, sand_clouds) {
            continue;
        }

        let distance_score = candidate.chebyshev_distance(threat_pos) * 20;
        let score = distance_score - tile_cost;
        if score > best_score {
            best_score = score;
            best_direction = Some(dir);
        }
    }

    best_direction
}

fn first_step_toward(
    start: GridVec,
    goal: GridVec,
    entity: Entity,
    game_map: &GameMapResource,
    spatial: &SpatialIndex,
    blockers: &Query<(), With<BlocksMovement>>,
    aggressive: bool,
) -> Option<GridVec> {
    let step = if aggressive {
        a_star_first_step(start, goal, |pos| {
            tile_cost_for_ai_chase(pos, entity, game_map, spatial, blockers)
        })
    } else {
        a_star_first_step(start, goal, |pos| {
            tile_cost_for_ai(pos, entity, game_map, spatial, blockers)
        })
    };

    step.or_else(|| {
        let direct = (goal - start).king_step();
        if direct.is_zero() {
            None
        } else {
            let next = start + direct;
            if (if aggressive {
                tile_cost_for_ai_chase(next, entity, game_map, spatial, blockers)
            } else {
                tile_cost_for_ai(next, entity, game_map, spatial, blockers)
            })
            .is_some()
            {
                Some(direct)
            } else {
                None
            }
        }
    })
}

fn flank_goal(entity: Entity, turn: u32, my_pos: GridVec, target_pos: GridVec) -> GridVec {
    let bearing = (target_pos - my_pos).king_step();
    if bearing.is_zero() {
        return target_pos;
    }

    let left = target_pos + bearing.rotate_90_ccw() * 2;
    let right = target_pos + bearing.rotate_90_cw() * 2;
    if turn_hash(entity, turn, 0xF1A4_9A73) & 1 == 0 {
        left
    } else {
        right
    }
}

fn patrol_direction(
    entity: Entity,
    turn: u32,
    my_pos: GridVec,
    origin: GridVec,
    game_map: &GameMapResource,
    spatial: &SpatialIndex,
    blockers: &Query<(), With<BlocksMovement>>,
) -> Option<GridVec> {
    if my_pos.chebyshev_distance(origin) > PATROL_RADIUS {
        return first_step_toward(my_pos, origin, entity, game_map, spatial, blockers, false);
    }

    let mut best_direction = None;
    let mut best_score = i32::MIN;

    for (index, dir) in GridVec::DIRECTIONS_8.iter().copied().enumerate() {
        let candidate = my_pos + dir;
        let tile_cost = match tile_cost_for_ai(candidate, entity, game_map, spatial, blockers) {
            Some(cost) => cost,
            None => continue,
        };
        let radius = candidate.chebyshev_distance(origin);
        if radius > PATROL_RADIUS {
            continue;
        }
        let noise = (turn_hash(entity, turn, index as u64 + 0x9E37) % 13) as i32;
        let score = noise * 3 - tile_cost - radius * 2;
        if score > best_score {
            best_score = score;
            best_direction = Some(dir);
        }
    }

    best_direction
}

fn live_target_position(
    target: Entity,
    player_entity: Option<Entity>,
    player_info: Option<(Entity, GridVec)>,
    npc_overview: &Query<
        (Entity, &Position, Option<&Faction>, Option<&AiTarget>),
        Without<PlayerControlled>,
    >,
) -> Option<GridVec> {
    if player_info.is_some_and(|(entity, _)| entity == target) {
        return player_info.map(|(_, pos)| pos);
    }
    if player_entity == Some(target) {
        return None;
    }
    npc_overview
        .get(target)
        .ok()
        .map(|(_, position, _, _)| position.as_grid_vec())
}

fn memory_goal(memory: Option<&AiMemory>, current_turn: u32) -> Option<GridVec> {
    let memory = memory?;
    let goal = memory.last_known_pos?;
    if current_turn.saturating_sub(memory.last_seen_turn) <= MEMORY_DURATION {
        Some(goal)
    } else {
        None
    }
}

fn maybe_reload_gun(choice: Option<GunChoice>, item_kinds: &mut Query<&mut ItemKind>) -> bool {
    let Some(choice) = choice else {
        return false;
    };

    let Ok(mut kind) = item_kinds.get_mut(choice.entity) else {
        return false;
    };

    if let ItemKind::Gun {
        loaded, capacity, ..
    } = kind.as_mut()
    {
        if *loaded < *capacity {
            *loaded += 1;
            return true;
        }
    }

    false
}

fn collect_visible_hostiles(
    entity: Entity,
    my_pos: GridVec,
    my_faction: Option<Faction>,
    current_target: Option<Entity>,
    viewshed: Option<&Viewshed>,
    player_info: Option<(Entity, GridVec)>,
    npc_overview: &Query<
        (Entity, &Position, Option<&Faction>, Option<&AiTarget>),
        Without<PlayerControlled>,
    >,
) -> Vec<Threat> {
    let mut hostiles = Vec::new();

    if let Some((player_entity, player_pos)) = player_info {
        let distance = my_pos.chebyshev_distance(player_pos);
        if has_visibility(viewshed, player_pos) || distance <= PROXIMITY_OVERRIDE_RANGE {
            let mut score = threat_score(distance);
            if current_target == Some(player_entity) {
                score += 18;
            }
            hostiles.push(Threat {
                entity: player_entity,
                pos: player_pos,
                score,
            });
        }
    }

    if let Some(my_faction) = my_faction {
        for (other_entity, other_position, other_faction, other_target) in npc_overview {
            if other_entity == entity {
                continue;
            }
            if !other_faction.is_some_and(|faction| factions_are_hostile(my_faction, *faction)) {
                continue;
            }

            let target_pos = other_position.as_grid_vec();
            let distance = my_pos.chebyshev_distance(target_pos);
            if !has_visibility(viewshed, target_pos) && distance > PROXIMITY_OVERRIDE_RANGE {
                continue;
            }

            let mut score = threat_score(distance);
            if current_target == Some(other_entity) {
                score += 18;
            }
            if other_target.is_some_and(|target| target.entity == entity) {
                score += 24;
            }

            hostiles.push(Threat {
                entity: other_entity,
                pos: target_pos,
                score,
            });
        }
    }

    hostiles
}

pub fn factions_are_hostile(a: Faction, b: Faction) -> bool {
    a != b
}

pub fn ai_system(
    mut commands: Commands,
    mut ai_query: Query<
        (
            (
                Entity,
                &Position,
                &mut AiState,
                Option<&mut Viewshed>,
                &mut Energy,
                Option<&Faction>,
                Option<&mut AiLookDir>,
                Option<&PatrolOrigin>,
            ),
            (
                Option<&mut Inventory>,
                Option<&mut Health>,
                Option<&mut Stamina>,
                Option<&mut AiMemory>,
                Option<&AiPersonality>,
                Option<&AiTarget>,
                Option<&AimingStyle>,
                Option<&mut Cursor>,
            ),
            Option<&AiPursuitBoost>,
        ),
        Without<PlayerControlled>,
    >,
    player_query: Query<(Entity, &Position, &Health), With<PlayerControlled>>,
    npc_overview: Query<
        (Entity, &Position, Option<&Faction>, Option<&AiTarget>),
        Without<PlayerControlled>,
    >,
    floor_items: Query<(Entity, &Position), With<Item>>,
    (game_map, spatial, turn_counter, spell_particles): (
        Res<GameMapResource>,
        Res<SpatialIndex>,
        Res<TurnCounter>,
        Res<SpellParticles>,
    ),
    blockers: Query<(), With<BlocksMovement>>,
    mut item_kinds: Query<&mut ItemKind>,
    (mut move_intents, mut attack_intents, mut ranged_intents): (
        MessageWriter<MoveIntent>,
        MessageWriter<AttackIntent>,
        MessageWriter<RangedAttackIntent>,
    ),
    (mut spell_intents, mut molotov_intents, mut use_item_intents, mut pickup_intents): (
        MessageWriter<SpellCastIntent>,
        MessageWriter<MolotovCastIntent>,
        MessageWriter<UseItemIntent>,
        MessageWriter<PickupItemIntent>,
    ),
) {
    let player_data = player_query
        .single()
        .ok()
        .map(|(entity, position, health)| (entity, position.as_grid_vec(), !health.is_dead()));
    let player_entity = player_data.map(|(entity, _, _)| entity);
    let player_info =
        player_data.and_then(|(entity, position, alive)| alive.then_some((entity, position)));

    let sand_cloud_tiles: HashSet<GridVec> = spell_particles
        .particles
        .iter()
        .filter(|(_, life, delay, _, _, _)| *delay == 0 && *life > 0)
        .map(|(pos, _, _, _, _, _)| *pos)
        .collect();

    let current_turn = turn_counter.0;

    let mut faction_alerts: HashMap<Faction, Vec<GridVec>> = HashMap::new();
    for (
        (entity, position, ai_state, _viewshed, _energy, faction, _look_dir, _origin),
        (_inventory, _health, _stamina, ai_memory, _personality, ai_target, _aiming_style, _cursor),
        _pursuit_boost,
    ) in &mut ai_query
    {
        let Some(faction) = faction.copied() else {
            continue;
        };
        if !matches!(*ai_state, AiState::Chasing | AiState::Fleeing) {
            continue;
        }

        let target_from_component = ai_target.and_then(|target| {
            if player_entity == Some(target.entity) && player_info.is_none() {
                None
            } else {
                Some(target.last_pos)
            }
        });
        let target_from_memory = memory_goal(ai_memory.as_deref(), current_turn);
        let fallback = Some(position.as_grid_vec());

        if let Some(alert_pos) = target_from_component.or(target_from_memory).or(fallback) {
            let _ = entity;
            faction_alerts.entry(faction).or_default().push(alert_pos);
        }
    }

    for (
        (
            entity,
            position,
            mut ai_state,
            mut viewshed,
            mut energy,
            faction,
            mut ai_look_dir,
            patrol_origin,
        ),
        (
            inventory,
            health,
            stamina,
            mut ai_memory,
            personality,
            ai_target,
            aiming_style,
            mut cursor,
        ),
        pursuit_boost,
    ) in &mut ai_query
    {
        if !energy.can_act() {
            continue;
        }

        let my_pos = position.as_grid_vec();
        let my_faction = faction.copied();
        let personality = personality.copied().unwrap_or_default();
        let viewshed_ref = viewshed.as_deref();

        if let Some(memory) = ai_memory.as_deref_mut() {
            if memory.prev_pos == Some(my_pos) {
                memory.stationary_turns = memory.stationary_turns.saturating_add(1);
            } else {
                memory.stationary_turns = 0;
            }
            memory.prev_pos = Some(my_pos);
        }

        let inventory_view = inventory.as_deref();
        let tactics = analyze_inventory(inventory_view, &mut item_kinds);
        let has_heal = tactics.heal.is_some();

        if player_info.is_none()
            && player_entity
                .is_some_and(|player| ai_target.is_some_and(|target| target.entity == player))
        {
            if let Some(memory) = ai_memory.as_deref_mut() {
                memory.last_known_pos = None;
                memory.search_attempts = 0;
            }
            clear_engagement(&mut commands, entity);
        }

        let current_target_entity = ai_target.map(|target| target.entity);
        let mut visible_hostiles = collect_visible_hostiles(
            entity,
            my_pos,
            my_faction,
            current_target_entity,
            viewshed_ref,
            player_info,
            &npc_overview,
        );
        visible_hostiles.sort_by_key(|threat| threat.score);
        let best_visible = visible_hostiles.last().copied();

        let viewshed_range = viewshed_ref
            .map(|viewshed| viewshed.range as i32)
            .unwrap_or(8);
        let awareness = effective_awareness_range(viewshed_range * 2, pursuit_boost, current_turn);

        let mut target = best_visible.map(|threat| TrackedTarget {
            entity: threat.entity,
            pos: threat.pos,
            visible: true,
            locked: ai_target
                .is_some_and(|current| current.entity == threat.entity && current.locked),
        });

        if target.is_none() {
            if let Some(current_target) = ai_target {
                let target_timeout = if current_target.locked {
                    TARGET_LOCK_TIMEOUT
                } else {
                    MEMORY_DURATION
                };
                let target_position = live_target_position(
                    current_target.entity,
                    player_entity,
                    player_info,
                    &npc_overview,
                );
                let still_valid = target_position.is_some()
                    && current_turn.saturating_sub(current_target.last_seen) <= target_timeout
                    && (current_target.locked
                        || my_pos.chebyshev_distance(current_target.last_pos) <= awareness);
                if still_valid {
                    target = Some(TrackedTarget {
                        entity: current_target.entity,
                        pos: current_target.last_pos,
                        visible: false,
                        locked: current_target.locked,
                    });
                } else {
                    commands.entity(entity).remove::<AiTarget>();
                    commands.entity(entity).remove::<AiPursuitBoost>();
                }
            }
        }

        let mut objective = target
            .map(|target| target.pos)
            .or_else(|| memory_goal(ai_memory.as_deref(), current_turn));

        if objective.is_none() {
            if let Some(my_faction) = my_faction {
                if let Some(alerts) = faction_alerts.get(&my_faction) {
                    if let Some(alert_pos) = alerts
                        .iter()
                        .copied()
                        .filter(|alert| my_pos.chebyshev_distance(*alert) <= ALLY_SHARE_RANGE)
                        .min_by_key(|alert| my_pos.chebyshev_distance(*alert))
                    {
                        objective = Some(alert_pos);
                        if let Some(memory) = ai_memory.as_deref_mut() {
                            memory.last_known_pos = Some(alert_pos);
                            memory.last_seen_turn = current_turn;
                            memory.search_attempts = 0;
                        }
                    }
                }
            }
        }

        let mut current_style = aiming_style.copied();

        if let Some(target) = target {
            let target_changed = current_target_entity != Some(target.entity);
            current_style = Some(ensure_aiming_style(
                &mut commands,
                entity,
                current_style,
                current_turn,
                personality,
                target_changed,
            ));
            set_cursor(&mut commands, entity, &mut cursor, target.pos);

            let last_seen = if target.visible {
                current_turn
            } else {
                ai_target
                    .map(|current| current.last_seen)
                    .unwrap_or(current_turn)
            };
            commands.entity(entity).insert(AiTarget {
                entity: target.entity,
                last_pos: target.pos,
                last_seen,
                locked: target.locked,
            });

            if let Some(memory) = ai_memory.as_deref_mut() {
                memory.last_known_pos = Some(target.pos);
                if target.visible {
                    memory.last_seen_turn = current_turn;
                }
                memory.search_attempts = 0;
            }

            if target.visible {
                commands.entity(entity).insert(AiPursuitBoost {
                    extra_range: PURSUIT_AWARENESS_BOOST,
                    last_spotted_turn: current_turn,
                });
            }
        } else if let Some(goal) = objective {
            set_cursor(&mut commands, entity, &mut cursor, goal);
            commands.entity(entity).remove::<AiTarget>();
            commands.entity(entity).remove::<AiPursuitBoost>();
        } else {
            clear_engagement(&mut commands, entity);
        }

        if let Some(fire_center) = nearest_fire_threat(my_pos, &game_map) {
            if let Some(dir) =
                flee_direction(my_pos, fire_center, entity, &game_map, &spatial, &blockers)
            {
                move_intents.write(MoveIntent {
                    entity,
                    dx: dir.x,
                    dy: dir.y,
                });
                update_look_dir(dir, &mut ai_look_dir, &mut viewshed);
                energy.spend_action();
                continue;
            }
        }

        let health_ref = health.as_deref();
        if let (Some(health), Some(heal)) = (health_ref, tactics.heal) {
            if health.fraction() <= heal_threshold(personality) {
                let _ = heal.amount;
                use_item_intents.write(UseItemIntent {
                    user: entity,
                    item_index: heal.index,
                });
                energy.spend_action();
                continue;
            }
        }

        let should_retreat = should_flee(health_ref, has_heal, personality);
        if should_retreat && objective.is_some() {
            *ai_state = AiState::Fleeing;
        } else if objective.is_some() {
            *ai_state = AiState::Chasing;
        } else {
            *ai_state = AiState::Patrolling;
        }

        if inventory_view.is_some_and(|inv| inv.items.len() < 9) && objective.is_none() {
            if spatial
                .entities_at(&my_pos)
                .iter()
                .any(|&item| floor_items.get(item).is_ok())
            {
                pickup_intents.write(PickupItemIntent { picker: entity });
                energy.spend_action();
                continue;
            }
        }

        match *ai_state {
            AiState::Fleeing => {
                if let Some(goal) = objective {
                    if let Some(dir) =
                        flee_direction(my_pos, goal, entity, &game_map, &spatial, &blockers)
                    {
                        move_intents.write(MoveIntent {
                            entity,
                            dx: dir.x,
                            dy: dir.y,
                        });
                        update_look_dir(dir, &mut ai_look_dir, &mut viewshed);
                        energy.spend_action();
                        continue;
                    }
                }

                *ai_state = AiState::Patrolling;
            }
            AiState::Chasing => {
                if let Some(target) = target {
                    let mut target_pos = target.pos;
                    if target.visible {
                        target_pos = target.pos;
                    }
                    let to_target = target_pos - my_pos;
                    let distance = my_pos.chebyshev_distance(target_pos);
                    let facing = to_target.king_step();
                    if !facing.is_zero() {
                        update_look_dir(facing, &mut ai_look_dir, &mut viewshed);
                    }

                    if distance <= 1 && target.visible {
                        attack_intents.write(AttackIntent {
                            attacker: entity,
                            target: target.entity,
                        });
                        commands.entity(entity).insert(AiTarget {
                            entity: target.entity,
                            last_pos: target_pos,
                            last_seen: current_turn,
                            locked: true,
                        });
                        energy.spend_action();
                        continue;
                    }

                    let style = current_style;
                    let gun = tactics.loaded_gun.map(|choice| GunChoice {
                        entity: choice.entity,
                        profile: adjust_profile_for_style(choice.profile, style, personality),
                    });
                    let bow = tactics.bow.map(|choice| BowChoice {
                        attack: choice.attack,
                        profile: adjust_profile_for_style(choice.profile, style, personality),
                    });
                    let has_los =
                        has_clear_line_of_sight(my_pos, target_pos, &game_map, &sand_cloud_tiles);
                    let friendly_blocked = has_friendly_in_path(
                        my_pos,
                        target_pos,
                        my_faction,
                        entity,
                        &spatial,
                        &npc_overview,
                    );

                    let stamina_ok = stamina
                        .as_deref()
                        .is_some_and(|stamina| stamina.current >= SPELL_STAMINA_COST);
                    let cluster = count_hostiles_near(
                        target_pos,
                        tactics
                            .grenade
                            .map(|choice| choice.radius)
                            .or_else(|| tactics.molotov.map(|choice| choice.radius))
                            .unwrap_or(2),
                        my_faction,
                        player_info,
                        &npc_overview,
                    );

                    if target.visible && stamina_ok && has_los {
                        if let Some(grenade) = tactics.grenade {
                            if (EXPLOSIVE_MIN_RANGE..=EXPLOSIVE_MAX_RANGE).contains(&distance)
                                && cluster >= 2
                                && !blast_hits_friendlies(
                                    target_pos,
                                    grenade.radius,
                                    my_pos,
                                    my_faction,
                                    entity,
                                    &npc_overview,
                                )
                            {
                                let _ = grenade.damage;
                                spell_intents.write(SpellCastIntent {
                                    caster: entity,
                                    radius: grenade.radius,
                                    target: target_pos,
                                    grenade_index: grenade.index,
                                });
                                commands.entity(entity).insert(AiTarget {
                                    entity: target.entity,
                                    last_pos: target_pos,
                                    last_seen: current_turn,
                                    locked: true,
                                });
                                energy.spend_action();
                                continue;
                            }
                        } else if let Some(molotov) = tactics.molotov {
                            if (EXPLOSIVE_MIN_RANGE..=EXPLOSIVE_MAX_RANGE).contains(&distance)
                                && cluster >= 2
                                && !blast_hits_friendlies(
                                    target_pos,
                                    molotov.radius,
                                    my_pos,
                                    my_faction,
                                    entity,
                                    &npc_overview,
                                )
                            {
                                molotov_intents.write(MolotovCastIntent {
                                    caster: entity,
                                    radius: molotov.radius,
                                    damage: molotov.damage,
                                    target: target_pos,
                                    item_index: molotov.index,
                                });
                                commands.entity(entity).insert(AiTarget {
                                    entity: target.entity,
                                    last_pos: target_pos,
                                    last_seen: current_turn,
                                    locked: true,
                                });
                                energy.spend_action();
                                continue;
                            }
                        }
                    }

                    if let Some(gun) = gun {
                        let in_range =
                            distance >= gun.profile.min_range && distance <= gun.profile.max_range;
                        let can_blind_fire = matches!(style, Some(AimingStyle::Suppression))
                            && !target.visible
                            && ai_target.is_some_and(|current| {
                                current.entity == target.entity
                                    && current.locked
                                    && current_turn.saturating_sub(current.last_seen) <= 1
                            });
                        if in_range
                            && has_los
                            && !friendly_blocked
                            && (target.visible || can_blind_fire)
                        {
                            ranged_intents.write(RangedAttackIntent {
                                attacker: entity,
                                range: gun.profile.max_range,
                                dx: to_target.x,
                                dy: to_target.y,
                                gun_item: Some(gun.entity),
                            });
                            commands.entity(entity).insert(AiTarget {
                                entity: target.entity,
                                last_pos: target_pos,
                                last_seen: current_turn,
                                locked: true,
                            });
                            energy.spend_action();
                            continue;
                        }
                    }

                    if let Some(bow) = bow {
                        let in_range = target.visible
                            && has_los
                            && !friendly_blocked
                            && distance >= bow.profile.min_range
                            && distance <= bow.profile.max_range;
                        if in_range {
                            let max_component = to_target.x.abs().max(to_target.y.abs()).max(1);
                            let scale = bow.profile.max_range.div_euclid(max_component).max(1);
                            let endpoint =
                                my_pos + GridVec::new(to_target.x * scale, to_target.y * scale);
                            crate::systems::projectile::spawn_arrow(
                                &mut commands,
                                my_pos,
                                endpoint,
                                bow.attack,
                                entity,
                            );
                            commands.entity(entity).insert(AiTarget {
                                entity: target.entity,
                                last_pos: target_pos,
                                last_seen: current_turn,
                                locked: true,
                            });
                            energy.spend_action();
                            continue;
                        }
                    }

                    if tactics.loaded_gun.is_none()
                        && tactics.reloadable_gun.is_some()
                        && distance > 2
                        && (!target.visible || distance >= 4)
                        && maybe_reload_gun(tactics.reloadable_gun, &mut item_kinds)
                    {
                        energy.spend_action();
                        continue;
                    }

                    if let Some(profile) = gun
                        .map(|gun| gun.profile)
                        .or_else(|| bow.map(|bow| bow.profile))
                    {
                        if distance < profile.min_range && distance > 1 {
                            if let Some(dir) = retreat_direction(
                                my_pos,
                                target_pos,
                                entity,
                                &game_map,
                                &spatial,
                                &blockers,
                                &sand_cloud_tiles,
                                target.visible,
                            ) {
                                move_intents.write(MoveIntent {
                                    entity,
                                    dx: dir.x,
                                    dy: dir.y,
                                });
                                update_look_dir(dir, &mut ai_look_dir, &mut viewshed);
                                energy.spend_action();
                                continue;
                            }
                        }
                    }

                    let flank_now = ai_memory
                        .as_deref()
                        .is_some_and(|memory| memory.stationary_turns >= STUCK_FLANK_TURNS);
                    let goal = if flank_now || (target.visible && (!has_los || friendly_blocked)) {
                        flank_goal(entity, current_turn, my_pos, target_pos)
                    } else {
                        target_pos
                    };

                    if let Some(dir) = first_step_toward(
                        my_pos, goal, entity, &game_map, &spatial, &blockers, true,
                    ) {
                        move_intents.write(MoveIntent {
                            entity,
                            dx: dir.x,
                            dy: dir.y,
                        });
                        update_look_dir(dir, &mut ai_look_dir, &mut viewshed);
                        energy.spend_action();
                        continue;
                    }

                    energy.spend_action();
                    continue;
                }

                if let Some(goal) = objective {
                    if my_pos == goal {
                        if is_rotating(&ai_look_dir) {
                            rotate_look_dir(&mut ai_look_dir, &mut viewshed);
                            energy.spend_action();
                            continue;
                        }

                        if let Some(memory) = ai_memory.as_deref_mut() {
                            if memory.search_attempts < MAX_SEARCH_SWEEPS {
                                memory.search_attempts += 1;
                                begin_circular_rotation(&mut ai_look_dir, &mut viewshed);
                                energy.spend_action();
                                continue;
                            }

                            memory.last_known_pos = None;
                            memory.search_attempts = 0;
                        }

                        clear_engagement(&mut commands, entity);
                        *ai_state = AiState::Patrolling;
                    } else if let Some(dir) = first_step_toward(
                        my_pos, goal, entity, &game_map, &spatial, &blockers, false,
                    ) {
                        move_intents.write(MoveIntent {
                            entity,
                            dx: dir.x,
                            dy: dir.y,
                        });
                        update_look_dir(dir, &mut ai_look_dir, &mut viewshed);
                    }

                    energy.spend_action();
                    continue;
                }

                *ai_state = AiState::Patrolling;
            }
            AiState::Idle | AiState::Patrolling => {}
        }

        if tactics.reloadable_gun.is_some()
            && objective.is_none()
            && maybe_reload_gun(tactics.reloadable_gun, &mut item_kinds)
        {
            energy.spend_action();
            continue;
        }

        if objective.is_none() && inventory_view.is_some_and(|inv| inv.items.len() < 9) {
            if let Some(item_pos) = floor_items
                .iter()
                .map(|(_, position)| position.as_grid_vec())
                .filter(|item_pos| has_visibility(viewshed_ref, *item_pos))
                .filter(|item_pos| my_pos.chebyshev_distance(*item_pos) <= ITEM_INTEREST_RANGE)
                .min_by_key(|item_pos| my_pos.chebyshev_distance(*item_pos))
            {
                if item_pos == my_pos {
                    pickup_intents.write(PickupItemIntent { picker: entity });
                } else if let Some(dir) = first_step_toward(
                    my_pos, item_pos, entity, &game_map, &spatial, &blockers, false,
                ) {
                    move_intents.write(MoveIntent {
                        entity,
                        dx: dir.x,
                        dy: dir.y,
                    });
                    update_look_dir(dir, &mut ai_look_dir, &mut viewshed);
                }
                energy.spend_action();
                continue;
            }
        }

        if is_rotating(&ai_look_dir) {
            rotate_look_dir(&mut ai_look_dir, &mut viewshed);
            energy.spend_action();
            continue;
        }

        if current_turn % look_interval(entity) == 0 {
            rotate_look_dir(&mut ai_look_dir, &mut viewshed);
            energy.spend_action();
            continue;
        }

        if let Some(origin) = patrol_origin.map(|origin| origin.0) {
            if let Some(dir) = patrol_direction(
                entity,
                current_turn,
                my_pos,
                origin,
                &game_map,
                &spatial,
                &blockers,
            ) {
                move_intents.write(MoveIntent {
                    entity,
                    dx: dir.x,
                    dy: dir.y,
                });
                update_look_dir(dir, &mut ai_look_dir, &mut viewshed);
                energy.spend_action();
                continue;
            }
        }

        energy.spend_action();
    }
}

pub fn energy_accumulate_system(mut query: Query<(&Speed, &mut Energy)>) {
    for (speed, mut energy) in &mut query {
        energy.accumulate(speed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walkable(_: GridVec) -> bool {
        true
    }

    #[test]
    fn a_star_returns_adjacent_step() {
        let start = GridVec::new(4, 4);
        let goal = GridVec::new(5, 5);
        assert_eq!(
            a_star_first_step_pub(start, goal, walkable),
            Some(GridVec::new(1, 1))
        );
    }

    #[test]
    fn a_star_routes_around_wall() {
        let start = GridVec::new(0, 0);
        let goal = GridVec::new(3, 0);
        let walls = HashSet::from([GridVec::new(1, 0), GridVec::new(1, 1)]);
        let step = a_star_first_step_pub(start, goal, |pos| !walls.contains(&pos));
        assert_eq!(step, Some(GridVec::new(1, -1)));
    }

    #[test]
    fn pursuit_boost_decays_over_time() {
        let boost = AiPursuitBoost {
            extra_range: 8,
            last_spotted_turn: 10,
        };
        assert_eq!(effective_awareness_range(12, Some(&boost), 10), 20);
        assert_eq!(effective_awareness_range(12, Some(&boost), 16), 18);
        assert_eq!(effective_awareness_range(12, Some(&boost), 40), 12);
    }

    #[test]
    fn faction_hostility_is_strictly_cross_faction() {
        assert!(factions_are_hostile(Faction::Outlaws, Faction::Lawmen));
        assert!(!factions_are_hostile(Faction::Police, Faction::Police));
    }

    #[test]
    fn flee_triggers_without_healing() {
        let health = Health {
            current: 5,
            max: 20,
        };
        let personality = AiPersonality {
            aggression: 0.9,
            courage: 0.2,
        };
        assert!(should_flee(Some(&health), false, personality));
        assert!(!should_flee(Some(&health), true, personality));
    }
}
