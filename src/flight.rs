use bevy::asset::RenderAssetUsages;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use rand::RngExt as _;
use std::f32::consts::TAU;

use crate::galaxy_map::{GalaxyGrid, SectorKind};
use crate::hud::credits_closed;
use crate::hud_bridge::HudStats;
use bevy::input::touch::Touches;

use crate::mouse::{cursor_world_pos, screen_to_world_pos};
use crate::state::{AppState, Campaign, DefeatReason, WarpTarget};
use crate::virtual_input::{VirtualFirePending, VirtualNudge};

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

/// Flat score award per enemy kill. Placeholder scoring: no combo/accuracy
/// bonuses yet, just a tally.
const SCORE_PER_KILL: u32 = 100;
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
const DEEP_MIN_REACH: f32 = 0.3;
const DEEP_MAX_REACH: f32 = 1.05;
const DEEP_MIN_SIZE: f32 = 1.0;
const DEEP_MAX_SIZE: f32 = 3.0;
const DEEP_MIN_ALPHA: f32 = 0.1;
const DEEP_MAX_ALPHA: f32 = 0.45;
// The deep layer gets its own (wider) spread instead of reusing the
// enemy-approach spread — at full reach it needs to actually clear the
// window's corners, or the layer reads as stuck in the middle no matter
// how well the radius distribution is balanced.
const DEEP_SPREAD_X: f32 = 560.0;
const DEEP_SPREAD_Y: f32 = 400.0;
const DEEP_PARALLAX: f32 = 0.18;

// The far backdrop drifts too, but far more subtly than the deep layer —
// it's the most distant thing in the scene, so it should barely seem to
// move at all.
const FAR_PARALLAX: f32 = 0.05;

// The far backdrop: pinprick stars, spawned once and never radiating or
// approaching — they only drift a hair with aim (`FAR_PARALLAX`). Pure
// sense of vast, near-fixed distance behind everything else.
const FAR_STAR_COUNT: usize = 190;
const FAR_STAR_FIELD_HALF_WIDTH: f32 = 650.0;
const FAR_STAR_FIELD_HALF_HEIGHT: f32 = 420.0;
const FAR_STAR_MIN_SIZE: f32 = 1.3;
const FAR_STAR_MAX_SIZE: f32 = 2.6;
const FAR_STAR_MIN_ALPHA: f32 = 0.18;
const FAR_STAR_MAX_ALPHA: f32 = 0.5;
// A rarer scattering of bigger, brighter stars punched through the
// backdrop so the far field reads as pronounced rather than a flat haze.
const FAR_BRIGHT_STAR_CHANCE: f32 = 0.12;
const FAR_BRIGHT_STAR_MIN_SIZE: f32 = 2.6;
const FAR_BRIGHT_STAR_MAX_SIZE: f32 = 3.6;
const FAR_BRIGHT_STAR_MIN_ALPHA: f32 = 0.55;
const FAR_BRIGHT_STAR_MAX_ALPHA: f32 = 0.85;
const STREAK_COLOR_CHANCE: f32 = 0.18;

// Small debris/spark burst spawned wherever an enemy is destroyed.
const EXPLOSION_PARTICLE_COUNT: usize = 10;
const EXPLOSION_MIN_SPEED: f32 = 60.0;
const EXPLOSION_MAX_SPEED: f32 = 220.0;
const EXPLOSION_LIFETIME: f32 = 0.35;

// Free wandering: on top of the straight approach-toward-camera radial
// path, each enemy also gets a Lissajous-style wobble, so the flight path
// reads as a smooth, organic curve rather than a laser-straight line.
const WOBBLE_MIN_FREQ: f32 = 0.5;
const WOBBLE_MAX_FREQ: f32 = 1.6;
const WOBBLE_MIN_AMP: f32 = 25.0;
const WOBBLE_MAX_AMP: f32 = 75.0;

// Enemy laser fire: a telegraphed shot at the player's current aim point.
// Dodge by moving the crosshair away before the timer runs out.
const LASER_DAMAGE: f32 = 16.0;
const LASER_CHARGE_SECONDS: f32 = 0.9;
const LASER_DODGE_MARGIN: f32 = 55.0;
const ENEMY_FIRE_MIN_INTERVAL: f32 = 1.8;
const ENEMY_FIRE_MAX_INTERVAL: f32 = 3.6;
const ENEMY_FIRE_MIN_T: f32 = 0.35;

// Impact feedback for a landed enemy shot: the shooter flashes bright red,
// and the whole screen punches with a brief translucent white flash.
const HIT_FLASH_COLOR: Color = Color::srgb(1.0, 0.08, 0.08);
const HIT_FLASH_LIFETIME: f32 = 0.22;
const IMPACT_FLASH_ALPHA: f32 = 0.5;
const IMPACT_FLASH_LIFETIME: f32 = 0.15;

// "Headlights": an enemy is dim when it's just a speck at the vanishing
// point and brightens up to full color as it closes in, like it's only
// really lit once it's caught in the beam.
const ENEMY_MIN_BRIGHTNESS: f32 = 0.15;

// A radial dark overlay sitting above the starfield: black at
// VIGNETTE_CENTER_ALPHA in the middle, fading to fully transparent by
// VIGNETTE_MAX_RADIUS, so the deep field reads as darker/hazier out toward
// the vanishing point, same as real distance haze. Built from concentric
// non-overlapping rings rather than a single blended shape, so there's no
// alpha-compounding surprises where they meet.
const VIGNETTE_RINGS: usize = 48;
const VIGNETTE_MAX_RADIUS: f32 = 700.0;
const VIGNETTE_CENTER_ALPHA: f32 = 0.5;

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
    let pos = dir * reach * Vec2::new(DEEP_SPREAD_X, DEEP_SPREAD_Y) * t;
    let size = DEEP_MIN_SIZE + (DEEP_MAX_SIZE - DEEP_MIN_SIZE) * t;
    let alpha = DEEP_MIN_ALPHA + (DEEP_MAX_ALPHA - DEEP_MIN_ALPHA) * t;
    (pos, size, alpha)
}

/// Most streaks are white; occasionally roll a tinted one so the warp
/// field isn't monochrome.
fn random_star_tint(rng: &mut rand::rngs::ThreadRng) -> Color {
    if rng.random_range(0.0..1.0) < STREAK_COLOR_CHANCE {
        Color::hsl(
            rng.random_range(0.0..360.0),
            rng.random_range(0.5..0.9),
            rng.random_range(0.62..0.8),
        )
    } else {
        Color::WHITE
    }
}

/// A smooth Lissajous-style wobble layered on top of an enemy's straight
/// radial approach, so its path through the field curves and drifts
/// instead of arrowing straight at the camera. Fades in with `t` (depth
/// progress) so far-away specks don't visibly jitter.
fn wobble_offset(approach: &Approach, elapsed: f32) -> Vec2 {
    let t = (1.0 - (approach.depth / FAR_DEPTH)).clamp(0.0, 1.0);
    Vec2::new(
        (elapsed * approach.wobble_freq.x + approach.wobble_phase).sin(),
        (elapsed * approach.wobble_freq.y + approach.wobble_phase * 1.3).cos(),
    ) * approach.wobble_amp
        * t
}

/// Builds an `points`-pointed star mesh (alternating outer/inner radius),
/// fanned out from a center vertex — for silhouettes no convex primitive
/// can express.
fn star_mesh(points: usize, inner_r: f32, outer_r: f32) -> Mesh {
    let rim = points * 2;
    let mut positions = Vec::with_capacity(rim + 1);
    let mut normals = Vec::with_capacity(rim + 1);
    let mut uvs = Vec::with_capacity(rim + 1);
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push([0.5, 0.5]);
    for i in 0..rim {
        let angle = i as f32 / rim as f32 * TAU + std::f32::consts::FRAC_PI_2;
        let r = if i % 2 == 0 { outer_r } else { inner_r };
        let (x, y) = (angle.cos() * r, angle.sin() * r);
        positions.push([x, y, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([0.5 + x, 0.5 - y]);
    }
    let mut indices = Vec::with_capacity(rim * 3);
    for i in 0..rim {
        indices.push(0u32);
        indices.push((i + 1) as u32);
        indices.push(((i + 1) % rim + 1) as u32);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

/// Builds a tapered, curved tentacle strip: a quadratic-bezier centerline
/// from the body (y=0) down to a tip (y=-1), offset sideways by `curve`,
/// with width shrinking from base to tip. `curve` also determines which
/// way it whips — negative for a left-leaning tentacle, positive for right.
fn tentacle_mesh(curve: f32) -> Mesh {
    const SEGMENTS: usize = 10;
    let p0 = Vec2::new(0.0, 0.0);
    let p1 = Vec2::new(curve * 0.6, -0.5);
    let p2 = Vec2::new(curve, -1.0);
    let bezier = |t: f32| p0.lerp(p1, t).lerp(p1.lerp(p2, t), t);

    let mut positions = Vec::with_capacity((SEGMENTS + 1) * 2);
    let mut normals = Vec::with_capacity((SEGMENTS + 1) * 2);
    let mut uvs = Vec::with_capacity((SEGMENTS + 1) * 2);
    for i in 0..=SEGMENTS {
        let t = i as f32 / SEGMENTS as f32;
        let center = bezier(t);
        let tangent = (bezier((t + 0.01).min(1.0)) - center).normalize_or_zero();
        let normal = Vec2::new(-tangent.y, tangent.x);
        let half_w = 0.12 * (1.0 - t) + 0.015;
        let left = center + normal * half_w;
        let right = center - normal * half_w;
        positions.push([left.x, left.y, 0.0]);
        positions.push([right.x, right.y, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([0.0, t]);
        uvs.push([1.0, t]);
    }
    let mut indices = Vec::with_capacity(SEGMENTS * 6);
    for i in 0..SEGMENTS {
        let a = (i * 2) as u32;
        let (b, c, d) = (a + 1, a + 2, a + 3);
        indices.extend_from_slice(&[a, b, c, b, d, c]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
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
    Spike, // spiky star-beast
    Claw,  // clawed/finned monster
}

const ENEMY_KINDS: [EnemyKind; 8] = [
    EnemyKind::Orb,
    EnemyKind::Ring,
    EnemyKind::Wing,
    EnemyKind::Hex,
    EnemyKind::Blob,
    EnemyKind::Shard,
    EnemyKind::Spike,
    EnemyKind::Claw,
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
            EnemyKind::Spike => star_mesh(7, 0.2, 0.55),
            EnemyKind::Claw => Mesh::from(CircularSector::new(0.55, 1.9).mesh()),
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
            EnemyKind::Spike => (5.0, 40.0),    // molten orange
            EnemyKind::Claw => (100.0, 165.0),  // toxic teal-green
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
    spike: Handle<Mesh>,
    claw: Handle<Mesh>,
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
            EnemyKind::Spike => self.spike.clone(),
            EnemyKind::Claw => self.claw.clone(),
        }
    }
}

/// One-shot sound effect handles, loaded once on entering the sector and
/// reused for every player shot, enemy shot, kill, and hit.
#[derive(Resource)]
struct SfxHandles {
    laser: Handle<AudioSource>,
    enemy_laser: Handle<AudioSource>,
    explosion: Handle<AudioSource>,
    impact: Handle<AudioSource>,
}

/// Facial/body-horror trim spawned as children on every enemy so silhouettes
/// read as living monsters rather than bare geometric shapes: angry glowing
/// eyes with angled brows, a pair of fangs, and a fan of curling tentacles.
/// Meshes are built once (unit scale, matching the body's ~1x1 bounding
/// box) and reused across every spawn.
#[derive(Resource)]
struct MonsterFeatures {
    eye_socket: Handle<Mesh>,
    pupil: Handle<Mesh>,
    brow: Handle<Mesh>,
    fang: Handle<Mesh>,
    tentacle_l: Handle<Mesh>,
    tentacle_c: Handle<Mesh>,
    tentacle_r: Handle<Mesh>,
    socket_material: Handle<ColorMaterial>,
    pupil_material: Handle<ColorMaterial>,
    fang_material: Handle<ColorMaterial>,
    tentacle_material: Handle<ColorMaterial>,
}

#[derive(Component)]
struct FlightUi;

#[derive(Component)]
struct Enemy;

/// The enemy's true body color, so a `HitFlash` always has something
/// correct to revert to even if flashes overlap.
#[derive(Component)]
struct BaseColor(Color);

/// Briefly overrides an enemy's material to bright red when one of its
/// shots lands, then `tick_hit_flash` restores `BaseColor`.
#[derive(Component)]
struct HitFlash(Timer);

#[derive(Component)]
struct Approach {
    angle: Vec2,
    depth: f32,
    speed: f32,
    wobble_phase: f32,
    wobble_freq: Vec2,
    wobble_amp: f32,
    fire_timer: Timer,
}

/// A telegraphed enemy shot at the player's aim point: the sprite grows
/// and pulses over `timer`, and on expiry damages the player unless the
/// crosshair has moved clear of `target`.
#[derive(Component)]
struct EnemyLaser {
    target: Vec2,
    timer: Timer,
    source: Entity,
}

#[derive(Component)]
struct Star {
    dir: Vec2,
    reach: f32,
    depth: f32,
    speed: f32,
    tint: Color,
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

/// A fixed pinprick in the far backdrop: never radiates or approaches, but
/// still drifts a little against your aim (see `move_far_stars`) — the
/// faintest, slowest parallax layer, well behind `DeepStar`.
#[derive(Component)]
struct FarStar {
    base_pos: Vec2,
}

#[derive(Component)]
struct CrosshairMarker;

#[derive(Component)]
struct FadeOut(Timer);

/// A single spark/debris chip from a destroyed enemy: drifts outward on
/// `velocity` while `FadeOut` (on the same entity) handles its lifetime and
/// alpha fade.
#[derive(Component)]
struct ExplosionParticle {
    velocity: Vec2,
}

#[derive(Resource, Default)]
struct EnemiesRemaining(u32);

#[derive(Resource, Default)]
struct CrosshairPos(Vec2);

/// Eased trailing copy of `CrosshairPos`, used only to offset the
/// background star layers (`move_far_stars`/`move_deep_stars`). A touch tap
/// snaps `CrosshairPos` straight to the tap point — great for aiming, but
/// fed directly into the parallax offset it read as a jump-cut in the
/// starfield the instant a finger landed. Chasing the raw crosshair with
/// this smoothed value instead keeps the parallax drifting continuously no
/// matter how abruptly the aim point itself moves.
#[derive(Resource, Default)]
struct ParallaxPos(Vec2);

/// How fast `ParallaxPos` closes the gap to `CrosshairPos`, in "fraction of
/// the remaining distance per second" (exponential smoothing). Higher is
/// snappier; this is tuned to still read as an instant response to mouse/key
/// aiming while smoothing out a touch tap's sudden jump.
const PARALLAX_SMOOTHING: f32 = 6.0;

#[derive(Resource)]
struct SpawnTimer(Timer);

/// Per-level difficulty knobs, computed once on entering the sector.
#[derive(Resource, Clone, Copy)]
struct Difficulty {
    enemy_count: u32,
    speed_mult: f32,
}

impl Difficulty {
    fn for_level(level: u32) -> Self {
        Self {
            enemy_count: (5 + level).min(16),
            speed_mult: (1.0 + (level.saturating_sub(1)) as f32 * 0.08).min(2.0),
        }
    }
}

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
                    touch_aim,
                    update_parallax,
                    move_far_stars,
                    move_stars,
                    move_deep_stars,
                    move_enemies,
                    enemy_fire,
                    move_enemy_lasers,
                    tick_hit_flash,
                    shoot,
                    move_explosion_particles,
                    tick_fade_out,
                    check_abort,
                    push_flight_hud_stats,
                )
                    .chain()
                    .run_if(in_state(AppState::Flight).and_then(credits_closed)),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    campaign: Res<Campaign>,
) {
    commands.insert_resource(SfxHandles {
        laser: asset_server.load("sounds/laser.wav"),
        enemy_laser: asset_server.load("sounds/enemy_laser.wav"),
        explosion: asset_server.load("sounds/explosion.wav"),
        impact: asset_server.load("sounds/impact.wav"),
    });

    let difficulty = Difficulty::for_level(campaign.level);
    commands.insert_resource(difficulty);
    commands.insert_resource(EnemiesRemaining(difficulty.enemy_count));
    commands.insert_resource(CrosshairPos::default());
    commands.insert_resource(ParallaxPos::default());
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
        spike: meshes.add(EnemyKind::Spike.build_mesh()),
        claw: meshes.add(EnemyKind::Claw.build_mesh()),
    });
    commands.insert_resource(MonsterFeatures {
        eye_socket: meshes.add(Circle::new(0.11).mesh()),
        pupil: meshes.add(Circle::new(0.045).mesh()),
        brow: meshes.add(
            Triangle2d::new(
                Vec2::new(-0.1, 0.0),
                Vec2::new(0.1, 0.0),
                Vec2::new(0.0, 0.06),
            )
            .mesh(),
        ),
        fang: meshes.add(
            Triangle2d::new(
                Vec2::new(-0.045, 0.0),
                Vec2::new(0.045, 0.0),
                Vec2::new(0.0, -0.16),
            )
            .mesh(),
        ),
        tentacle_l: meshes.add(tentacle_mesh(-0.35)),
        tentacle_c: meshes.add(tentacle_mesh(0.1)),
        tentacle_r: meshes.add(tentacle_mesh(0.4)),
        socket_material: materials.add(ColorMaterial::from(Color::srgb(0.04, 0.04, 0.05))),
        pupil_material: materials.add(ColorMaterial::from(Color::srgb(1.0, 0.15, 0.08))),
        fang_material: materials.add(ColorMaterial::from(Color::srgb(0.95, 0.95, 0.88))),
        tentacle_material: materials.add(ColorMaterial::from(Color::srgb(0.12, 0.05, 0.08))),
    });

    let mut rng = rand::rng();

    // Far backdrop: pinprick stars, well behind everything else and almost
    // still — they only nudge with aim via `FarStar`/`move_far_stars`.
    for _ in 0..FAR_STAR_COUNT {
        let pos = Vec2::new(
            rng.random_range(-FAR_STAR_FIELD_HALF_WIDTH..FAR_STAR_FIELD_HALF_WIDTH),
            rng.random_range(-FAR_STAR_FIELD_HALF_HEIGHT..FAR_STAR_FIELD_HALF_HEIGHT),
        );
        let (size, alpha) = if rng.random_range(0.0..1.0) < FAR_BRIGHT_STAR_CHANCE {
            (
                rng.random_range(FAR_BRIGHT_STAR_MIN_SIZE..FAR_BRIGHT_STAR_MAX_SIZE),
                rng.random_range(FAR_BRIGHT_STAR_MIN_ALPHA..FAR_BRIGHT_STAR_MAX_ALPHA),
            )
        } else {
            (
                rng.random_range(FAR_STAR_MIN_SIZE..FAR_STAR_MAX_SIZE),
                rng.random_range(FAR_STAR_MIN_ALPHA..FAR_STAR_MAX_ALPHA),
            )
        };
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, alpha),
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_translation(pos.extend(-95.0)),
            FarStar { base_pos: pos },
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
        let tint = random_star_tint(&mut rng);
        let (pos, len, alpha) = star_visual(dir, reach, depth);
        commands.spawn((
            Sprite {
                color: tint.with_alpha(alpha),
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
                tint,
            },
            FlightUi,
        ));
    }

    // Distance-haze vignette: concentric rings above the starfield, black
    // and 50% opaque at the vanishing point, fading to fully clear by the
    // edge — sells the far field as darker/hazier with distance.
    for i in 0..VIGNETTE_RINGS {
        let outer_r = VIGNETTE_MAX_RADIUS * (i + 1) as f32 / VIGNETTE_RINGS as f32;
        let inner_r = VIGNETTE_MAX_RADIUS * i as f32 / VIGNETTE_RINGS as f32;
        // Sample alpha at the band's inner edge (closest to center) so the
        // ring's opacity matches what it's adjacent to, and square the
        // falloff so it thins out gently rather than linearly.
        let band_t = 1.0 - inner_r / VIGNETTE_MAX_RADIUS;
        let alpha = VIGNETTE_CENTER_ALPHA * band_t * band_t;
        let mesh = if i == 0 {
            Mesh::from(Circle::new(outer_r).mesh())
        } else {
            Mesh::from(Annulus::new(inner_r, outer_r).mesh())
        };
        commands.spawn((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgba(0.0, 0.0, 0.0, alpha)))),
            Transform::from_xyz(0.0, 0.0, -10.0),
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

    // No on-canvas instructions or Remaining/Health text here anymore: both
    // read from the fixed 900x650 game space, which the web frontend's
    // cover-fit layout can crop on some aspect ratios (see App.css) -- the
    // in-flight stat readout now lives in the React-rendered HUD overlay
    // instead (pinned to the real screen corners via `hud_bridge.rs`), and
    // the instructions only ever needed to be seen once, on the title
    // screen, where they still are.
}

fn teardown(
    mut commands: Commands,
    query: Query<Entity, With<FlightUi>>,
    mut hud_stats: ResMut<HudStats>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<EnemiesRemaining>();
    commands.remove_resource::<CrosshairPos>();
    commands.remove_resource::<ParallaxPos>();
    commands.remove_resource::<SpawnTimer>();
    commands.remove_resource::<EnemyShapes>();
    commands.remove_resource::<MonsterFeatures>();
    commands.remove_resource::<Difficulty>();
    commands.remove_resource::<SfxHandles>();
    // Hide the HUD overlay's stat readout outside Flight, same as the old
    // on-canvas Remaining/Health text being despawned along with FlightUi
    // above.
    hud_stats.visible = false;
}

fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<SpawnTimer>,
    remaining: Res<EnemiesRemaining>,
    difficulty: Res<Difficulty>,
    shapes: Res<EnemyShapes>,
    monster: Res<MonsterFeatures>,
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
    let speed = APPROACH_SPEED_BASE * rng.random_range(0.8..1.3) * difficulty.speed_mult;
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

    commands
        .spawn((
            Mesh2d(shapes.handle(kind)),
            MeshMaterial2d(materials.add(ColorMaterial::from(color))),
            Transform::from_translation(pos.extend(0.0)).with_scale(Vec3::splat(size)),
            Enemy,
            BaseColor(color),
            Approach {
                angle,
                depth,
                speed,
                wobble_phase: rng.random_range(0.0..TAU),
                wobble_freq: Vec2::new(
                    rng.random_range(WOBBLE_MIN_FREQ..WOBBLE_MAX_FREQ),
                    rng.random_range(WOBBLE_MIN_FREQ..WOBBLE_MAX_FREQ),
                ),
                wobble_amp: rng.random_range(WOBBLE_MIN_AMP..WOBBLE_MAX_AMP),
                fire_timer: Timer::from_seconds(
                    rng.random_range(ENEMY_FIRE_MIN_INTERVAL..ENEMY_FIRE_MAX_INTERVAL),
                    TimerMode::Once,
                ),
            },
            FlightUi,
        ))
        .with_children(|parent| {
            // Mean, glowing eyes with angry inward-angled brows — the same
            // trim on every silhouette so it always reads as a living
            // monster rather than a bare shape.
            for side in [-1.0_f32, 1.0] {
                parent.spawn((
                    Mesh2d(monster.eye_socket.clone()),
                    MeshMaterial2d(monster.socket_material.clone()),
                    Transform::from_xyz(side * 0.16, 0.08, 0.01),
                ));
                parent.spawn((
                    Mesh2d(monster.pupil.clone()),
                    MeshMaterial2d(monster.pupil_material.clone()),
                    Transform::from_xyz(side * 0.16, 0.06, 0.02),
                ));
                parent.spawn((
                    Mesh2d(monster.brow.clone()),
                    MeshMaterial2d(monster.socket_material.clone()),
                    Transform::from_xyz(side * 0.17, 0.19, 0.01)
                        .with_rotation(Quat::from_rotation_z(-side * 0.5)),
                ));
            }

            // A pair of fangs at the bottom of the face.
            for side in [-1.0_f32, 1.0] {
                parent.spawn((
                    Mesh2d(monster.fang.clone()),
                    MeshMaterial2d(monster.fang_material.clone()),
                    Transform::from_xyz(side * 0.08, -0.16, 0.01)
                        .with_rotation(Quat::from_rotation_z(side * 0.12)),
                ));
            }

            // A fan of curling tentacles hanging beneath the body.
            for (mesh, x, s) in [
                (monster.tentacle_l.clone(), -0.22, 0.55),
                (monster.tentacle_c.clone(), 0.0, 0.62),
                (monster.tentacle_r.clone(), 0.22, 0.55),
            ] {
                parent.spawn((
                    Mesh2d(mesh),
                    MeshMaterial2d(monster.tentacle_material.clone()),
                    Transform::from_xyz(x, -0.26, -0.01).with_scale(Vec3::splat(s)),
                ));
            }
        });
}

/// How far the crosshair moves per CSS pixel of drag on the on-screen
/// wheel — a trackpad-style relative sensitivity, not the wheel's own
/// radius, since dragging is a repeatable stroke (lift and drag again) same
/// as a real trackpad, not a one-shot mapping of the whole wheel surface to
/// the whole crosshair range.
const VIRTUAL_NUDGE_SENSITIVITY: f32 = 1.5;

fn move_crosshair(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    virtual_nudge: Res<VirtualNudge>,
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
    // The on-screen wheel (web, tall-portrait layout only) works like a
    // trackpad, not a joystick: it reports drag *movement*, already
    // accumulated for this frame, added straight onto the crosshair
    // position the same way a mouse delta would be — not a held direction
    // scaled by time, and no recentering when the finger lifts.
    crosshair.0 += virtual_nudge.0 * VIRTUAL_NUDGE_SENSITIVITY;
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

/// Aiming by touch: tracks whichever finger is down, same as `mouse_aim`.
/// Firing on the initial touch is handled separately in `shoot` (keyed off
/// `Touches::any_just_pressed`), so a tap moves the crosshair here and
/// fires there in the same frame, then a drag just keeps aiming without
/// re-firing.
fn touch_aim(
    touches: Res<Touches>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut crosshair: ResMut<CrosshairPos>,
    mut query: Query<&mut Transform, With<CrosshairMarker>>,
) {
    let Some(touch) = touches.iter().next() else {
        return;
    };
    let Some(world_pos) = screen_to_world_pos(touch.position(), &camera_q) else {
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

/// Eases `ParallaxPos` toward `CrosshairPos` every frame so the background
/// star layers never see an instant jump in aim point, even when the aim
/// point itself does (a touch tap, most notably).
fn update_parallax(time: Res<Time>, crosshair: Res<CrosshairPos>, mut parallax: ResMut<ParallaxPos>) {
    let t = (PARALLAX_SMOOTHING * time.delta_secs()).min(1.0);
    parallax.0 = parallax.0.lerp(crosshair.0, t);
}

/// Nudges the far backdrop opposite your aim, like `move_deep_stars` but at
/// a fraction of the strength — the most distant layer should barely seem
/// to move.
fn move_far_stars(parallax: Res<ParallaxPos>, mut query: Query<(&mut Transform, &FarStar)>) {
    let offset = -parallax.0 * FAR_PARALLAX;
    for (mut transform, star) in &mut query {
        transform.translation.x = star.base_pos.x + offset.x;
        transform.translation.y = star.base_pos.y + offset.y;
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
            star.tint = random_star_tint(&mut rng);
            transform.rotation = Quat::from_rotation_z(star.dir.y.atan2(star.dir.x));
        }
        let (pos, len, alpha) = star_visual(star.dir, star.reach, star.depth);
        transform.translation = pos.extend(-50.0);
        sprite.custom_size = Some(Vec2::new(len, 1.6));
        sprite.color = star.tint.with_alpha(alpha);
    }
}

/// Drifts the deep backdrop opposite your aim — as you look right, distant
/// scenery appears to slide left, the classic background-parallax cue.
fn move_deep_stars(
    time: Res<Time>,
    parallax: Res<ParallaxPos>,
    mut query: Query<(&mut Transform, &mut Sprite, &mut DeepStar)>,
) {
    let mut rng = rand::rng();
    let offset = -parallax.0 * DEEP_PARALLAX;
    for (mut transform, mut sprite, mut star) in &mut query {
        star.depth -= star.speed * time.delta_secs();
        if star.depth <= 0.0 {
            let theta = rng.random_range(0.0..TAU);
            star.dir = Vec2::new(theta.cos(), theta.sin());
            star.reach = rng.random_range(DEEP_MIN_REACH..DEEP_MAX_REACH);
            star.speed = rng.random_range(DEEP_MIN_SPEED..DEEP_MAX_SPEED);
            // Area-uniform respawn depth, matching the initial setup spread,
            // so replacements land scattered across the whole field instead
            // of every one being reborn dead-center (which reads as a
            // permanent clump in the middle for a layer this slow).
            // Occasionally still lands near FAR_DEPTH, i.e. freshly "born"
            // from the vanishing point.
            star.depth = FAR_DEPTH * (1.0 - rng.random_range(0.0f32..1.0).sqrt());
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
    mut next_state: ResMut<NextState<AppState>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<
        (
            Entity,
            &mut Transform,
            &mut Approach,
            &MeshMaterial2d<ColorMaterial>,
            &BaseColor,
        ),
        (With<Enemy>, Without<HitFlash>),
    >,
) {
    let elapsed = time.elapsed_secs();
    for (entity, mut transform, mut approach, mat_handle, base) in &mut query {
        approach.depth -= approach.speed * time.delta_secs();
        if approach.depth <= 0.0 {
            commands.entity(entity).despawn();
            campaign.fuel = (campaign.fuel - 10.0).max(0.0);
            if campaign.fuel <= 0.0 {
                campaign.defeat_reason = Some(DefeatReason::OutOfFuel);
                next_state.set(AppState::GameOver);
            }
            continue;
        }
        let (pos, size) = project(approach.angle, approach.depth);
        let wobble = wobble_offset(&approach, elapsed);
        let t = (1.0 - approach.depth / FAR_DEPTH).clamp(0.0, 1.0);
        let z = 100.0 * t;
        transform.translation = (pos + wobble).extend(z);
        transform.scale = Vec3::splat(size);

        // Headlights: dim out near the vanishing point, full color once
        // it's closed in. Skipped while a hit-flash is overriding the
        // material (see the `Without<HitFlash>` filter above).
        let brightness = ENEMY_MIN_BRIGHTNESS + (1.0 - ENEMY_MIN_BRIGHTNESS) * t;
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            let s = base.0.to_srgba();
            mat.color = Color::srgba(
                s.red * brightness,
                s.green * brightness,
                s.blue * brightness,
                s.alpha,
            );
        }
    }
}

/// Enemies close enough to be a credible threat occasionally telegraph a
/// laser at the player's current aim point.
fn enemy_fire(
    mut commands: Commands,
    time: Res<Time>,
    crosshair: Res<CrosshairPos>,
    sfx: Res<SfxHandles>,
    mut enemies: Query<(Entity, &mut Approach), With<Enemy>>,
) {
    let mut rng = rand::rng();
    for (entity, mut approach) in &mut enemies {
        if !approach.fire_timer.tick(time.delta()).just_finished() {
            continue;
        }
        let t = 1.0 - approach.depth / FAR_DEPTH;
        approach.fire_timer = Timer::from_seconds(
            rng.random_range(ENEMY_FIRE_MIN_INTERVAL..ENEMY_FIRE_MAX_INTERVAL),
            TimerMode::Once,
        );
        if t < ENEMY_FIRE_MIN_T {
            continue;
        }
        commands.spawn((AudioPlayer(sfx.enemy_laser.clone()), PlaybackSettings::DESPAWN));
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 0.25, 0.2, 0.0),
                custom_size: Some(Vec2::splat(10.0)),
                ..default()
            },
            Transform::from_translation(crosshair.0.extend(160.0)),
            EnemyLaser {
                target: crosshair.0,
                timer: Timer::from_seconds(LASER_CHARGE_SECONDS, TimerMode::Once),
                source: entity,
            },
            FlightUi,
        ));
    }
}

/// Grows and pulses each telegraphed laser, then resolves it on expiry:
/// damage if the crosshair is still near the target, a clean miss if the
/// player moved away in time.
fn move_enemy_lasers(
    mut commands: Commands,
    time: Res<Time>,
    crosshair: Res<CrosshairPos>,
    mut campaign: ResMut<Campaign>,
    mut next_state: ResMut<NextState<AppState>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    shooters: Query<&MeshMaterial2d<ColorMaterial>, With<Enemy>>,
    sfx: Res<SfxHandles>,
    mut query: Query<(Entity, &mut Sprite, &mut EnemyLaser)>,
) {
    for (entity, mut sprite, mut laser) in &mut query {
        laser.timer.tick(time.delta());
        let t = laser.timer.fraction();
        let pulse = 0.5 + 0.5 * (t * TAU * 4.0).sin();
        sprite.custom_size = Some(Vec2::splat(10.0 + t * 26.0));
        sprite.color = Color::srgba(1.0, 0.25 + 0.2 * pulse, 0.2, (0.25 + 0.65 * t).min(0.9));

        if laser.timer.is_finished() {
            let dodged = crosshair.0.distance(laser.target) > LASER_DODGE_MARGIN;
            if !dodged {
                campaign.health = (campaign.health - LASER_DAMAGE).max(0.0);
                commands.spawn((AudioPlayer(sfx.impact.clone()), PlaybackSettings::DESPAWN));
                commands.spawn((
                    Sprite {
                        color: Color::srgb(1.0, 0.3, 0.2),
                        custom_size: Some(Vec2::splat(60.0)),
                        ..default()
                    },
                    Transform::from_translation(laser.target.extend(160.0)),
                    FadeOut(Timer::from_seconds(0.25, TimerMode::Once)),
                    FlightUi,
                ));

                // The shooter flashes bright red on impact.
                if let Ok(mat_handle) = shooters.get(laser.source) {
                    if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                        mat.color = HIT_FLASH_COLOR;
                    }
                    commands
                        .entity(laser.source)
                        .insert(HitFlash(Timer::from_seconds(
                            HIT_FLASH_LIFETIME,
                            TimerMode::Once,
                        )));
                }

                // A brief translucent white punch across the whole screen
                // to sell the impact.
                commands.spawn((
                    Sprite {
                        color: Color::srgba(1.0, 1.0, 1.0, IMPACT_FLASH_ALPHA),
                        custom_size: Some(Vec2::splat(4000.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, 300.0),
                    FadeOut(Timer::from_seconds(IMPACT_FLASH_LIFETIME, TimerMode::Once)),
                    FlightUi,
                ));

                if campaign.health <= 0.0 {
                    campaign.defeat_reason = Some(DefeatReason::Destroyed);
                    next_state.set(AppState::GameOver);
                }
            }
            commands.entity(entity).despawn();
        }
    }
}

/// Restores an enemy's true body color once its impact flash expires.
fn tick_hit_flash(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<(
        Entity,
        &MeshMaterial2d<ColorMaterial>,
        &BaseColor,
        &mut HitFlash,
    )>,
) {
    for (entity, mat_handle, base, mut flash) in &mut query {
        if flash.0.tick(time.delta()).is_finished() {
            if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                mat.color = base.0;
            }
            commands.entity(entity).remove::<HitFlash>();
        }
    }
}

fn shoot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut virtual_fire: ResMut<VirtualFirePending>,
    crosshair: Res<CrosshairPos>,
    mut remaining: ResMut<EnemiesRemaining>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
    mut grid: ResMut<GalaxyGrid>,
    mut campaign: ResMut<Campaign>,
    mut next_state: ResMut<NextState<AppState>>,
    mut warp_target: ResMut<WarpTarget>,
    sfx: Res<SfxHandles>,
) {
    // Space/click fire on their own press edge, independent of arrow-key
    // or mouse-motion aiming (those read `pressed`/`CursorMoved` in their
    // own systems and never consume Space's or the mouse button's state) —
    // a tap does the same via `touches.any_just_pressed`, moving the
    // crosshair in `touch_aim` and firing here in the same frame. The
    // on-screen wheel's FIRE button reports through `VirtualFirePending`
    // instead, set by a JS call with no matching Bevy input event of its
    // own to key off; consume it unconditionally below so a stale press
    // from a frame nothing else handled doesn't leak into the next one.
    let fired = virtual_fire.0;
    virtual_fire.0 = false;
    if !keys.just_pressed(KeyCode::Space)
        && !mouse.just_pressed(MouseButton::Left)
        && !touches.any_just_pressed()
        && !fired
    {
        return;
    }
    commands.spawn((AudioPlayer(sfx.laser.clone()), PlaybackSettings::DESPAWN));

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
        campaign.score += SCORE_PER_KILL;
        if remaining.0 > 0 {
            remaining.0 -= 1;
        }
        commands.spawn((AudioPlayer(sfx.explosion.clone()), PlaybackSettings::DESPAWN));
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
        // Small debris/spark burst so a kill reads as a little explosion,
        // not just a flash.
        let mut rng = rand::rng();
        for _ in 0..EXPLOSION_PARTICLE_COUNT {
            let theta = rng.random_range(0.0..TAU);
            let speed = rng.random_range(EXPLOSION_MIN_SPEED..EXPLOSION_MAX_SPEED);
            let velocity = Vec2::new(theta.cos(), theta.sin()) * speed;
            let size = rng.random_range(3.0..8.0);
            let hue = rng.random_range(20.0..55.0);
            commands.spawn((
                Sprite {
                    color: Color::hsl(hue, 0.9, 0.6),
                    custom_size: Some(Vec2::splat(size)),
                    ..default()
                },
                Transform::from_translation(pos.extend(151.0)),
                ExplosionParticle { velocity },
                FadeOut(Timer::from_seconds(EXPLOSION_LIFETIME, TimerMode::Once)),
                FlightUi,
            ));
        }
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
        *warp_target = WarpTarget(AppState::GalaxyMap);
        next_state.set(AppState::Warp);
    }
}

/// Carries destroyed-enemy debris outward; alpha fade and despawn are
/// handled by the `FadeOut` component on the same entity.
fn move_explosion_particles(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &ExplosionParticle)>,
) {
    for (mut transform, particle) in &mut query {
        transform.translation += (particle.velocity * time.delta_secs()).extend(0.0);
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

/// Keeps the React-rendered HUD overlay's stat readout in sync with the
/// live game state (see `hud_bridge.rs`) — replaces what used to be
/// on-canvas Remaining/Health text, which the web frontend's cover-fit
/// layout could crop off-screen depending on the window's aspect ratio.
fn push_flight_hud_stats(
    remaining: Res<EnemiesRemaining>,
    campaign: Res<Campaign>,
    mut hud_stats: ResMut<HudStats>,
) {
    let next = HudStats {
        visible: true,
        remaining: remaining.0,
        health: campaign.health.max(0.0),
        fuel: campaign.fuel.max(0.0),
        score: campaign.score,
        // Not this system's concern — carried through as-is so it doesn't
        // clobber whatever `hud.rs`'s toggle_mute last set.
        muted: hud_stats.muted,
    };
    if *hud_stats != next {
        *hud_stats = next;
    }
}
