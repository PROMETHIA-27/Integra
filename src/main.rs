use ai::*;
use assets::*;
use bevy::math::{vec2, vec3};
use bevy::prelude::*;
use bevy::render::texture::ImageSettings;
use bevy::utils::Instant;
use bevy_editor_pls::prelude::*;
use bevy_mod_wanderlust::*;
use bevy_rapier3d::prelude::*;
use bevy_rapier3d::rapier::prelude::JointAxesMask;
use director::*;
use rand::prelude::*;
use utils::*;

mod ai;
mod assets;
mod director;
mod utils;

#[derive(Component)]
struct MainCamera;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum AppState {
    Loading,
    Running,
}

#[derive(Component, Clone, Default, Reflect)]
#[reflect(Component)]
struct CustomPhysicsData {
    #[reflect(ignore)]
    part_tree_root: Option<Entity>,
    disable_collision: bool,
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
                disable_collision: false,
            });
        let root2 = user_data
            .get(context.collider2())
            .unwrap_or(&CustomPhysicsData {
                part_tree_root: None,
                disable_collision: false,
            });
        if root1.part_tree_root == root2.part_tree_root {
            None
        } else if root1.disable_collision || root2.disable_collision {
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
        .add_plugin(RapierPhysicsPlugin::<&CustomPhysicsData>::default().with_physics_scale(32.0))
        .add_plugin(RapierDebugRenderPlugin::default())
        .insert_resource(PhysicsHooksWithQueryResource(Box::new(CustomPhysicsHooks)))
        .add_plugin(WanderlustPlugin)
        .add_plugin(AiPlugin)
        .add_plugin(DirectorPlugin)
        .add_plugin(UtilPlugin)
        .add_plugin(assets::AssetPlugin)
        .insert_resource(LastMousePosition(Vec2::ZERO))
        .register_type::<CustomPhysicsData>()
        .add_event::<GrabModeEvent>()
        .add_startup_system(setup_marker_image)
        .add_system_set(SystemSet::on_update(AppState::Loading).with_system(start_game_when_ready))
        .add_system_set(
            SystemSet::on_enter(AppState::Running)
                .with_system(start_game)
                .with_system(spawn_grabby_hand),
        )
        .add_system_set(
            SystemSet::on_update(AppState::Running)
                .label("preupdate")
                .with_system(track_mouse_position),
        )
        .add_system_set(
            SystemSet::on_update(AppState::Running)
                .after("preupdate")
                .with_system(pass_inputs_to_controller)
                .with_system(animate_moving_parts)
                .with_system(apply_stats)
                .with_system(fire_player_weapons)
                .with_system(track_grabby_hand_to_mouse)
                .with_system(grab_parts)
                .with_system(show_markers),
        )
        .add_startup_system(setup.label("setup"))
        .run();
}

#[derive(Component)]
struct Player;

fn setup(mut commands: Commands) {
    let cam_bundle = Camera2dBundle {
        transform: Transform::from_xyz(0.0, 0.0, 500.0),
        ..default()
    };

    commands.spawn_bundle(cam_bundle).insert(MainCamera);
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

#[derive(Component)]
struct PlayerOwned;

const DAMPING_FACTOR: f32 = 4.0;

fn start_game(mut c: Commands, parts: Res<PartTable>) {
    let mut player = c.spawn_part(&parts["Box Chassis"]);
    player
        .insert_bundle((
            Transform::from_xyz(0.0, 0.0, 0.0),
            ControllerSettings {
                acceleration: 100.0,
                max_speed: 100.0,
                max_acceleration_force: 100.0,
                up_vector: Vec3::Y,
                force_scale: vec3(1.0, 1.0, 0.0),
                ..default()
            },
            LockedAxes::TRANSLATION_LOCKED_Z | LockedAxes::ROTATION_LOCKED,
            Damping {
                linear_damping: DAMPING_FACTOR,
                ..default()
            },
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
        .insert_bundle((Player, PlayerOwned));

    for i in 0..4 {
        player.spawn_part_on_hardpoint(&parts["Float Leg"], i, Some(PlayerOwned));
    }
    player
        .spawn_part_on_hardpoint(&parts["Box Head"], 4, Some(PlayerOwned))
        .spawn_part_on_hardpoint(&parts["Blaster"], 4, Some(PlayerOwned));
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
    roots: Query<(Entity, &ControllerInput), With<PartTreeRoot>>,
    parents: Query<&PartChildren>,
    mut parts: Query<(&mut PartSprite, &mut Handle<Image>)>,
) {
    for (root, input) in roots.iter() {
        let mut stack = vec![root];
        while stack.len() > 0 {
            let next = stack.pop().unwrap();

            match parents.get(next) {
                Ok(children) => stack.extend(children.iter().filter_map(|&c| c)),
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
                        if input.movement.length_squared() != 0.0 {
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
                    PartAnimation::OnShoot { idle, .. } => idle.clone(),
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

#[derive(Component)]
struct EnemyOwned;

struct LastMousePosition(Vec2);

fn track_mouse_position(
    mut position: ResMut<LastMousePosition>,
    windows: Res<Windows>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    let (camera, cam_tf) = camera.single();
    let window = windows.get_primary().unwrap();
    let mouse_pos = match window.cursor_position() {
        Some(v) => v,
        None => return,
    };

    let mouse_pos = {
        let window_size = vec2(window.width() as f32, window.height() as f32);
        let ndc = (mouse_pos / window_size) * 2.0 - Vec2::ONE;
        let ndc_to_world = cam_tf.compute_matrix() * camera.projection_matrix().inverse();
        let world_pos = ndc_to_world.project_point3(ndc.extend(-1.0));
        world_pos.truncate()
    };

    position.0 = mouse_pos;
}

fn fire_player_weapons(
    mut c: Commands,
    mouse_button: Res<Input<MouseButton>>,
    mouse_pos: Res<LastMousePosition>,
    player: Query<Entity, With<Player>>,
    mut parts: Query<(&GlobalTransform, &mut PartInfo, Option<&PartChildren>)>,
) {
    if !mouse_button.pressed(MouseButton::Left) {
        return;
    };

    let player = player.single();

    let mut stack = vec![player];
    while !stack.is_empty() {
        let next = stack.pop().unwrap();

        let (tf, mut info, children) = match parts.get_mut(next) {
            Ok(v) => v,
            _ => continue,
        };

        match children {
            Some(children) => stack.extend(children.iter().filter_map(|c| c.as_ref().cloned())),
            _ => (),
        }

        if let Some(weapon) = &mut info.weapon {
            match weapon {
                PartWeapon::Projectile {
                    spread,
                    projectile,
                    cooldown,
                    last_shot,
                } => {
                    if last_shot.elapsed().as_secs_f32() >= *cooldown {
                        let dir = (mouse_pos.0 - tf.translation().truncate()).extend(0.0);
                        let spread = thread_rng()
                            .gen_range(-*spread / 2.0..*spread / 2.0)
                            .to_radians();
                        let dir = Quat::from_axis_angle(Vec3::Z, spread) * dir;
                        let bundle = WeaponProjectileBundle::new(
                            player,
                            projectile,
                            tf.translation() - Vec3::Z,
                            dir,
                        );
                        c.spawn_bundle(bundle);
                        *last_shot = Instant::now();
                    }
                }
            }
        }
    }
}

#[derive(Component)]
struct GrabbyHand;

fn spawn_grabby_hand(mut c: Commands) {
    c.spawn().insert_bundle((
        Transform::default(),
        GlobalTransform::default(),
        RigidBody::KinematicPositionBased,
        GrabbyHand,
    ));
}

fn track_grabby_hand_to_mouse(
    mut hand: Query<&mut Transform, With<GrabbyHand>>,
    mouse_pos: Res<LastMousePosition>,
) {
    let mut hand = match hand.get_single_mut() {
        Ok(v) => v,
        _ => return,
    };

    hand.translation = mouse_pos.0.extend(0.0);
}

#[derive(Component)]
struct Grabbed;

fn grab_parts(
    mut c: Commands,
    mouse_button: Res<Input<MouseButton>>,
    mouse_pos: Res<LastMousePosition>,
    cam: Query<&GlobalTransform, With<MainCamera>>,
    ctx: Res<RapierContext>,
    mut parts: Query<&mut CustomPhysicsData, With<PartDef>>,
    roots: Query<&PartTreeRoot>,
    parents: Query<&PartChildren>,
    hand: Query<(Entity, Option<&ImpulseJoint>), With<GrabbyHand>>,
    grabbed: Query<Entity, With<Grabbed>>,
    player_owned: Query<(), With<PlayerOwned>>,
    enemy_owned: Query<(), With<EnemyOwned>>,
    mut writer: EventWriter<GrabModeEvent>,
    markers: Query<(&GlobalTransform, &HardpointMarker)>,
) {
    if !mouse_button.just_released(MouseButton::Right) {
        return;
    }

    let (hand, joint) = match hand.get_single() {
        Ok(h) => h,
        _ => return,
    };

    if joint.is_some() {
        c.entity(hand).remove::<ImpulseJoint>();
        let grabbed = grabbed.single();
        let root = parts
            .get(grabbed)
            .unwrap()
            .part_tree_root
            .unwrap_or(grabbed);
        set_collision(root, &parents, &mut parts, true);
        c.entity(grabbed).remove::<Grabbed>();
        writer.send(GrabModeEvent::Stopped);

        let marker = markers
            .iter()
            .map(|(tf, h)| {
                (
                    tf,
                    h,
                    tf.translation().truncate().distance_squared(mouse_pos.0),
                )
            })
            .filter(|(_, _, d)| *d < 2500.0)
            .reduce(|(tf, h, d), (tf2, h2, d2)| if d <= d2 { (tf, h, d) } else { (tf2, h2, d2) });

        if let Some((_, m, _)) = marker {
            c.attach_part(m.part.unwrap(), grabbed, m.index);
        }

        return;
    }

    let cam_pos = match cam.get_single() {
        Ok(c) => c.translation(),
        _ => return,
    };

    let (part, _) = match ctx.cast_ray(
        cam_pos,
        mouse_pos.0.extend(0.0) - cam_pos,
        Real::MAX,
        true,
        QueryFilter::new().predicate(&|entity| parts.contains(entity)),
    ) {
        Some(p) => p,
        _ => return,
    };

    if enemy_owned.contains(part) || roots.contains(part) {
        return;
    }

    let joint = GenericJointBuilder::new(JointAxesMask::LIN_AXES)
        .local_anchor1(Vec3::ZERO)
        .build();
    c.entity(hand).insert(ImpulseJoint::new(part, joint));
    c.detach_part(part);
    c.entity(part).insert(Grabbed);

    set_collision(part, &parents, &mut parts, false);
    writer.send(GrabModeEvent::Started(part));
}

fn set_collision(
    root: Entity,
    parents: &Query<&PartChildren>,
    parts: &mut Query<&mut CustomPhysicsData, With<PartDef>>,
    collision: bool,
) {
    let mut stack = vec![root];
    while !stack.is_empty() {
        let next = stack.pop().unwrap();

        match parents.get(next) {
            Ok(children) => children
                .iter()
                .filter_map(|&c| c)
                .for_each(|c| stack.push(c)),
            _ => (),
        }

        parts.get_mut(next).unwrap().disable_collision = !collision;
    }
}

#[derive(Component, Default)]
struct HardpointMarker {
    part: Option<Entity>,
    index: usize,
}

#[derive(Bundle, Default)]
struct HardpointBundle {
    transform: Transform,
    global_transform: GlobalTransform,
    vis: Visibility,
    comp_vis: ComputedVisibility,
    sprite: Sprite,
    texture: Handle<Image>,
    hardpoint: HardpointMarker,
}

impl HardpointBundle {
    fn new(part: &PartDef, part_id: Entity, index: usize, marker: Handle<Image>) -> Self {
        let (pos, _, _) = part.hardpoints().nth(index).unwrap();
        let pos = pos.extend(10.0);

        Self {
            transform: Transform::from_translation(pos),
            sprite: Sprite {
                color: Color::Rgba {
                    red: 0.0,
                    blue: 0.9,
                    green: 0.8,
                    alpha: 0.2,
                },
                ..default()
            },
            texture: marker,
            hardpoint: HardpointMarker {
                part: Some(part_id),
                index,
            },
            ..default()
        }
    }
}

enum GrabModeEvent {
    Started(Entity),
    Stopped,
}

struct MarkerImage(Handle<Image>);

fn setup_marker_image(mut c: Commands, ass: Res<AssetServer>) {
    c.insert_resource(MarkerImage(ass.load("png/square.png")));
}

fn show_markers(
    mut c: Commands,
    mut reader: EventReader<GrabModeEvent>,
    parts: Query<(Entity, &PartDef, &PartChildren), With<PlayerOwned>>,
    marker_img: Res<MarkerImage>,
    markers: Query<Entity, With<HardpointMarker>>,
) {
    for event in reader.iter() {
        match event {
            GrabModeEvent::Started(grabbed) => {
                for (part, def, children) in parts.iter() {
                    if part == *grabbed {
                        continue;
                    }
                    for i in children
                        .iter()
                        .enumerate()
                        .filter_map(|(i, c)| c.is_none().then_some(i))
                    {
                        let marker = HardpointBundle::new(def, part, i, marker_img.0.clone());
                        let marker = c.spawn_bundle(marker).id();
                        c.entity(part).add_child(marker);
                    }
                }
            }
            GrabModeEvent::Stopped => {
                for marker in markers.iter() {
                    c.entity(marker).despawn();
                }
            }
        }
    }
}
