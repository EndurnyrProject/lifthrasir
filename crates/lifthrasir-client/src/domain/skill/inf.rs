/// The RO `INF` value decoded into a skill form and target shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Passive,
    Active,
    Supportive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    None,
    Enemy,
    Ground,
    SelfTarget,
    Ally,
}

pub fn form(inf: u32) -> Form {
    match inf {
        0 => Form::Passive,
        16 => Form::Supportive,
        _ => Form::Active,
    }
}

pub fn target(inf: u32) -> Target {
    match inf {
        1 => Target::Enemy,
        2 => Target::Ground,
        4 => Target::SelfTarget,
        16 => Target::Ally,
        _ => Target::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inf_maps_to_form_and_target() {
        assert_eq!((form(0), target(0)), (Form::Passive, Target::None));
        assert_eq!((form(1), target(1)), (Form::Active, Target::Enemy));
        assert_eq!((form(2), target(2)), (Form::Active, Target::Ground));
        assert_eq!((form(4), target(4)), (Form::Active, Target::SelfTarget));
        assert_eq!((form(16), target(16)), (Form::Supportive, Target::Ally));
        assert_eq!((form(99), target(99)), (Form::Active, Target::None));
    }
}
