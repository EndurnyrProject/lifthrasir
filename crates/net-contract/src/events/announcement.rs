use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// A server broadcast/announcement message.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct AnnouncementReceived {
    pub text: String,
    /// `0xRRGGBB`; `0` means "use client default".
    pub color: u32,
    pub style: AnnouncementStyle,
    pub source_name: String,
}

/// Where/how an announcement is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementStyle {
    Top,
    Center,
    Local,
}
