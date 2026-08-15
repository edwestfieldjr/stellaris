use bevy::input::touch::Touches;
use bevy::prelude::*;
use rand::RngExt as _;
use std::collections::HashMap;

use crate::mouse::{cursor_world_pos, screen_to_world_pos};
use crate::state::{AppState, Campaign};

/// The galaxy grid always renders inside roughly this many pixels square,
/// regardless of how big `size` gets, so later (bigger) levels still fit
/// the window.
const GRID_FOOTPRINT: f32 = 520.0;
const MIN_CELL: f32 = 26.0;
const MAX_CELL: f32 = 64.0;
const SPREAD_INTERVAL: f32 = 6.0;
const BANNER_SECONDS: f32 = 3.0;
// If the player hasn't warped into a Zerlak sector by the time this runs
// out, the Zerlak fleet doesn't wait — one gets picked and warped into
// automatically.
const DECISION_SECONDS: f32 = 2.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SectorKind {
    Empty,
    Zerlak,
    Friendly,
    Cleared,
}

/// The galaxy's sector grid. Lives for the whole run, independent of which
/// state is active, so clearing a sector in Flight sticks when you return.
/// Regenerated (bigger, denser) each time a level is fully cleared.
#[derive(Resource)]
pub struct GalaxyGrid {
    pub sectors: HashMap<(i32, i32), SectorKind>,
    pub size: i32,
}

impl GalaxyGrid {
    /// Builds a fresh grid for the given campaign level: bigger and more
    /// Zerlak-heavy each time, capped so it never outgrows the window.
    pub fn generate(level: u32) -> Self {
        let size = (5 + level as i32).min(10);
        let zerlak_chance = (0.26 + level as f32 * 0.025).min(0.5);
        let friendly_chance = 0.16;
        let mut rng = rand::rng();
        let mut sectors = HashMap::new();
        for x in 0..size {
            for y in 0..size {
                let roll = rng.random_range(0.0..1.0);
                let kind = if roll < zerlak_chance {
                    SectorKind::Zerlak
                } else if roll < zerlak_chance + friendly_chance {
                    SectorKind::Friendly
                } else {
                    SectorKind::Empty
                };
                sectors.insert((x, y), kind);
            }
        }
        sectors.insert((0, 0), SectorKind::Cleared);
        Self { sectors, size }
    }

    fn cell_size(&self) -> f32 {
        (GRID_FOOTPRINT / self.size as f32).clamp(MIN_CELL, MAX_CELL)
    }

    fn to_world(&self, x: i32, y: i32) -> Vec3 {
        let cell = self.cell_size();
        let origin = -(self.size as f32 - 1.0) * cell / 2.0;
        Vec3::new(origin + x as f32 * cell, origin + y as f32 * cell, 0.0)
    }
}

impl Default for GalaxyGrid {
    fn default() -> Self {
        Self::generate(1)
    }
}

#[derive(Resource)]
struct SpreadTimer(Timer);

/// Counts down the player's window to warp into a Zerlak sector; on expiry
/// `decision_countdown` picks one and warps in automatically.
#[derive(Resource)]
struct DecisionTimer(Timer);

/// Transient "LEVEL N" style announcement shown at the top of the screen.
#[derive(Resource, Default)]
struct Banner {
    timer: Option<Timer>,
}

#[derive(Component)]
struct GalaxyMapUi;

/// Sector squares + selection cursor: rebuilt from scratch whenever the
/// grid is regenerated for a new level, unlike the rest of the HUD.
#[derive(Component)]
struct GridVisual;

/// One of four thin bars forming a highlight frame around the selected
/// cell; `0` is its offset from the cell center.
#[derive(Component)]
struct Cursor(Vec2);

#[derive(Component)]
struct FuelText;

#[derive(Component)]
struct LevelText;

#[derive(Component)]
struct BannerText;

#[derive(Component)]
struct CountdownText;

pub struct GalaxyMapPlugin;

impl Plugin for GalaxyMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GalaxyGrid>()
            .init_resource::<Banner>()
            .add_systems(OnEnter(AppState::GalaxyMap), setup)
            .add_systems(OnExit(AppState::GalaxyMap), teardown)
            .add_systems(
                Update,
                (
                    move_cursor,
                    mouse_hover,
                    warp_input,
                    touch_select,
                    decision_countdown,
                    check_level_complete,
                    spread_zerlaks,
                    quit_to_title,
                    update_fuel_text,
                    update_level_text,
                    update_banner,
                    update_countdown_text,
                )
                    .chain()
                    .run_if(in_state(AppState::GalaxyMap)),
            );
    }
}

fn sector_color(kind: SectorKind) -> Color {
    match kind {
        SectorKind::Empty => Color::srgb(0.15, 0.15, 0.2),
        SectorKind::Zerlak => Color::srgb(0.7, 0.15, 0.15),
        SectorKind::Friendly => Color::srgb(0.15, 0.5, 0.7),
        SectorKind::Cleared => Color::srgb(0.2, 0.35, 0.2),
    }
}

fn setup(mut commands: Commands, grid: Res<GalaxyGrid>, campaign: Res<Campaign>) {
    commands.insert_resource(SpreadTimer(Timer::from_seconds(
        SPREAD_INTERVAL,
        TimerMode::Repeating,
    )));
    commands.insert_resource(DecisionTimer(Timer::from_seconds(
        DECISION_SECONDS,
        TimerMode::Once,
    )));

    spawn_grid_visuals(&mut commands, &grid, &campaign);

    commands.spawn((
        Text::new(format!("Fuel: {:.0}", campaign.fuel.max(0.0))),
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
        Text::new(format!("Level {}", campaign.level)),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.4)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(10.0),
            ..default()
        },
        LevelText,
        GalaxyMapUi,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(28.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.85, 0.3)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        BannerText,
        GalaxyMapUi,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.55, 0.2)),
        TextLayout::default().with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(60.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        CountdownText,
        GalaxyMapUi,
    ));

    commands.spawn((
        Text::new(
            "Arrows/mouse: select    Enter/Space/click: warp    Esc: quit to title    (Zerlak = red, Friendly = blue)",
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

fn spawn_grid_visuals(commands: &mut Commands, grid: &GalaxyGrid, campaign: &Campaign) {
    let cell = grid.cell_size();
    for (&(x, y), &kind) in grid.sectors.iter() {
        commands.spawn((
            Sprite {
                color: sector_color(kind),
                custom_size: Some(Vec2::splat(cell - cell * 0.1)),
                ..default()
            },
            Transform::from_translation(grid.to_world(x, y)),
            GalaxyMapUi,
            GridVisual,
        ));
    }

    let center = grid.to_world(campaign.sector.0, campaign.sector.1);
    let half = cell / 2.0;
    let thickness = (cell * 0.06).max(2.0);
    let bars = [
        (Vec2::new(cell, thickness), Vec2::new(0.0, half)),
        (Vec2::new(cell, thickness), Vec2::new(0.0, -half)),
        (Vec2::new(thickness, cell), Vec2::new(-half, 0.0)),
        (Vec2::new(thickness, cell), Vec2::new(half, 0.0)),
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
            GridVisual,
        ));
    }
}

fn teardown(mut commands: Commands, query: Query<Entity, With<GalaxyMapUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<SpreadTimer>();
    commands.remove_resource::<DecisionTimer>();
}

fn snap_cursor_visuals(
    grid: &GalaxyGrid,
    sector: (i32, i32),
    cursor_query: &mut Query<(&Cursor, &mut Transform)>,
) {
    let center = grid.to_world(sector.0, sector.1);
    for (cursor, mut transform) in cursor_query.iter_mut() {
        transform.translation = center + cursor.0.extend(1.0);
    }
}

fn move_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    grid: Res<GalaxyGrid>,
    mut campaign: ResMut<Campaign>,
    mut cursor_query: Query<(&Cursor, &mut Transform)>,
) {
    let mut moved = false;
    if keys.just_pressed(KeyCode::ArrowLeft) && campaign.sector.0 > 0 {
        campaign.sector.0 -= 1;
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowRight) && campaign.sector.0 < grid.size - 1 {
        campaign.sector.0 += 1;
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) && campaign.sector.1 > 0 {
        campaign.sector.1 -= 1;
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowUp) && campaign.sector.1 < grid.size - 1 {
        campaign.sector.1 += 1;
        moved = true;
    }

    if moved {
        snap_cursor_visuals(&grid, campaign.sector, &mut cursor_query);
    }
}

/// Selects whichever cell the mouse is hovering, but only on frames the
/// cursor actually moved, so it doesn't fight arrow-key input by
/// re-asserting a stale position every frame.
/// Converts a world-space position into a grid cell, if it lands inside
/// the grid's bounds. Shared by mouse hover and touch selection.
fn world_pos_to_cell(grid: &GalaxyGrid, world_pos: Vec2) -> Option<(i32, i32)> {
    let cell = grid.cell_size();
    let origin = -(grid.size as f32 - 1.0) * cell / 2.0;
    let gx = ((world_pos.x - origin) / cell).round() as i32;
    let gy = ((world_pos.y - origin) / cell).round() as i32;
    if gx < 0 || gx >= grid.size || gy < 0 || gy >= grid.size {
        return None;
    }
    Some((gx, gy))
}

fn mouse_hover(
    mut motion: MessageReader<CursorMoved>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    grid: Res<GalaxyGrid>,
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
    let Some(cell) = world_pos_to_cell(&grid, world_pos) else {
        return;
    };
    if campaign.sector == cell {
        return;
    }
    campaign.sector = cell;
    snap_cursor_visuals(&grid, campaign.sector, &mut cursor_query);
}

/// Applies the outcome of warping/selecting into `campaign.sector`: costs
/// or refunds fuel depending on what's there, marks it cleared unless it's
/// a Zerlak fight, and checks for an out-of-fuel game over. Shared by
/// keyboard/mouse warp, touch selection, and the auto-pick countdown.
fn resolve_sector_choice(
    campaign: &mut Campaign,
    grid: &mut GalaxyGrid,
    next_state: &mut NextState<AppState>,
) {
    let Some(kind) = grid.sectors.get(&campaign.sector).copied() else {
        return;
    };
    match kind {
        SectorKind::Zerlak => {
            campaign.fuel -= 5.0;
            next_state.set(AppState::Warp);
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
    if campaign.fuel <= 0.0 {
        campaign.fuel = 0.0;
        campaign.defeat_reason = Some(crate::state::DefeatReason::OutOfFuel);
        next_state.set(AppState::GameOver);
    }
}

fn warp_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut campaign: ResMut<Campaign>,
    mut grid: ResMut<GalaxyGrid>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !keys.just_pressed(KeyCode::Enter)
        && !keys.just_pressed(KeyCode::Space)
        && !mouse.just_pressed(MouseButton::Left)
    {
        return;
    }
    resolve_sector_choice(&mut campaign, &mut grid, &mut next_state);
}

/// A tap both picks the tapped cell and immediately commits to it — touch
/// has no separate "hover" step, so select and confirm happen together.
fn touch_select(
    touches: Res<Touches>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut campaign: ResMut<Campaign>,
    mut grid: ResMut<GalaxyGrid>,
    mut next_state: ResMut<NextState<AppState>>,
    mut cursor_query: Query<(&Cursor, &mut Transform)>,
) {
    let Some(touch) = touches.iter_just_pressed().next() else {
        return;
    };
    let Some(world_pos) = screen_to_world_pos(touch.position(), &camera_q) else {
        return;
    };
    let Some(cell) = world_pos_to_cell(&grid, world_pos) else {
        return;
    };
    campaign.sector = cell;
    snap_cursor_visuals(&grid, campaign.sector, &mut cursor_query);
    resolve_sector_choice(&mut campaign, &mut grid, &mut next_state);
}

/// If the player hasn't warped into a Zerlak sector before the countdown
/// runs out, the fleet doesn't wait: a random Zerlak sector is selected and
/// warped into automatically, same as a manual pick.
fn decision_countdown(
    time: Res<Time>,
    mut timer: ResMut<DecisionTimer>,
    mut grid: ResMut<GalaxyGrid>,
    mut campaign: ResMut<Campaign>,
    mut next_state: ResMut<NextState<AppState>>,
    mut cursor_query: Query<(&Cursor, &mut Transform)>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    let zerlak_cells: Vec<(i32, i32)> = grid
        .sectors
        .iter()
        .filter(|(_, k)| **k == SectorKind::Zerlak)
        .map(|(&pos, _)| pos)
        .collect();
    if zerlak_cells.is_empty() {
        return;
    }
    let mut rng = rand::rng();
    campaign.sector = zerlak_cells[rng.random_range(0..zerlak_cells.len())];
    snap_cursor_visuals(&grid, campaign.sector, &mut cursor_query);
    resolve_sector_choice(&mut campaign, &mut grid, &mut next_state);
}

/// Once every Zerlak sector in the current grid is gone, regenerate a
/// bigger, denser galaxy for the next level rather than just... stopping.
fn check_level_complete(
    mut commands: Commands,
    mut grid: ResMut<GalaxyGrid>,
    mut campaign: ResMut<Campaign>,
    mut banner: ResMut<Banner>,
    old_visuals: Query<Entity, With<GridVisual>>,
) {
    if !grid.is_changed() {
        return;
    }
    if grid.sectors.values().any(|k| *k == SectorKind::Zerlak) {
        return;
    }
    campaign.level += 1;
    *grid = GalaxyGrid::generate(campaign.level);
    campaign.sector = (0, 0);
    campaign.fuel = (campaign.fuel + 40.0).min(100.0);
    banner.timer = Some(Timer::from_seconds(BANNER_SECONDS, TimerMode::Once));

    for entity in &old_visuals {
        commands.entity(entity).despawn();
    }
    spawn_grid_visuals(&mut commands, &grid, &campaign);
}

fn spread_zerlaks(
    time: Res<Time>,
    mut timer: ResMut<SpreadTimer>,
    mut grid: ResMut<GalaxyGrid>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    let zerlak_cells: Vec<(i32, i32)> = grid
        .sectors
        .iter()
        .filter(|(_, k)| **k == SectorKind::Zerlak)
        .map(|(&pos, _)| pos)
        .collect();
    if zerlak_cells.is_empty() {
        return;
    }
    let mut rng = rand::rng();
    let origin = zerlak_cells[rng.random_range(0..zerlak_cells.len())];
    let neighbors = [
        (origin.0 - 1, origin.1),
        (origin.0 + 1, origin.1),
        (origin.0, origin.1 - 1),
        (origin.0, origin.1 + 1),
    ];
    let candidates: Vec<(i32, i32)> = neighbors
        .into_iter()
        .filter(|pos| matches!(grid.sectors.get(pos), Some(SectorKind::Empty | SectorKind::Cleared)))
        .collect();
    if !candidates.is_empty() {
        let target = candidates[rng.random_range(0..candidates.len())];
        grid.sectors.insert(target, SectorKind::Zerlak);
    }
}

fn quit_to_title(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Title);
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

fn update_level_text(campaign: Res<Campaign>, mut query: Query<&mut Text, With<LevelText>>) {
    if !campaign.is_changed() {
        return;
    }
    if let Ok(mut text) = query.single_mut() {
        **text = format!("Level {}", campaign.level);
    }
}

/// Updates every frame (not gated on change) so the hundredths digit
/// actually ticks smoothly instead of jumping once a second.
fn update_countdown_text(
    timer: Res<DecisionTimer>,
    mut query: Query<&mut Text, With<CountdownText>>,
) {
    let remaining = timer.0.remaining_secs().max(0.0);
    if let Ok(mut text) = query.single_mut() {
        **text = format!("Zerlak lock in {remaining:.2}s");
    }
}

fn update_banner(
    time: Res<Time>,
    campaign: Res<Campaign>,
    mut banner: ResMut<Banner>,
    mut query: Query<&mut Text, With<BannerText>>,
) {
    let Some(timer) = banner.timer.as_mut() else {
        return;
    };
    timer.tick(time.delta());
    if let Ok(mut text) = query.single_mut() {
        **text = format!("LEVEL {} - the Zerlak Empire regroups...", campaign.level);
    }
    if timer.is_finished() {
        banner.timer = None;
        if let Ok(mut text) = query.single_mut() {
            **text = String::new();
        }
    }
}
