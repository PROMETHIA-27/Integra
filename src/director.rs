use bevy::math::vec3;
use bevy::prelude::*;
use bevy_mod_wanderlust::*;
use bevy_rapier3d::prelude::*;
use rand::{thread_rng, Rng};

use crate::ai::*;
use crate::assets::*;
use crate::{Enemy, EnemyOwned, Player, DAMPING_FACTOR};

pub struct DirectorPlugin;

impl Plugin for DirectorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpawnTimer(0.0))
            .add_system(update_player_score)
            .add_system(spawn);
    }
}

struct PlayerScore(f32);

fn update_player_score(
    mut c: Commands,
    player: Query<&PartTreeRoot, With<Player>>,
    score: Option<ResMut<PlayerScore>>,
) {
    let player = match player.get_single() {
        Ok(p) => p,
        _ => return,
    };

    let score_val = calculate_score(&player.cumulative_stats);
    if let Some(mut res) = score {
        res.0 = score_val;
    } else {
        c.insert_resource(PlayerScore(score_val));
    }
}

fn calculate_score(stats: &PartStats) -> f32 {
    stats.acceleration.unwrap() * 0.01
        + stats.speed.unwrap() * 0.05
        + stats.force.unwrap() * 0.01
        + stats.hp as f32 * 0.1
}

struct SpawnTimer(f32);

fn spawn(
    mut c: Commands,
    parts: Option<Res<PartTable>>,
    mut timer: ResMut<SpawnTimer>,
    time: Res<Time>,
    score: Option<Res<PlayerScore>>,
    player: Query<&GlobalTransform, With<Player>>,
) {
    let player = player.get_single();

    if player.is_err() {
        return;
    }
    if score.is_none() {
        return;
    }
    if parts.is_none() {
        return;
    }

    let player = player.unwrap();
    let score = score.unwrap().0;
    let parts = parts.unwrap();

    timer.0 -= time.delta_seconds();
    if timer.0 <= 0.0 {
        timer.0 = thread_rng().gen_range(10.0..=10.0 + 400.0 / score);
    } else {
        return;
    }

    let enemy_count = thread_rng().gen_range(1..=1 + (score / 50.0).floor() as usize);
    for _ in 0..enemy_count {
        let range = (score / 20.0) as usize;
        let minimum = (score / 4.0) as usize;
        let part_count = thread_rng().gen_range(minimum..minimum + range);
        let dir =
            Quat::from_axis_angle(Vec3::Z, thread_rng().gen_range(0.0..=std::f32::consts::TAU))
                * Vec3::Y;
        let pos = player.translation() + dir * thread_rng().gen_range(1000.0..=1500.0);
        generate_enemy(&mut c, pos, part_count, &parts);
    }
}

fn generate_enemy(c: &mut Commands, position: Vec3, part_count: usize, parts: &PartTable) {
    let chassis_parts = parts
        .values()
        .filter(|p| p.def.chassis.unwrap_or_default())
        .collect::<Vec<_>>();
    let chassis = chassis_parts[thread_rng().gen_range(0..chassis_parts.len())];
    let extents = Vec2::from((chassis.size.0 as f32, chassis.size.1 as f32)).extend(100.0) / 2.0;

    let remaining_parts = part_count;
    let enemy = c
        .spawn_part(chassis)
        .insert_bundle((AggressiveAi, Enemy, EnemyOwned))
        .insert_bundle(CharacterControllerBundle {
            transform: Transform::from_translation(position),
            settings: ControllerSettings {
                up_vector: Vec3::Y,
                force_scale: vec3(1.0, 1.0, 0.0),
                ..default()
            },
            physics: ControllerPhysicsBundle {
                collider: Collider::cuboid(extents.x, extents.y, extents.z),
                locked_axes: LockedAxes::TRANSLATION_LOCKED_Z | LockedAxes::ROTATION_LOCKED,
                damping: Damping {
                    linear_damping: DAMPING_FACTOR,
                    ..default()
                },
                ..default()
            },
            ..default()
        })
        .id();
    let open_points = chassis
        .def
        .hardpoints
        .iter()
        .enumerate()
        .map(|(i, _)| (enemy, i))
        .collect();

    extend_part_tree(c, parts, remaining_parts, open_points);
}

fn extend_part_tree(
    c: &mut Commands,
    parts: &PartTable,
    mut remaining_parts: usize,
    mut open_points: Vec<(Entity, usize)>,
) {
    while open_points.len() > 0 && remaining_parts > 0 {
        let (entity, hardpoint) =
            open_points.swap_remove(thread_rng().gen_range(0..open_points.len()));
        let part = parts
            .values()
            .nth(thread_rng().gen_range(0..parts.len()))
            .unwrap();

        let mut child = c.entity(entity);
        let child = child.spawn_part_on_hardpoint(part, hardpoint, Some(EnemyOwned));
        let child = child.id();

        open_points.extend(
            part.def
                .hardpoints
                .iter()
                .enumerate()
                .map(|(i, _)| (child, i)),
        );

        remaining_parts -= 1;
    }
}
