use bevy::prelude::*;

/// Converts the primary window's current cursor position into world-space
/// coordinates under the primary camera. `None` if the cursor is outside
/// the window or either query comes up empty.
pub fn cursor_world_pos(
    windows: &Query<&Window>,
    camera_q: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    let (camera, camera_transform) = camera_q.single().ok()?;
    camera.viewport_to_world_2d(camera_transform, cursor).ok()
}
