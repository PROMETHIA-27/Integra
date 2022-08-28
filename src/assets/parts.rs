use super::projectiles::*;
use bevy::asset::{AssetLoader, LoadContext, LoadState, LoadedAsset};
use bevy::ecs::system::EntityCommands;
use bevy::math::vec3;
use bevy::prelude::*;
use bevy::reflect::{FromReflect, TypeUuid};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::{CompressedImageFormats, ImageType};
use bevy::utils::{HashMap, Instant};
use bevy_rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::utils::UtilCommandExt;
use crate::{CustomPhysicsData, PlayerOwned, EnemyOwned};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Reflect, FromReflect)]
pub enum Order {
    #[serde(rename = "above")]
    Above,
    #[serde(rename = "below")]
    Below,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Reflect, FromReflect)]
pub struct Hardpoint {
    pub position: (f32, f32),
    pub direction: (f32, f32),
    pub order: Order,
}

#[derive(Clone, Debug, Deserialize, Serialize, Reflect, FromReflect)]
#[serde(tag = "type")]
pub enum DefAnimation {
    #[serde(rename = "on move")]
    OnMove { idle: String, sequence: Vec<String> },
    #[serde(rename = "on shoot")]
    OnShoot { idle: String, sequence: Vec<String> },
}

#[derive(Clone, Debug, Deserialize, Serialize, Reflect, FromReflect)]
#[serde(tag = "type")]
pub enum DefSprite {
    #[serde(rename = "basic")]
    Basic { path: String },
    #[serde(rename = "animation")]
    Animation { animation: DefAnimation },
}

impl DefSprite {
    pub fn is_animation(&self) -> bool {
        match self {
            DefSprite::Basic { .. } => false,
            DefSprite::Animation { .. } => true,
        }
    }
}

#[derive(Component, Copy, Clone, Default, Debug, Deserialize, Serialize, Reflect, FromReflect)]
#[reflect(Component)]
pub struct PartStats {
    pub hp: u32,
    pub speed: Option<f32>,
    pub acceleration: Option<f32>,
    pub force: Option<f32>,
}

impl std::ops::Add<PartStats> for PartStats {
    type Output = Self;

    fn add(self, rhs: PartStats) -> Self::Output {
        Self {
            hp: self.hp + rhs.hp,
            speed: Some(self.speed.unwrap_or_default() + rhs.speed.unwrap_or_default()),
            acceleration: Some(
                self.acceleration.unwrap_or_default() + rhs.acceleration.unwrap_or_default(),
            ),
            force: Some(self.force.unwrap_or_default() + rhs.force.unwrap_or_default()),
        }
    }
}

impl std::ops::AddAssign<PartStats> for PartStats {
    fn add_assign(&mut self, rhs: PartStats) {
        *self = *self + rhs;
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Reflect, FromReflect)]
#[serde(tag = "type")]
pub enum PartWeaponDef {
    #[serde(rename = "projectile")]
    Projectile {
        spread: f32,
        cooldown: f32,
        projectile: WeaponProjectileDef,
    },
}

#[derive(Component, Clone, Debug, Deserialize, Serialize, TypeUuid, Reflect, FromReflect)]
#[uuid = "c3eda9f1-b731-4156-ae80-173056a0f25b"]
pub struct PartDef {
    pub name: String,
    pub origin: (f32, f32),
    pub direction: (f32, f32),
    pub stay_upright: Option<bool>,
    pub chassis: Option<bool>,
    pub sprite: DefSprite,
    pub stats: PartStats,
    pub hardpoints: Vec<Hardpoint>,
    pub weapon: Option<PartWeaponDef>,
}

impl PartDef {
    pub fn hardpoints(&self) -> impl Iterator<Item = (Vec2, Vec2, Order)> + '_ {
        self.hardpoints.iter().map(move |point| {
            (
                Vec2::from(point.position),
                Vec2::from(point.direction).normalize(),
                point.order,
            )
        })
    }
}

#[derive(Clone, Debug, Reflect, FromReflect)]
pub enum PartAnimation {
    OnMove {
        idle: Handle<Image>,
        sequence: Vec<Handle<Image>>,
    },
    OnShoot {
        idle: Handle<Image>,
        sequence: Vec<Handle<Image>>,
    },
}

#[derive(Component, Clone, Debug, Reflect, FromReflect)]
pub enum PartSprite {
    Basic(Handle<Image>),
    Animation {
        current: usize,
        rate: usize,
        timer: usize,
        anim: PartAnimation,
    },
}

#[derive(Clone, Debug, Reflect, FromReflect)]
pub enum PartWeapon {
    Projectile {
        spread: f32,
        cooldown: f32,
        last_shot: Instant,
        projectile: WeaponProjectile,
    },
}

#[derive(Clone, Debug, TypeUuid, Reflect, FromReflect)]
#[uuid = "b87ec074-126b-4e1d-9e88-d5ca48e735ea"]
pub struct Part {
    pub def: PartDef,
    pub sprite: PartSprite,
    pub size: (u32, u32),
    pub weapon: Option<PartWeapon>,
}

#[derive(Clone, Component, Deref, DerefMut, Reflect, FromReflect)]
pub struct PartChildren(Vec<Option<Entity>>);

#[derive(Clone, Component, Deref, DerefMut, Reflect, FromReflect)]
pub struct PartParent(Entity);

#[derive(Component, Clone, Default, Reflect, FromReflect)]
#[reflect(Component)]
pub struct PartTreeRoot {
    pub cumulative_stats: PartStats,
}

pub fn accumulate_part_stats(
    mut roots: Query<(Entity, &mut PartTreeRoot)>,
    parts: Query<(&PartStats, Option<&PartChildren>)>,
) {
    let mut stack = vec![];
    for (root_id, mut root) in roots.iter_mut() {
        root.cumulative_stats = default();
        stack.push(root_id);

        while !stack.is_empty() {
            let next = stack.pop().unwrap();

            let (stats, children) = parts.get(next).unwrap();

            children.map(|children| {
                children
                    .iter()
                    .filter_map(|c| c.as_ref())
                    .for_each(|&child| {
                        stack.push(child);
                    })
            });

            root.cumulative_stats += *stats;
        }
    }
}

#[derive(Component, Clone, Debug, Reflect, FromReflect)]
pub struct PartInfo {
    pub weapon: Option<PartWeapon>,
}

#[derive(Bundle, Clone)]
pub struct PartBundle {
    pub def: PartDef,
    pub info: PartInfo,
    pub stats: PartStats,
    pub part_sprite: PartSprite,
    pub part_children: PartChildren,
    pub image: Handle<Image>,
    pub sprite: Sprite,
    pub collider: Collider,
    pub(crate) custom_data: CustomPhysicsData,
    pub active_hooks: ActiveHooks,
    pub mass_properties: ColliderMassProperties,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visibility: ComputedVisibility,
    pub rigidbody: RigidBody,
    pub gravity: GravityScale,
    pub damping: Damping,
    pub locked_axes: LockedAxes,
}

impl PartBundle {
    pub fn new(part: &Part) -> Self {
        let image = match &part.sprite {
            PartSprite::Basic(sprite) => sprite,
            PartSprite::Animation { anim, .. } => match anim {
                PartAnimation::OnMove { idle, .. } => idle,
                PartAnimation::OnShoot { idle, .. } => idle,
            },
        };

        Self {
            def: part.def.clone(),
            info: PartInfo {
                weapon: part.weapon.clone(),
            },
            stats: part.def.stats.clone(),
            part_sprite: part.sprite.clone(),
            part_children: PartChildren(
                std::iter::repeat(None)
                    .take(part.def.hardpoints.len())
                    .collect(),
            ),
            image: image.clone(),
            sprite: Sprite::default(),
            collider: Collider::cuboid(part.size.0 as f32 / 2.0, part.size.1 as f32 / 2.0, 50.0),
            custom_data: CustomPhysicsData {
                part_tree_root: None,
                disable_collision: false,
            },
            active_hooks: ActiveHooks::FILTER_CONTACT_PAIRS,
            mass_properties: ColliderMassProperties::Mass(1.0),
            transform: Transform::from_xyz(-part.def.origin.0, -part.def.origin.1, 0.0),
            global_transform: default(),
            visibility: default(),
            computed_visibility: default(),
            rigidbody: RigidBody::Dynamic,
            gravity: GravityScale(0.0),
            damping: Damping {
                linear_damping: crate::DAMPING_FACTOR,
                angular_damping: crate::DAMPING_FACTOR,
            },
            locked_axes: LockedAxes::TRANSLATION_LOCKED_Z
        }
    }
}

#[derive(Default, Deref, DerefMut)]
pub struct PartHandles(Vec<Handle<Part>>);

#[derive(Default, Deref, DerefMut)]
pub struct PartTable(HashMap<String, Part>);

pub fn load_parts(assets: ResMut<AssetServer>, mut parts: ResMut<PartHandles>) {
    parts.0 = assets
        .load_folder("toml/parts")
        .unwrap()
        .into_iter()
        .map(|handle| handle.typed::<Part>())
        .collect();
    info!("Loading parts...");
}

pub struct PartLoader {
    supported_compressed_formats: CompressedImageFormats,
}

impl FromWorld for PartLoader {
    fn from_world(world: &mut World) -> Self {
        let supported_compressed_formats = match world.get_resource::<RenderDevice>() {
            Some(render_device) => CompressedImageFormats::from_features(render_device.features()),
            None => CompressedImageFormats::all(),
        };
        Self {
            supported_compressed_formats,
        }
    }
}

impl AssetLoader for PartLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async {
            let def = toml::from_slice::<PartDef>(bytes)?;

            let sprite_paths = match &def.sprite {
                DefSprite::Basic { path } => vec![path],
                DefSprite::Animation { animation } => match animation {
                    DefAnimation::OnMove { idle, sequence } => {
                        sequence.into_iter().chain([idle]).collect()
                    }
                    DefAnimation::OnShoot { idle, sequence } => {
                        sequence.into_iter().chain([idle]).collect()
                    }
                },
            };

            let mut sprites = Vec::with_capacity(sprite_paths.len());
            let mut size = None;

            for path in sprite_paths.iter() {
                let ext = std::path::Path::new(path)
                    .extension()
                    .ok_or(bevy::asset::Error::msg("Sprite has invalid extension"))?
                    .to_str()
                    .ok_or(bevy::asset::Error::msg("Sprite has invalid extension"))?;
                let bytes = load_context.read_asset_bytes(path).await?;

                let image = Image::from_buffer(
                    &bytes,
                    ImageType::Extension(ext),
                    self.supported_compressed_formats,
                    true,
                )?;

                let descriptor = &image.texture_descriptor;
                match size {
                    Some(size) => {
                        debug_assert_eq!(size, (descriptor.size.width, descriptor.size.height))
                    }
                    None => size = Some((descriptor.size.width, descriptor.size.height)),
                }

                sprites.push(image);
            }

            let size = size.unwrap();

            let mut sprites = sprites
                .into_iter()
                .enumerate()
                .map(|(i, sprite)| {
                    load_context
                        .set_labeled_asset(&format!("sprite{}", i), LoadedAsset::new(sprite))
                })
                .collect::<Vec<_>>();

            let sprite = match &def.sprite {
                DefSprite::Basic { .. } => PartSprite::Basic(sprites.remove(0)),
                DefSprite::Animation { animation } => match animation {
                    DefAnimation::OnMove { .. } => {
                        let idle = sprites.pop().unwrap();
                        let sequence = sprites;
                        PartSprite::Animation {
                            current: 0,
                            rate: 5,
                            timer: 0,
                            anim: PartAnimation::OnMove { idle, sequence },
                        }
                    }
                    DefAnimation::OnShoot { .. } => {
                        let idle = sprites.pop().unwrap();
                        let sequence = sprites;
                        PartSprite::Animation {
                            current: 0,
                            rate: 5,
                            timer: 0,
                            anim: PartAnimation::OnShoot { idle, sequence },
                        }
                    }
                },
            };

            let weapon = match &def.weapon {
                Some(PartWeaponDef::Projectile {
                    projectile,
                    spread,
                    cooldown,
                }) => {
                    let sprite = {
                        let ext = std::path::Path::new(&projectile.sprite_path)
                            .extension()
                            .ok_or(bevy::asset::Error::msg("Sprite has invalid extension"))?
                            .to_str()
                            .ok_or(bevy::asset::Error::msg("Sprite has invalid extension"))?;
                        let bytes = load_context
                            .read_asset_bytes(&projectile.sprite_path)
                            .await?;

                        Image::from_buffer(
                            &bytes,
                            ImageType::Extension(ext),
                            self.supported_compressed_formats,
                            true,
                        )?
                    };

                    let size = sprite.size().as_uvec2().into();

                    let sprite =
                        load_context.set_labeled_asset("projectile", LoadedAsset::new(sprite));

                    Some(PartWeapon::Projectile {
                        spread: *spread,
                        cooldown: *cooldown,
                        last_shot: Instant::now(),
                        projectile: WeaponProjectile {
                            sprite,
                            size,
                            damage: projectile.damage,
                            velocity: projectile.velocity.unwrap_or_default(),
                            acceleration: projectile.acceleration.unwrap_or_default(),
                        },
                    })
                }
                None => None,
            };

            info!("Part {} loaded", &def.name);

            let sprite_paths = sprite_paths.into_iter().cloned().collect::<Vec<_>>();

            let mut asset = LoadedAsset::new(Part {
                def,
                sprite,
                size,
                weapon,
            });
            for path in sprite_paths {
                asset.add_dependency((&path).into());
            }

            load_context.set_default_asset(asset);

            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["part.toml"]
    }
}

pub struct PartsLoadedEvent;

pub fn track_parts_loaded(
    mut c: Commands,
    assets: Res<AssetServer>,
    parts: Res<Assets<Part>>,
    mut table: ResMut<PartTable>,
    handles: Option<Res<PartHandles>>,
    mut writer: EventWriter<PartsLoadedEvent>,
) {
    let handles = match handles {
        Some(handles) => handles,
        None => return,
    };
    if let LoadState::Loaded = assets.get_group_load_state(handles.iter().map(|h| h.id)) {
        table.0 = handles
            .iter()
            .map(|h| {
                let part = parts.get(h).unwrap().clone();
                (part.def.name.clone(), part)
            })
            .collect();
        c.remove_resource::<PartHandles>();
        writer.send(PartsLoadedEvent);
    }
}

pub trait PartCommandsExt<'w, 's> {
    fn spawn_part<'a>(&'a mut self, part: &Part) -> EntityCommands<'w, 's, 'a>;

    fn attach_part(&mut self, parent: Entity, part: Entity, hardpoint: usize) -> &mut Self;

    fn detach_part(&mut self, part: Entity) -> &mut Self;
    
    fn despawn_part(&mut self, part: Entity) -> &mut Self;
}

impl<'w, 's> PartCommandsExt<'w, 's> for Commands<'w, 's> {
    fn spawn_part<'a>(&'a mut self, part: &Part) -> EntityCommands<'w, 's, 'a> {
        let mut commands = self.spawn();
        let mut bundle = PartBundle::new(part);
        bundle.custom_data.part_tree_root = Some(commands.id());
        commands
            .insert_bundle(bundle)
            .insert_bundle((PartTreeRoot::default(), LockedAxes::TRANSLATION_LOCKED_Z | LockedAxes::ROTATION_LOCKED));
        commands
    }

    fn attach_part(&mut self, parent: Entity, part: Entity, hardpoint: usize) -> &mut Self {
        self.add(move |world: &mut World| {
            let entity = match world.get_entity(parent) {
                Some(entity) => entity,
                None => {
                    warn!("Failed to attach part to entity. Reason: Nonexistent entity.");
                    return;
                },
            };

            let part_tree_root = match entity.get::<CustomPhysicsData>() {
                Some(&CustomPhysicsData { part_tree_root, .. }) => part_tree_root,
                _ => {
                    warn!("Failed to attach part to entity. Reason: Entity did not have CustomPhysicsData.");
                    return;
                },
            };

            let (origin, (pos, dir, order)) = match entity.get::<PartDef>() {
                Some(part) => (part.origin, match part.hardpoints().nth(hardpoint) {
                    Some(hardpoint) => hardpoint,
                    None => {
                        warn!("Failed to attach part to entity. Reason: Invalid hardpoint index {} in part {}.", hardpoint, part.name);
                        return;
                    },
                }),
                None => {
                    warn!("Failed to attach part to entity. Reason: Entity not a part.");
                    return;
                },
            };
            let z = match order {
                Order::Above => 0.1,
                Order::Below => -0.1,
            };

            let ownership = if entity.contains::<PlayerOwned>() {
                1
            } else if entity.contains::<EnemyOwned>() {
                2
            } else {
                0
            };

            let entity_pos = entity.get::<Transform>().unwrap().translation;

            let mut stack = vec![part];
            while !stack.is_empty() {
                let next = stack.pop().unwrap();
                let mut next = world.entity_mut(next);

                if ownership == 1 {
                    next.insert(PlayerOwned);
                } else if ownership == 2 {
                    next.insert(EnemyOwned);
                }

                next.get_mut::<CustomPhysicsData>().unwrap().part_tree_root = part_tree_root;
            }

            let def = world.entity(part).get::<PartDef>().unwrap();
            let part_dir = def.direction.into();
            let mut rot = Quat::from_rotation_arc_2d(part_dir, dir);
            if def.stay_upright.unwrap_or_default()
                && part_dir.angle_between(dir) > 90.0f32.to_radians()
            {
                rot = Quat::from_axis_angle(Vec3::X, 180.0f32.to_radians()) * rot;
            }

            let mut transform = Transform::from_xyz(pos.x - def.origin.0, pos.y - def.origin.1, z);
            let origin = def.origin;
            transform.rotate_around(transform.translation + Vec2::from(origin).extend(0.0), rot);
            let mut joint = FixedJoint::new();
            joint.set_contacts_enabled(false);
            joint.set_local_anchor1(transform.translation);
            joint.set_local_basis1(transform.rotation);
            transform.translation += entity_pos;
            world.entity_mut(part).insert_bundle((transform, ImpulseJoint::new(parent, joint), PartParent(parent), LockedAxes::TRANSLATION_LOCKED_Z)).remove::<PartTreeRoot>();
            
            let mut entity = world.entity_mut(parent);
            
            match entity.get_mut::<PartChildren>() {
                Some(mut children) => match children.get_mut(hardpoint) {
                    Some(slot) => *slot = Some(part),
                    None => {
                        warn!("Failed to add part to PartChildren. Reason: PartChildren not as long as hardpoint list.");
                        return;
                    },
                },
                None => {
                    warn!("Failed to add part to PartChildren. Reason: PartChildren component not present.");
                    return;
                },
            }
        });

        self
    }

    fn detach_part(&mut self, part: Entity) -> &mut Self {
        self.add(move |world: &mut World| {
            let entity = match world.get_entity(part) {
                Some(e) => e,
                None => return,
            };

            if let Some(parent) = entity.get::<PartParent>() {
                let parent_id = parent.0;
                drop(parent);

                let mut parent = world.get_entity_mut(parent_id).unwrap();
                if let Some(mut children) = parent.get_mut::<PartChildren>() {
                    _ = children
                        .iter()
                        .position(|e| e.is_some() && e.unwrap() == part)
                        .map(|idx| children[idx] = None)
                }
            }

            if let Some(children) = world.entity(part).get::<PartChildren>() {
                let children: Vec<Entity> = children.iter().filter_map(|&c| c).collect();
                let mut stack = vec![];
                for child in children {
                    let mut child = world.entity_mut(child);
                    child.remove::<PartParent>();
                    child.remove::<ImpulseJoint>();
                    child.insert(LockedAxes::TRANSLATION_LOCKED_Z | LockedAxes::ROTATION_LOCKED);
                    let id = child.id();
                    stack.clear();
                    stack.push(id);
                    while !stack.is_empty() {
                        let next = stack.pop().unwrap();
                        let mut next = world.entity_mut(next);
                        next.remove::<PlayerOwned>();
                        next.remove::<EnemyOwned>();
                        next.get::<Children>()
                            .map(|children| children.iter().for_each(|&c| stack.push(c)));
                        next.get_mut::<CustomPhysicsData>()
                            .map(|mut physics| {
                                physics.part_tree_root = Some(id); 
                                physics.disable_collision = false; 
                            });

                        next.get::<PartChildren>().unwrap().iter().filter_map(|&c| c).for_each(|c| stack.push(c));
                    }
                }

                world.entity_mut(part).get_mut::<PartChildren>().unwrap().iter_mut().for_each(|c| *c = None);
            }

            let id = part;
            let mut part = world.entity_mut(id);
            part.remove::<ImpulseJoint>();
            part.get_mut::<CustomPhysicsData>().unwrap().part_tree_root = None;
        });
        self
    }

    fn despawn_part(&mut self, part: Entity) -> &mut Self {
        self.detach_part(part).entity(part).despawn();

        self
    }
}

pub trait PartEntityCommandsExt<'w, 's> {
    fn spawn_part_on_hardpoint<'c>(
        &'c mut self,
        part: &Part,
        hardpoint: usize,
        additional_comp: Option<impl Component>
    ) -> EntityCommands<'w, 's, 'c>;
}

impl<'w, 's, 'a> PartEntityCommandsExt<'w, 's> for EntityCommands<'w, 's, 'a> {
    fn spawn_part_on_hardpoint<'c>(
        &'c mut self,
        part: &Part,
        hardpoint: usize,
        additional_comp: Option<impl Component>
    ) -> EntityCommands<'w, 's, 'c> {
        let id = self.id();
        let mut part = self.commands().spawn_bundle(PartBundle::new(part));
        if let Some(comp) = additional_comp {
            part.insert(comp);
        }
        let part = part.id();
        self.commands().attach_part(id, part, hardpoint);

        self.commands().entity(part)
    }
}
