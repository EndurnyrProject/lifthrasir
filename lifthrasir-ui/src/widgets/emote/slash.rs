//! Emote slash-command parsing.
//!
//! `chat_input_control` (`chat_box.rs`) calls [`parse_emote_slash`] before
//! `parse_party_slash`; a recognized emote alias is written as `EmoteRequested`
//! instead of a normal chat message or a party slash command.

use game_engine::domain::emote::table::emote_id_from_alias;

/// Parse one chat line into an emote id. Only input starting with `/` is a
/// candidate, so a normal chat line always falls through. Returns `None` for
/// anything the emote table doesn't recognize (including party commands like
/// `/pinvite`), so the caller falls through to `parse_party_slash`/chat.
pub fn parse_emote_slash(input: &str) -> Option<u32> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let token = trimmed.split_whitespace().next()?;
    emote_id_from_alias(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_named_alias() {
        assert_eq!(parse_emote_slash("/surprise"), Some(0));
    }

    #[test]
    fn resolves_numeric_alias() {
        assert_eq!(parse_emote_slash("/2"), Some(2));
    }

    #[test]
    fn resolves_dice_alias() {
        assert_eq!(parse_emote_slash("/dice1"), Some(58));
    }

    #[test]
    fn plain_chat_line_is_none() {
        assert_eq!(parse_emote_slash("hello"), None);
    }

    #[test]
    fn missing_leading_slash_is_none() {
        assert_eq!(parse_emote_slash("surprise"), None);
    }

    #[test]
    fn unknown_slash_is_none() {
        assert_eq!(parse_emote_slash("/unknownxyz"), None);
    }

    #[test]
    fn party_slash_is_none() {
        assert_eq!(parse_emote_slash("/pinvite"), None);
    }
}
