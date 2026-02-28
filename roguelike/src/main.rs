use std::time::Duration;

use bevy::{app::AppExit, prelude::*};
use bevy_ratatui::event::KeyMessage;
use bevy_ratatui::{RatatuiContext, RatatuiPlugins};
use ratatui::crossterm::event::KeyCode;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

use roguelike::gamemap::GameMap;
use roguelike::typedefs::MyPoint;

/// Bevy resource holding the game map.
#[derive(Resource)]
struct GameMapResource(GameMap);

/// Bevy resource holding the camera/player position.
#[derive(Resource)]
struct CameraPosition(MyPoint);

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                Duration::from_secs_f32(1. / 30.),
            )),
            RatatuiPlugins::default(),
        ))
        .insert_resource(GameMapResource(GameMap::new(120, 80)))
        .insert_resource(CameraPosition((60, 40)))
        .add_systems(PreUpdate, input_system)
        .add_systems(Update, draw_system)
        .run();
}

fn draw_system(
    mut context: ResMut<RatatuiContext>,
    game_map: Res<GameMapResource>,
    camera: Res<CameraPosition>,
) -> Result {
    context.draw(|frame| {
        let area = frame.area();
        let render_width = area.width;
        let render_height = area.height.saturating_sub(1); // reserve 1 row for status

        let render_packet =
            game_map.0.create_render_packet(&camera.0, render_width, render_height);

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
        frame.render_widget(Paragraph::new(Text::from(render_lines)).on_black(), game_area);

        // Status bar
        let status_area = ratatui::layout::Rect {
            x: area.x,
            y: area.y + render_height,
            width: area.width,
            height: 1,
        };
        let status = Line::from(format!(
            " Roguelike | Pos: ({}, {}) | WASD: move | Q: quit",
            camera.0 .0, camera.0 .1
        ));
        frame.render_widget(Paragraph::new(status).on_dark_gray(), status_area);
    })?;

    Ok(())
}

fn input_system(
    mut messages: MessageReader<KeyMessage>,
    mut exit: MessageWriter<AppExit>,
    mut camera: ResMut<CameraPosition>,
) {
    for message in messages.read() {
        match message.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                exit.write_default();
            }
            KeyCode::Char('w') | KeyCode::Up => {
                camera.0 .1 += 1;
            }
            KeyCode::Char('s') | KeyCode::Down => {
                camera.0 .1 -= 1;
            }
            KeyCode::Char('a') | KeyCode::Left => {
                camera.0 .0 -= 1;
            }
            KeyCode::Char('d') | KeyCode::Right => {
                camera.0 .0 += 1;
            }
            _ => {}
        }
    }
}
