use bevy::prelude::*;
use bevy_mod_wanderlust::ControllerInput;

pub struct AggressivePlugin;

impl Plugin for AggressivePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(aggressive_ai);
    }
}

#[derive(Component)]
pub struct AggressiveAi;

fn aggressive_ai(
    mut ai: Query<(&AggressiveAi, &GlobalTransform, &mut ControllerInput), Without<crate::Player>>,
    player: Query<&GlobalTransform, With<crate::Player>>,
) {
    let player = match player.get_single() {
        Ok(p) => p,
        _ => return,
    };

    for (ai, tf, mut input) in ai.iter_mut() {
        let dir = (player.translation() - tf.translation()).normalize();

        input.movement = dir;
    }
}
