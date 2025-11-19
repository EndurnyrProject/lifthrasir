use super::events::*;

crate::define_event_writers! {
    TauriMessageWriters {
        login: LoginRequestedEvent,
        server_selection: ServerSelectionRequestedEvent,
        char_list: GetCharacterListRequestedEvent,
        char_select: SelectCharacterRequestedEvent,
        char_create: CreateCharacterRequestedEvent,
        char_delete: DeleteCharacterRequestedEvent,
        hairstyles: GetHairstylesRequestedEvent,
        keyboard: KeyboardInputEvent,
        mouse: MousePositionEvent,
        mouse_click: MouseClickEvent,
        camera_rotation: CameraRotationEvent,
        char_status: GetCharacterStatusRequestedEvent,
        chat: ChatRequestedEvent,
    }
}
