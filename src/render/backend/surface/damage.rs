use super::*;

/// Clips `rect` to `size`, returning `None` when no pixel remains.
pub(crate) fn clipped_damage(rect: Rect, size: Size) -> Option<(i32, i32, u32, u32)> {
    let left = (rect.x as i64).max(0);
    let top = (rect.y as i64).max(0);
    let right = (rect.x as i64 + rect.width as i64).min(size.width as i64);
    let bottom = (rect.y as i64 + rect.height as i64).min(size.height as i64);
    if right <= left || bottom <= top {
        return None;
    }
    Some((left as i32, top as i32, (right - left) as u32, (bottom - top) as u32))
}

/// Whether `outer` fully contains `inner`.
pub(crate) fn contains_region(outer: (i32, i32, u32, u32), inner: (i32, i32, u32, u32)) -> bool {
    let (ox, oy, ow, oh) = outer;
    let (ix, iy, iw, ih) = inner;
    ix >= ox
        && iy >= oy
        && (ix as i64 + iw as i64) <= (ox as i64 + ow as i64)
        && (iy as i64 + ih as i64) <= (oy as i64 + oh as i64)
}

/// How many damage rectangles a surface keeps before collapsing them into one.
///
/// The list is already folded on overlap, so this only bounds the scattered case. Collapsing to
/// the bounding rectangle can cover pixels that did not change, but it never loses damage, and
/// the alternative — an unbounded number of per-rectangle copies — is the worse trade (G-29).
pub(crate) const MAX_DAMAGE_REGIONS: usize = 32;

/// Whether two device-space regions share at least one pixel.
///
/// Rectangles that merely touch do not overlap: the end coordinate is exclusive, so an empty
/// intersection is not damage.
pub(crate) fn regions_intersect(a: (i32, i32, u32, u32), b: (i32, i32, u32, u32)) -> bool {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    (ax as i64) < (bx as i64 + bw as i64)
        && (bx as i64) < (ax as i64 + aw as i64)
        && (ay as i64) < (by as i64 + bh as i64)
        && (by as i64) < (ay as i64 + ah as i64)
}

/// The smallest region covering both inputs, in device-space coordinates.
pub(crate) fn union_region(
    a: (i32, i32, u32, u32),
    b: (i32, i32, u32, u32),
) -> (i32, i32, u32, u32) {
    let left = (a.0 as i64).min(b.0 as i64);
    let top = (a.1 as i64).min(b.1 as i64);
    let right = (a.0 as i64 + a.2 as i64).max(b.0 as i64 + b.2 as i64);
    let bottom = (a.1 as i64 + a.3 as i64).max(b.1 as i64 + b.3 as i64);
    (left as i32, top as i32, (right - left) as u32, (bottom - top) as u32)
}
