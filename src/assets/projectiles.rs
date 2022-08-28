use std::time::Duration;

use bevy::prelude::*;
use bevy::reflect::FromReflect;
use bevy::utils::Instant;
use bevy_rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::CustomPhysicsData;

use super::parts::*;

pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<WeaponProjectile>()
            .register_type::<Projectile>()
            .add_system(apply_projectiles)
            .add_system(despawn_old_projectiles);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Reflect, FromReflect)]
pub struct WeaponProjectileDef {
    pub sprite_path: String,
    pub damage: u32,
    pub velocity: Option<f32>,
    pub acceleration: Option<f32>,
}

#[derive(Clone, Debug, Reflect, FromReflect)]
pub struct WeaponProjectile {
    pub sprite: Handle<Image>,
    pub size: (u32, u32),
    pub damage: u32,
    pub velocity: f32,
    pub acceleration: f32,
}

#[derive(Component, Reflect, FromReflect)]
pub struct Projectile {
    pub damage: u32,
}

#[derive(Bundle)]
pub struct WeaponProjectileBundle {
    #[bundle]
    pub sprite: SpriteBundle,
    pub velocity: Velocity,
    pub rigidbody: RigidBody,
    pub collider: Collider,
    pub projectile: Projectile,
    pub(crate) custom_physics: CustomPhysicsData,
    pub hooks: ActiveHooks,
    pub locked: LockedAxes,
    pub gravity: GravityScale,
    pub events: ActiveEvents,
    lifetime: ProjectileLifetime,
}

impl WeaponProjectileBundle {
    pub fn new(source: Entity, proj: &WeaponProjectile, pos: Vec3, dir: Vec3) -> Self {
        let dir = dir.normalize_or_zero();
        Self {
            velocity: Velocity {
                linvel: dir * proj.velocity,
                ..default()
            },
            rigidbody: RigidBody::Dynamic,
            collider: Collider::cuboid(proj.size.0 as f32 / 2.0, proj.size.1 as f32 / 2.0, 50.0),
            projectile: Projectile {
                damage: proj.damage,
            },
            custom_physics: CustomPhysicsData {
                part_tree_root: Some(source),
                disable_collision: false,
            },
            hooks: ActiveHooks::FILTER_CONTACT_PAIRS,
            locked: LockedAxes::TRANSLATION_LOCKED_Z | LockedAxes::ROTATION_LOCKED,
            gravity: GravityScale(0.0),
            sprite: SpriteBundle {
                texture: proj.sprite.clone(),
                transform: Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_arc_2d(Vec2::Y, dir.truncate())),
                ..default()
            },
            events: ActiveEvents::COLLISION_EVENTS,
            lifetime: ProjectileLifetime(Instant::now(), Duration::from_secs(30)),
        }
    }
}

fn apply_projectiles(
    mut c: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    projectiles: Query<(Entity, &Projectile)>,
    mut parts: Query<(Entity, &mut PartStats)>,
) {
    for event in collision_events.iter() {
        let (left, right) = match event {
            &CollisionEvent::Started(left, right, _) => (left, right),
            _ => continue,
        };

        let ((proj_id, projectile), (part_id, mut stats)) = if let Ok(p) = projectiles.get(left) {
            if let Ok(stats) = parts.get_mut(right) {
                (p, stats)
            } else {
                continue;
            }
        } else if let Ok(p) = projectiles.get(right) {
            if let Ok(stats) = parts.get_mut(left) {
                (p, stats)
            } else {
                continue;
            }
        } else {
            continue;
        };

        c.entity(proj_id).despawn_recursive();

        stats.hp = stats.hp.saturating_sub(projectile.damage);
        if stats.hp == 0 {
            c.despawn_part(part_id);
        }
    }
}

#[derive(Component, Clone, Debug)]
struct ProjectileLifetime(Instant, Duration);

fn despawn_old_projectiles(mut c: Commands, projectiles: Query<(Entity, &ProjectileLifetime)>) {
    for (id, projectile) in projectiles.iter() {
        if projectile.0.elapsed() >= projectile.1 {
            c.entity(id).despawn();
        }
    }
}
