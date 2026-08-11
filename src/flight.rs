use bevy::prelude::*;
use rand::RngExt as _;

use crate::galaxy_map::{GalaxyGrid, SectorKind};
use crate::state::{AppState, Campaign};

const PLAYER_SPEED: f32 = 300.0;
const BULLET_SPEED: f32 = 500.0;
const ENEMY_SPEED: f32 = 80.0;
const SPAWN_INTERVAL: f32 = 1.2;
const ARENA_HALF_WIDTH: f32 = 380.0;
const ARENA_BOTTOM: f32 = -260.0;
const ARENA_TOP: f32 = 260.0;

#[derive(Component)]
struct FlightUi;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Bullet;

#[derive(Component)]
struct Enemy;

#[derive(Component)]
struct StatusText;

#[derive(Resource, Default)]
struct EnemiesRemaining(u32);

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
                    move_player,
                    shoot,
                    move_bullets,
                    move_enemies,
                    bullet_enemy_collisions,
                    check_abort,
                    update_status_text,
                )
                    .run_if(in_state(AppState::Flight)),
            );
    }
}

fn setup(mut commands: Commands) {
    commands.insert_resource(EnemiesRemaining(6));
    commands.insert_resource(SpawnTimer(Timer::from_seconds(
        SPAWN_INTERVAL,
        TimerMode::Repeating,
    )));

    commands.spawn((
        Sprite {
            color: Color::srgb(0.9, 0.9, 0.2),
            custom_size: Some(Vec2::new(28.0, 28.0)),
            ..default()
        },
        Transform::from_xyz(0.0, ARENA_BOTTOM, 0.0),
        Player,
        FlightUi,
    ));

    commands.spawn((
        Text::new("Zylon sector: destroy all fighters (Space to fire, Esc to retreat)"),
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
    commands.remove_resource::<SpawnTimer>();
}

fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<SpawnTimer>,
    remaining: Res<EnemiesRemaining>,
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
    let x = rng.random_range(-ARENA_HALF_WIDTH..ARENA_HALF_WIDTH);
    commands.spawn((
        Sprite {
            color: Color::srgb(0.7, 0.15, 0.15),
            custom_size: Some(Vec2::new(24.0, 24.0)),
            ..default()
        },
        Transform::from_xyz(x, ARENA_TOP, 0.0),
        Enemy,
        FlightUi,
    ));
}

fn move_player(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };
    let mut dx = 0.0;
    if keys.pressed(KeyCode::ArrowLeft) {
        dx -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        dx += 1.0;
    }
    transform.translation.x = (transform.translation.x + dx * PLAYER_SPEED * time.delta_secs())
        .clamp(-ARENA_HALF_WIDTH, ARENA_HALF_WIDTH);
}

fn shoot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, With<Player>>,
) {
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    commands.spawn((
        Sprite {
            color: Color::srgb(0.3, 1.0, 0.5),
            custom_size: Some(Vec2::new(4.0, 14.0)),
            ..default()
        },
        Transform::from_translation(player_transform.translation),
        Bullet,
        FlightUi,
    ));
}

fn move_bullets(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform), With<Bullet>>,
) {
    for (entity, mut transform) in &mut query {
        transform.translation.y += BULLET_SPEED * time.delta_secs();
        if transform.translation.y > ARENA_TOP + 40.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn move_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut campaign: ResMut<Campaign>,
    mut query: Query<(Entity, &mut Transform), With<Enemy>>,
) {
    for (entity, mut transform) in &mut query {
        transform.translation.y -= ENEMY_SPEED * time.delta_secs();
        if transform.translation.y < ARENA_BOTTOM {
            commands.entity(entity).despawn();
            campaign.fuel = (campaign.fuel - 10.0).max(0.0);
        }
    }
}

fn bullet_enemy_collisions(
    mut commands: Commands,
    mut remaining: ResMut<EnemiesRemaining>,
    bullets: Query<(Entity, &Transform), With<Bullet>>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
    mut grid: ResMut<GalaxyGrid>,
    campaign: Res<Campaign>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let mut hit_enemies = Vec::new();
    let mut hit_bullets = Vec::new();

    for (bullet_entity, bullet_transform) in &bullets {
        for (enemy_entity, enemy_transform) in &enemies {
            if hit_enemies.contains(&enemy_entity) {
                continue;
            }
            let dist = bullet_transform
                .translation
                .distance(enemy_transform.translation);
            if dist < 20.0 {
                hit_enemies.push(enemy_entity);
                hit_bullets.push(bullet_entity);
                break;
            }
        }
    }

    for entity in hit_bullets {
        commands.entity(entity).despawn();
    }
    for entity in hit_enemies {
        commands.entity(entity).despawn();
        if remaining.0 > 0 {
            remaining.0 -= 1;
        }
    }

    if remaining.0 == 0 {
        grid.sectors.insert(campaign.sector, SectorKind::Cleared);
        next_state.set(AppState::GalaxyMap);
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
