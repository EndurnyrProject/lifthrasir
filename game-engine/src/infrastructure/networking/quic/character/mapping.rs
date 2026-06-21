use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::infrastructure::networking::char_messages::{
    CharacterCreated, CharacterCreationFailed, CharacterDeleted, CharacterDeletionFailed,
    CharacterServerConnected, CharacterSlotInfoReceived, ZoneServerInfoReceived,
};
use crate::infrastructure::networking::char_types::{self, CharCreationError, CharDeletionError};
use crate::infrastructure::networking::quic::proto::aesir::net;

pub fn character_to_char_info(c: net::Character) -> char_types::CharacterInfo {
    char_types::CharacterInfo {
        char_id: c.gid,
        base_exp: c.base_exp,
        zeny: c.zeny,
        job_exp: c.job_exp,
        job_level: c.job_level,
        body_state: 0,
        health_state: 0,
        option: c.option,
        karma: c.karma,
        manner: c.manner,
        status_point: c.status_point as u16,
        hp: c.hp,
        max_hp: c.max_hp,
        sp: c.sp,
        max_sp: c.max_sp,
        walk_speed: 0,
        class: c.class as u16,
        hair: c.hair as u16,
        body: 0,
        weapon: c.weapon as u16,
        base_level: c.base_level as u16,
        skill_point: c.skill_point as u16,
        head_bottom: c.head_bottom as u16,
        shield: c.shield as u16,
        head_top: c.head_top as u16,
        head_mid: c.head_mid as u16,
        hair_color: c.hair_color as u16,
        clothes_color: c.clothes_color as u16,
        name: c.name,
        str: c.str as u8,
        agi: c.agi as u8,
        vit: c.vit as u8,
        int: c.int as u8,
        dex: c.dex as u8,
        luk: c.luk as u8,
        char_num: c.char_num as u8,
        rename: c.rename as u8, // proto u32, domain u8; 0/1 flag in practice, truncation intentional
        last_map: c.last_map,
        delete_date: c.delete_date as u32, // proto u64 unix ts; aesir sends 0 for non-pending-delete, truncation harmless
        robe: c.robe,
        char_slot_change: 0,
        char_rename: 0,
        sex: c.sex as u8,
    }
}

pub fn char_list_to_connected(l: &net::CharList) -> CharacterServerConnected {
    CharacterServerConnected {
        max_slots: l.valid_slots as u8,
        available_slots: l.normal_slots as u8,
        premium_slots: l.premium_slots as u8,
        characters: l
            .characters
            .iter()
            .cloned()
            .map(character_to_char_info)
            .collect(),
    }
}

pub fn char_list_to_slot_info(l: &net::CharList) -> CharacterSlotInfoReceived {
    CharacterSlotInfoReceived {
        slot_info: char_types::CharacterSlotInfo {
            normal_slots: l.normal_slots as u8,
            premium_slots: l.premium_slots as u8,
            billing_slots: l.billing_slots as u8,
            producible_slots: l.producible_slots as u8,
            valid_slots: l.valid_slots as u8,
        },
    }
}

pub fn zone_server_info_to_event(z: net::ZoneServerInfo) -> ZoneServerInfoReceived {
    // NOTE: aesir is trusted; a malformed ip degrades to 0.0.0.0 rather than panicking.
    let ip = Ipv4Addr::from_str(&z.ip)
        .map(|a| a.octets())
        .unwrap_or([0, 0, 0, 0]);
    ZoneServerInfoReceived {
        zone_server_info: char_types::ZoneServerInfo {
            char_id: z.char_id,
            map_name: z.map_name,
            ip,
            port: z.port as u16,
        },
    }
}

pub fn char_created(c: net::CharCreated) -> Option<CharacterCreated> {
    c.character.map(|ch| CharacterCreated {
        character: character_to_char_info(ch),
    })
}

pub fn char_create_failed(f: net::CharCreateFailed) -> CharacterCreationFailed {
    CharacterCreationFailed {
        error: CharCreationError::from(f.reason_code as u8),
    }
}

pub fn delete_ack(a: net::DeleteCharAck) -> Result<CharacterDeleted, CharacterDeletionFailed> {
    if a.result == 0 {
        Ok(CharacterDeleted { char_id: a.char_id })
    } else {
        Err(CharacterDeletionFailed {
            char_id: a.char_id,
            error: CharDeletionError::from(a.result),
        })
    }
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
            base_exp: 1234,
            job_exp: 567,
            zeny: 9999,
            hp: 4000,
            max_hp: 4200,
            sp: 300,
            max_sp: 350,
            str: 90,
            agi: 80,
            vit: 70,
            int: 1,
            dex: 60,
            luk: 5,
            status_point: 11,
            skill_point: 22,
            hair: 12,
            hair_color: 3,
            clothes_color: 4,
            weapon: 13,
            shield: 2,
            head_top: 100,
            head_mid: 101,
            head_bottom: 102,
            robe: 200,
            char_num: 1,
            last_map: "prontera".into(),
            sex: 1,
            option: 0,
            karma: 0,
            manner: 0,
            rename: 0,
            delete_date: 0,
        }
    }

    #[test]
    fn char_list_maps_to_connected_preserving_appearance() {
        let list = net::CharList {
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
            page_count: 1,
            pincode_enabled: false,
        };

        let connected = char_list_to_connected(&list);

        assert_eq!(connected.max_slots, 12);
        assert_eq!(connected.available_slots, 9);
        assert_eq!(connected.premium_slots, 3);
        assert_eq!(connected.characters.len(), 2);

        let alice = &connected.characters[0];
        assert_eq!(alice.char_id, 150001);
        assert_eq!(alice.name, "Alice");
        assert_eq!(alice.class, 7);
        assert_eq!(alice.base_level, 99);
        assert_eq!(alice.job_level, 50);
        assert_eq!(alice.hair, 12);
        assert_eq!(alice.hair_color, 3);
        assert_eq!(alice.clothes_color, 4);
        assert_eq!(alice.weapon, 13);
        assert_eq!(alice.shield, 2);
        assert_eq!(alice.head_top, 100);
        assert_eq!(alice.head_mid, 101);
        assert_eq!(alice.head_bottom, 102);
        assert_eq!(alice.robe, 200);
        assert_eq!(alice.char_num, 1);
        assert_eq!(alice.sex, 1);

        // omitted runtime/cosmetic fields default to 0
        assert_eq!(alice.body_state, 0);
        assert_eq!(alice.health_state, 0);
        assert_eq!(alice.walk_speed, 0);
        assert_eq!(alice.body, 0);
        assert_eq!(alice.char_slot_change, 0);
        assert_eq!(alice.char_rename, 0);

        assert_eq!(connected.characters[1].name, "Bob");
        assert_eq!(connected.characters[1].char_id, 150002);
    }

    #[test]
    fn char_list_maps_to_slot_info() {
        let list = net::CharList {
            account_id: 2000001,
            normal_slots: 9,
            premium_slots: 3,
            billing_slots: 1,
            producible_slots: 9,
            valid_slots: 12,
            characters: vec![],
            page_count: 1,
            pincode_enabled: false,
        };

        let info = char_list_to_slot_info(&list).slot_info;
        assert_eq!(info.normal_slots, 9);
        assert_eq!(info.premium_slots, 3);
        assert_eq!(info.billing_slots, 1);
        assert_eq!(info.producible_slots, 9);
        assert_eq!(info.valid_slots, 12);
    }

    #[test]
    fn zone_server_info_parses_ip() {
        let z = net::ZoneServerInfo {
            char_id: 150001,
            map_name: "prontera".into(),
            ip: "127.0.0.1".into(),
            port: 5121,
        };

        let event = zone_server_info_to_event(z);
        assert_eq!(event.zone_server_info.char_id, 150001);
        assert_eq!(event.zone_server_info.map_name, "prontera");
        assert_eq!(event.zone_server_info.ip_string(), "127.0.0.1");
        assert_eq!(event.zone_server_info.port, 5121);
    }

    #[test]
    fn zone_server_info_malformed_ip_degrades_to_zero() {
        let z = net::ZoneServerInfo {
            char_id: 1,
            map_name: "prontera".into(),
            ip: "not-an-ip".into(),
            port: 5121,
        };

        let event = zone_server_info_to_event(z);
        assert_eq!(event.zone_server_info.ip_string(), "0.0.0.0");
    }

    #[test]
    fn char_created_none_character_returns_none() {
        assert!(char_created(net::CharCreated { character: None }).is_none());
    }

    #[test]
    fn char_create_failed_maps_reason_code() {
        let failed = char_create_failed(net::CharCreateFailed { reason_code: 0 });
        assert_eq!(failed.error, CharCreationError::NameExists);
    }

    #[test]
    fn delete_ack_result_zero_is_ok() {
        let ok = delete_ack(net::DeleteCharAck {
            char_id: 150001,
            result: 0,
            delete_date: 0,
        });
        match ok {
            Ok(deleted) => assert_eq!(deleted.char_id, 150001),
            Err(_) => panic!("expected deletion success"),
        }
    }

    #[test]
    fn delete_ack_nonzero_result_is_err() {
        let err = delete_ack(net::DeleteCharAck {
            char_id: 150001,
            result: 4,
            delete_date: 0,
        });
        match err {
            Err(failure) => {
                assert_eq!(failure.char_id, 150001);
                assert_eq!(failure.error, CharDeletionError::CannotDelete);
            }
            Ok(_) => panic!("expected deletion failure"),
        }
    }
}
