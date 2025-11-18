use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum StatusParameter {
    Speed = 0,
    BaseExp = 1,
    JobExp = 2,
    Karma = 3,
    Manner = 4,
    Hp = 5,
    MaxHp = 6,
    Sp = 7,
    MaxSp = 8,
    StatusPoint = 9,
    BaseLevel = 11,
    SkillPoint = 12,
    Str = 13,
    Agi = 14,
    Vit = 15,
    Int = 16,
    Dex = 17,
    Luk = 18,
    Class = 19,
    Zeny = 20,
    Sex = 21,
    NextBaseExp = 22,
    NextJobExp = 23,
    Weight = 24,
    MaxWeight = 25,
    UStr = 32,
    UAgi = 33,
    UVit = 34,
    UInt = 35,
    UDex = 36,
    ULuk = 37,
    Atk1 = 41,
    Atk2 = 42,
    MAtk1 = 43,
    MAtk2 = 44,
    Def1 = 45,
    Def2 = 46,
    MDef1 = 47,
    MDef2 = 48,
    Hit = 49,
    Flee1 = 50,
    Flee2 = 51,
    Critical = 52,
    Aspd = 53,
    JobLevel = 55,
    Upper = 56,
    Partner = 57,
    Cart = 58,
    Fame = 59,
    Unbreakable = 60,
    CartInfo = 99,
    KilledGid = 118,
    BaseJob = 119,
    BaseClass = 120,
    KillerRid = 121,
    KilledRid = 122,
    Sitting = 123,
    CharMove = 124,
    CharRename = 125,
    CharFont = 126,
    BankVault = 127,
    RouletteBronze = 128,
}

impl StatusParameter {
    pub fn from_var_id(var_id: u16) -> Option<Self> {
        match var_id {
            0 => Some(Self::Speed),
            1 => Some(Self::BaseExp),
            2 => Some(Self::JobExp),
            3 => Some(Self::Karma),
            4 => Some(Self::Manner),
            5 => Some(Self::Hp),
            6 => Some(Self::MaxHp),
            7 => Some(Self::Sp),
            8 => Some(Self::MaxSp),
            9 => Some(Self::StatusPoint),
            11 => Some(Self::BaseLevel),
            12 => Some(Self::SkillPoint),
            13 => Some(Self::Str),
            14 => Some(Self::Agi),
            15 => Some(Self::Vit),
            16 => Some(Self::Int),
            17 => Some(Self::Dex),
            18 => Some(Self::Luk),
            19 => Some(Self::Class),
            20 => Some(Self::Zeny),
            21 => Some(Self::Sex),
            22 => Some(Self::NextBaseExp),
            23 => Some(Self::NextJobExp),
            24 => Some(Self::Weight),
            25 => Some(Self::MaxWeight),
            32 => Some(Self::UStr),
            33 => Some(Self::UAgi),
            34 => Some(Self::UVit),
            35 => Some(Self::UInt),
            36 => Some(Self::UDex),
            37 => Some(Self::ULuk),
            41 => Some(Self::Atk1),
            42 => Some(Self::Atk2),
            43 => Some(Self::MAtk1),
            44 => Some(Self::MAtk2),
            45 => Some(Self::Def1),
            46 => Some(Self::Def2),
            47 => Some(Self::MDef1),
            48 => Some(Self::MDef2),
            49 => Some(Self::Hit),
            50 => Some(Self::Flee1),
            51 => Some(Self::Flee2),
            52 => Some(Self::Critical),
            53 => Some(Self::Aspd),
            55 => Some(Self::JobLevel),
            56 => Some(Self::Upper),
            57 => Some(Self::Partner),
            58 => Some(Self::Cart),
            59 => Some(Self::Fame),
            60 => Some(Self::Unbreakable),
            99 => Some(Self::CartInfo),
            118 => Some(Self::KilledGid),
            119 => Some(Self::BaseJob),
            120 => Some(Self::BaseClass),
            121 => Some(Self::KillerRid),
            122 => Some(Self::KilledRid),
            123 => Some(Self::Sitting),
            124 => Some(Self::CharMove),
            125 => Some(Self::CharRename),
            126 => Some(Self::CharFont),
            127 => Some(Self::BankVault),
            128 => Some(Self::RouletteBronze),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Speed => "Speed",
            Self::BaseExp => "Base Experience",
            Self::JobExp => "Job Experience",
            Self::Karma => "Karma",
            Self::Manner => "Manner",
            Self::Hp => "HP",
            Self::MaxHp => "Max HP",
            Self::Sp => "SP",
            Self::MaxSp => "Max SP",
            Self::StatusPoint => "Status Points",
            Self::BaseLevel => "Base Level",
            Self::SkillPoint => "Skill Points",
            Self::Str => "STR",
            Self::Agi => "AGI",
            Self::Vit => "VIT",
            Self::Int => "INT",
            Self::Dex => "DEX",
            Self::Luk => "LUK",
            Self::Class => "Class",
            Self::Zeny => "Zeny",
            Self::Sex => "Sex",
            Self::NextBaseExp => "Next Base Experience",
            Self::NextJobExp => "Next Job Experience",
            Self::Weight => "Weight",
            Self::MaxWeight => "Max Weight",
            Self::UStr => "Upper STR",
            Self::UAgi => "Upper AGI",
            Self::UVit => "Upper VIT",
            Self::UInt => "Upper INT",
            Self::UDex => "Upper DEX",
            Self::ULuk => "Upper LUK",
            Self::Atk1 => "ATK1",
            Self::Atk2 => "ATK2",
            Self::MAtk1 => "MATK1",
            Self::MAtk2 => "MATK2",
            Self::Def1 => "DEF1",
            Self::Def2 => "DEF2",
            Self::MDef1 => "MDEF1",
            Self::MDef2 => "MDEF2",
            Self::Hit => "HIT",
            Self::Flee1 => "FLEE1",
            Self::Flee2 => "FLEE2",
            Self::Critical => "Critical",
            Self::Aspd => "ASPD",
            Self::JobLevel => "Job Level",
            Self::Upper => "Upper",
            Self::Partner => "Partner",
            Self::Cart => "Cart",
            Self::Fame => "Fame",
            Self::Unbreakable => "Unbreakable",
            Self::CartInfo => "Cart Info",
            Self::KilledGid => "Killed GID",
            Self::BaseJob => "Base Job",
            Self::BaseClass => "Base Class",
            Self::KillerRid => "Killer RID",
            Self::KilledRid => "Killed RID",
            Self::Sitting => "Sitting",
            Self::CharMove => "Character Move",
            Self::CharRename => "Character Rename",
            Self::CharFont => "Character Font",
            Self::BankVault => "Bank Vault",
            Self::RouletteBronze => "Roulette Bronze",
        }
    }

    pub fn uses_long_packet(&self) -> bool {
        matches!(
            self,
            Self::BaseExp | Self::JobExp | Self::NextBaseExp | Self::NextJobExp
        )
    }
}

#[derive(Component, Debug, Clone)]
pub struct CharacterStatus {
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub base_exp: u32,
    pub job_exp: u32,
    pub next_base_exp: u32,
    pub next_job_exp: u32,
    pub base_level: u32,
    pub job_level: u32,
    pub str: u32,
    pub agi: u32,
    pub vit: u32,
    pub int: u32,
    pub dex: u32,
    pub luk: u32,
    pub ustr: u32,
    pub uagi: u32,
    pub uvit: u32,
    pub uint: u32,
    pub udex: u32,
    pub uluk: u32,
    pub status_point: u32,
    pub skill_point: u32,
    pub weight: u32,
    pub max_weight: u32,
    pub zeny: u32,
    pub bank_vault: u32,
    pub atk1: u32,
    pub atk2: u32,
    pub matk1: u32,
    pub matk2: u32,
    pub def1: u32,
    pub def2: u32,
    pub mdef1: u32,
    pub mdef2: u32,
    pub hit: u32,
    pub flee1: u32,
    pub flee2: u32,
    pub critical: u32,
    pub aspd: u32,
    pub speed: u32,
    pub karma: u32,
    pub manner: u32,
    pub fame: u32,
    pub cart: u32,
    pub upper: u32,
    pub partner: u32,
    pub unbreakable: u32,
    pub class: u32,
    pub sex: u32,
    pub cart_info: u32,
    pub killed_gid: u32,
    pub base_job: u32,
    pub base_class: u32,
    pub killer_rid: u32,
    pub killed_rid: u32,
    pub sitting: u32,
    pub char_move: u32,
    pub char_rename: u32,
    pub char_font: u32,
    pub roulette_bronze: u32,
}

impl Default for CharacterStatus {
    fn default() -> Self {
        Self {
            hp: 100,
            max_hp: 100,
            sp: 100,
            max_sp: 100,
            base_exp: 0,
            job_exp: 0,
            next_base_exp: 100,
            next_job_exp: 100,
            base_level: 1,
            job_level: 1,
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
            ustr: 0,
            uagi: 0,
            uvit: 0,
            uint: 0,
            udex: 0,
            uluk: 0,
            status_point: 0,
            skill_point: 0,
            weight: 0,
            max_weight: 2000,
            zeny: 0,
            bank_vault: 0,
            atk1: 0,
            atk2: 0,
            matk1: 0,
            matk2: 0,
            def1: 0,
            def2: 0,
            mdef1: 0,
            mdef2: 0,
            hit: 0,
            flee1: 0,
            flee2: 0,
            critical: 0,
            aspd: 0,
            speed: 150,
            karma: 0,
            manner: 0,
            fame: 0,
            cart: 0,
            upper: 0,
            partner: 0,
            unbreakable: 0,
            class: 0,
            sex: 0,
            cart_info: 0,
            killed_gid: 0,
            base_job: 0,
            base_class: 0,
            killer_rid: 0,
            killed_rid: 0,
            sitting: 0,
            char_move: 0,
            char_rename: 0,
            char_font: 0,
            roulette_bronze: 0,
        }
    }
}

impl CharacterStatus {
    pub fn update_param(&mut self, param: StatusParameter, value: u32) {
        match param {
            StatusParameter::Hp => self.hp = value.min(self.max_hp),
            StatusParameter::MaxHp => self.max_hp = value,
            StatusParameter::Sp => self.sp = value.min(self.max_sp),
            StatusParameter::MaxSp => self.max_sp = value,
            StatusParameter::BaseExp => self.base_exp = value,
            StatusParameter::JobExp => self.job_exp = value,
            StatusParameter::NextBaseExp => self.next_base_exp = value,
            StatusParameter::NextJobExp => self.next_job_exp = value,
            StatusParameter::BaseLevel => self.base_level = value,
            StatusParameter::JobLevel => self.job_level = value,
            StatusParameter::Str => self.str = value,
            StatusParameter::Agi => self.agi = value,
            StatusParameter::Vit => self.vit = value,
            StatusParameter::Int => self.int = value,
            StatusParameter::Dex => self.dex = value,
            StatusParameter::Luk => self.luk = value,
            StatusParameter::UStr => self.ustr = value,
            StatusParameter::UAgi => self.uagi = value,
            StatusParameter::UVit => self.uvit = value,
            StatusParameter::UInt => self.uint = value,
            StatusParameter::UDex => self.udex = value,
            StatusParameter::ULuk => self.uluk = value,
            StatusParameter::StatusPoint => self.status_point = value,
            StatusParameter::SkillPoint => self.skill_point = value,
            StatusParameter::Weight => self.weight = value.min(self.max_weight),
            StatusParameter::MaxWeight => self.max_weight = value,
            StatusParameter::Zeny => self.zeny = value,
            StatusParameter::BankVault => self.bank_vault = value,
            StatusParameter::Atk1 => self.atk1 = value,
            StatusParameter::Atk2 => self.atk2 = value,
            StatusParameter::MAtk1 => self.matk1 = value,
            StatusParameter::MAtk2 => self.matk2 = value,
            StatusParameter::Def1 => self.def1 = value,
            StatusParameter::Def2 => self.def2 = value,
            StatusParameter::MDef1 => self.mdef1 = value,
            StatusParameter::MDef2 => self.mdef2 = value,
            StatusParameter::Hit => self.hit = value,
            StatusParameter::Flee1 => self.flee1 = value,
            StatusParameter::Flee2 => self.flee2 = value,
            StatusParameter::Critical => self.critical = value,
            StatusParameter::Aspd => self.aspd = value,
            StatusParameter::Speed => self.speed = value,
            StatusParameter::Karma => self.karma = value,
            StatusParameter::Manner => self.manner = value,
            StatusParameter::Fame => self.fame = value,
            StatusParameter::Cart => self.cart = value,
            StatusParameter::Upper => self.upper = value,
            StatusParameter::Partner => self.partner = value,
            StatusParameter::Unbreakable => self.unbreakable = value,
            StatusParameter::Class => self.class = value,
            StatusParameter::Sex => self.sex = value,
            StatusParameter::CartInfo => self.cart_info = value,
            StatusParameter::KilledGid => self.killed_gid = value,
            StatusParameter::BaseJob => self.base_job = value,
            StatusParameter::BaseClass => self.base_class = value,
            StatusParameter::KillerRid => self.killer_rid = value,
            StatusParameter::KilledRid => self.killed_rid = value,
            StatusParameter::Sitting => self.sitting = value,
            StatusParameter::CharMove => self.char_move = value,
            StatusParameter::CharRename => self.char_rename = value,
            StatusParameter::CharFont => self.char_font = value,
            StatusParameter::RouletteBronze => self.roulette_bronze = value,
        }
    }

    pub fn get_param(&self, param: StatusParameter) -> u32 {
        match param {
            StatusParameter::Hp => self.hp,
            StatusParameter::MaxHp => self.max_hp,
            StatusParameter::Sp => self.sp,
            StatusParameter::MaxSp => self.max_sp,
            StatusParameter::BaseExp => self.base_exp,
            StatusParameter::JobExp => self.job_exp,
            StatusParameter::NextBaseExp => self.next_base_exp,
            StatusParameter::NextJobExp => self.next_job_exp,
            StatusParameter::BaseLevel => self.base_level,
            StatusParameter::JobLevel => self.job_level,
            StatusParameter::Str => self.str,
            StatusParameter::Agi => self.agi,
            StatusParameter::Vit => self.vit,
            StatusParameter::Int => self.int,
            StatusParameter::Dex => self.dex,
            StatusParameter::Luk => self.luk,
            StatusParameter::UStr => self.ustr,
            StatusParameter::UAgi => self.uagi,
            StatusParameter::UVit => self.uvit,
            StatusParameter::UInt => self.uint,
            StatusParameter::UDex => self.udex,
            StatusParameter::ULuk => self.uluk,
            StatusParameter::StatusPoint => self.status_point,
            StatusParameter::SkillPoint => self.skill_point,
            StatusParameter::Weight => self.weight,
            StatusParameter::MaxWeight => self.max_weight,
            StatusParameter::Zeny => self.zeny,
            StatusParameter::BankVault => self.bank_vault,
            StatusParameter::Atk1 => self.atk1,
            StatusParameter::Atk2 => self.atk2,
            StatusParameter::MAtk1 => self.matk1,
            StatusParameter::MAtk2 => self.matk2,
            StatusParameter::Def1 => self.def1,
            StatusParameter::Def2 => self.def2,
            StatusParameter::MDef1 => self.mdef1,
            StatusParameter::MDef2 => self.mdef2,
            StatusParameter::Hit => self.hit,
            StatusParameter::Flee1 => self.flee1,
            StatusParameter::Flee2 => self.flee2,
            StatusParameter::Critical => self.critical,
            StatusParameter::Aspd => self.aspd,
            StatusParameter::Speed => self.speed,
            StatusParameter::Karma => self.karma,
            StatusParameter::Manner => self.manner,
            StatusParameter::Fame => self.fame,
            StatusParameter::Cart => self.cart,
            StatusParameter::Upper => self.upper,
            StatusParameter::Partner => self.partner,
            StatusParameter::Unbreakable => self.unbreakable,
            StatusParameter::Class => self.class,
            StatusParameter::Sex => self.sex,
            StatusParameter::CartInfo => self.cart_info,
            StatusParameter::KilledGid => self.killed_gid,
            StatusParameter::BaseJob => self.base_job,
            StatusParameter::BaseClass => self.base_class,
            StatusParameter::KillerRid => self.killer_rid,
            StatusParameter::KilledRid => self.killed_rid,
            StatusParameter::Sitting => self.sitting,
            StatusParameter::CharMove => self.char_move,
            StatusParameter::CharRename => self.char_rename,
            StatusParameter::CharFont => self.char_font,
            StatusParameter::RouletteBronze => self.roulette_bronze,
        }
    }

    pub fn hp_percentage(&self) -> f32 {
        if self.max_hp == 0 {
            0.0
        } else {
            (self.hp as f32 / self.max_hp as f32) * 100.0
        }
    }

    pub fn sp_percentage(&self) -> f32 {
        if self.max_sp == 0 {
            0.0
        } else {
            (self.sp as f32 / self.max_sp as f32) * 100.0
        }
    }

    pub fn base_exp_percentage(&self) -> f32 {
        if self.next_base_exp == 0 {
            0.0
        } else {
            (self.base_exp as f32 / self.next_base_exp as f32) * 100.0
        }
    }

    pub fn job_exp_percentage(&self) -> f32 {
        if self.next_job_exp == 0 {
            0.0
        } else {
            (self.job_exp as f32 / self.next_job_exp as f32) * 100.0
        }
    }

    pub fn weight_percentage(&self) -> f32 {
        if self.max_weight == 0 {
            0.0
        } else {
            (self.weight as f32 / self.max_weight as f32) * 100.0
        }
    }

    pub fn is_overweight(&self) -> bool {
        self.weight_percentage() >= 50.0
    }

    pub fn is_critically_overweight(&self) -> bool {
        self.weight_percentage() >= 90.0
    }

    pub fn is_dead(&self) -> bool {
        self.hp == 0
    }

    pub fn total_atk(&self) -> u32 {
        self.atk1 + self.atk2
    }

    pub fn total_matk(&self) -> u32 {
        self.matk1 + self.matk2
    }

    pub fn total_def(&self) -> u32 {
        self.def1 + self.def2
    }

    pub fn total_mdef(&self) -> u32 {
        self.mdef1 + self.mdef2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_parameter_from_var_id() {
        assert_eq!(StatusParameter::from_var_id(5), Some(StatusParameter::Hp));
        assert_eq!(
            StatusParameter::from_var_id(6),
            Some(StatusParameter::MaxHp)
        );
        assert_eq!(
            StatusParameter::from_var_id(20),
            Some(StatusParameter::Zeny)
        );
        assert_eq!(StatusParameter::from_var_id(9999), None);
    }

    #[test]
    fn test_uses_long_packet() {
        assert!(StatusParameter::BaseExp.uses_long_packet());
        assert!(StatusParameter::JobExp.uses_long_packet());
        assert!(StatusParameter::NextBaseExp.uses_long_packet());
        assert!(StatusParameter::NextJobExp.uses_long_packet());
        assert!(!StatusParameter::Hp.uses_long_packet());
        assert!(!StatusParameter::Zeny.uses_long_packet());
    }

    #[test]
    fn test_status_update_param() {
        let mut status = CharacterStatus::default();

        status.update_param(StatusParameter::MaxHp, 1000);
        assert_eq!(status.max_hp, 1000);

        status.update_param(StatusParameter::Hp, 500);
        assert_eq!(status.hp, 500);

        status.update_param(StatusParameter::BaseExp, 12345);
        assert_eq!(status.base_exp, 12345);
    }

    #[test]
    fn test_hp_percentage() {
        let mut status = CharacterStatus::default();
        status.hp = 50;
        status.max_hp = 100;

        assert_eq!(status.hp_percentage(), 50.0);
    }

    #[test]
    fn test_weight_status() {
        let mut status = CharacterStatus::default();
        status.weight = 1000;
        status.max_weight = 2000;

        assert_eq!(status.weight_percentage(), 50.0);
        assert!(status.is_overweight());
        assert!(!status.is_critically_overweight());

        status.weight = 1900;
        assert!(status.is_critically_overweight());
    }

    #[test]
    fn test_is_dead() {
        let mut status = CharacterStatus::default();
        assert!(!status.is_dead());

        status.hp = 0;
        assert!(status.is_dead());
    }

    #[test]
    fn test_total_stats() {
        let mut status = CharacterStatus::default();
        status.atk1 = 100;
        status.atk2 = 50;

        assert_eq!(status.total_atk(), 150);
    }
}
