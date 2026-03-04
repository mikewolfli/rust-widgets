//! Demo of adaptive rendering quality management.

use rust_widgets::quality::{
    QualityConfig, QualityLevel, QualityManager, GpuCapability, FrameTimeMonitor,
};
use std::time::Duration;

fn main() {
    println!("Adaptive Rendering Quality Demo");
    println!("=================================\n");

    demo_quality_levels();
    demo_quality_config();
    demo_frame_time_monitor();
    demo_quality_manager();
    demo_gpu_capability();
}

fn demo_quality_levels() {
    println!("1. Quality Levels");
    println!("----------------");

    let high = QualityLevel::High;
    let medium = QualityLevel::Medium;
    let low = QualityLevel::Low;

    println!("High level: {:?}", high);
    println!("Medium level: {:?}", medium);
    println!("Low level: {:?}", low);

    println!("\nLevel ordering:");
    println!("Low < Medium: {}", low < medium);
    println!("Medium < High: {}", medium < high);

    println!("\nNavigation:");
    println!("High.lower() = {:?}", high.lower());
    println!("Medium.lower() = {:?}", medium.lower());
    println!("Low.lower() = {:?}", low.lower());

    println!("Low.higher() = {:?}", low.higher());
    println!("Medium.higher() = {:?}", medium.higher());
    println!("High.higher() = {:?}", high.higher());

    println!();
}

fn demo_quality_config() {
    println!("2. Quality Configuration");
    println!("------------------------");

    let config = QualityConfig {
        target_frame_rate: 60.0,
        degrade_threshold: 1.5,
        upgrade_threshold: 0.7,
        max_quality: QualityLevel::High,
        min_quality: QualityLevel::Low,
        degrade_frame_count: 5,
        upgrade_frame_count: 10,
    };

    println!("Target frame rate: {} FPS", config.target_frame_rate);
    println!("Target frame duration: {:.4} seconds", config.target_frame_duration());
    println!("Degrade threshold: {:.4} seconds", config.degrade_frame_duration());
    println!("Upgrade threshold: {:.4} seconds", config.upgrade_frame_duration());
    println!("Degrade frame count: {}", config.degrade_frame_count);
    println!("Upgrade frame count: {}", config.upgrade_frame_count);

    println!("\nNormalized config:");
    let normalized = config.normalized();
    println!("Degrade threshold: {:.4}", normalized.degrade_threshold);
    println!("Upgrade threshold: {:.4}", normalized.upgrade_threshold);

    println!();
}

fn demo_frame_time_monitor() {
    println!("3. Frame Time Monitor");
    println!("---------------------");

    let mut monitor = FrameTimeMonitor::new(60.0);

    println!("Target frame time: {:.4} seconds", monitor.target_frame_time());

    println!("\nRecording 10 frames at 60 FPS:");
    for i in 0..10 {
        let frame_time = 1.0 / 60.0;
        monitor.record_frame(frame_time);
        println!("Frame {}: {:.4}s, Avg FPS: {:.1}", i + 1, frame_time, monitor.current_fps());
    }

    println!("\nAverage frame time: {:.4} seconds", monitor.average_frame_time());
    println!("Current FPS: {:.1}", monitor.current_fps());

    println!("\nRecording 10 slow frames (30 FPS):");
    for i in 0..10 {
        let frame_time = 1.0 / 30.0;
        monitor.record_frame(frame_time);
        if i < 5 {
            println!("Frame {}: {:.4}s", i + 1, frame_time);
        }
    }

    let should_degrade = monitor.should_degrade(0.020, 5);
    println!("Should degrade (threshold 0.020s, 5 frames): {}", should_degrade);

    println!("\nRecording 10 fast frames (120 FPS):");
    for _ in 0..10 {
        let frame_time = 1.0 / 120.0;
        monitor.record_frame(frame_time);
    }

    let should_upgrade = monitor.should_upgrade(0.020, 5);
    println!("Should upgrade (threshold 0.020s, 5 frames): {}", should_upgrade);

    println!();
}

fn demo_quality_manager() {
    println!("4. Quality Manager");
    println!("------------------");

    let config = QualityConfig {
        target_frame_rate: 60.0,
        degrade_threshold: 1.5,
        upgrade_threshold: 0.7,
        max_quality: QualityLevel::High,
        min_quality: QualityLevel::Low,
        degrade_frame_count: 3,
        upgrade_frame_count: 5,
    };

    let mut manager = QualityManager::with_config(config);

    println!("Initial quality level: {:?}", manager.quality_level());
    println!("Current FPS: {:.1}", manager.current_fps());

    println!("\nSimulating slow rendering (30 FPS):");
    for i in 0..5 {
        let frame_duration = Duration::from_secs_f32(1.0 / 30.0);
        manager.finish_frame(frame_duration);
        println!("Frame {}: Quality = {:?}, FPS = {:.1}",
                 i + 1, manager.quality_level(), manager.current_fps());
    }

    println!("\nSimulating fast rendering (120 FPS):");
    for i in 0..10 {
        let frame_duration = Duration::from_secs_f32(1.0 / 120.0);
        manager.finish_frame(frame_duration);
        if i >= 5 {
            println!("Frame {}: Quality = {:?}, FPS = {:.1}",
                     i + 1, manager.quality_level(), manager.current_fps());
        }
    }

    println!("\nManual quality level control:");
    manager.set_quality_level(QualityLevel::Low);
    println!("Set to Low: {:?}", manager.quality_level());

    manager.set_quality_level(QualityLevel::High);
    println!("Set to High: {:?}", manager.quality_level());

    println!("\nResetting quality manager:");
    manager.reset();
    println!("After reset: {:?}", manager.quality_level());

    println!();
}

fn demo_gpu_capability() {
    println!("5. GPU Capability");
    println!("------------------");

    let discrete_gpu = GpuCapability {
        supports_high_quality: true,
        is_integrated: false,
        performance_tier: 5,
    };

    let integrated_gpu = GpuCapability {
        supports_high_quality: true,
        is_integrated: true,
        performance_tier: 3,
    };

    let low_end_gpu = GpuCapability {
        supports_high_quality: false,
        is_integrated: false,
        performance_tier: 1,
    };

    println!("Discrete GPU:");
    println!("  Supports high quality: {}", discrete_gpu.supports_high_quality);
    println!("  Is integrated: {}", discrete_gpu.is_integrated);
    println!("  Performance tier: {}", discrete_gpu.performance_tier);
    println!("  Recommended quality: {:?}", discrete_gpu.recommended_initial_quality());

    println!("\nIntegrated GPU:");
    println!("  Supports high quality: {}", integrated_gpu.supports_high_quality);
    println!("  Is integrated: {}", integrated_gpu.is_integrated);
    println!("  Performance tier: {}", integrated_gpu.performance_tier);
    println!("  Recommended quality: {:?}", integrated_gpu.recommended_initial_quality());

    println!("\nLow-end GPU:");
    println!("  Supports high quality: {}", low_end_gpu.supports_high_quality);
    println!("  Is integrated: {}", low_end_gpu.is_integrated);
    println!("  Performance tier: {}", low_end_gpu.performance_tier);
    println!("  Recommended quality: {:?}", low_end_gpu.recommended_initial_quality());

    println!("\nDefault capability:");
    let default = GpuCapability::default();
    println!("  Supports high quality: {}", default.supports_high_quality);
    println!("  Is integrated: {}", default.is_integrated);
    println!("  Performance tier: {}", default.performance_tier);
    println!("  Recommended quality: {:?}", default.recommended_initial_quality());

    println!();
}

#[allow(dead_code)]
fn demo_integration_example() {
    println!("6. Integration Example");
    println!("----------------------");

    println!("Example of integrating quality management into a render loop:\n");

    println!("```rust");
    println!("use rust_widgets::quality::{{QualityManager, QualityLevel}};");
    println!("use std::time::Instant;");
    println!();
    println!("fn render_loop(backend: &mut dyn PaintBackend, manager: &mut QualityManager) {{");
    println!("    loop {{");
    println!("        let frame_start = Instant::now();");
    println!();
    println!("        // Query current quality level");
    println!("        let quality = manager.quality_level();");
    println!();
    println!("        // Render based on quality level");
    println!("        match quality {{");
    println!("            QualityLevel::High => {{");
    println!("                // Full effects: anti-aliasing, shadows, complex shaders");
    println!("                render_with_full_effects(backend);");
    println!("            }}");
    println!("            QualityLevel::Medium => {{");
    println!("                // Basic effects: simple shaders, no shadows");
    println!("                render_with_basic_effects(backend);");
    println!("            }}");
    println!("            QualityLevel::Low => {{");
    println!("                // Minimal rendering: solid fills, no textures");
    println!("                render_minimal(backend);");
    println!("            }}");
    println!("        }}");
    println!();
    println!("        // Record frame duration and update quality");
    println!("        let frame_duration = frame_start.elapsed();");
    println!("        manager.finish_frame(frame_duration);");
    println!();
    println!("        // Optional: log quality changes");
    println!("        if quality != manager.quality_level() {{");
    println!("            println!(\"Quality changed: {{:?}} -> {{:?}}\",");
    println!("                     quality, manager.quality_level());");
    println!("        }}");
    println!("    }}");
    println!("}}");
    println!("```");
}

#[allow(dead_code)]
fn main_extended() {
    demo_quality_levels();
    demo_quality_config();
    demo_frame_time_monitor();
    demo_quality_manager();
    demo_gpu_capability();
    demo_integration_example();
}
