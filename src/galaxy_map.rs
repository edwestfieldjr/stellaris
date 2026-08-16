use bevy::input::touch::Touches;
use bevy::prelude::*;
use rand::RngExt as _;
use std::collections::HashMap;

use crate::hud::credits_closed;
use crate::hud_bridge::{ScreenKind, ScreenText};
use crate::mouse::{cursor_world_pos, screen_to_world_pos};
use crate::state::{AppState, Campaign, WarpTarget};
use crate::virtual_input::{VirtualFirePending, VirtualNudge};

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

/// Accumulated on-screen-wheel drag since the last cell step (tall-portrait
/// layout's trackpad — see `virtual_input.rs`). Sector selection is
/// inherently a grid-cell choice, so this steps `campaign.sector` by one
/// cell at a time the same way arrow keys do, rather than moving a
/// free-floating cursor.
#[derive(Resource, Default)]
struct GalaxyNudgeAccum(Vec2);

/// Drag distance (px) that banks one cell step.
const GALAXY_NUDGE_STEP: f32 = 60.0;

pub struct GalaxyMapPlugin;

impl Plugin for GalaxyMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GalaxyGrid>()
            .init_resource::<Banner>()
            .init_resource::<GalaxyNudgeAccum>()
            .add_systems(OnEnter(AppState::GalaxyMap), setup)
            .add_systems(OnExit(AppState::GalaxyMap), teardown)
            .add_systems(
                Update,
                (
                    move_cursor,
                    virtual_nudge_cursor,
                    mouse_hover,
                    warp_input,
                    touch_select,
                    decision_countdown,
                    check_level_complete,
                    spread_zerlaks,
                    quit_to_title,
                    push_galaxy_screen_text,
                )
                    .chain()
                    .run_if(in_state(AppState::GalaxyMap).and_then(credits_closed)),
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

    // No on-canvas Fuel/Level/banner/countdown/instructions text anymore —
    // all of it read from the fixed 900x650 game space, which the web
    // frontend's cover-fit layout can crop on some aspect ratios (see
    // hud_bridge.rs). It's all in the React-rendered overlay instead now;
    // `push_galaxy_screen_text` keeps it fed every frame this screen is up.
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

/// Same idea as `move_cursor`'s arrow-key handling, driven by the
/// tall-portrait layout's on-screen trackpad instead: banks drag distance
/// per axis and steps the selection by one cell each time a full
/// `GALAXY_NUDGE_STEP` accumulates, rather than moving continuously.
fn virtual_nudge_cursor(
    virtual_nudge: Res<VirtualNudge>,
    grid: Res<GalaxyGrid>,
    mut campaign: ResMut<Campaign>,
    mut accum: ResMut<GalaxyNudgeAccum>,
    mut cursor_query: Query<(&Cursor, &mut Transform)>,
) {
    accum.0 += virtual_nudge.0;
    let mut moved = false;

    while accum.0.x >= GALAXY_NUDGE_STEP && campaign.sector.0 < grid.size - 1 {
        campaign.sector.0 += 1;
        accum.0.x -= GALAXY_NUDGE_STEP;
        moved = true;
    }
    while accum.0.x <= -GALAXY_NUDGE_STEP && campaign.sector.0 > 0 {
        campaign.sector.0 -= 1;
        accum.0.x += GALAXY_NUDGE_STEP;
        moved = true;
    }
    while accum.0.y >= GALAXY_NUDGE_STEP && campaign.sector.1 < grid.size - 1 {
        campaign.sector.1 += 1;
        accum.0.y -= GALAXY_NUDGE_STEP;
        moved = true;
    }
    while accum.0.y <= -GALAXY_NUDGE_STEP && campaign.sector.1 > 0 {
        campaign.sector.1 -= 1;
        accum.0.y += GALAXY_NUDGE_STEP;
        moved = true;
    }
    // Clamp rather than let it bank unboundedly against an edge — otherwise
    // pushing against the grid's border for a while, then having room to
    // move again (say, after a level regenerates a bigger grid), would
    // dump the cursor several cells at once from stored-up drag.
    accum.0 = accum.0.clamp(Vec2::splat(-GALAXY_NUDGE_STEP), Vec2::splat(GALAXY_NUDGE_STEP));

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
    warp_target: &mut WarpTarget,
) {
    let Some(kind) = grid.sectors.get(&campaign.sector).copied() else {
        return;
    };
    match kind {
        SectorKind::Zerlak => {
            campaign.fuel -= 5.0;
            *warp_target = WarpTarget(AppState::Flight);
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
    mut virtual_fire: ResMut<VirtualFirePending>,
    mut campaign: ResMut<Campaign>,
    mut grid: ResMut<GalaxyGrid>,
    mut next_state: ResMut<NextState<AppState>>,
    mut warp_target: ResMut<WarpTarget>,
) {
    // The tall-portrait layout's on-screen trackpad reports its tap
    // through `VirtualFirePending` — no matching Bevy input event of its
    // own to key off, so it's consumed here unconditionally like the
    // other input sources, same pattern as flight.rs's `shoot`.
    let fired = std::mem::take(&mut virtual_fire.0);
    if !keys.just_pressed(KeyCode::Enter)
        && !keys.just_pressed(KeyCode::Space)
        && !mouse.just_pressed(MouseButton::Left)
        && !fired
    {
        return;
    }
    resolve_sector_choice(&mut campaign, &mut grid, &mut next_state, &mut warp_target);
}

/// A tap both picks the tapped cell and immediately commits to it — touch
/// has no separate "hover" step, so select and confirm happen together.
fn touch_select(
    touches: Res<Touches>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut campaign: ResMut<Campaign>,
    mut grid: ResMut<GalaxyGrid>,
    mut next_state: ResMut<NextState<AppState>>,
    mut warp_target: ResMut<WarpTarget>,
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
    resolve_sector_choice(&mut campaign, &mut grid, &mut next_state, &mut warp_target);
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
    mut warp_target: ResMut<WarpTarget>,
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
    resolve_sector_choice(&mut campaign, &mut grid, &mut next_state, &mut warp_target);
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

/// Keeps the React-rendered HUD overlay fed with Fuel/Level/banner/
/// countdown every frame this screen is up — replaces what used to be
/// four separate on-canvas Text-updating systems (see `hud_bridge.rs`).
/// Recomputes unconditionally rather than gating on individual pieces
/// changing: the countdown needs a fresh value every frame anyway (for its
/// hundredths digit to tick smoothly instead of jumping once a second),
/// and this is a menu screen, not a hot path.
fn push_galaxy_screen_text(
    time: Res<Time>,
    campaign: Res<Campaign>,
    timer: Res<DecisionTimer>,
    mut banner: ResMut<Banner>,
    mut screen_text: ResMut<ScreenText>,
) {
    let banner_text = if let Some(banner_timer) = banner.timer.as_mut() {
        banner_timer.tick(time.delta());
        if banner_timer.is_finished() {
            banner.timer = None;
            String::new()
        } else {
            format!("LEVEL {} - the Zerlak Empire regroups...", campaign.level)
        }
    } else {
        String::new()
    };

    *screen_text = ScreenText {
        screen: ScreenKind::GalaxyMap,
        fuel: campaign.fuel.max(0.0),
        level: campaign.level,
        banner: banner_text,
        countdown: timer.0.remaining_secs().max(0.0),
        defeat_reason: String::new(),
    };
}
