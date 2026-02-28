use std::time::Duration;

use bevy::{app::AppExit, prelude::*};
use bevy_ratatui::event::KeyMessage;
use bevy_ratatui::{RatatuiContext, RatatuiPlugins};
use ratatui::crossterm::event::KeyCode;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

use roguelike::fov::compute_fov;
use roguelike::gamemap::GameMap;
use roguelike::typedefs::{CoordinateUnit, MyPoint, RatColor, SPAWN_X, SPAWN_Y};
use roguelike::typeenums::Furniture;

/// Bevy resource holding the game map.
#[derive(Resource)]
struct GameMapResource(GameMap);

/// Bevy resource holding the camera position (follows the player).
#[derive(Resource)]
struct CameraPosition(MyPoint);

/// Bevy resource holding the player's world position.
#[derive(Resource)]
struct PlayerPosition(MyPoint);

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                Duration::from_secs_f32(1. / 30.),
            )),
            RatatuiPlugins::default(),
        ))
        .insert_resource(GameMapResource(GameMap::new(120, 80)))
        .insert_resource(PlayerPosition((SPAWN_X, SPAWN_Y)))
        .insert_resource(CameraPosition((SPAWN_X, SPAWN_Y)))
        .add_systems(PreUpdate, input_system)
        .add_systems(Update, (camera_follow_system, draw_system).chain())
        .run();
}

/// Updates the camera to follow the player position.
fn camera_follow_system(player: Res<PlayerPosition>, mut camera: ResMut<CameraPosition>) {
    camera.0 = player.0;
}

fn draw_system(
    mut context: ResMut<RatatuiContext>,
    game_map: Res<GameMapResource>,
    camera: Res<CameraPosition>,
    player: Res<PlayerPosition>,
) -> Result {
    context.draw(|frame| {
        let area = frame.area();
        let render_width = area.width;
        let render_height = area.height.saturating_sub(1); // reserve 1 row for status
        let fov_radius = (render_width.max(render_height) / 2) as CoordinateUnit;
        let visible_tiles = compute_fov(player.0, fov_radius, |x, y| {
            game_map.0.get_voxel_at(&(x, y)).is_none_or(|voxel| {
                voxel
                    .furniture
                    .as_ref()
                    .is_some_and(Furniture::blocks_movement)
            })
        });

        let mut render_packet =
            game_map
                .0
                .create_render_packet(&camera.0, render_width, render_height, &visible_tiles);

        // Overlay the player @ at their screen-relative position
        let w_radius = render_width as CoordinateUnit / 2;
        let h_radius = render_height as CoordinateUnit / 2;
        let player_screen_x = player.0.0 - (camera.0.0 - w_radius);
        let player_screen_y = player.0.1 - (camera.0.1 - h_radius);

        if player_screen_x >= 0
            && player_screen_x < render_width as CoordinateUnit
            && player_screen_y >= 0
            && player_screen_y < render_height as CoordinateUnit
        {
            let bg = render_packet[player_screen_y as usize][player_screen_x as usize]
                .2
                .clone();
            render_packet[player_screen_y as usize][player_screen_x as usize] =
                ("@".into(), RatColor::White, bg);
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

        let game_area = ratatui::layout::Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: render_height,
        };
        frame.render_widget(
            Paragraph::new(Text::from(render_lines)).on_black(),
            game_area,
        );

        // Status bar
        let status_area = ratatui::layout::Rect {
            x: area.x,
            y: area.y + render_height,
            width: area.width,
            height: 1,
        };
        let status = Line::from(format!(
            " Roguelike | Player: ({}, {}) | WASD/Arrows: move | Q: quit",
            player.0.0, player.0.1
        ));
        frame.render_widget(Paragraph::new(status).on_dark_gray(), status_area);
    })?;

    Ok(())
}

fn input_system(
    mut messages: MessageReader<KeyMessage>,
    mut exit: MessageWriter<AppExit>,
    game_map: Res<GameMapResource>,
    mut player: ResMut<PlayerPosition>,
) {
    for message in messages.read() {
        match message.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                exit.write_default();
            }
            KeyCode::Char('w') | KeyCode::Up => {
                let next = (player.0.0, player.0.1 + 1);
                if game_map.0.can_move_to(&next) {
                    player.0 = next;
                }
            }
            KeyCode::Char('s') | KeyCode::Down => {
                let next = (player.0.0, player.0.1 - 1);
                if game_map.0.can_move_to(&next) {
                    player.0 = next;
                }
            }
            KeyCode::Char('a') | KeyCode::Left => {
                let next = (player.0.0 - 1, player.0.1);
                if game_map.0.can_move_to(&next) {
                    player.0 = next;
                }
            }
            KeyCode::Char('d') | KeyCode::Right => {
                let next = (player.0.0 + 1, player.0.1);
                if game_map.0.can_move_to(&next) {
                    player.0 = next;
                }
            }
            _ => {}
        }
    }
}
