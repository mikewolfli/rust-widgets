// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Turning a window's size, scale and quality intent into the configuration the target is told to
//! use.
//!
//! This is a pure function of four inputs. It returns `wgpu`'s own
//! [`wgpu::SurfaceConfiguration`] rather than a parallel struct: the value is what
//! `Surface::configure` consumes, every field of it is meaningful, and a second "same but ours"
//! type would have to be mapped back field by field — the duplicate-vocabulary shape principle #54
//! forbids. Everything that can be decided without a device is decided here, so the rules are unit
//! tests (plan B29.08).
//!
//! # Colour space, and why the format order depends on the quality intent
//!
//! The software frames this library produces are **sRGB-encoded bytes**. Two properties decide
//! whether presenting them changes them:
//!
//! * **Encoding.** A `*Unorm` target stores whatever byte is written and the compositor treats it
//!   as sRGB — so an sRGB byte written verbatim reads back verbatim. A `*Srgb` target treats the
//!   written value as *linear* and encodes on store, so the same byte comes out different. Exact
//!   therefore prefers `*Unorm`; Smooth prefers `*Srgb`, because blending in linear space is what
//!   makes an antialiased edge look right.
//! * **Channel order.** A `Bgra8*` target stores the shader's red component in the blue slot by
//!   format definition. Rendering does *not* need a manual swap — the format does it — but a
//!   readback comparator does, which is what [`needs_channel_swap`] is for.
//!
//! [`needs_srgb_decode`] is the matching flag for the first property: when the chosen format is
//! `*Srgb`, the uploader must linearise the sRGB bytes before writing them, or the compositor
//! encodes them twice.

use crate::wgpu_backend::RenderQuality;

/// Format preference when the frame must stay byte-identical.
///
/// Verbatim storage first (`*Unorm`), then the matching channel order (RGBA before BGRA).
const EXACT_FORMAT_PREFERENCE: [wgpu::TextureFormat; 4] = [
    wgpu::TextureFormat::Rgba8Unorm,
    wgpu::TextureFormat::Bgra8Unorm,
    wgpu::TextureFormat::Rgba8UnormSrgb,
    wgpu::TextureFormat::Bgra8UnormSrgb,
];

/// Format preference when a smoother edge is worth a difference.
///
/// Linear-space blending first (`*Srgb`), then the matching channel order.
const SMOOTH_FORMAT_PREFERENCE: [wgpu::TextureFormat; 4] = [
    wgpu::TextureFormat::Rgba8UnormSrgb,
    wgpu::TextureFormat::Bgra8UnormSrgb,
    wgpu::TextureFormat::Rgba8Unorm,
    wgpu::TextureFormat::Bgra8Unorm,
];

/// The configuration to use for a `size` (logical pixels) target at `scale`.
///
/// `scale` is the window's scale factor (`1.0` for a non-retina desktop). A non-finite or
/// non-positive scale is treated as `1.0`; the physical size is rounded rather than truncated so a
/// 1.5× window does not silently lose a pixel row.
///
/// When `caps` offers none of the 8-bit sRGB-family formats the presented frame could be written
/// in, the answer still has a shape (the first preference) but [`is_supported_by`] reports `false`
/// — the caller turns that into "this host cannot present" instead of letting the backend reject a
/// configuration it never promised.
pub fn choose_config(
    caps: &wgpu::SurfaceCapabilities,
    size: (u32, u32),
    scale: f32,
    quality: RenderQuality,
) -> wgpu::SurfaceConfiguration {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let width = scale_axis(size.0, scale);
    let height = scale_axis(size.1, scale);
    let preference = match quality {
        RenderQuality::ByteExact => EXACT_FORMAT_PREFERENCE,
        RenderQuality::Antialiased => SMOOTH_FORMAT_PREFERENCE,
    };
    let format = preference
        .iter()
        .copied()
        .find(|candidate| caps.formats.contains(candidate))
        .unwrap_or(EXACT_FORMAT_PREFERENCE[0]);
    let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
        wgpu::CompositeAlphaMode::Opaque
    } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
        wgpu::CompositeAlphaMode::PreMultiplied
    } else {
        // `Inherit`, `PostMultiplied` and `Auto` are not modelled by the uploader: the first two
        // would need a channel transform the software frame does not have, and `Auto` is a request
        // for the backend to pick, which this function does itself. Naming `Opaque` here keeps the
        // answer total; `is_supported_by` is what reports that it was not offered.
        wgpu::CompositeAlphaMode::Opaque
    };
    // Copying *out* of the presented frame is only needed by the verification entry point, and the
    // usage is only requested when the surface advertises it: asking for an unsupported usage makes
    // `configure` fail validation instead of failing one diagnostic.
    let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
    if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
        usage |= wgpu::TextureUsages::COPY_SRC;
    }
    wgpu::SurfaceConfiguration {
        usage,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: Vec::new(),
        color_space: wgpu::SurfaceColorSpace::Auto,
    }
}

/// The mode to use when `mode` is rejected, if any.
///
/// `AutoVsync` falls back to `Fifo`: the driver may refuse a low-latency mode on a composited
/// desktop, and `Fifo` is guaranteed by every backend the library targets. `Fifo` itself has no
/// fallback because it is the guaranteed mode.
pub const fn fallback_present_mode(mode: wgpu::PresentMode) -> Option<wgpu::PresentMode> {
    match mode {
        wgpu::PresentMode::AutoVsync => Some(wgpu::PresentMode::Fifo),
        _ => None,
    }
}

/// Whether a readback comparator must swap red and blue to recover the frame's RGBA order.
///
/// Rendering does not need this (the target's format maps components); reading a presented frame
/// back does, because the bytes come out in the target's order.
pub const fn needs_channel_swap(format: wgpu::TextureFormat) -> bool {
    matches!(format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb)
}

/// Whether an uploader must linearise the frame's sRGB bytes before writing them.
pub const fn needs_srgb_decode(format: wgpu::TextureFormat) -> bool {
    // Spelled as a match rather than `TextureFormat::is_srgb()` because that method is not `const`
    // on the pinned `wgpu` version; the four formats are the only ones this module chooses.
    matches!(format, wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb)
}

/// Whether an uploader must multiply colour channels by alpha before writing them.
pub const fn needs_premultiplication(alpha_mode: wgpu::CompositeAlphaMode) -> bool {
    matches!(alpha_mode, wgpu::CompositeAlphaMode::PreMultiplied)
}

/// Whether `caps` really accepts `config`.
///
/// [`choose_config`] always answers with *some* configuration so the caller has one value to
/// inspect; this is the honest query for "can that configuration be used at all". A presenter that
/// is told `false` reports [`super::PresentationStatus::Unsupported`] instead of letting the
/// backend reject a configuration it never promised.
pub fn is_supported_by(
    config: &wgpu::SurfaceConfiguration,
    caps: &wgpu::SurfaceCapabilities,
) -> bool {
    caps.formats.contains(&config.format) && caps.alpha_modes.contains(&config.alpha_mode)
}

fn scale_axis(logical: u32, scale: f32) -> u32 {
    let scaled = (logical as f32) * scale;
    if !scaled.is_finite() || scaled < 1.0 {
        return 1;
    }
    // `round` rather than `as u32`: a truncating cast drops a whole pixel row on a 1.5× display.
    (scaled.round() as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::{
        choose_config, fallback_present_mode, is_supported_by, needs_channel_swap,
        needs_premultiplication, needs_srgb_decode,
    };
    use crate::wgpu_backend::RenderQuality;

    fn caps(
        formats: &[wgpu::TextureFormat],
        alpha_modes: &[wgpu::CompositeAlphaMode],
    ) -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: formats.to_vec(),
            format_capabilities: Vec::new(),
            present_modes: vec![wgpu::PresentMode::Fifo, wgpu::PresentMode::AutoVsync],
            alpha_modes: alpha_modes.to_vec(),
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
        }
    }

    fn everything() -> wgpu::SurfaceCapabilities {
        caps(
            &[
                wgpu::TextureFormat::Bgra8UnormSrgb,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureFormat::Rgba8Unorm,
            ],
            &[wgpu::CompositeAlphaMode::PreMultiplied, wgpu::CompositeAlphaMode::Opaque],
        )
    }

    #[test]
    fn the_physical_size_rounds_the_scaled_logical_size() {
        let config = choose_config(&everything(), (800, 600), 2.0, RenderQuality::ByteExact);
        assert_eq!((config.width, config.height), (1600, 1200));

        let config = choose_config(&everything(), (801, 601), 1.5, RenderQuality::ByteExact);
        assert_eq!((config.width, config.height), (1202, 902), "1.5x rounds rather than truncates");
    }

    #[test]
    fn a_degenerate_size_or_scale_never_produces_zero() {
        let config = choose_config(&everything(), (0, 0), 0.0, RenderQuality::ByteExact);
        assert_eq!((config.width, config.height), (1, 1));

        let config = choose_config(&everything(), (10, 10), f32::NAN, RenderQuality::ByteExact);
        assert_eq!((config.width, config.height), (10, 10));
    }

    #[test]
    fn exact_prefers_verbatim_storage_over_srgb() {
        let config = choose_config(&everything(), (10, 10), 1.0, RenderQuality::ByteExact);
        assert_eq!(config.format, wgpu::TextureFormat::Rgba8Unorm);
        assert!(!needs_srgb_decode(config.format));
        assert!(!needs_channel_swap(config.format));
    }

    #[test]
    fn smooth_prefers_srgb_storage() {
        let config = choose_config(&everything(), (10, 10), 1.0, RenderQuality::Antialiased);
        assert_eq!(config.format, wgpu::TextureFormat::Rgba8UnormSrgb);
        assert!(needs_srgb_decode(config.format));
        assert!(!needs_channel_swap(config.format));
    }

    #[test]
    fn a_bgra_only_target_asks_for_the_channel_swap() {
        let only_bgra =
            caps(&[wgpu::TextureFormat::Bgra8Unorm], &[wgpu::CompositeAlphaMode::Opaque]);
        let config = choose_config(&only_bgra, (10, 10), 1.0, RenderQuality::ByteExact);
        assert_eq!(config.format, wgpu::TextureFormat::Bgra8Unorm);
        assert!(needs_channel_swap(config.format));
        assert!(is_supported_by(&config, &only_bgra));
    }

    #[test]
    fn opaque_is_preferred_when_the_target_offers_it() {
        let config = choose_config(&everything(), (10, 10), 1.0, RenderQuality::ByteExact);
        assert_eq!(config.alpha_mode, wgpu::CompositeAlphaMode::Opaque);
        assert!(!needs_premultiplication(config.alpha_mode));
    }

    #[test]
    fn a_premultiplied_only_target_is_reported_as_needing_premultiplication() {
        let premultiplied_only =
            caps(&[wgpu::TextureFormat::Rgba8Unorm], &[wgpu::CompositeAlphaMode::PreMultiplied]);
        let config = choose_config(&premultiplied_only, (10, 10), 1.0, RenderQuality::ByteExact);
        assert_eq!(config.alpha_mode, wgpu::CompositeAlphaMode::PreMultiplied);
        assert!(needs_premultiplication(config.alpha_mode));
        assert!(is_supported_by(&config, &premultiplied_only));
    }

    #[test]
    fn an_empty_capability_list_is_answered_honestly_as_unsupported() {
        let empty = wgpu::SurfaceCapabilities {
            formats: Vec::new(),
            format_capabilities: Vec::new(),
            present_modes: Vec::new(),
            alpha_modes: Vec::new(),
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
        };
        let config = choose_config(&empty, (10, 10), 1.0, RenderQuality::ByteExact);
        assert_eq!(config.format, wgpu::TextureFormat::Rgba8Unorm, "a value is still returned");
        assert!(!is_supported_by(&config, &empty), "but the caller is told it cannot be used");
    }

    #[test]
    fn present_mode_defaults_to_auto_vsync_and_falls_back_to_fifo() {
        let config = choose_config(&everything(), (10, 10), 1.0, RenderQuality::ByteExact);
        assert_eq!(config.present_mode, wgpu::PresentMode::AutoVsync);
        assert_eq!(fallback_present_mode(config.present_mode), Some(wgpu::PresentMode::Fifo));
        assert_eq!(fallback_present_mode(wgpu::PresentMode::Fifo), None);
    }

    #[test]
    fn the_quality_intent_is_the_only_difference_between_the_two_format_orders() {
        let exact = choose_config(&everything(), (32, 32), 1.0, RenderQuality::ByteExact);
        let smooth = choose_config(&everything(), (32, 32), 1.0, RenderQuality::Antialiased);
        assert_eq!((exact.width, exact.height), (smooth.width, smooth.height));
        assert_eq!(exact.alpha_mode, smooth.alpha_mode);
        assert_eq!(exact.present_mode, smooth.present_mode);
        assert_ne!(exact.format, smooth.format);
    }

    #[test]
    fn a_source_copy_usage_is_requested_only_when_the_target_offers_it() {
        let plain = choose_config(&everything(), (4, 4), 1.0, RenderQuality::ByteExact);
        assert_eq!(plain.usage, wgpu::TextureUsages::RENDER_ATTACHMENT);

        let mut with_copy = everything();
        with_copy.usages |= wgpu::TextureUsages::COPY_SRC;
        let config = choose_config(&with_copy, (4, 4), 1.0, RenderQuality::ByteExact);
        assert!(config.usage.contains(wgpu::TextureUsages::COPY_SRC));
    }

    #[test]
    fn the_frame_latency_and_view_formats_are_the_ones_the_presenter_relies_on() {
        let config = choose_config(&everything(), (4, 4), 1.0, RenderQuality::ByteExact);
        assert_eq!(config.desired_maximum_frame_latency, 2);
        assert!(config.view_formats.is_empty(), "no sRGB-view reinterpretation is requested");
        assert_eq!(config.color_space, wgpu::SurfaceColorSpace::Auto);
    }
}
