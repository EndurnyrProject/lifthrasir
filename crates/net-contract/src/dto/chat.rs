/// Who a chat line reaches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ChatChannel {
    /// Players in view of the speaker.
    #[default]
    Local,
    /// Online members of the speaker's party.
    Party,
    /// Online members of the speaker's guild.
    Guild,
    /// Every online player.
    Trade,
    /// One player, named by the line's `whisper_to`.
    Whisper,
}

/// Marks where an item link sits in chat text: the n-th one stands for the n-th
/// linked item.
pub const CHAT_LINK_MARK: char = '\u{fffc}';

/// Most items one chat line can link.
pub const CHAT_MAX_LINKS: usize = 3;
