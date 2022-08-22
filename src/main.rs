use bevy::ecs::query::WorldQuery;
use bevy::math::vec3;
use bevy::prelude::*;
use bevy_dolly::{drivers::follow::MovableLookAt, prelude::*};
use bevy_editor_pls::prelude::*;

#[derive(Component)]
struct MainCamera;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EditorPlugin)
        .add_dolly_component(MainCamera)
        .add_startup_system(setup.label("setup"))
        .add_startup_system_to_stage(StartupStage::PostStartup, add_player_skeleton)
        .add_system(update_camera)
        .run();
}

#[derive(Component)]
struct Player;

struct PlayerSkeleton {
    base: Entity,
    neck: Entity,
    leg_fl: Entity,
    leg_fr: Entity,
    leg_bl: Entity,
    leg_br: Entity,
    outer_fl: Entity,
    outer_fr: Entity,
    outer_bl: Entity,
    outer_br: Entity,
}

#[derive(Component)]
enum NeverComponent {}

type HierarchyQuery<'w, 's, 'c, T = ()> = Query<
    'w,
    's,
    (Option<&'c Children>, Option<&'c Parent>, T),
    Or<(With<Children>, With<Parent>)>,
>;

impl PlayerSkeleton {
    pub fn from_scene<F: WorldQuery>(
        scene: Entity,
        query: HierarchyQuery<Option<&Name>>,
    ) -> Option<Self> {
        println!("Step!");
        println!("Inspect: {:?}", query.get(scene));
        let (children, _, _) = query.get(scene).ok()?;
        println!("Step!");
        let &scene = children?.get(0)?;
        println!("Step!");
        let (children, _, _) = query.get(scene).ok()?;
        let &armature = children?.get(0)?;
        println!("Step!");
        let (children, _, _) = query.get(armature).ok()?;
        let &base = children?.into_iter().find(|&&c| {
            query
                .get(c)
                .map(|(_, _, n)| n.map(|n| n.as_str()).unwrap_or("") == "BaseBone")
                .unwrap_or(false)
        })?;
        let (children, _, _) = query.get(base).ok()?;
        println!("Step!");
        let (
            mut neck,
            mut leg_fl,
            mut leg_fr,
            mut leg_bl,
            mut leg_br,
            mut outer_fl,
            mut outer_fr,
            mut outer_bl,
            mut outer_br,
        ) = (None, None, None, None, None, None, None, None, None);
        for &child in children? {
            let (children, _, name) = query.get(child).ok()?;
            match name.map(|n| n.as_str()) {
                Some("NeckBone") => neck = Some(child),
                Some("LegBone.FL") => {
                    leg_fl = Some(child);
                    outer_fl = Some(*children?.into_iter().find(|&&c| {
                        query
                            .get(c)
                            .map(|(_, _, n)| n.map(|n| n.as_str()).unwrap_or("") == "Outer.FL")
                            .unwrap_or(false)
                    })?);
                }
                Some("LegBone.FR") => {
                    leg_fr = Some(child);
                    outer_fr = Some(*children?.into_iter().find(|&&c| {
                        query
                            .get(c)
                            .map(|(_, _, n)| n.map(|n| n.as_str()).unwrap_or("") == "Outer.FR")
                            .unwrap_or(false)
                    })?);
                }
                Some("LegBone.BL") => {
                    leg_bl = Some(child);
                    outer_bl = Some(*children?.into_iter().find(|&&c| {
                        query
                            .get(c)
                            .map(|(_, _, n)| n.map(|n| n.as_str()).unwrap_or("") == "Outer.BL")
                            .unwrap_or(false)
                    })?);
                }
                Some("LegBone.BR") => {
                    leg_br = Some(child);
                    outer_br = Some(*children?.into_iter().find(|&&c| {
                        query
                            .get(c)
                            .map(|(_, _, n)| n.map(|n| n.as_str()).unwrap_or("") == "Outer.BR")
                            .unwrap_or(false)
                    })?);
                }
                _ => (),
            }
        }

        println!("Step!");

        Some(Self {
            base,
            neck: neck?,
            leg_fl: leg_fl?,
            leg_fr: leg_fr?,
            leg_bl: leg_bl?,
            leg_br: leg_br?,
            outer_fl: outer_fl?,
            outer_fr: outer_fr?,
            outer_bl: outer_bl?,
            outer_br: outer_br?,
        })
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn_bundle((
        Rig::builder()
            .with(MovableLookAt::from_position_target(vec3(0.0, 0.0, 0.0)))
            .build(),
        MainCamera,
    ));

    commands
        .spawn_bundle(Camera3dBundle::default())
        .insert(MainCamera);

    commands
        .spawn_bundle(SceneBundle {
            scene: asset_server.load("gltf/robot.glb#Scene0"),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        })
        .insert(Player);

    commands.spawn_bundle(DirectionalLightBundle {
        transform: Transform::from_rotation(Quat::from_axis_angle(Vec3::Y, 45.0f32.to_radians())),
        ..default()
    });
}

fn add_player_skeleton(
    mut c: Commands,
    // mut queries: ParamSet<(Query<Entity, With<Player>>, HierarchyQuery<Option<&Name>>)>,
    mut queries: ParamSet<(
        Query<Entity, With<Player>>,
        Query<
            Entity,
            // (Option<&Children>, Option<&Parent>, Option<&Name>),
            // Or<(With<Children>, With<Parent>)>,
            With<Parent>,
        >,
    )>,
) {
    let player = queries.p0().single();
    println!("Len: {}", queries.p1().iter().len());
    // c.insert_resource(PlayerSkeleton::from_scene::<Entity>(player, queries.p1()).unwrap());
}

fn update_camera(
    player: Query<((&Transform,),), With<Player>>,
    mut rig: Query<&mut Rig, Without<Player>>,
) {
    let ((player,),) = player.single();
    let mut rig = rig.single_mut();

    rig.driver_mut::<MovableLookAt>()
        .set_position_target(player.translation, player.rotation);
}
