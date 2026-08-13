use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use rand::RngExt as _;
use std::f32::consts::TAU;

use crate::galaxy_map::{GalaxyGrid, SectorKind};
use crate::mouse::cursor_world_pos;
use crate::state::{AppState, Campaign};

// Depth runs from FAR_DEPTH (a speck near the vanishing point) down to 0.0
// (right in front of the canopy). Screen position and sprite size are both
// derived from depth each frame, which is what sells the first-person
// "things grow as they close in" read.
const FAR_DEPTH: f32 = 900.0;
const APPROACH_SPEED_BASE: f32 = 150.0;
const MAX_SPREAD_X: f32 = 380.0;
const MAX_SPREAD_Y: f32 = 250.0;
const MIN_SIZE: f32 = 8.0;
const MAX_SIZE: f32 = 90.0;
const CROSSHAIR_SPEED: f32 = 320.0;
const HIT_MARGIN: f32 = 16.0;
const SPAWN_INTERVAL: f32 = 1.1;
const GUN_ORIGIN: Vec2 = Vec2::new(0.0, -300.0);
const TRACER_LIFETIME: f32 = 0.1;
const FLASH_LIFETIME: f32 = 0.2;

const STAR_COUNT: usize = 90;
const STAR_MIN_SPEED: f32 = 220.0;
const STAR_MAX_SPEED: f32 = 480.0;
const STAR_MIN_REACH: f32 = 0.4;
const STAR_MAX_REACH: f32 = 1.6;
const STAR_MAX_LEN: f32 = 55.0;

// The deep layer: dim, distant points sitting behind the warp streaks. It
// radiates from the same center point, just far slower and staying small
// and faint even up close, and it also drifts opposite your aim — the
// classic background-parallax "look around" cue, layered on top of its
// own slow approach.
const DEEP_STAR_COUNT: usize = 140;
const DEEP_MIN_SPEED: f32 = 12.0;
const DEEP_MAX_SPEED: f32 = 40.0;
const DEEP_MIN_REACH: f32 = 0.5;
const DEEP_MAX_REACH: f32 = 1.3;
const DEEP_MIN_SIZE: f32 = 1.0;
const DEEP_MAX_SIZE: f32 = 3.0;
const DEEP_MIN_ALPHA: f32 = 0.1;
const DEEP_MAX_ALPHA: f32 = 0.45;
const DEEP_PARALLAX: f32 = 0.18;

// The far backdrop: pinprick stars, spawned once and never touched by any
// per-frame system — no radiation, no parallax. Pure sense of vast, fixed
// distance behind everything else.
const FAR_STAR_COUNT: usize = 110;
const FAR_STAR_FIELD_HALF_WIDTH: f32 = 650.0;
const FAR_STAR_FIELD_HALF_HEIGHT: f32 = 420.0;
const FAR_STAR_MIN_SIZE: f32 = 1.0;
const FAR_STAR_MAX_SIZE: f32 = 1.5;
const FAR_STAR_MIN_ALPHA: f32 = 0.05;
const FAR_STAR_MAX_ALPHA: f32 = 0.16;

/// Projects an enemy's fixed approach angle + current depth into a screen
/// position and sprite size. `angle` is a direction in roughly [-1, 1] on
/// each axis; depth 0 means "at the canopy", FAR_DEPTH means "a speck at
/// the vanishing point".
fn project(angle: Vec2, depth: f32) -> (Vec2, f32) {
    let t = (1.0 - (depth / FAR_DEPTH)).clamp(0.0, 1.0);
    let pos = Vec2::new(angle.x * MAX_SPREAD_X, angle.y * MAX_SPREAD_Y) * t;
    let size = MIN_SIZE + (MAX_SIZE - MIN_SIZE) * t;
    (pos, size)
}

/// Position, streak length, and alpha for a star at a given depth. Stars
/// radiate outward from a fixed vanishing point at screen center and
/// stretch into streaks as they close in, reaching all the way to the
/// play area's edges — a warp-speed screensaver look, independent of aim.
fn star_visual(dir: Vec2, reach: f32, depth: f32) -> (Vec2, f32, f32) {
    let t = (1.0 - (depth / FAR_DEPTH)).clamp(0.0, 1.0);
    let pos = dir * reach * Vec2::new(MAX_SPREAD_X, MAX_SPREAD_Y) * t;
    let len = 2.0 + (STAR_MAX_LEN - 2.0) * t * t;
    let alpha = 0.15 + 0.85 * t;
    (pos, len, alpha)
}

/// Position, size, and alpha for a deep-layer point at a given depth: the
/// same center-out radiation as `star_visual`, just far slower and capped
/// to a small, dim size so it always reads as distant background.
fn deep_star_visual(dir: Vec2, reach: f32, depth: f32) -> (Vec2, f32, f32) {
    let t = (1.0 - (depth / FAR_DEPTH)).clamp(0.0, 1.0);
    let pos = dir * reach * Vec2::new(MAX_SPREAD_X, MAX_SPREAD_Y) * t;
    let size = DEEP_MIN_SIZE + (DEEP_MAX_SIZE - DEEP_MIN_SIZE) * t;
    let alpha = DEEP_MIN_ALPHA + (DEEP_MAX_ALPHA - DEEP_MIN_ALPHA) * t;
    (pos, size, alpha)
}

/// A silhouette for enemies, from planet-like orbs to monster/ship hybrids.
/// Each kind also has a signature hue range, so instances of the same kind
/// still land as slightly different "combinations in between".
#[derive(Clone, Copy)]
enum EnemyKind {
    Orb,   // planet-like sphere
    Ring,  // ringed planet / alien eye
    Wing,  // classic arrowhead fighter
    Hex,   // insectoid carapace
    Blob,  // amorphous space monster
    Shard, // crystalline monster/ship
}

const ENEMY_KINDS: [EnemyKind; 6] = [
    EnemyKind::Orb,
    EnemyKind::Ring,
    EnemyKind::Wing,
    EnemyKind::Hex,
    EnemyKind::Blob,
    EnemyKind::Shard,
];

impl EnemyKind {
    /// Builds this kind's mesh at unit scale (roughly a 1x1 bounding box),
    /// so a `Transform::scale` of `size` gives a final size of `size`.
    fn build_mesh(self) -> Mesh {
        match self {
            EnemyKind::Orb => Mesh::from(Circle::new(0.5).mesh()),
            EnemyKind::Ring => Mesh::from(Annulus::new(0.28, 0.5).mesh()),
            EnemyKind::Wing => Mesh::from(
                Triangle2d::new(
                    Vec2::new(0.0, 0.55),
                    Vec2::new(-0.5, -0.35),
                    Vec2::new(0.5, -0.35),
                )
                .mesh(),
            ),
            EnemyKind::Hex => Mesh::from(RegularPolygon::new(0.5, 6).mesh()),
            EnemyKind::Blob => Mesh::from(Ellipse::new(0.5, 0.34).mesh()),
            EnemyKind::Shard => Mesh::from(Rhombus::new(0.7, 0.5).mesh()),
        }
    }

    /// Hue range (degrees) this kind's color is randomized within.
    fn hue_range(self) -> (f32, f32) {
        match self {
            EnemyKind::Orb => (190.0, 260.0),   // planet blues/purples
            EnemyKind::Ring => (30.0, 55.0),    // Saturn-like gold
            EnemyKind::Wing => (0.0, 20.0),     // fighter red/orange
            EnemyKind::Hex => (85.0, 140.0),    // insectoid green
            EnemyKind::Blob => (280.0, 330.0),  // alien magenta
            EnemyKind::Shard => (170.0, 200.0), // icy cyan
        }
    }
}

/// Mesh handles for each enemy silhouette, built once on entering the
/// sector and reused (with a fresh, randomly colored material) per spawn.
#[derive(Resource)]
struct EnemyShapes {
    orb: Handle<Mesh>,
    ring: Handle<Mesh>,
    wing: Handle<Mesh>,
    hex: Handle<Mesh>,
    blob: Handle<Mesh>,
    shard: Handle<Mesh>,
}

impl EnemyShapes {
    fn handle(&self, kind: EnemyKind) -> Handle<Mesh> {
        match kind {
            EnemyKind::Orb => self.orb.clone(),
            EnemyKind::Ring => self.ring.clone(),
            EnemyKind::Wing => self.wing.clone(),
            EnemyKind::Hex => self.hex.clone(),
            EnemyKind::Blob => self.blob.clone(),
            EnemyKind::Shard => self.shard.clone(),
        }
    }
}

#[derive(Component)]
struct FlightUi;

#[derive(Component)]
struct Enemy;

#[derive(Component)]
struct Approach {
    angle: Vec2,
    depth: f32,
    speed: f32,
}

#[derive(Component)]
struct Star {
    dir: Vec2,
    reach: f32,
    depth: f32,
    speed: f32,
}

/// A point in the distant backdrop, radiating out from center like `Star`
/// but much slower; its rendered position also picks up a pointer-parallax
/// offset on top (see `move_deep_stars`).
#[derive(Component)]
struct DeepStar {
    dir: Vec2,
    reach: f32,
    depth: f32,
    speed: f32,
}

#[derive(Component)]
struct CrosshairMarker;

#[derive(Component)]
struct FadeOut(Timer);

#[derive(Component)]
struct StatusText;

#[derive(Resource, Default)]
struct EnemiesRemaining(u32);

#[derive(Resource, Default)]
struct CrosshairPos(Vec2);

#[derive(Resource)]
struct SpawnTimer(Timer);

pub struct FlightPlugin;

impl Plugin for FlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Flight), setup)
            .add_systems(OnExit(AppState::Flight), teardown)
            .add_systems(
                Update,
                (
                    spawn_enemies,
                    move_crosshair,
                    mouse_aim,
                    move_stars,
                    move_deep_stars,
                    move_enemies,
                    shoot,
                    tick_fade_out,
                    check_abort,
                    update_status_text,
                )
                    .chain()
                    .run_if(in_state(AppState::Flight)),
            );
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(EnemiesRemaining(6));
    commands.insert_resource(CrosshairPos::default());
    commands.insert_resource(SpawnTimer(Timer::from_seconds(
        SPAWN_INTERVAL,
        TimerMode::Repeating,
    )));
    commands.insert_resource(EnemyShapes {
        orb: meshes.add(EnemyKind::Orb.build_mesh()),
        ring: meshes.add(EnemyKind::Ring.build_mesh()),
        wing: meshes.add(EnemyKind::Wing.build_mesh()),
        hex: meshes.add(EnemyKind::Hex.build_mesh()),
        blob: meshes.add(EnemyKind::Blob.build_mesh()),
        shard: meshes.add(EnemyKind::Shard.build_mesh()),
    });

    let mut rng = rand::rng();

    // Far backdrop: static pinprick stars, well behind everything else and
    // completely still.
    for _ in 0..FAR_STAR_COUNT {
        let pos = Vec2::new(
            rng.random_range(-FAR_STAR_FIELD_HALF_WIDTH..FAR_STAR_FIELD_HALF_WIDTH),
            rng.random_range(-FAR_STAR_FIELD_HALF_HEIGHT..FAR_STAR_FIELD_HALF_HEIGHT),
        );
        let size = rng.random_range(FAR_STAR_MIN_SIZE..FAR_STAR_MAX_SIZE);
        let alpha = rng.random_range(FAR_STAR_MIN_ALPHA..FAR_STAR_MAX_ALPHA);
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, alpha),
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_translation(pos.extend(-95.0)),
            FlightUi,
        ));
    }

    // Deep backdrop: sits behind the warp streaks, radiating from the same
    // center much more slowly, and also drifts opposite your aim (see
    // `move_deep_stars`).
    for _ in 0..DEEP_STAR_COUNT {
        let theta = rng.random_range(0.0..TAU);
        let dir = Vec2::new(theta.cos(), theta.sin());
        let reach = rng.random_range(DEEP_MIN_REACH..DEEP_MAX_REACH);
        let speed = rng.random_range(DEEP_MIN_SPEED..DEEP_MAX_SPEED);
        // Area-uniform initial placement (sqrt of a uniform sample) so the
        // whole playfield is populated at once, not just clustered near
        // the center the way a plain uniform depth sample would leave it.
        let depth = FAR_DEPTH * (1.0 - rng.random_range(0.0f32..1.0).sqrt());
        let (pos, size, alpha) = deep_star_visual(dir, reach, depth);
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, alpha),
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_translation(pos.extend(-70.0)),
            DeepStar {
                dir,
                reach,
                depth,
                speed,
            },
            FlightUi,
        ));
    }

    // Warp-speed starfield: each star streams outward from the vanishing
    // point and loops back once it passes the canopy (see `move_stars`).
    // Initial depths are staggered so they don't all launch in lockstep.
    for _ in 0..STAR_COUNT {
        let theta = rng.random_range(0.0..TAU);
        let dir = Vec2::new(theta.cos(), theta.sin());
        let reach = rng.random_range(STAR_MIN_REACH..STAR_MAX_REACH);
        let speed = rng.random_range(STAR_MIN_SPEED..STAR_MAX_SPEED);
        // Area-uniform initial placement, same reasoning as the deep layer.
        let depth = FAR_DEPTH * (1.0 - rng.random_range(0.0f32..1.0).sqrt());
        let (pos, len, alpha) = star_visual(dir, reach, depth);
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, alpha),
                custom_size: Some(Vec2::new(len, 1.6)),
                ..default()
            },
            Transform::from_translation(pos.extend(-50.0))
                .with_rotation(Quat::from_rotation_z(theta)),
            Star {
                dir,
                reach,
                depth,
                speed,
            },
            FlightUi,
        ));
    }

    // Crosshair: a horizontal + vertical bar sharing a center point.
    for size in [Vec2::new(22.0, 3.0), Vec2::new(3.0, 22.0)] {
        commands.spawn((
            Sprite {
                color: Color::srgb(0.3, 1.0, 0.5),
                custom_size: Some(size),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 200.0),
            CrosshairMarker,
            FlightUi,
        ));
    }

    commands.spawn((
        Text::new(
            "Zylon sector: destroy all fighters (Arrows/mouse: aim, Space/click: fire, Esc: retreat)",
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
        FlightUi,
    ));

    commands.spawn((
        Text::new("Remaining: 6"),
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
        StatusText,
        FlightUi,
    ));
}

fn teardown(mut commands: Commands, query: Query<Entity, With<FlightUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<EnemiesRemaining>();
    commands.remove_resource::<CrosshairPos>();
    commands.remove_resource::<SpawnTimer>();
    commands.remove_resource::<EnemyShapes>();
}

fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<SpawnTimer>,
    remaining: Res<EnemiesRemaining>,
    shapes: Res<EnemyShapes>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    existing: Query<(), With<Enemy>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    let alive = existing.iter().count() as u32;
    if alive >= remaining.0 {
        return;
    }
    let mut rng = rand::rng();
    // Base direction, then skewed up to ±25% per axis so approach paths
    // aren't perfectly straight radial lines out of the origin.
    let mut angle = Vec2::new(rng.random_range(-1.0..1.0), rng.random_range(-0.7..0.7));
    angle *= Vec2::new(rng.random_range(0.75..1.25), rng.random_range(0.75..1.25));
    let speed = APPROACH_SPEED_BASE * rng.random_range(0.8..1.3);
    // Origin randomized 0-50% of the way out from the vanishing point (in
    // whatever direction `angle` points), so enemies don't all appear to
    // be born from the exact same dead-center speck.
    let start_t = rng.random_range(0.0..0.5);
    let depth = FAR_DEPTH * (1.0 - start_t);
    let (pos, size) = project(angle, depth);

    let kind = ENEMY_KINDS[rng.random_range(0..ENEMY_KINDS.len())];
    let (hue_min, hue_max) = kind.hue_range();
    let color = Color::hsl(
        rng.random_range(hue_min..hue_max),
        rng.random_range(0.55..0.85),
        rng.random_range(0.45..0.65),
    );

    commands.spawn((
        Mesh2d(shapes.handle(kind)),
        MeshMaterial2d(materials.add(ColorMaterial::from(color))),
        Transform::from_translation(pos.extend(0.0)).with_scale(Vec3::splat(size)),
        Enemy,
        Approach {
            angle,
            depth,
            speed,
        },
        FlightUi,
    ));
}

fn move_crosshair(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut crosshair: ResMut<CrosshairPos>,
    mut query: Query<&mut Transform, With<CrosshairMarker>>,
) {
    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    crosshair.0 = (crosshair.0 + dir * CROSSHAIR_SPEED * time.delta_secs())
        .clamp(Vec2::new(-MAX_SPREAD_X, -MAX_SPREAD_Y), Vec2::new(MAX_SPREAD_X, MAX_SPREAD_Y));

    for mut transform in &mut query {
        transform.translation.x = crosshair.0.x;
        transform.translation.y = crosshair.0.y;
    }
}

/// Aiming by mouse: snaps the crosshair straight to the cursor, but only on
/// frames it actually moved, so it doesn't fight arrow-key nudges.
fn mouse_aim(
    mut motion: MessageReader<CursorMoved>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut crosshair: ResMut<CrosshairPos>,
    mut query: Query<&mut Transform, With<CrosshairMarker>>,
) {
    if motion.is_empty() {
        return;
    }
    motion.clear();
    let Some(world_pos) = cursor_world_pos(&windows, &camera_q) else {
        return;
    };
    crosshair.0 = world_pos.clamp(
        Vec2::new(-MAX_SPREAD_X, -MAX_SPREAD_Y),
        Vec2::new(MAX_SPREAD_X, MAX_SPREAD_Y),
    );
    for mut transform in &mut query {
        transform.translation.x = crosshair.0.x;
        transform.translation.y = crosshair.0.y;
    }
}

fn move_stars(time: Res<Time>, mut query: Query<(&mut Transform, &mut Sprite, &mut Star)>) {
    let mut rng = rand::rng();
    for (mut transform, mut sprite, mut star) in &mut query {
        star.depth -= star.speed * time.delta_secs();
        if star.depth <= 0.0 {
            let theta = rng.random_range(0.0..TAU);
            star.dir = Vec2::new(theta.cos(), theta.sin());
            star.reach = rng.random_range(STAR_MIN_REACH..STAR_MAX_REACH);
            star.speed = rng.random_range(STAR_MIN_SPEED..STAR_MAX_SPEED);
            star.depth = FAR_DEPTH;
            transform.rotation = Quat::from_rotation_z(star.dir.y.atan2(star.dir.x));
        }
        let (pos, len, alpha) = star_visual(star.dir, star.reach, star.depth);
        transform.translation = pos.extend(-50.0);
        sprite.custom_size = Some(Vec2::new(len, 1.6));
        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
}

/// Drifts the deep backdrop opposite your aim — as you look right, distant
/// scenery appears to slide left, the classic background-parallax cue.
fn move_deep_stars(
    time: Res<Time>,
    crosshair: Res<CrosshairPos>,
    mut query: Query<(&mut Transform, &mut Sprite, &mut DeepStar)>,
) {
    let mut rng = rand::rng();
    let offset = -crosshair.0 * DEEP_PARALLAX;
    for (mut transform, mut sprite, mut star) in &mut query {
        star.depth -= star.speed * time.delta_secs();
        if star.depth <= 0.0 {
            let theta = rng.random_range(0.0..TAU);
            star.dir = Vec2::new(theta.cos(), theta.sin());
            star.reach = rng.random_range(DEEP_MIN_REACH..DEEP_MAX_REACH);
            star.speed = rng.random_range(DEEP_MIN_SPEED..DEEP_MAX_SPEED);
            star.depth = FAR_DEPTH;
        }
        let (pos, size, alpha) = deep_star_visual(star.dir, star.reach, star.depth);
        transform.translation.x = pos.x + offset.x;
        transform.translation.y = pos.y + offset.y;
        sprite.custom_size = Some(Vec2::splat(size));
        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
}

fn move_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut campaign: ResMut<Campaign>,
    mut query: Query<(Entity, &mut Transform, &mut Approach), With<Enemy>>,
) {
    for (entity, mut transform, mut approach) in &mut query {
        approach.depth -= approach.speed * time.delta_secs();
        if approach.depth <= 0.0 {
            commands.entity(entity).despawn();
            campaign.fuel = (campaign.fuel - 10.0).max(0.0);
            continue;
        }
        let (pos, size) = project(approach.angle, approach.depth);
        let z = 100.0 * (1.0 - approach.depth / FAR_DEPTH);
        transform.translation = pos.extend(z);
        transform.scale = Vec3::splat(size);
    }
}

fn shoot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    crosshair: Res<CrosshairPos>,
    mut remaining: ResMut<EnemiesRemaining>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
    mut grid: ResMut<GalaxyGrid>,
    campaign: Res<Campaign>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !keys.just_pressed(KeyCode::Space) && !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let mut best: Option<(Entity, f32, Vec2)> = None;
    for (entity, transform) in &enemies {
        let pos = transform.translation.truncate();
        let radius = transform.scale.x / 2.0 + HIT_MARGIN;
        let dist = crosshair.0.distance(pos);
        if dist < radius && best.is_none_or(|(_, best_dist, _)| dist < best_dist) {
            best = Some((entity, dist, pos));
        }
    }

    let target = if let Some((entity, _, pos)) = best {
        commands.entity(entity).despawn();
        if remaining.0 > 0 {
            remaining.0 -= 1;
        }
        commands.spawn((
            Sprite {
                color: Color::srgb(1.0, 0.9, 0.3),
                custom_size: Some(Vec2::splat(40.0)),
                ..default()
            },
            Transform::from_translation(pos.extend(150.0)),
            FadeOut(Timer::from_seconds(FLASH_LIFETIME, TimerMode::Once)),
            FlightUi,
        ));
        pos
    } else {
        crosshair.0
    };

    // Tracer: a thin rect stretched from the gun to the aim point.
    let delta = target - GUN_ORIGIN;
    let midpoint = GUN_ORIGIN + delta / 2.0;
    let length = delta.length().max(1.0);
    let angle = delta.y.atan2(delta.x);
    commands.spawn((
        Sprite {
            color: Color::srgb(0.3, 1.0, 0.5),
            custom_size: Some(Vec2::new(length, 2.0)),
            ..default()
        },
        Transform::from_translation(midpoint.extend(140.0))
            .with_rotation(Quat::from_rotation_z(angle)),
        FadeOut(Timer::from_seconds(TRACER_LIFETIME, TimerMode::Once)),
        FlightUi,
    ));

    if remaining.0 == 0 {
        grid.sectors.insert(campaign.sector, SectorKind::Cleared);
        next_state.set(AppState::GalaxyMap);
    }
}

fn tick_fade_out(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FadeOut, &mut Sprite)>,
) {
    for (entity, mut fade, mut sprite) in &mut query {
        fade.0.tick(time.delta());
        sprite.color = sprite.color.with_alpha(fade.0.fraction_remaining());
        if fade.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn check_abort(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::GalaxyMap);
    }
}

fn update_status_text(remaining: Res<EnemiesRemaining>, mut query: Query<&mut Text, With<StatusText>>) {
    if !remaining.is_changed() {
        return;
    }
    if let Ok(mut text) = query.single_mut() {
        **text = format!("Remaining: {}", remaining.0);
    }
}
