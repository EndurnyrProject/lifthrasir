use super::theme::*;
use super::widgets::*;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy_lunex::prelude::*;

/// System to handle mouse wheel scrolling on scrollable panels
pub fn handle_scroll_wheel(
    mut wheel_events: EventReader<MouseWheel>,
    mut scroll_query: Query<(&mut ScrollablePanel, &Interaction)>,
) {
    // Only process if we have wheel events
    if wheel_events.is_empty() {
        return;
    }

    // Sum up all wheel deltas
    let total_delta: f32 = wheel_events.read().map(|event| event.y).sum();

    // Apply scroll to panels that are being hovered
    for (mut panel, interaction) in scroll_query.iter_mut() {
        // Only scroll if mouse is over the panel
        if *interaction == Interaction::Hovered || *interaction == Interaction::Pressed {
            // Update scroll offset (negative because wheel up = scroll up = negative offset change)
            panel.scroll_offset -= total_delta * panel.scroll_speed;

            // Clamp scroll offset to valid range
            let max_scroll = panel.max_scroll();
            panel.scroll_offset = panel.scroll_offset.clamp(0.0, max_scroll);
        }
    }
}

/// System to update scroll content position based on scroll offset
pub fn update_scroll_content_position(
    scroll_query: Query<(&ScrollablePanel, &Children), Changed<ScrollablePanel>>,
    clip_query: Query<&Children>,
    mut content_query: Query<&mut Transform, With<ScrollContent>>,
) {
    for (panel, panel_children) in scroll_query.iter() {
        // Find the clip container
        for clip_child in panel_children.iter() {
            if let Ok(clip_children) = clip_query.get(clip_child) {
                // Find the scroll content
                for content_child in clip_children.iter() {
                    if let Ok(mut transform) = content_query.get_mut(content_child) {
                        // Update the content position based on scroll offset
                        // Move content up (negative Y) as scroll offset increases
                        transform.translation.y = panel.scroll_offset;
                    }
                }
            }
        }
    }
}

/// System to calculate content height from children
/// For now, this is simplified - content height should be set manually or calculated from child transforms
pub fn calculate_content_height(
    mut scroll_query: Query<(&mut ScrollablePanel, &Children)>,
    clip_query: Query<&Children>,
    content_query: Query<&Children, With<ScrollContent>>,
    child_transform_query: Query<&Transform>,
) {
    for (mut panel, panel_children) in scroll_query.iter_mut() {
        // Find the clip container
        for clip_child in panel_children.iter() {
            if let Ok(clip_children) = clip_query.get(clip_child) {
                // Find the scroll content
                for content_child in clip_children.iter() {
                    if let Ok(content_children) = content_query.get(content_child) {
                        // Calculate total height needed by finding the lowest child
                        let mut max_bottom = 0.0f32;

                        for child in content_children.iter() {
                            if let Ok(transform) = child_transform_query.get(child) {
                                // Simple calculation: assume children are positioned vertically
                                // In practice, you may need more sophisticated bounds calculation
                                let bottom = transform.translation.y.abs() + 50.0; // Rough estimate with 50px height
                                max_bottom = max_bottom.max(bottom);
                            }
                        }

                        // Update content height
                        panel.content_height = max_bottom;

                        // Clamp current scroll offset if content shrunk
                        let max_scroll = panel.max_scroll();
                        if panel.scroll_offset > max_scroll {
                            panel.scroll_offset = max_scroll;
                        }
                    }
                }
            }
        }
    }
}

/// System to update scrollbar visibility based on content overflow
pub fn update_scrollbar_visibility(
    scroll_query: Query<(&ScrollablePanel, &Children), Changed<ScrollablePanel>>,
    mut scrollbar_query: Query<&mut Visibility, With<ScrollBar>>,
) {
    for (panel, children) in scroll_query.iter() {
        // Find the scrollbar child
        for child in children.iter() {
            if let Ok(mut visibility) = scrollbar_query.get_mut(child) {
                // Show scrollbar if content exceeds max height
                *visibility = if panel.needs_scrollbar() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

/// System to update scrollbar thumb size and position
pub fn update_scrollbar_thumb(
    scroll_query: Query<(&ScrollablePanel, &Children), Changed<ScrollablePanel>>,
    scrollbar_query: Query<&Children, With<ScrollBar>>,
    mut thumb_query: Query<&mut Transform, With<ScrollThumb>>,
) {
    for (panel, panel_children) in scroll_query.iter() {
        // Find the scrollbar
        for scrollbar_child in panel_children.iter() {
            if let Ok(scrollbar_children) = scrollbar_query.get(scrollbar_child) {
                // Find the thumb
                for thumb_child in scrollbar_children.iter() {
                    if let Ok(mut thumb_transform) = thumb_query.get_mut(thumb_child) {
                        // Calculate thumb position based on scroll ratio
                        let scroll_ratio = panel.scroll_ratio();
                        let track_height = panel.max_height;
                        let visible_ratio = panel.visible_ratio();
                        let thumb_height = (track_height * visible_ratio).max(SCROLLBAR_MIN_THUMB_HEIGHT);
                        let max_thumb_travel = track_height - thumb_height;
                        let thumb_y = scroll_ratio * max_thumb_travel;

                        // Update thumb position
                        thumb_transform.translation.y = -thumb_y; // Negative because Bevy Y is up
                    }
                }
            }
        }
    }
}

/// System to handle scrollbar thumb dragging
pub fn handle_scrollbar_drag(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut scroll_query: Query<(&mut ScrollablePanel, &Children)>,
    scrollbar_query: Query<&Children, With<ScrollBar>>,
    mut thumb_query: Query<(&mut ScrollThumb, &Interaction)>,
    mut cursor_moved: EventReader<CursorMoved>,
) {
    // Get cursor position if moved
    let cursor_delta_y: f32 = cursor_moved
        .read()
        .filter_map(|event| event.delta)
        .map(|delta| delta.y)
        .sum();

    // Check if left mouse button is pressed
    let is_dragging = mouse_button.pressed(MouseButton::Left);

    for (mut panel, panel_children) in scroll_query.iter_mut() {
        // Find the scrollbar
        for scrollbar_child in panel_children.iter() {
            if let Ok(scrollbar_children) = scrollbar_query.get(scrollbar_child) {
                // Find the thumb
                for thumb_child in scrollbar_children.iter() {
                    if let Ok((mut thumb, interaction)) = thumb_query.get_mut(thumb_child) {
                        // Start dragging when clicking on thumb
                        if *interaction == Interaction::Pressed && !thumb.is_dragging {
                            thumb.is_dragging = true;
                            thumb.scroll_start = panel.scroll_offset;
                            thumb.drag_start_y = 0.0; // Reset delta tracking
                        }

                        // Stop dragging when releasing mouse
                        if !is_dragging {
                            thumb.is_dragging = false;
                        }

                        // Update scroll while dragging
                        if thumb.is_dragging && cursor_delta_y.abs() > 0.0 {
                            // Calculate how much to scroll based on drag distance
                            let track_height = panel.max_height;
                            let thumb_height = (track_height * panel.visible_ratio())
                                .max(SCROLLBAR_MIN_THUMB_HEIGHT);
                            let max_thumb_travel = track_height - thumb_height;

                            // Convert cursor delta to scroll delta (negative because Bevy Y is up)
                            if max_thumb_travel > 0.0 {
                                let scroll_delta = (-cursor_delta_y / max_thumb_travel) * panel.max_scroll();
                                panel.scroll_offset += scroll_delta;

                                // Clamp scroll offset
                                let max_scroll = panel.max_scroll();
                                panel.scroll_offset = panel.scroll_offset.clamp(0.0, max_scroll);
                            }
                        }
                    }
                }
            }
        }
    }
}
