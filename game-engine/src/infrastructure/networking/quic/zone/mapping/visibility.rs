use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::{UnitEntered, UnitLeft};

pub fn unit_spawn(s: net::UnitSpawn) -> UnitEntered {
    UnitEntered {
        gid: s.gid,
        aid: s.aid,
        object_type: s.object_type,
        job: s.job,
        x: s.x,
        y: s.y,
        dir: s.dir,
        speed: s.speed,
        hp: s.hp,
        max_hp: s.max_hp,
        clevel: s.clevel,
        body_state: s.body_state,
        health_state: s.health_state,
        effect_state: s.effect_state,
        head: s.head,
        weapon: s.weapon,
        shield: s.shield,
        accessory: s.accessory,
        accessory2: s.accessory2,
        accessory3: s.accessory3,
        head_palette: s.head_palette,
        body_palette: s.body_palette,
        head_dir: s.head_dir,
        robe: s.robe,
        guild_id: s.guild_id,
        sex: s.sex,
        is_boss: s.is_boss,
        name: s.name,
        moving: s.moving,
        dst_x: s.dst_x,
        dst_y: s.dst_y,
        move_start_time: s.move_start_time,
    }
}

pub fn unit_despawn(d: net::UnitDespawn) -> UnitLeft {
    UnitLeft {
        gid: d.gid,
        reason: d.reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spawn() -> net::UnitSpawn {
        net::UnitSpawn {
            gid: 150001,
            aid: 2000001,
            object_type: 0,
            job: 7,
            x: 100,
            y: 200,
            dir: 4,
            speed: 150,
            hp: 4000,
            max_hp: 4200,
            clevel: 99,
            body_state: 1,
            health_state: 2,
            effect_state: 3,
            head: 12,
            weapon: 13,
            shield: 2,
            accessory: 100,
            accessory2: 101,
            accessory3: 102,
            head_palette: 5,
            body_palette: 6,
            head_dir: 1,
            robe: 200,
            guild_id: 42,
            sex: 1,
            is_boss: false,
            name: "Alice".into(),
            moving: false,
            dst_x: 0,
            dst_y: 0,
            move_start_time: 0,
            virtue: 0,
        }
    }

    #[test]
    fn unit_spawn_moving_preserves_move_fields() {
        let entered = unit_spawn(net::UnitSpawn {
            moving: true,
            dst_x: 110,
            dst_y: 210,
            move_start_time: 555,
            ..sample_spawn()
        });

        assert!(entered.moving);
        assert_eq!(entered.dst_x, 110);
        assert_eq!(entered.dst_y, 210);
        assert_eq!(entered.move_start_time, 555);
        assert_eq!(entered.gid, 150001);
        assert_eq!(entered.aid, 2000001);
        assert_eq!(entered.x, 100);
        assert_eq!(entered.y, 200);
        assert_eq!(entered.name, "Alice");
    }

    #[test]
    fn unit_spawn_idle_has_no_move() {
        let entered = unit_spawn(sample_spawn());

        assert!(!entered.moving);
        assert_eq!(entered.dst_x, 0);
        assert_eq!(entered.dst_y, 0);
        assert_eq!(entered.move_start_time, 0);
    }

    #[test]
    fn unit_spawn_carries_full_appearance() {
        let entered = unit_spawn(sample_spawn());

        assert_eq!(entered.object_type, 0);
        assert_eq!(entered.job, 7);
        assert_eq!(entered.dir, 4);
        assert_eq!(entered.speed, 150);
        assert_eq!(entered.hp, 4000);
        assert_eq!(entered.max_hp, 4200);
        assert_eq!(entered.clevel, 99);
        assert_eq!(entered.body_state, 1);
        assert_eq!(entered.health_state, 2);
        assert_eq!(entered.effect_state, 3);
        assert_eq!(entered.head, 12);
        assert_eq!(entered.weapon, 13);
        assert_eq!(entered.shield, 2);
        assert_eq!(entered.accessory, 100);
        assert_eq!(entered.accessory2, 101);
        assert_eq!(entered.accessory3, 102);
        assert_eq!(entered.head_palette, 5);
        assert_eq!(entered.body_palette, 6);
        assert_eq!(entered.head_dir, 1);
        assert_eq!(entered.robe, 200);
        assert_eq!(entered.guild_id, 42);
        assert_eq!(entered.sex, 1);
        assert!(!entered.is_boss);
    }

    #[test]
    fn unit_despawn_maps_gid_and_reason() {
        let left = unit_despawn(net::UnitDespawn {
            gid: 150001,
            reason: 1,
        });

        assert_eq!(left.gid, 150001);
        assert_eq!(left.reason, 1);
    }
}
