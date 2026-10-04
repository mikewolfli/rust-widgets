// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GPU backend selection with an explicit degradation ladder.
//!
//! `wgpu` can drive several graphics APIs, and a host does not always offer the
//! best one. A virtual machine with no Vulkan/Metal/DX12 driver may still expose
//! OpenGL ES through ANGLE/EGL; a headless CI container may offer no hardware
//! backend at all and only succeed with the CPU (software) adapter. Requesting
//! `Backends::all()` in one shot hides *which* tier was chosen and gives the
//! caller no way to report a degraded start-up.
//!
//! This module turns that into an explicit, ordered ladder:
//!
//! 1. [`GpuBackendTier::Primary`] — Vulkan, Metal, DX12, browser WebGPU.
//! 2. [`GpuBackendTier::OpenGlEs`] — the GL backend: OpenGL ES on
//!    Linux/Android, WebGL on the web, and desktop OpenGL through
//!    ANGLE on Windows and macOS.
//! 3. [`GpuBackendTier::Software`] — the CPU fallback adapter, always attempted
//!    last because it is correct but slow.
//!
//! Each tier is tried in order until one yields an adapter, and the winner is
//! reported through [`GpuBackendSelection`] so hosts can log the degradation
//! rather than discovering it as a mysterious performance drop.
//!
//! # Environment override
//!
//! `WGPU_BACKEND` is honoured, matching `wgpu`'s own behaviour: an explicit
//! `WGPU_BACKEND=gl` pins the process to the GL ladder. When the variable is set,
//! custom ordering is skipped and the requested backends are used verbatim, so a
//! user debugging a driver issue always gets exactly what they asked for.

/// The graphics API tier a `wgpu` adapter came from.
///
/// Ordered from best to most degraded; see the module docs for the fallback
/// ladder. `Ord` follows the declaration order, so `Primary < OpenGlEs <
/// Software`, and a caller can test `chosen > GpuBackendTier::Primary` to detect
/// that start-up was degraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GpuBackendTier {
    /// Vulkan, Metal, DX12 or browser WebGPU — the first tier of `wgpu` support.
    Primary,
    /// The GL backend: OpenGL ES (Linux/Android), WebGL (web), desktop OpenGL
    /// via ANGLE (Windows/macOS). Second tier: widely available but with fewer
    /// modern features.
    OpenGlEs,
    /// CPU/software rasterization. Correct everywhere, but the slowest option,
    /// and normally reached only on a host with no usable GPU driver.
    Software,
}

impl GpuBackendTier {
    /// A short, human-readable label for logs and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Self::Primary => "primary (Vulkan/Metal/DX12/WebGPU)",
            Self::OpenGlEs => "OpenGL ES (GL backend)",
            Self::Software => "software (CPU fallback)",
        }
    }

    /// Whether this tier is a degraded, below-primary path.
    ///
    /// Hosts use this to emit a one-time warning when the process could not get
    /// a primary backend, instead of silently running at reduced capability.
    pub fn is_degraded(self) -> bool {
        self != Self::Primary
    }

    /// Maps a `wgpu` adapter's backend to its ladder tier.
    ///
    /// `Noop` is mapped to [`GpuBackendTier::Software`] because it performs no
    /// real rasterization and can only be the terminal fallback in a build that
    /// explicitly enables it.
    pub fn from_backend(backend: wgpu::Backend) -> Self {
        match backend {
            wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::Dx12 => Self::Primary,
            wgpu::Backend::Gl => Self::OpenGlEs,
            wgpu::Backend::BrowserWebGpu => Self::Primary,
            wgpu::Backend::Noop => Self::Software,
        }
    }
}

/// Which tier was used, and which backends that tier requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuBackendSelection {
    /// The tier the selected adapter belongs to.
    pub tier: GpuBackendTier,
    /// The concrete `wgpu` backend of the selected adapter.
    pub backend: wgpu::Backend,
}

impl GpuBackendSelection {
    /// Whether the caller should log a degradation warning.
    pub fn is_degraded(&self) -> bool {
        self.tier.is_degraded()
    }
}

/// The ordered `(tier, backends)` ladder, best first.
///
/// Browser WebGPU is deliberately *not* listed in [`GpuBackendTier::Primary`]:
/// `wgpu` decides WebGPU support when the `Instance` is created rather than from
/// the adapter, so on `wasm32` the browser target must be requested explicitly.
/// See [`ladder_for_target`].
pub const BACKEND_LADDER: &[(GpuBackendTier, wgpu::Backends)] = &[
    (GpuBackendTier::Primary, wgpu::Backends::PRIMARY),
    (GpuBackendTier::OpenGlEs, wgpu::Backends::GL),
    (GpuBackendTier::Software, wgpu::Backends::empty()),
];

/// Returns the ladder to walk for the compilation target.
///
/// On `wasm32` the only backend is the browser's WebGPU, so the ladder collapses
/// to a single rung; on native targets it is [`BACKEND_LADDER`] unchanged.
pub fn ladder_for_target() -> &'static [(GpuBackendTier, wgpu::Backends)] {
    #[cfg(target_arch = "wasm32")]
    {
        &[(GpuBackendTier::Primary, wgpu::Backends::BROWSER_WEBGPU)]
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        BACKEND_LADDER
    }
}

/// Resolves the backends an `Instance` should be created with.
///
/// Returns the union of the whole degradation ladder and any explicit
/// `WGPU_BACKEND` value. The union matters: `Instance` creation decides which
/// backend modules are even loaded, so narrowing it to a single rung would make
/// the remaining rungs unreachable, and narrowing it to `WGPU_BACKEND=gl` alone
/// would turn a recoverable situation into a hard failure on hosts whose GL
/// driver is missing (macOS without ANGLE, for example).
///
/// The environment value still steers *selection* through
/// [`backends_from_env`]; this function only guarantees the instance is capable
/// of reaching every rung so the fallback can actually do its job.
pub fn instance_backends() -> wgpu::Backends {
    let from_ladder = ladder_for_target()
        .iter()
        .fold(wgpu::Backends::empty(), |acc, (_, backends)| acc | *backends);

    match backends_from_env() {
        // An explicit choice adds its backend to the instance without removing
        // the ladder's reachability.
        Some(pinned) => from_ladder | pinned,
        None => from_ladder,
    }
}

/// Resolves the backends to request, honouring an explicit `WGPU_BACKEND`.
///
/// Returns `Some(backends)` when the environment pins a choice, which tells the
/// caller to skip the ladder and use exactly those backends. `None` means "walk
/// the ladder".
///
/// `wgpu` itself reads this variable, but only after an `Instance` already
/// exists; reading it here lets the ladder be bypassed deliberately, so a
/// developer forcing `WGPU_BACKEND=gl` is not silently overridden by a
/// higher-priority tier succeeding first.
///
/// Note the interaction with [`instance_backends`]: selection is pinned, but the
/// instance is still created with the full ladder so a failed pin produces a
/// clear error instead of an instance that cannot be probed at all.
pub fn backends_from_env() -> Option<wgpu::Backends> {
    wgpu::Backends::from_env()
}

/// Maps an adapter's device type and backend to its ladder tier.
///
/// The adapter's reported backend is the primary signal, but a CPU (software)
/// adapter must always report [`GpuBackendTier::Software`] even when the driver
/// exposes it through a hardware backend name (lavapipe reports Vulkan,
/// llvmpipe reports GL). Otherwise the software fallback would be misreported
/// as `Primary`/`OpenGlEs` and `is_degraded()` would stay silent.
fn tier_of_adapter(device_type: wgpu::DeviceType, backend: wgpu::Backend) -> GpuBackendTier {
    if device_type == wgpu::DeviceType::Cpu {
        GpuBackendTier::Software
    } else {
        GpuBackendTier::from_backend(backend)
    }
}

/// Ranks a candidate adapter for [`select_adapter_with_gl_fallback`].
///
/// Lower is better. The CPU adapter is always last; among hardware adapters a
/// discrete GPU outranks an integrated one under `HighPerformance` and vice
/// versa under `LowPower`, keeping the choice consistent with the requested
/// power preference. The sort is stable, so equal-scoring adapters keep their
/// enumeration order.
fn rank_adapter(device_type: wgpu::DeviceType, power_preference: wgpu::PowerPreference) -> u8 {
    match device_type {
        wgpu::DeviceType::Cpu => 4,
        wgpu::DeviceType::DiscreteGpu => match power_preference {
            wgpu::PowerPreference::LowPower => 1,
            _ => 0,
        },
        wgpu::DeviceType::IntegratedGpu => match power_preference {
            wgpu::PowerPreference::HighPerformance => 1,
            _ => 0,
        },
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
    }
}

/// Picks the best adapter restricted to `backends` that can present to
/// `compatible_surface`.
///
/// `request_adapter` cannot filter by backend set — it returns the default
/// adapter and silently ignores a pin — so each rung enumerates its own backend
/// set and filters/ranks the result here.
async fn best_adapter_for_backends(
    instance: &wgpu::Instance,
    backends: wgpu::Backends,
    power_preference: wgpu::PowerPreference,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Option<wgpu::Adapter> {
    let mut candidates = instance.enumerate_adapters(backends).await;
    if let Some(surface) = compatible_surface {
        candidates.retain(|adapter| adapter.is_surface_supported(surface));
    }
    candidates.sort_by_key(|adapter| {
        let info = adapter.get_info();
        rank_adapter(info.device_type, power_preference)
    });
    candidates.into_iter().next()
}

/// Picks the best available adapter by walking the ladder.
///
/// Tries each rung in order with `power_preference`, returning the first adapter
/// that materializes together with the tier it came from. Returns `None` when
/// every rung fails, which on native targets means no usable GPU backend at all
/// and the caller should fall back to software rendering outside `wgpu`.
///
/// `force_fallback_adapter` is applied to the final rung only: asking for the
/// CPU adapter earlier would mask a working hardware GL path.
pub async fn select_adapter_with_gl_fallback(
    instance: &wgpu::Instance,
    power_preference: wgpu::PowerPreference,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Option<(wgpu::Adapter, GpuBackendTier)> {
    // An explicit environment choice wins outright. It has to be applied by
    // filtering the enumerated adapters: `wgpu` honours `WGPU_BACKEND` when the
    // `Instance` is created, but `instance_backends()` deliberately widens the
    // instance to keep every ladder rung reachable, so `request_adapter` alone
    // would hand back the default (primary) adapter and silently ignore the pin.
    if let Some(pinned) = backends_from_env() {
        let mut candidates = instance.enumerate_adapters(pinned).await;
        if candidates.is_empty() {
            log::error!("[gpu] WGPU_BACKEND pinned {pinned:?} but no adapter exposes that backend");
            return None;
        }
        // Prefer a hardware adapter over the CPU one when several qualify.
        candidates.sort_by_key(|adapter| adapter.get_info().device_type == wgpu::DeviceType::Cpu);
        let adapter = candidates.into_iter().next()?;
        let backend = adapter.get_info().backend;
        return Some((adapter, GpuBackendTier::from_backend(backend)));
    }

    let ladder = ladder_for_target();

    for (tier, backends) in ladder.iter() {
        // Each rung restricts its search to the rung's own backend set, so a GL
        // adapter can only win on the OpenGL ES rung and a Vulkan/Metal/DX12
        // adapter only on the primary rung. The software rung is the exception:
        // it carries `Backends::empty()` (there is no "software" backend bit)
        // and instead asks wgpu for the forced CPU fallback adapter.
        let adapter = if backends.is_empty() {
            match instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference,
                    compatible_surface,
                    force_fallback_adapter: true,
                    apply_limit_buckets: false,
                })
                .await
            {
                Ok(adapter) => adapter,
                Err(error) => {
                    log::debug!(
                        "[gpu] fallback adapter request failed for tier {}: {error:?}",
                        tier.label(),
                    );
                    continue;
                }
            }
        } else {
            match best_adapter_for_backends(
                instance,
                *backends,
                power_preference,
                compatible_surface,
            )
            .await
            {
                Some(adapter) => adapter,
                None => {
                    log::debug!("[gpu] no adapter on tier {}", tier.label());
                    continue;
                }
            }
        };

        // Report the tier from the adapter that actually won, never the label of
        // the rung that happened to be probed, so `is_degraded()` stays honest.
        let info = adapter.get_info();
        return Some((adapter, tier_of_adapter(info.device_type, info.backend)));
    }

    log::warn!("[gpu] no adapter available on any tier of the degradation ladder");
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder must be ordered best-to-worst, because `select_adapter_with_gl_fallback`
    /// returns on the first success. A reordered table would silently prefer
    /// OpenGL ES over Vulkan/Metal.
    #[test]
    fn ladder_is_ordered_from_primary_to_software() {
        let tiers: Vec<GpuBackendTier> = BACKEND_LADDER.iter().map(|(tier, _)| *tier).collect();
        assert_eq!(
            tiers,
            vec![GpuBackendTier::Primary, GpuBackendTier::OpenGlEs, GpuBackendTier::Software]
        );

        // `Ord` must agree with the table order so `is_degraded`/comparisons work.
        assert!(GpuBackendTier::Primary < GpuBackendTier::OpenGlEs);
        assert!(GpuBackendTier::OpenGlEs < GpuBackendTier::Software);
    }

    /// The OpenGL ES rung must actually request the GL backend — this is the
    /// support this module exists to add.
    #[test]
    fn opengl_es_rung_requests_the_gl_backend() {
        let (_, backends) =
            BACKEND_LADDER.iter().find(|(t, _)| *t == GpuBackendTier::OpenGlEs).expect("GL rung");
        assert_eq!(*backends, wgpu::Backends::GL);
        assert!(backends.contains(wgpu::Backends::GL));
    }

    /// The primary rung must not accidentally include GL, otherwise the GL
    /// fallback would be indistinguishable from the primary path.
    #[test]
    fn primary_rung_excludes_gl() {
        let (_, backends) = BACKEND_LADDER
            .iter()
            .find(|(t, _)| *t == GpuBackendTier::Primary)
            .expect("primary rung");
        assert_eq!(*backends, wgpu::Backends::PRIMARY);
        assert!(!backends.contains(wgpu::Backends::GL));
    }

    #[test]
    fn backend_maps_to_expected_tier() {
        assert_eq!(GpuBackendTier::from_backend(wgpu::Backend::Vulkan), GpuBackendTier::Primary);
        assert_eq!(GpuBackendTier::from_backend(wgpu::Backend::Metal), GpuBackendTier::Primary);
        assert_eq!(GpuBackendTier::from_backend(wgpu::Backend::Dx12), GpuBackendTier::Primary);
        assert_eq!(GpuBackendTier::from_backend(wgpu::Backend::Gl), GpuBackendTier::OpenGlEs);
        assert_eq!(GpuBackendTier::from_backend(wgpu::Backend::Noop), GpuBackendTier::Software);
    }

    /// Only `Primary` is non-degraded; the other two must report degradation so
    /// hosts can warn instead of silently running slower.
    #[test]
    fn degradation_is_reported_for_non_primary_tiers() {
        assert!(!GpuBackendTier::Primary.is_degraded());
        assert!(GpuBackendTier::OpenGlEs.is_degraded());
        assert!(GpuBackendTier::Software.is_degraded());

        let gl = GpuBackendSelection { tier: GpuBackendTier::OpenGlEs, backend: wgpu::Backend::Gl };
        assert!(gl.is_degraded());
    }

    /// A CPU adapter must report `Software` regardless of the backend name the
    /// driver advertises, so the software fallback is never misreported as a
    /// hardware tier and `is_degraded()` stays honest.
    #[test]
    fn cpu_adapter_maps_to_software_regardless_of_backend() {
        assert_eq!(
            tier_of_adapter(wgpu::DeviceType::Cpu, wgpu::Backend::Vulkan),
            GpuBackendTier::Software,
        );
        assert_eq!(
            tier_of_adapter(wgpu::DeviceType::Cpu, wgpu::Backend::Gl),
            GpuBackendTier::Software,
        );
        assert_eq!(
            tier_of_adapter(wgpu::DeviceType::Cpu, wgpu::Backend::Noop),
            GpuBackendTier::Software,
        );
        // A real GPU still maps through its backend name.
        assert_eq!(
            tier_of_adapter(wgpu::DeviceType::DiscreteGpu, wgpu::Backend::Metal),
            GpuBackendTier::Primary,
        );
        assert_eq!(
            tier_of_adapter(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Gl),
            GpuBackendTier::OpenGlEs,
        );
    }

    /// Candidate ranking must honour the power preference and keep the CPU
    /// adapter last, so per-rung selection stays consistent with
    /// `power_preference`.
    #[test]
    fn adapter_ranking_honours_power_preference() {
        // HighPerformance prefers discrete over integrated.
        assert!(
            rank_adapter(wgpu::DeviceType::DiscreteGpu, wgpu::PowerPreference::HighPerformance)
                < rank_adapter(
                    wgpu::DeviceType::IntegratedGpu,
                    wgpu::PowerPreference::HighPerformance
                )
        );
        // LowPower prefers integrated over discrete.
        assert!(
            rank_adapter(wgpu::DeviceType::IntegratedGpu, wgpu::PowerPreference::LowPower)
                < rank_adapter(wgpu::DeviceType::DiscreteGpu, wgpu::PowerPreference::LowPower)
        );
        // The CPU adapter is always ranked last.
        assert!(
            rank_adapter(wgpu::DeviceType::DiscreteGpu, wgpu::PowerPreference::HighPerformance)
                < rank_adapter(wgpu::DeviceType::Cpu, wgpu::PowerPreference::HighPerformance)
        );
        assert!(
            rank_adapter(wgpu::DeviceType::IntegratedGpu, wgpu::PowerPreference::LowPower)
                < rank_adapter(wgpu::DeviceType::Cpu, wgpu::PowerPreference::LowPower)
        );
    }

    /// A real adapter must be found through the ladder on a GPU-capable host,
    /// and the reported tier must match the adapter's actual backend. Skips
    /// silently where `wgpu` finds nothing (headless CI), so it stays honest.
    #[test]
    fn ladder_selects_a_real_adapter_and_reports_its_true_tier() {
        let ladder = ladder_for_target();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: ladder.iter().fold(wgpu::Backends::empty(), |acc, (_, b)| acc | *b),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        // Block on the async selection without an async runtime: driving the
        // future to completion with a trivial waker is enough here because
        // `request_adapter` on a native target resolves without yielding.
        let selected = block_on_ladder(select_adapter_with_gl_fallback(
            &instance,
            wgpu::PowerPreference::HighPerformance,
            None,
        ));

        let Some((adapter, tier)) = selected else {
            // No adapter at all: nothing to assert, and no false confidence.
            return;
        };

        let info = adapter.get_info();
        assert_eq!(
            tier,
            tier_of_adapter(info.device_type, info.backend),
            "reported tier {:?} disagrees with adapter backend {:?}",
            tier,
            info.backend,
        );
    }

    /// Minimal executor: drives one future to completion on the current thread.
    ///
    /// `pollster` is only a dependency of the `gpu-wgpu` feature, so the unit
    /// tests cannot rely on it when built with a narrower feature set. A no-op
    /// waker is sufficient here because `request_adapter`/`enumerate_adapters`
    /// resolve without needing a real wake-up.
    fn block_on_ladder<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, Waker};

        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    /// The instance must be able to reach every rung, even when `WGPU_BACKEND`
    /// pins one backend.
    ///
    /// Regression guard: `wgpu` applies `WGPU_BACKEND` at instance creation, so
    /// passing it straight through narrowed the instance to a single backend and
    /// made the other rungs unreachable — on macOS that turned `WGPU_BACKEND=gl`
    /// into an unrecoverable dead end instead of a clear diagnostic.
    #[test]
    fn instance_backends_keeps_the_whole_ladder_reachable() {
        let union = instance_backends();
        for (tier, backends) in ladder_for_target() {
            // The software rung is `empty()`; OR-ing it is a no-op and must not
            // be mistaken for "the ladder is missing a rung".
            if backends.is_empty() {
                continue;
            }
            assert!(
                union.contains(*backends),
                "instance backends {union:?} cannot reach tier {} ({backends:?})",
                tier.label(),
            );
        }
    }

    /// A pinned `WGPU_BACKEND` that the host cannot provide must yield `None`,
    /// never a silent fallback to a different backend.
    ///
    /// This is the observable contract that replaced the silent-ignore bug: the
    /// caller sees "no adapter" and must report it, rather than unknowingly
    /// running on a backend the operator did not ask for.
    #[test]
    fn pinned_backend_that_is_unavailable_yields_none_without_substitution() {
        let Some(pinned) = backends_from_env() else {
            return; // Nothing pinned in this environment: the guard is inactive.
        };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: instance_backends(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        let available = block_on_ladder(instance.enumerate_adapters(pinned));
        let selected = block_on_ladder(select_adapter_with_gl_fallback(
            &instance,
            wgpu::PowerPreference::HighPerformance,
            None,
        ));

        match selected {
            // Whenever the pin resolves, the chosen adapter must genuinely be on
            // that backend. A different backend would be the silent substitution
            // this test exists to prevent.
            Some((adapter, tier)) => {
                let info = adapter.get_info();
                assert_eq!(
                    tier,
                    GpuBackendTier::from_backend(info.backend),
                    "reported tier must describe the selected adapter",
                );
                assert!(
                    pinned.contains(wgpu::Backends::from(info.backend)),
                    "selected backend {:?} is outside the pinned set {pinned:?}",
                    info.backend,
                );
            }
            // The host cannot serve the pin; an empty candidate list is the
            // honest explanation, and `None` is the correct result.
            None => assert!(
                available.is_empty(),
                "missing adapter while {pinned:?} still exposes candidates",
            ),
        }
    }
}
