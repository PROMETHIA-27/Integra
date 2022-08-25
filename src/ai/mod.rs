use bevy::prelude::*;

mod aggressive;

pub use aggressive::*;

pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(AggressivePlugin);
    }
}
