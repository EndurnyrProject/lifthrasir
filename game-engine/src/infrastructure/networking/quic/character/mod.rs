pub mod flow;
pub mod mapping;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;
use bevy_quinnet::client::certificate::CertificateVerificationMode;
use bevy_quinnet::client::connection::{ClientAddrConfiguration, ConnectionLocalId};
use bevy_quinnet::client::{
    ClientConnectionConfiguration, ClientConnectionConfigurationDefaultables, QuinnetClient,
};
use bevy_quinnet::shared::error::AsyncChannelError;

use crate::infrastructure::networking::char_types::{self, CharacterSlotInfo};
use crate::infrastructure::networking::quic::channels;
use crate::infrastructure::networking::quic::connection::QuicConnection;
use crate::infrastructure::networking::quic::proto::aesir::net;

/// Phase of the long-lived QUIC char-server session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CharPhase {
    #[default]
    Disconnected,
    Connecting,
    HelloSent,
    AuthSent,
    Ready,
    Selecting,
    Done,
    Failed,
}

/// Session credentials carried from login, sent in `SessionAuth`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PendingAuth {
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: u32,
}

/// Drives the QUIC char-server flow: tracks the session phase, owns the
/// seq-counting `QuicConnection`, and holds the session credentials.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct QuicCharState {
    pub phase: CharPhase,
    pub conn: QuicConnection,
    pub auth: PendingAuth,
}

impl QuicCharState {
    /// Begin a fresh char-server session: reset the seq counter, stash credentials,
    /// and arm the `Connecting` phase so `char_send_hello` fires once the connection opens.
    pub fn start_connecting(&mut self, auth: PendingAuth) {
        self.conn = QuicConnection::default();
        self.auth = auth;
        self.phase = CharPhase::Connecting;
    }
}

/// The current character list and slot allocation as last reported by the char server.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::character_domain_plugin::CharacterDomainAutoPlugin)]
pub struct CharacterRoster {
    pub characters: Vec<char_types::CharacterInfo>,
    pub slot_info: CharacterSlotInfo,
    pub page_count: u32,
}

impl CharacterRoster {
    /// Replace the roster from a server `CharList`.
    pub fn update_from_char_list(&mut self, list: &net::CharList) {
        self.characters = list
            .characters
            .iter()
            .cloned()
            .map(mapping::character_to_char_info)
            .collect();
        self.slot_info = mapping::char_list_to_slot_info(list).slot_info;
        self.page_count = list.page_count;
    }
}

/// Opens the QUIC connection to the aesir char server.
///
/// Closes any existing connection first so the new char connection becomes the
/// unambiguous default for `client.connection_mut()` (one-active-connection invariant;
/// login leaves its connection open). Dev cert handling: `SkipVerification` (self-signed).
pub fn connect(
    client: &mut QuinnetClient,
    addr: &str,
) -> Result<ConnectionLocalId, AsyncChannelError> {
    client.close_all_connections();
    let addr_config = ClientAddrConfiguration::from_strings(addr, "0.0.0.0:0")
        .expect("valid char server address");
    client.open_connection(ClientConnectionConfiguration {
        addr_config,
        cert_mode: CertificateVerificationMode::SkipVerification,
        defaultables: ClientConnectionConfigurationDefaultables {
            send_channels_cfg: channels::send_channels_config(),
            ..Default::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_character(gid: u32, name: &str) -> net::Character {
        net::Character {
            gid,
            name: name.into(),
            class: 7,
            base_level: 99,
            job_level: 50,
            base_exp: 0,
            job_exp: 0,
            zeny: 0,
            hp: 0,
            max_hp: 0,
            sp: 0,
            max_sp: 0,
            str: 0,
            agi: 0,
            vit: 0,
            int: 0,
            dex: 0,
            luk: 0,
            status_point: 0,
            skill_point: 0,
            hair: 0,
            hair_color: 0,
            clothes_color: 0,
            weapon: 0,
            shield: 0,
            head_top: 0,
            head_mid: 0,
            head_bottom: 0,
            robe: 0,
            char_num: 0,
            last_map: "prontera".into(),
            sex: 0,
            option: 0,
            karma: 0,
            manner: 0,
            rename: 0,
            delete_date: 0,
        }
    }

    fn sample_char_list() -> net::CharList {
        net::CharList {
            account_id: 2000001,
            normal_slots: 9,
            premium_slots: 3,
            billing_slots: 0,
            producible_slots: 9,
            valid_slots: 12,
            characters: vec![
                sample_character(150001, "Alice"),
                sample_character(150002, "Bob"),
            ],
            page_count: 2,
            pincode_enabled: false,
        }
    }

    #[test]
    fn start_connecting_resets_and_arms() {
        let mut state = QuicCharState {
            phase: CharPhase::Failed,
            ..Default::default()
        };
        state.start_connecting(PendingAuth {
            account_id: 1,
            login_id1: 2,
            login_id2: 3,
            sex: 1,
        });
        assert_eq!(state.phase, CharPhase::Connecting);
        assert_eq!(state.auth.account_id, 1);
        assert_eq!(state.auth.sex, 1);
    }

    #[test]
    fn update_from_char_list_fills_roster() {
        let mut roster = CharacterRoster::default();
        roster.update_from_char_list(&sample_char_list());

        assert_eq!(roster.characters.len(), 2);
        assert_eq!(roster.characters[0].name, "Alice");
        assert_eq!(roster.characters[1].name, "Bob");
        assert_eq!(roster.slot_info.normal_slots, 9);
        assert_eq!(roster.slot_info.valid_slots, 12);
        assert_eq!(roster.page_count, 2);
    }
}
