use bevy_auto_plugin::prelude::{AutoPlugin, auto_add_plugin};

pub mod channels;
pub mod character;
pub mod connection;
pub mod dispatch;
pub mod envelope;
pub mod login;
pub mod proto;
pub mod send;
pub mod zone;

#[auto_add_plugin(plugin = AesirNetPlugin, init)]
use bevy_quinnet::client::QuinnetClientPlugin;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct AesirNetPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn aesir_plugin_inits_zone_state() {
        let mut app = App::new();
        app.add_plugins(net_contract::NetContractPlugin);
        app.add_plugins(AesirNetPlugin);

        assert!(
            app.world()
                .contains_resource::<crate::zone::QuicZoneState>()
        );
    }
}
