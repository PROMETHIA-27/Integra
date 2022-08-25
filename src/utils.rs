use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;

pub struct UtilPlugin;

impl Plugin for UtilPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MarkerPool>()
            .add_startup_system(load_marker);
    }
}

struct MarkerImage(Handle<Image>);

fn load_marker(mut c: Commands, ass: Res<AssetServer>) {
    let handle = ass.load("png/marker.png");
    c.insert_resource(MarkerImage(handle));
}

#[derive(Component)]
struct Marker;

#[derive(Default, Deref, DerefMut)]
struct MarkerPool(Vec<Entity>);

pub trait UtilCommandExt {
    fn spawn_marker(&mut self, x: f32, y: f32) -> &mut Self;
}

impl<'w, 's> UtilCommandExt for Commands<'w, 's> {
    fn spawn_marker(&mut self, x: f32, y: f32) -> &mut Self {
        self.add(move |world: &mut World| {
            let texture = world.resource::<MarkerImage>().0.clone();
            world
                .spawn()
                .insert_bundle(SpriteBundle {
                    transform: Transform::from_xyz(x, y, 0.0),
                    texture,
                    ..default()
                })
                .insert(Marker);
        });
        self
    }
}

pub trait UtilEntityCommandsExt {
    fn spawn_marker_child(&mut self, x: f32, y: f32) -> Entity;
}

impl<'w, 's, 'a> UtilEntityCommandsExt for EntityCommands<'w, 's, 'a> {
    fn spawn_marker_child(&mut self, x: f32, y: f32) -> Entity {
        let marker = self.commands().spawn().id();
        self.commands().add(move |world: &mut World| {
            let texture = world.resource::<MarkerImage>().0.clone();
            world
                .entity_mut(marker)
                .insert_bundle(SpriteBundle {
                    transform: Transform::from_xyz(x, y, 10.0),
                    texture,
                    ..default()
                })
                .insert(Marker);
        });
        self.add_child(marker);
        marker
    }
}

pub trait VecExt<T> {
    fn wrapping_get(&self, index: usize) -> Option<&T>;

    fn wrapping_get_mut(&mut self, index: usize) -> Option<&mut T>;
}

impl<T> VecExt<T> for Vec<T> {
    fn wrapping_get(&self, index: usize) -> Option<&T> {
        let index = index % self.len();
        self.get(index)
    }

    fn wrapping_get_mut(&mut self, index: usize) -> Option<&mut T> {
        let index = index % self.len();
        self.get_mut(index)
    }
}
