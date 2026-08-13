use bevy::prelude::*;
use std::collections::HashMap;

use crate::mouse::cursor_world_pos;
use crate::state::{AppState, Campaign};

const GRID_SIZE: i32 = 6;
const CELL: f32 = 64.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SectorKind {
    Empty,
    Zylon,
    Friendly,
    Cleared,
}

/// The galaxy's sector grid. Lives for the whole run, independent of which
/// state is active, so clearing a sector in Flight sticks when you return.
#[derive(Resource)]
pub struct GalaxyGrid {
    pub sectors: HashMap<(i32, i32), SectorKind>,
}

impl Default for GalaxyGrid {
    fn default() -> Self {
        let mut sectors = HashMap::new();
        for x in 0..GRID_SIZE {
            for y in 0..GRID_SIZE {
                let kind = match (x + y * 3) % 5 {
                    0 => SectorKind::Zylon,
                    1 => SectorKind::Friendly,
                    _ => SectorKind::Empty,
                };
                sectors.insert((x, y), kind);
            }
        }
        sectors.insert((0, 0), SectorKind::Cleared);
        Self { sectors }
    }
}

#[derive(Component)]
struct GalaxyMapUi;

/// One of four thin bars forming a highlight frame around the selected
/// cell; `0` is its offset from the cell center.
#[derive(Component)]
struct Cursor(Vec2);

#[derive(Component)]
struct FuelText;

pub struct GalaxyMapPlugin;

impl Plugin for GalaxyMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GalaxyGrid>()
            .add_systems(OnEnter(AppState::GalaxyMap), setup)
            .add_systems(OnExit(AppState::GalaxyMap), teardown)
            .add_systems(
                Update,
                (move_cursor, mouse_hover, warp_input, update_fuel_text)
                    .run_if(in_state(AppState::GalaxyMap)),
            );
    }
}

fn sector_color(kind: SectorKind) -> Color {
    match kind {
        SectorKind::Empty => Color::srgb(0.15, 0.15, 0.2),
        SectorKind::Zylon => Color::srgb(0.7, 0.15, 0.15),
        SectorKind::Friendly => Color::srgb(0.15, 0.5, 0.7),
        SectorKind::Cleared => Color::srgb(0.2, 0.35, 0.2),
    }
}

fn grid_to_world(x: i32, y: i32) -> Vec3 {
    let origin = -(GRID_SIZE as f32 - 1.0) * CELL / 2.0;
    Vec3::new(origin + x as f32 * CELL, origin + y as f32 * CELL, 0.0)
}

fn setup(mut commands: Commands, grid: Res<GalaxyGrid>, campaign: Res<Campaign>) {
    for (&(x, y), &kind) in grid.sectors.iter() {
        commands.spawn((
            Sprite {
                color: sector_color(kind),
                custom_size: Some(Vec2::splat(CELL - 6.0)),
                ..default()
            },
            Transform::from_translation(grid_to_world(x, y)),
            GalaxyMapUi,
        ));
    }

    let center = grid_to_world(campaign.sector.0, campaign.sector.1);
    let half = CELL / 2.0;
    let bars = [
        (Vec2::new(CELL, 4.0), Vec2::new(0.0, half)),
        (Vec2::new(CELL, 4.0), Vec2::new(0.0, -half)),
        (Vec2::new(4.0, CELL), Vec2::new(-half, 0.0)),
        (Vec2::new(4.0, CELL), Vec2::new(half, 0.0)),
    ];
    for (size, offset) in bars {
        commands.spawn((
            Sprite {
                color: Color::srgb(1.0, 0.9, 0.2),
                custom_size: Some(size),
                ..default()
            },
            Transform::from_translation(center + offset.extend(1.0)),
            Cursor(offset),
            GalaxyMapUi,
        ));
    }

    commands.spawn((
        Text::new("Fuel: 100"),
        TextFont {
            font_size: bevy::text::FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        FuelText,
        GalaxyMapUi,
    ));

    commands.spawn((
        Text::new(
            "Arrows/mouse: select    Enter/click: warp    (Zylon = red, Friendly = blue)",
        ),
        TextFont {
            font_size: bevy::text::FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        GalaxyMapUi,
    ));
}

fn teardown(mut commands: Commands, query: Query<Entity, With<GalaxyMapUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn snap_cursor_visuals(sector: (i32, i32), cursor_query: &mut Query<(&Cursor, &mut Transform)>) {
    let center = grid_to_world(sector.0, sector.1);
    for (cursor, mut transform) in cursor_query.iter_mut() {
        transform.translation = center + cursor.0.extend(1.0);
    }
}

fn move_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mut campaign: ResMut<Campaign>,
    mut cursor_query: Query<(&Cursor, &mut Transform)>,
) {
    let mut moved = false;
    if keys.just_pressed(KeyCode::ArrowLeft) && campaign.sector.0 > 0 {
        campaign.sector.0 -= 1;
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowRight) && campaign.sector.0 < GRID_SIZE - 1 {
        campaign.sector.0 += 1;
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) && campaign.sector.1 > 0 {
        campaign.sector.1 -= 1;
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowUp) && campaign.sector.1 < GRID_SIZE - 1 {
        campaign.sector.1 += 1;
        moved = true;
    }

    if moved {
        snap_cursor_visuals(campaign.sector, &mut cursor_query);
    }
}

/// Selects whichever cell the mouse is hovering, but only on frames the
/// cursor actually moved, so it doesn't fight arrow-key input by
/// re-asserting a stale position every frame.
fn mouse_hover(
    mut motion: MessageReader<CursorMoved>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut campaign: ResMut<Campaign>,
    mut cursor_query: Query<(&Cursor, &mut Transform)>,
) {
    if motion.is_empty() {
        return;
    }
    motion.clear();
    let Some(world_pos) = cursor_world_pos(&windows, &camera_q) else {
        return;
    };
    let origin = -(GRID_SIZE as f32 - 1.0) * CELL / 2.0;
    let gx = ((world_pos.x - origin) / CELL).round() as i32;
    let gy = ((world_pos.y - origin) / CELL).round() as i32;
    if gx < 0 || gx >= GRID_SIZE || gy < 0 || gy >= GRID_SIZE {
        return;
    }
    if campaign.sector == (gx, gy) {
        return;
    }
    campaign.sector = (gx, gy);
    snap_cursor_visuals(campaign.sector, &mut cursor_query);
}

fn warp_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut campaign: ResMut<Campaign>,
    mut grid: ResMut<GalaxyGrid>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !keys.just_pressed(KeyCode::Enter) && !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(kind) = grid.sectors.get(&campaign.sector).copied() else {
        return;
    };
    match kind {
        SectorKind::Zylon => {
            campaign.fuel -= 5.0;
            next_state.set(AppState::Flight);
        }
        SectorKind::Friendly => {
            campaign.fuel = (campaign.fuel + 25.0).min(100.0);
            grid.sectors.insert(campaign.sector, SectorKind::Cleared);
        }
        SectorKind::Empty => {
            campaign.fuel -= 2.0;
            grid.sectors.insert(campaign.sector, SectorKind::Cleared);
        }
        SectorKind::Cleared => {}
    }
}

fn update_fuel_text(campaign: Res<Campaign>, mut query: Query<&mut Text, With<FuelText>>) {
    if !campaign.is_changed() {
        return;
    }
    if let Ok(mut text) = query.single_mut() {
        **text = format!("Fuel: {:.0}", campaign.fuel.max(0.0));
    }
}
