/// Macro to dispatch type-erased events to their corresponding EventWriters
///
/// This macro eliminates the boilerplate of manually downcasting each event type
/// from the EventBuffer and sending it to the appropriate Bevy EventWriter.
///
/// # Usage
///
/// ```ignore
/// dispatch_events!(event_buffer, event_writers, [
///     (EventType1, field_name1),
///     (EventType2, field_name2),
///     // ... more event types
/// ]);
/// ```
///
/// # Example
///
/// ```ignore
/// let mut event_buffer = EventBuffer::new();
/// client.update(&mut event_buffer)?;
///
/// dispatch_events!(event_buffer, events, [
///     (CharacterServerConnected, connected),
///     (ZoneServerInfoReceived, zone_info),
///     (CharacterCreated, char_created),
/// ]);
/// ```
#[macro_export]
macro_rules! dispatch_events {
    ($buffer:expr, $writers:expr, [$(($event_type:ty, $field:ident)),* $(,)?]) => {
        for event in $buffer.drain() {
            $(
                if let Some(e) = event.downcast_ref::<$event_type>() {
                    $writers.$field.write(e.clone());
                    continue;
                }
            )*
        }
    };
}
