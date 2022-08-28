use bevy::prelude::*;

mod parts;
mod projectiles;

pub use parts::{
    DefSprite, Hardpoint, Order, Part, PartAnimation, PartBundle, PartChildren, PartCommandsExt,
    PartDef, PartEntityCommandsExt, PartInfo, PartSprite, PartStats, PartTable, PartTreeRoot,
    PartWeapon, PartWeaponDef, PartsLoadedEvent,
};
pub use projectiles::*;

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        let part_loader = parts::PartLoader::from_world(&mut app.world);
        app.add_plugin(ProjectilePlugin)
            .init_resource::<parts::PartHandles>()
            .init_resource::<parts::PartTable>()
            .register_type::<Order>()
            .register_type::<Hardpoint>()
            .register_type::<PartDef>()
            .register_type::<Part>()
            .register_type::<parts::PartChildren>()
            .register_type::<parts::PartStats>()
            .register_type::<parts::PartTreeRoot>()
            .add_event::<parts::PartsLoadedEvent>()
            .add_asset::<parts::PartDef>()
            .add_asset::<parts::Part>()
            .add_asset_loader(part_loader)
            .add_startup_system(parts::load_parts)
            .add_system(parts::track_parts_loaded)
            .add_system_to_stage(CoreStage::PreUpdate, parts::accumulate_part_stats);
    }
}
