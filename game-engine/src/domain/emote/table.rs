pub const MAX_EMOTE_ID: u32 = 88;

/// Each entry is `(aliases, sound)`. `aliases[0]` is always the aesir
/// canonical name from `emotion.ex`; any further entries are classic RO
/// `/command` aliases for the same emote, sourced from the client's
/// emoticon command list.
const EMOTES: [(&[&str], Option<&str>); MAX_EMOTE_ID as usize] = [
    (&["surprise", "!"], None),
    (&["question", "?"], None),
    (&["delight", "ho"], None),
    (&["throb", "lv"], None),
    (&["sweat", "swt"], None),
    (&["aha", "ic"], None),
    (&["fret", "an"], None),
    (&["anger", "ag"], None),
    (&["money", "$"], None),
    (&["think", "..."], None),
    (&["scissor", "gawi"], None),
    (&["rock", "bawi"], None),
    (&["wrap", "bo"], None),
    (&["flag"], None),
    (&["bigthrob", "lv2"], None),
    (&["thanks", "thx"], None),
    (&["kek", "wah"], None),
    (&["sorry", "sry"], None),
    (&["smile", "heh"], None),
    (&["profusely_sweat", "swt2"], None),
    (&["scratch", "hmm"], None),
    (&["best", "no1"], None),
    (&["stare_about", "??"], None),
    (&["huk", "omg"], None),
    (&["o", "oh"], None),
    (&["x"], None),
    (&["help", "hlp"], None),
    (&["go"], None),
    (&["cry", "sob"], None),
    (&["kik", "gg"], None),
    (&["chup", "kis"], None),
    (&["chupchup", "kis2"], None),
    (&["hng", "pif"], None),
    (&["ok"], None),
    (&["chat_prohibit"], None),
    (&["indonesia_flag"], None),
    (&["stare", "bzz"], None),
    (&["hungry", "rice"], None),
    (&["cool", "awsm"], None),
    (&["merong", "meh"], None),
    (&["shy"], None),
    (&["goodboy", "pat"], None),
    (&["sptime", "mp"], None),
    (&["sexy", "slur"], None),
    (&["comeon", "com"], None),
    (&["sleepy", "yawn"], None),
    (&["congratulation", "grat"], None),
    (&["hptime", "hp"], None),
    (&["ph_flag"], None),
    (&["my_flag"], None),
    (&["si_flag"], None),
    (&["br_flag"], None),
    (&["spark", "fsh"], None),
    (&["confuse", "spin"], None),
    (&["ohno", "sigh"], None),
    (&["hum", "dum"], None),
    (&["blabla", "crwd"], None),
    (&["otl", "desp"], None),
    (&["dice1"], None),
    (&["dice2"], None),
    (&["dice3"], None),
    (&["dice4"], None),
    (&["dice5"], None),
    (&["dice6"], None),
    (&["india_flag"], None),
    (&["luv", "love"], None),
    (&["flag8"], None),
    (&["flag9"], None),
    (&["mobile"], None),
    (&["mail"], None),
    (&["antenna0"], None),
    (&["antenna1"], None),
    (&["antenna2"], None),
    (&["antenna3"], None),
    (&["hum2"], None),
    (&["abs"], None),
    (&["oops"], None),
    (&["spit"], None),
    (&["ene"], None),
    (&["panic"], None),
    (&["whisp"], None),
    (&["yut1"], None),
    (&["yut2"], None),
    (&["yut3"], None),
    (&["yut4"], None),
    (&["yut5"], None),
    (&["yut6"], None),
    (&["yut7"], None),
];

pub fn emote_id_from_alias(input: &str) -> Option<u32> {
    let stripped = input.strip_prefix('/').unwrap_or(input);

    if let Ok(id) = stripped.parse::<u32>() {
        return (id < MAX_EMOTE_ID).then_some(id);
    }

    EMOTES
        .iter()
        .position(|(aliases, _)| {
            aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(stripped))
        })
        .map(|id| id as u32)
}

pub fn emote_sound(id: u32) -> Option<&'static str> {
    EMOTES.get(id as usize).and_then(|(_, sound)| *sound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emote_id_from_alias_resolves_leading_slash_alias() {
        assert_eq!(emote_id_from_alias("/surprise"), Some(0));
    }

    #[test]
    fn emote_id_from_alias_resolves_bare_alias() {
        assert_eq!(emote_id_from_alias("surprise"), Some(0));
    }

    #[test]
    fn emote_id_from_alias_is_case_insensitive() {
        assert_eq!(emote_id_from_alias("/Surprise"), Some(0));
    }

    #[test]
    fn emote_id_from_alias_resolves_dice_alias() {
        assert_eq!(emote_id_from_alias("/dice1"), Some(58));
    }

    #[test]
    fn emote_id_from_alias_resolves_numeric() {
        assert_eq!(emote_id_from_alias("/2"), Some(2));
    }

    #[test]
    fn emote_id_from_alias_rejects_unknown_alias() {
        assert_eq!(emote_id_from_alias("/not_a_real_emote"), None);
    }

    #[test]
    fn emote_id_from_alias_rejects_out_of_range_number() {
        assert_eq!(emote_id_from_alias("/999"), None);
    }

    #[test]
    fn all_ids_below_max_and_present() {
        assert_eq!(EMOTES.len(), MAX_EMOTE_ID as usize);
    }

    #[test]
    fn aliases_are_unique() {
        let mut aliases: Vec<&str> = EMOTES
            .iter()
            .flat_map(|(aliases, _)| aliases.iter().copied())
            .collect();
        aliases.sort_unstable();
        aliases.dedup();
        let total: usize = EMOTES.iter().map(|(aliases, _)| aliases.len()).sum();
        assert_eq!(aliases.len(), total);
    }

    #[test]
    fn classic_command_aliases_resolve_to_canonical_ids() {
        let cases = [
            ("/heh", 18),
            ("/sob", 28),
            ("/gg", 29),
            ("/ok", 33),
            ("/swt", 4),
            ("/ho", 2),
            ("/lv", 3),
            ("/gawi", 10),
            ("/bawi", 11),
            ("/bo", 12),
        ];

        for (alias, expected_id) in cases {
            assert_eq!(
                emote_id_from_alias(alias),
                Some(expected_id),
                "alias {alias}"
            );
        }
    }
}
