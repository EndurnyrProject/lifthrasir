//! Neutral item-skill targeting and gameplay feedback reasons.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemSkillTargetKind {
    SelfTarget,
    Unit,
    Ground,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAddFailure {
    Invalid,
    Overweight,
    Item,
    TooManyItems,
    TooManyOfItem,
    RefusedTime,
    StackLimit,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameplayNoticeKind {
    Busy,
    ImpossibleSkillArea,
    CannotEquipItem,
    MapCannotTeleport,
    MapCannotMemo,
    MapCannotUseSkill,
    Unknown(i32),
}
