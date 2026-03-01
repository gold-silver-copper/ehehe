use std::collections::HashSet;

use bevy::prelude::*;
use bevy_ratatui::RatatuiContext;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::components::{Health, Inventory, Mana, Name, Player, Position, Renderable, Viewshed};
use crate::grid_vec::GridVec;
use crate::resources::{
    CameraPosition, CombatLog, GameMapResource, GameState, HelpVisible, KillCount,
    SpellParticles, TurnCounter,
};
use crate::systems::input::KEYBINDINGS;
use crate::typedefs::{CoordinateUnit, MyPoint, RatColor};

/// Ticks and renders spell particles each frame.
pub fn particle_tick_system(mut particles: ResMut<SpellParticles>) {
    particles.tick();
}

/// Renders the game map and all `Renderable` entities to the terminal.
/// Uses the player's `Viewshed` to determine tile visibility, and the
/// `revealed_tiles` set for fog-of-war memory (dimmed rendering).
///
/// Layout:
/// ┌─────────────────────────────┬──────────────┐
/// │         Game Area           │  Side Panel   │
/// │                             │  (HP/Mana     │
/// │                             │   Inventory   │
/// │                             │   Visible)    │
/// ├─────────────────────────────┴──────────────┤
/// │  Status Bar                                 │
/// └─────────────────────────────────────────────┘
pub fn draw_system(
    mut context: ResMut<RatatuiContext>,
    game_map: Res<GameMapResource>,
    camera: Res<CameraPosition>,
    renderables: Query<(&Position, &Renderable, Option<&Name>)>,
    player_query: Query<
        (&Position, Option<&Viewshed>, Option<&Health>, Option<&Mana>, Option<&Inventory>),
        With<Player>,
    >,
    state: Res<State<GameState>>,
    combat_log: Res<CombatLog>,
    turn_counter: Res<TurnCounter>,
    kill_count: Res<KillCount>,
    help_visible: Res<HelpVisible>,
    spell_particles: Res<SpellParticles>,
) -> Result {
    context.draw(|frame| {
        let area = frame.area();

        // ── Top-level layout: main area + status bar (1 row) ────
        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let main_area = vert_chunks[0];
        let status_area = vert_chunks[1];

        // ── Main area: game viewport + side panel ───────────────
        let side_panel_width = 22u16;
        let horiz_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(side_panel_width),
            ])
            .split(main_area);

        let game_area = horiz_chunks[0];
        let side_area = horiz_chunks[1];

        let render_width = game_area.width;
        let render_height = game_area.height;

        // Collect the player's visible and revealed tiles.
        let (visible_tiles, revealed_tiles, player_hp, player_mana, player_inv): (
            Option<&HashSet<MyPoint>>,
            Option<&HashSet<MyPoint>>,
            Option<&Health>,
            Option<&Mana>,
            Option<&Inventory>,
        ) = player_query
            .single()
            .ok()
            .map(|(_, vs, hp, mp, inv)| {
                let (vis, rev) = vs
                    .map(|vs| (Some(&vs.visible_tiles), Some(&vs.revealed_tiles)))
                    .unwrap_or((None, None));
                (vis, rev, hp, mp, inv)
            })
            .unwrap_or((None, None, None, None, None));

        let mut render_packet = game_map.0.create_render_packet_with_fog(
            &camera.0,
            render_width,
            render_height,
            visible_tiles,
            revealed_tiles,
        );

        // Overlay all renderable entities at their screen-relative positions
        let w_radius = render_width as CoordinateUnit / 2;
        let h_radius = render_height as CoordinateUnit / 2;
        let bottom_left = camera.0 - GridVec::new(w_radius, h_radius);

        // Collect visible entities for the side panel.
        let mut visible_entity_infos: Vec<(String, RatColor, RatColor, String)> = Vec::new();

        for (pos, renderable, name) in &renderables {
            let screen = pos.as_grid_vec() - bottom_left;

            if screen.x >= 0
                && screen.x < render_width as CoordinateUnit
                && screen.y >= 0
                && screen.y < render_height as CoordinateUnit
            {
                // Only draw entities that are currently visible (not merely revealed)
                let entity_visible = visible_tiles
                    .map(|vt| vt.contains(&pos.as_grid_vec()))
                    .unwrap_or(true);
                if entity_visible {
                    let bg = render_packet[screen.y as usize][screen.x as usize].2;
                    render_packet[screen.y as usize][screen.x as usize] =
                        (renderable.symbol.clone(), renderable.fg, bg);

                    // Collect for visible entities panel.
                    let full_name = name.map_or("???".to_string(), |n| n.0.clone());
                    visible_entity_infos.push((
                        renderable.symbol.clone(),
                        renderable.fg,
                        renderable.bg,
                        full_name,
                    ));
                }
            }
        }

        // Overlay spell particles on the render packet.
        for (particle_pos, lifetime) in &spell_particles.particles {
            let screen = *particle_pos - bottom_left;
            if screen.x >= 0
                && screen.x < render_width as CoordinateUnit
                && screen.y >= 0
                && screen.y < render_height as CoordinateUnit
            {
                let visible = visible_tiles
                    .map(|vt| vt.contains(particle_pos))
                    .unwrap_or(true);
                if visible {
                    // Particle symbol and color fade with lifetime.
                    let intensity = (*lifetime as f32 / 6.0).min(1.0);
                    let r = (255.0 * intensity) as u8;
                    let g = (165.0 * intensity) as u8;
                    let symbol = if *lifetime > 3 { "*" } else { "·" };
                    let bg = render_packet[screen.y as usize][screen.x as usize].2;
                    render_packet[screen.y as usize][screen.x as usize] =
                        (symbol.into(), RatColor::Rgb(r, g, 0), bg);
                }
            }
        }

        let mut render_lines = Vec::new();

        for y in 0..render_height as usize {
            if y < render_packet.len() {
                let spans: Vec<Span> = render_packet[y]
                    .iter()
                    .map(|gt| Span::from(gt.0.clone()).fg(gt.1).bg(gt.2))
                    .collect();
                render_lines.push(Line::from(spans));
            }
        }

        // Reverse so that higher Y values are at the top (standard roguelike convention)
        render_lines.reverse();

        frame.render_widget(Paragraph::new(Text::from(render_lines)).on_black(), game_area);

        // ── Side Panel ──────────────────────────────────────────
        render_side_panel(
            frame,
            side_area,
            player_hp,
            player_mana,
            player_inv,
            &visible_entity_infos,
            &combat_log,
        );

        // ── Overlays ────────────────────────────────────────────

        // Show "PAUSED" overlay centered on game area when paused
        if *state.get() == GameState::Paused {
            let label = " PAUSED — press P to resume ";
            let label_width = label.len() as u16;
            if render_width >= label_width && render_height >= 1 {
                let cx = game_area.x + (render_width - label_width) / 2;
                let cy = game_area.y + render_height / 2;
                let pause_area = Rect {
                    x: cx,
                    y: cy,
                    width: label_width,
                    height: 1,
                };
                frame.render_widget(
                    Paragraph::new(Line::from(label).bold()).on_dark_gray(),
                    pause_area,
                );
            }
        }

        // Show "VICTORY" overlay centered on game area when the gate is destroyed
        if *state.get() == GameState::Victory {
            let label = " VICTORY! The Gate of Hell has been destroyed! Press Q to quit. ";
            let label_width = label.len() as u16;
            if render_width >= label_width && render_height >= 1 {
                let cx = game_area.x + (render_width - label_width) / 2;
                let cy = game_area.y + render_height / 2;
                let victory_area = Rect {
                    x: cx,
                    y: cy,
                    width: label_width,
                    height: 1,
                };
                frame.render_widget(
                    Paragraph::new(Line::from(label).bold()).on_yellow(),
                    victory_area,
                );
            }
        }

        // Show help overlay when toggled
        if help_visible.0 {
            render_help_overlay(frame, game_area);
        }

        // ── Status bar ──────────────────────────────────────────
        let player_info = player_query
            .single()
            .map(|(p, _, _, _, _)| format!("({}, {})", p.x, p.y))
            .unwrap_or_default();

        let recent_msgs = combat_log.recent(2);
        let last_msg = recent_msgs.join(" | ");

        let status = Line::from(format!(
            " Gate of Hell | {player_info} | Turn:{} Kills:{} | {last_msg} | ?: help",
            turn_counter.0, kill_count.0,
        ));
        frame.render_widget(Paragraph::new(status).on_dark_gray(), status_area);
    })?;

    Ok(())
}

/// Renders the side panel with HP gauge, Mana gauge, inventory, and visible entities.
fn render_side_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    player_hp: Option<&Health>,
    player_mana: Option<&Mana>,
    player_inv: Option<&Inventory>,
    visible_entities: &[(String, RatColor, RatColor, String)],
    _combat_log: &CombatLog,
) {
    // Divide the side panel into sections.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // HP gauge
            Constraint::Length(3), // Mana gauge
            Constraint::Length(6), // Inventory
            Constraint::Min(1),   // Visible entities
        ])
        .split(area);

    // ── HP Gauge ────────────────────────────────────────────────
    if let Some(hp) = player_hp {
        let ratio = if hp.max > 0 {
            (hp.current as f64 / hp.max as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("HP"))
            .gauge_style(
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Red)
                    .bg(ratatui::style::Color::DarkGray),
            )
            .ratio(ratio)
            .label(format!("{}/{}", hp.current, hp.max));
        frame.render_widget(gauge, chunks[0]);
    } else {
        frame.render_widget(
            Block::default().borders(Borders::ALL).title("HP"),
            chunks[0],
        );
    }

    // ── Mana Gauge ──────────────────────────────────────────────
    if let Some(mana) = player_mana {
        let ratio = if mana.max > 0 {
            (mana.current as f64 / mana.max as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Mana"))
            .gauge_style(
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Blue)
                    .bg(ratatui::style::Color::DarkGray),
            )
            .ratio(ratio)
            .label(format!("{}/{}", mana.current, mana.max));
        frame.render_widget(gauge, chunks[1]);
    } else {
        frame.render_widget(
            Block::default().borders(Borders::ALL).title("Mana"),
            chunks[1],
        );
    }

    // ── Inventory ───────────────────────────────────────────────
    let mut inv_lines: Vec<Line> = Vec::new();
    if let Some(inv) = player_inv {
        if inv.items.is_empty() {
            inv_lines.push(Line::from(" (empty)".dark_gray()));
        } else {
            for (i, _item) in inv.items.iter().enumerate().take(9) {
                inv_lines.push(Line::from(format!(" {}: item", i + 1)));
            }
        }
    } else {
        inv_lines.push(Line::from(" (none)".dark_gray()));
    }
    frame.render_widget(
        Paragraph::new(inv_lines)
            .block(Block::default().borders(Borders::ALL).title("Bag [1-9]")),
        chunks[2],
    );

    // ── Visible Entities ────────────────────────────────────────
    let max_visible = (chunks[3].height.saturating_sub(2)) as usize;
    let mut vis_lines: Vec<Line> = Vec::new();
    // Deduplicate: show each unique name only once.
    let mut seen_names: HashSet<String> = HashSet::new();
    for (sym, fg, _bg, name) in visible_entities {
        if seen_names.insert(name.clone()) {
            vis_lines.push(Line::from(vec![
                Span::from(format!(" {sym}")).fg(*fg),
                Span::from(format!(" {name}")).white(),
            ]));
            if vis_lines.len() >= max_visible {
                break;
            }
        }
    }
    if vis_lines.is_empty() {
        vis_lines.push(Line::from(" (nothing)".dark_gray()));
    }

    frame.render_widget(
        Paragraph::new(vis_lines)
            .block(Block::default().borders(Borders::ALL).title("Visible")),
        chunks[3],
    );
}

/// Renders the help overlay listing all keybindings.
fn render_help_overlay(frame: &mut ratatui::Frame, game_area: Rect) {
    let help_width = 42u16;
    let help_height = (KEYBINDINGS.len() as u16) + 4; // +4 for border + title + padding

    if game_area.width < help_width || game_area.height < help_height {
        return;
    }

    let cx = game_area.x + (game_area.width - help_width) / 2;
    let cy = game_area.y + (game_area.height - help_height) / 2;
    let help_area = Rect {
        x: cx,
        y: cy,
        width: help_width,
        height: help_height,
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    for (key, desc) in KEYBINDINGS {
        lines.push(Line::from(vec![
            Span::from(format!(" {key:<14}")).bold().yellow(),
            Span::from(format!("{desc}")).white(),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Controls (? to close) ")
                    .border_style(ratatui::style::Style::default().fg(ratatui::style::Color::Yellow)),
            )
            .on_black(),
        help_area,
    );
}
