use ai::*;
use assets::*;
use bevy::math::vec3;
use bevy::prelude::*;
use bevy::render::texture::ImageSettings;
use bevy_editor_pls::prelude::*;
use bevy_mod_wanderlust::*;
use bevy_rapier3d::prelude::*;
use rand::prelude::*;
use utils::*;

mod ai;
mod assets;
mod utils;

#[derive(Component)]
struct MainCamera;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum AppState {
    Loading,
    Running,
}

#[derive(Component, Clone)]
struct CustomPhysicsData {
    part_tree_root: Option<Entity>,
}

struct CustomPhysicsHooks;

impl PhysicsHooksWithQuery<&CustomPhysicsData> for CustomPhysicsHooks {
    fn filter_contact_pair(
        &self,
        context: PairFilterContextView,
        user_data: &Query<&CustomPhysicsData>,
    ) -> Option<SolverFlags> {
        let root1 = user_data
            .get(context.collider1())
            .unwrap_or(&CustomPhysicsData {
                part_tree_root: None,
            });
        let root2 = user_data
            .get(context.collider2())
            .unwrap_or(&CustomPhysicsData {
                part_tree_root: None,
            });
        if root1.part_tree_root == root2.part_tree_root {
            None
        } else {
            Some(SolverFlags::all())
        }
    }
}

fn main() {
    App::new()
        .add_state(AppState::Loading)
        .insert_resource(ImageSettings::default_nearest())
        .add_plugins(DefaultPlugins)
        .add_plugin(EditorPlugin)
        .add_plugin(RapierPhysicsPlugin::<&CustomPhysicsData>::default())
        .insert_resource(PhysicsHooksWithQueryResource(Box::new(CustomPhysicsHooks)))
        .add_plugin(WanderlustPlugin)
        .add_plugin(AiPlugin)
        .add_plugin(UtilPlugin)
        .add_plugin(assets::AssetPlugin)
        .add_system_set(SystemSet::on_update(AppState::Loading).with_system(start_game_when_ready))
        .add_system_set(SystemSet::on_enter(AppState::Running).with_system(start_game))
        .add_system_set(
            SystemSet::on_update(AppState::Running)
                .with_system(pass_inputs_to_controller)
                .with_system(animate_moving_parts)
                .with_system(apply_stats),
        )
        .add_startup_system(setup.label("setup"))
        .run();
}

#[derive(Component)]
struct Player;

fn setup(mut commands: Commands) {
    commands
        .spawn_bundle(Camera2dBundle::default())
        .insert(MainCamera);
}

fn start_game_when_ready(
    mut reader: EventReader<PartsLoadedEvent>,
    mut state: ResMut<State<AppState>>,
) {
    if let Some(_) = reader.iter().next() {
        info!("All assets loaded. Starting game.");

        state.set(AppState::Running).unwrap();
    }
}

fn start_game(mut c: Commands, parts: Res<PartTable>) {
    let mut player = c.spawn_part(&parts["Box Chassis"]);
    player
        .insert_bundle((
            Transform::from_xyz(0.0, 250.0, 0.0),
            ControllerSettings {
                acceleration: 100.0,
                max_speed: 100.0,
                max_acceleration_force: 100.0,
                up_vector: Vec3::Y,
                force_scale: vec3(1.0, 1.0, 0.0),
                ..default()
            },
            LockedAxes::TRANSLATION_LOCKED_Z | LockedAxes::ROTATION_LOCKED,
            RigidBody::default(),
            Velocity::default(),
            GravityScale(0.0),
            Sleeping::default(),
            ExternalImpulse::default(),
            ControllerState::default(),
            ControllerInput::default(),
            GlobalTransform::default(),
            Visibility::default(),
            ComputedVisibility::default(),
        ))
        .insert(Player);

    for i in 0..4 {
        player.spawn_part_on_hardpoint(&parts["Float Leg"], i);
    }
    player.spawn_part_on_hardpoint(&parts["Box Head"], 4);

    generate_enemy(&mut c, &*parts);
}

fn pass_inputs_to_controller(
    mut player: Query<&mut ControllerInput, With<Player>>,
    input: Res<Input<KeyCode>>,
) {
    let mut vector = Vec3::ZERO;
    if input.pressed(KeyCode::A) {
        vector += -Vec3::X;
    }
    if input.pressed(KeyCode::D) {
        vector += Vec3::X;
    }
    if input.pressed(KeyCode::S) {
        vector += -Vec3::Y;
    }
    if input.pressed(KeyCode::W) {
        vector += Vec3::Y;
    }
    vector = vector.normalize_or_zero();

    player.single_mut().movement = vector;
}

fn animate_moving_parts(
    roots: Query<(Entity, &Velocity), With<PartTreeRoot>>,
    parents: Query<&Children>,
    mut parts: Query<(&mut PartSprite, &mut Handle<Image>)>,
) {
    for (root, vel) in roots.iter() {
        let mut stack = vec![root];
        while stack.len() > 0 {
            let next = stack.pop().unwrap();

            match parents.get(next) {
                Ok(children) => stack.extend(children.iter()),
                _ => (),
            };

            let (mut sprite, mut image) = match parts.get_mut(next) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let next_sprite = match &mut *sprite {
                PartSprite::Basic(_) => continue,
                PartSprite::Animation {
                    anim,
                    current,
                    rate,
                    timer,
                } => match anim {
                    PartAnimation::OnMove { idle, sequence } => {
                        if vel.linvel.length_squared() > 5.0 {
                            *timer += 1;
                            if timer == rate {
                                *timer = 0;
                                *current += 1;
                            }

                            sequence.wrapping_get(*current).unwrap().clone()
                        } else {
                            idle.clone()
                        }
                    }
                },
            };

            *image = next_sprite;
        }
    }
}

fn apply_stats(mut q: Query<(&PartTreeRoot, &mut ControllerSettings)>) {
    for (root, mut settings) in q.iter_mut() {
        settings.max_speed = root.cumulative_stats.speed.unwrap_or_default();
        settings.acceleration = root.cumulative_stats.acceleration.unwrap_or_default();
        settings.max_acceleration_force = root.cumulative_stats.force.unwrap_or_default();
    }
}

#[derive(Component)]
struct Enemy;

fn generate_enemy(c: &mut Commands, parts: &PartTable) {
    let chassis_parts = parts
        .values()
        .filter(|p| p.def.chassis.unwrap_or_default())
        .collect::<Vec<_>>();
    let chassis = chassis_parts[thread_rng().gen_range(0..chassis_parts.len())];

    let remaining_parts = thread_rng().gen_range(4..7);
    let enemy = c
        .spawn_part(chassis)
        .insert_bundle((AggressiveAi, Enemy))
        .insert_bundle(CharacterControllerBundle {
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            settings: ControllerSettings {
                acceleration: 100.0,
                max_speed: 100.0,
                max_acceleration_force: 100.0,
                up_vector: Vec3::Y,
                force_scale: vec3(1.0, 1.0, 0.0),
                ..default()
            },
            physics: ControllerPhysicsBundle {
                locked_axes: LockedAxes::TRANSLATION_LOCKED_Z | LockedAxes::ROTATION_LOCKED,
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
        let child = child.spawn_part_on_hardpoint(part, hardpoint);
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
