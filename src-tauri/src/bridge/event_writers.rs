use super::events::*;

crate::define_event_writers! {
    TauriEventWriters {
        login: LoginRequestedEvent,
        server_selection: ServerSelectionRequestedEvent,
        char_list: GetCharacterListRequestedEvent,
        char_select: SelectCharacterRequestedEvent,
        char_create: CreateCharacterRequestedEvent,
        char_delete: DeleteCharacterRequestedEvent,
        hairstyles: GetHairstylesRequestedEvent,
        keyboard: KeyboardInputEvent,
        mouse: MousePositionEvent,
    }
}
