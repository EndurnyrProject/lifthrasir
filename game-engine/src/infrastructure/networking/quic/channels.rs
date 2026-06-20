use bevy_quinnet::shared::channels::{ChannelConfig, SendChannelsConfiguration};

pub const CONTROL: u8 = 0;
pub const GAMEPLAY: u8 = 1;
pub const WORLD: u8 = 2;
pub const BULK: u8 = 3;
pub const SNAPSHOTS: u8 = 4;

/// The 5 channels in aesir's fixed order; quinnet assigns ids 0..4 by position.
pub fn channel_configs() -> Vec<ChannelConfig> {
    vec![
        ChannelConfig::default_ordered_reliable(), // CONTROL
        ChannelConfig::default_ordered_reliable(), // GAMEPLAY
        ChannelConfig::default_ordered_reliable(), // WORLD
        ChannelConfig::default_ordered_reliable(), // BULK
        ChannelConfig::Unreliable,                 // SNAPSHOTS
    ]
}

pub fn send_channels_config() -> SendChannelsConfiguration {
    SendChannelsConfiguration::from_configs(channel_configs())
        .expect("channel count is within limit")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_has_five_channels() {
        assert_eq!(channel_configs().len(), 5);
    }

    #[test]
    fn snapshots_is_only_unreliable() {
        for (idx, cfg) in channel_configs().iter().enumerate() {
            let is_unreliable = matches!(cfg, ChannelConfig::Unreliable);
            assert_eq!(
                is_unreliable,
                idx == SNAPSHOTS as usize,
                "channel {idx} unreliable={is_unreliable}"
            );
        }
    }
}
