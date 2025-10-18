/// Macro to generate a SystemParam bundle for EventWriters
///
/// This reduces the number of function parameters by grouping related EventWriters
/// into a single logical parameter.
///
/// # Usage
///
/// ```ignore
/// define_event_writers! {
///     TauriEventWriters {
///         login: LoginRequestedEvent,
///         server_selection: ServerSelectionRequestedEvent,
///     }
/// }
/// ```
///
/// This generates:
/// ```ignore
/// #[derive(SystemParam)]
/// pub struct TauriEventWriters<'w> {
///     pub login: EventWriter<'w, LoginRequestedEvent>,
///     pub server_selection: EventWriter<'w, ServerSelectionRequestedEvent>,
/// }
/// ```
#[macro_export]
macro_rules! define_event_writers {
    ($struct_name:ident { $($field:ident: $event_type:ty),* $(,)? }) => {
        #[derive(bevy::ecs::system::SystemParam)]
        pub struct $struct_name<'w> {
            $(
                pub $field: bevy::prelude::EventWriter<'w, $event_type>,
            )*
        }
    };
}

/// Macro to dispatch Tauri incoming events to Bevy EventWriters
///
/// This macro eliminates boilerplate for two common event dispatch patterns:
/// 1. Fire-and-forget events (no response channel)
/// 2. Events with pending response sender
///
/// # Pattern 1: Fire-and-forget
///
/// Use this for events that don't need a response (e.g., keyboard/mouse input).
///
/// ```ignore
/// dispatch_tauri_event!(
///     event: KeyboardInputEvent { code, pressed },
///     writer: writers.keyboard
/// );
/// ```
///
/// **Expands to:**
/// ```ignore
/// writers.keyboard.write(KeyboardInputEvent { code, pressed });
/// ```
///
/// # Pattern 2: With pending sender
///
/// Use this for events that need to send a response back to the UI.
/// The response channel is stored in `pending` for later retrieval.
///
/// ```ignore
/// dispatch_tauri_event!(
///     pending: (pending.hairstyles, request_id, response_tx),
///     event: GetHairstylesRequestedEvent { request_id, gender },
///     writer: writers.hairstyles
/// );
/// ```
///
/// **Expands to:**
/// ```ignore
/// pending.hairstyles.senders.insert(request_id, response_tx);
/// writers.hairstyles.write(GetHairstylesRequestedEvent { request_id, gender });
/// ```
///
/// # Note on Correlation
///
/// The old Pattern 3 (with correlation) has been removed. Correlation should be
/// handled explicitly before calling this macro, using the correlation resource's
/// `insert` methods. This makes the code more explicit and easier to debug.
///
/// **Example:**
/// ```ignore
/// // Store correlation first
/// login_correlation.insert(username.clone(), request_id);
///
/// // Then dispatch event
/// dispatch_tauri_event!(
///     pending: (pending.logins, request_id, response_tx),
///     event: LoginRequestedEvent { request_id, username, password },
///     writer: writers.login
/// );
/// ```
///
/// # Debugging
///
/// If you get a compile error, check:
/// - All required fields are present: `event` and `writer` are always required
/// - `pending` tuple has exactly 3 elements: (collection, key, sender)
/// - The event type matches what the writer expects
/// - Variable names are correct (not typos)
#[macro_export]
macro_rules! dispatch_tauri_event {
    // Pattern 1: Fire-and-forget (no pending sender, no correlation)
    (
        event: $event:expr,
        writer: $writer:expr
    ) => {
        $writer.write($event);
    };

    // Pattern 2: With pending sender
    (
        pending: ($pending_collection:expr, $key:expr, $sender:expr),
        event: $event:expr,
        writer: $writer:expr
    ) => {
        // Store the response channel
        $pending_collection.senders.insert($key, $sender);

        // Send the event
        $writer.write($event);
    };
}
