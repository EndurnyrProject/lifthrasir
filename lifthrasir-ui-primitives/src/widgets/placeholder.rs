//! Hint text for [`EditableText`] fields.
//!
//! The 0.19 core text widget has no built-in prompt/placeholder, so each field that
//! wants one spawns a faint [`Text`] overlay tagged with [`Placeholder`] pointing at
//! the field entity. [`toggle_placeholders`] hides it as soon as the field is non-empty.

use bevy::prelude::*;
use bevy::text::EditableText;

/// A hint overlay for the [`EditableText`] field stored in `.0`. Shown while that
/// field is empty, hidden once it has content.
#[derive(Component)]
pub struct Placeholder(pub Entity);

pub struct PlaceholderPlugin;

impl Plugin for PlaceholderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, toggle_placeholders);
    }
}

fn toggle_placeholders(
    fields: Query<&EditableText>,
    mut placeholders: Query<(&Placeholder, &mut Visibility)>,
) {
    for (placeholder, mut visibility) in &mut placeholders {
        let empty = fields
            .get(placeholder.0)
            .is_ok_and(|field| field.value().to_string().is_empty());
        *visibility = if empty {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
