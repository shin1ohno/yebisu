//! L9: overlay O, including the toast slot (S-2). Owner: L9.

/// The toast slot as (bottom offset, width) in points, when overlay O
/// places it; `None` keeps the L2a placement.
pub(crate) fn toast_slot() -> Option<(f32, f32)> {
    None
}
