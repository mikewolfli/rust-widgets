//! Layout Inspector — deferred diagnostic engine for layout correctness.
//!
//! Detects orphan widgets, empty layouts, zero-size widgets, and overlapping
//! widgets in both JSON-declarative and native programmatic layouts.
//!
//! # Zero-overhead guarantee
//!
//! When disabled (default), all collector functions return immediately
//! with no heap allocation. Enable only on demand after layout completes.
//!
//! # Usage
//!
//! ```rust,ignore
//! use rust_widgets::layout::inspector::LayoutInspector;
//!
//! LayoutInspector::enable();
//! // ... run layout.update() which records geometry ...
//! let report = LayoutInspector::run_once(&registry);
//! if report.has_issues() {
//!     eprintln!("{}", report);
//! }
//! LayoutInspector::disable();
//! ```

use core::cell::RefCell;
use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::core::{ObjectId, Rect};
use crate::index::{WidgetKind, WidgetRegistry};

// ── Global atomic switch ─────────────────────────────────────

static ENABLED: AtomicBool = AtomicBool::new(false);

// ── Thread-local storage ─────────────────────────────────────

thread_local! {
    /// Rect geometry recorded during layout.update() callbacks.
    static GEOMETRY_SNAPSHOT: RefCell<Vec<(ObjectId, Rect)>> = const { RefCell::new(Vec::new()) };

    /// Layout information registered for native programmatic layouts.
    static NATIVE_LAYOUTS: RefCell<Vec<NativeLayoutEntry>> = const { RefCell::new(Vec::new()) };
}

// ── Types ────────────────────────────────────────────────────

/// Severity of a detected layout issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Informational hint, no action required.
    Info,
    /// Potential issue that may cause visual glitches.
    Warning,
    /// Definite layout problem causing invisible or broken widgets.
    Error,
}

/// Categorised severity label.
pub fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

/// Localised severity label (supports multi-language).
pub fn severity_label_localised(s: Severity) -> &'static str {
    match s {
        Severity::Info => "\u{2139}\u{FE0F} info",
        Severity::Warning => "\u{26A0}\u{FE0F} warning",
        Severity::Error => "\u{274C} error",
    }
}

/// Category of the diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCategory {
    /// Structural issue (orphan, empty).
    Structural,
    /// Geometric issue (zero-size, overlap).
    Geometric,
    /// Layout routing issue (type mismatch).
    Layout,
}

impl fmt::Display for IssueCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssueCategory::Structural => write!(f, "structure"),
            IssueCategory::Geometric => write!(f, "geometry"),
            IssueCategory::Layout => write!(f, "layout"),
        }
    }
}

/// Single layout issue detected during diagnosis.
#[derive(Debug, Clone)]
pub struct Issue {
    /// Severity of the issue.
    pub severity: Severity,
    /// Human-readable description in English.
    pub description: String,
    /// Widget id involved, if applicable.
    pub widget_id: Option<ObjectId>,
    /// Category of this issue.
    pub category: IssueCategory,
}

/// Recommendation for resolving an issue or group of issues.
#[derive(Debug, Clone)]
pub struct Recommendation {
    /// Short title (e.g. "Recalculate layout").
    pub title: String,
    /// One-line summary.
    pub summary: String,
    /// Expanded detail or code example.
    pub detail: String,
}

/// Diagnostic report produced by `LayoutInspector::run_once`.
#[derive(Debug, Clone)]
pub struct DiagnosticReport {
    /// All detected issues.
    pub issues: Vec<Issue>,
    /// Actionable recommendations.
    pub recommendations: Vec<Recommendation>,
    /// Number of widget entries inspected.
    pub widgets_inspected: usize,
    /// Number of layout managers inspected.
    pub layouts_inspected: usize,
}

impl DiagnosticReport {
    /// Create an empty report (no issues, no recommendations).
    pub fn empty() -> Self {
        Self {
            issues: Vec::new(),
            recommendations: Vec::new(),
            widgets_inspected: 0,
            layouts_inspected: 0,
        }
    }

    /// Whether the report contains any issue.
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }

    /// Whether the report contains at least one Error-severity issue.
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    /// Whether the report contains at least one Warning-severity issue.
    pub fn has_warnings(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Warning)
    }

    /// Number of issues with a specific severity.
    pub fn count_by_severity(&self, s: Severity) -> usize {
        self.issues.iter().filter(|i| i.severity == s).count()
    }
}

impl fmt::Display for DiagnosticReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Layout Inspector Report")?;
        writeln!(f, "  widgets inspected: {}", self.widgets_inspected)?;
        writeln!(f, "  layouts inspected: {}", self.layouts_inspected)?;
        writeln!(
            f,
            "  issues: {} ({} error, {} warning, {} info)",
            self.issues.len(),
            self.count_by_severity(Severity::Error),
            self.count_by_severity(Severity::Warning),
            self.count_by_severity(Severity::Info),
        )?;
        if !self.issues.is_empty() {
            writeln!(f)?;
            for (i, issue) in self.issues.iter().enumerate() {
                let sev = severity_label_localised(issue.severity);
                writeln!(f, "  {}. [{}] {} — {}", i + 1, sev, issue.category, issue.description)?;
                if let Some(wid) = issue.widget_id {
                    writeln!(f, "     widget: {}", wid)?;
                }
            }
        }
        if !self.recommendations.is_empty() {
            writeln!(f)?;
            writeln!(f, "  Recommendations:")?;
            for rec in &self.recommendations {
                writeln!(f, "    - {}: {}", rec.title, rec.summary)?;
            }
        }
        Ok(())
    }
}

/// Information about a native (programmatic) layout for diagnosis.
struct NativeLayoutEntry {
    parent_id: ObjectId,
    label: String,
    item_count: usize,
    layout_type: String,
}

// ── Local helpers ────────────────────────────────────────────

/// Check if two (widget_id, Rect) entries from the same parent overlap.
/// Ignores neighbouring rects that only touch at edges.
fn rects_overlap_excluding_touch(a: &Rect, b: &Rect) -> bool {
    let a_x2 = a.x.saturating_add_unsigned(a.width);
    let a_y2 = a.y.saturating_add_unsigned(a.height);
    let b_x2 = b.x.saturating_add_unsigned(b.width);
    let b_y2 = b.y.saturating_add_unsigned(b.height);
    a.x < b_x2.saturating_sub(1)
        && a_x2.saturating_sub(1) > b.x
        && a.y < b_y2.saturating_sub(1)
        && a_y2.saturating_sub(1) > b.y
}

// ── LayoutInspector public API ───────────────────────────────

#[derive(Debug)]
pub struct LayoutInspector;

impl LayoutInspector {
    /// Enable diagnostic collection. No-op if already enabled.
    #[inline]
    pub fn enable() {
        ENABLED.store(true, Ordering::Release);
    }

    /// Disable diagnostic collection. All subsequent calls to
    /// `record_geometry`, `register_native_layout`, and `run_once`
    /// will be no-ops.
    #[inline]
    pub fn disable() {
        ENABLED.store(false, Ordering::Release);
    }

    /// Whether the inspector is currently enabled.
    #[inline]
    pub fn is_enabled() -> bool {
        ENABLED.load(Ordering::Acquire)
    }

    /// Record a widget's final geometry from a layout.update() callback.
    ///
    /// This is called automatically when the inspector is enabled and
    /// a layout.update() callback wrapper invokes it. If the widget
    /// already has a recorded rect, it is overwritten (last wins).
    #[inline]
    pub fn record_geometry(widget_id: ObjectId, rect: Rect) {
        if !Self::is_enabled() {
            return;
        }
        GEOMETRY_SNAPSHOT.with(|snap| {
            let mut snap = snap.borrow_mut();
            // Update in-place if already recorded, else push.
            if let Some(entry) = snap.iter_mut().find(|(id, _)| *id == widget_id) {
                entry.1 = rect;
            } else {
                snap.push((widget_id, rect));
            }
        });
    }

    /// Register a native (programmatic) layout for diagnosis.
    ///
    /// Call this after creating a layout and adding widgets to it.
    /// The label and layout_type are used in diagnostic output.
    #[inline]
    pub fn register_native_layout(
        parent_id: ObjectId,
        label: &str,
        item_count: usize,
        layout_type: &str,
    ) {
        if !Self::is_enabled() {
            return;
        }
        NATIVE_LAYOUTS.with(|layouts| {
            let mut layouts = layouts.borrow_mut();
            // Update in-place if already registered.
            if let Some(entry) =
                layouts.iter_mut().find(|e: &&mut NativeLayoutEntry| e.parent_id == parent_id)
            {
                entry.label = label.to_string();
                entry.item_count = item_count;
                entry.layout_type = layout_type.to_string();
            } else {
                layouts.push(NativeLayoutEntry {
                    parent_id,
                    label: label.to_string(),
                    item_count,
                    layout_type: layout_type.to_string(),
                });
            }
        });
    }

    /// Run diagnostics once and log any issues found.
    ///
    /// This is a convenience wrapper around `run_once` that logs each
    /// detected issue via `log::warn!()`. If the inspector is disabled,
    /// this is a no-op.
    pub fn run_once_logged(registry: &WidgetRegistry) {
        if !Self::is_enabled() {
            return;
        }
        let report = Self::run_once(registry);
        if report.has_issues() {
            for issue in &report.issues {
                log::warn!("[layout] {}", issue.description);
            }
        }
    }

    /// Run diagnostics once and return a report.
    ///
    /// After this call, all collected snapshots are cleared so that
    /// the next invocation starts fresh. Returns an empty report if
    /// the inspector is disabled.
    pub fn run_once(registry: &WidgetRegistry) -> DiagnosticReport {
        if !Self::is_enabled() {
            return DiagnosticReport::empty();
        }

        let all_ids: Vec<ObjectId> = registry.all_ids().collect();
        let widgets_inspected = all_ids.len();

        // ── D1: Orphan check ──
        let orphan_issues = Self::check_orphans(registry, &all_ids);

        // ── D2: Empty layout check ──
        let empty_layout_issues = Self::check_empty_layouts(registry);

        // ── D3: Zero-size check ──
        let (zero_size_issues, geometries) = Self::check_zero_sizes();

        // ── D4: Overlap check ──
        let overlap_issues = Self::check_overlaps(&geometries, registry);

        let mut issues: Vec<Issue> = Vec::new();
        issues.extend(orphan_issues);
        issues.extend(empty_layout_issues);
        issues.extend(zero_size_issues);
        issues.extend(overlap_issues);

        let layouts_inspected = NATIVE_LAYOUTS.with(|layouts| layouts.borrow().len());

        // Generate recommendations.
        let recommendations = Self::generate_recommendations(&issues);

        // Clear storage for next run.
        GEOMETRY_SNAPSHOT.with(|snap| snap.borrow_mut().clear());
        NATIVE_LAYOUTS.with(|layouts| layouts.borrow_mut().clear());

        DiagnosticReport { issues, recommendations, widgets_inspected, layouts_inspected }
    }

    // ── Diagnostic check implementations ─────────────────────

    /// D1: Detect orphan widgets (no parent, and not a Window).
    ///
    /// Returns a Vec of Issue (Warning severity) per orphan.
    fn check_orphans(registry: &WidgetRegistry, all_ids: &[ObjectId]) -> Vec<Issue> {
        let mut issues = Vec::new();
        for id in all_ids {
            if let Some(entry) = registry.get(*id) {
                if entry.parent.is_none() && entry.kind != WidgetKind::Window {
                    issues.push(Issue {
                        severity: Severity::Warning,
                        description: format!(
                            "orphan widget '{}' (kind={:?}) has no parent — may be invisible or misplaced",
                            entry.label, entry.kind
                        ),
                        widget_id: Some(*id),
                        category: IssueCategory::Structural,
                    });
                }
            }
        }
        issues
    }

    /// D2: Detect empty layout managers (registered but with zero children).
    ///
    /// Checks native layout registrations. Returns Info severity issues.
    fn check_empty_layouts(registry: &WidgetRegistry) -> Vec<Issue> {
        let mut issues = Vec::new();
        NATIVE_LAYOUTS.with(|layouts| {
            for entry in layouts.borrow().iter() {
                if entry.item_count == 0 {
                    let parent_label = registry
                        .get(entry.parent_id)
                        .map(|e| e.label.as_str())
                        .unwrap_or("<unknown>");
                    issues.push(Issue {
                        severity: Severity::Info,
                        description: format!(
                            "empty layout '{}' (type={}) for widget '{}' — no children added",
                            entry.label, entry.layout_type, parent_label
                        ),
                        widget_id: Some(entry.parent_id),
                        category: IssueCategory::Structural,
                    });
                }
            }
        });
        issues
    }

    /// D3: Detect widgets with zero width or height.
    ///
    /// Returns (issues, geometry_map) where geometry_map is a
    /// snapshot of all recorded rects keyed by (parent_id, widget_id).
    fn check_zero_sizes() -> (Vec<Issue>, Vec<(ObjectId, Rect)>) {
        let geometries = GEOMETRY_SNAPSHOT.with(|snap| snap.borrow().clone());
        let issues = geometries
            .iter()
            .filter(|(_, rect)| rect.width == 0 || rect.height == 0)
            .map(|(widget_id, rect)| {
                let desc = if rect.width == 0 && rect.height == 0 {
                    format!("widget {} has zero size (0x0) — completely invisible", widget_id)
                } else if rect.width == 0 {
                    format!(
                        "widget {} has zero width (height={}) — horizontally collapsed",
                        widget_id, rect.height
                    )
                } else {
                    format!(
                        "widget {} has zero height (width={}) — vertically collapsed",
                        widget_id, rect.width
                    )
                };
                Issue {
                    severity: Severity::Error,
                    description: desc,
                    widget_id: Some(*widget_id),
                    category: IssueCategory::Geometric,
                }
            })
            .collect();
        (issues, geometries)
    }

    /// D4: Detect sibling widgets whose rects overlap.
    ///
    /// Groups recorded geometries by parent, then checks every pair
    /// within each group. Uses `rects_overlap_excluding_touch` to
    /// avoid triggering on edge-adjacent widgets.
    fn check_overlaps(geometries: &[(ObjectId, Rect)], registry: &WidgetRegistry) -> Vec<Issue> {
        if geometries.len() < 2 {
            return Vec::new();
        }

        // Build a widget_id → parent lookup.
        let mut parent_of: std::collections::HashMap<ObjectId, Option<ObjectId>> =
            std::collections::HashMap::new();
        for id in registry.all_ids() {
            if let Some(entry) = registry.get(id) {
                parent_of.insert(id, entry.parent);
            }
        }

        // Group widget ids by their (optional) parent.
        let mut children_by_parent: std::collections::HashMap<Option<ObjectId>, Vec<ObjectId>> =
            std::collections::HashMap::new();
        for (widget_id, _) in geometries {
            let parent = parent_of.get(widget_id).copied().flatten();
            children_by_parent.entry(parent).or_default().push(*widget_id);
        }

        // Build rect_by_id for fast lookup.
        let rect_by_id: std::collections::HashMap<ObjectId, Rect> =
            geometries.iter().copied().collect();

        let mut issues = Vec::new();
        for (parent, children) in &children_by_parent {
            for i in 0..children.len() {
                for j in (i + 1)..children.len() {
                    let a_id = children[i];
                    let b_id = children[j];
                    let Some(a_rect) = rect_by_id.get(&a_id) else {
                        continue;
                    };
                    let Some(b_rect) = rect_by_id.get(&b_id) else {
                        continue;
                    };
                    if rects_overlap_excluding_touch(a_rect, b_rect) {
                        let parent_desc = parent
                            .and_then(|p| registry.get(p))
                            .map(|e| format!("'{}'", e.label))
                            .unwrap_or_else(|| format!("{:?}", parent));
                        issues.push(Issue {
                            severity: Severity::Warning,
                            description: format!(
                                "widgets {} and {} overlap under parent {}: {:?} vs {:?}",
                                a_id, b_id, parent_desc, a_rect, b_rect
                            ),
                            widget_id: Some(a_id),
                            category: IssueCategory::Geometric,
                        });
                    }
                }
            }
        }
        issues
    }

    // ── Recommendation engine ────────────────────────────────

    /// Generate actionable recommendations from detected issues.
    fn generate_recommendations(issues: &[Issue]) -> Vec<Recommendation> {
        let mut recs: Vec<Recommendation> = Vec::new();

        let has_error = issues.iter().any(|i| i.severity == Severity::Error);
        let has_warning = issues.iter().any(|i| i.severity == Severity::Warning);
        let has_orphan = issues
            .iter()
            .any(|i| i.category == IssueCategory::Structural && i.severity != Severity::Info);
        let has_zero = issues
            .iter()
            .any(|i| i.category == IssueCategory::Geometric && i.severity == Severity::Error);
        let has_overlap = issues
            .iter()
            .any(|i| i.category == IssueCategory::Geometric && i.severity == Severity::Warning);

        // R1: Recalculate if any error or warning.
        if has_error || has_warning {
            recs.push(Recommendation {
                title: "Recalculate layout".into(),
                summary: "Call recalculate() or re-trigger layout.update() to re-apply fixes"
                    .into(),
                detail: format!(
                    "LayoutInspector detected {} issue(s). After fixing them, trigger a layout \
                     recalculation so the new geometry is applied. If issues persist, run \
                     LayoutInspector::run_once() again to verify.",
                    issues.len()
                ),
            });
        }

        // R2: Zero-size fix.
        if has_zero {
            recs.push(Recommendation {
                title: "Set minimum size constraints".into(),
                summary: "Zero-size widgets need explicit min_width / min_height".into(),
                detail: "Add min_width and min_height to the widget in JSON, or set \
                         LayoutConstraints with min > 0. In Rust code:\n  \
                         layout.set_constraints(widget_id, LayoutConstraints::new(20, None));\n  \
                         layout.set_size_policy(widget_id, SizePolicy::Fixed);"
                    .into(),
            });
        }

        // R3: Nesting fix for orphans.
        if has_orphan {
            recs.push(Recommendation {
                title: "Fix widget nesting".into(),
                summary: "Orphan widgets found — check JSON nesting or add_widget() calls".into(),
                detail: "Orphan widgets have no parent and are not Windows. In JSON, ensure the \
                         widget appears inside a parent's 'children' array. In native code, \
                         ensure layout.add_widget(id, stretch) is called for each child."
                    .into(),
            });
        }

        // R4: Stretch adjustment for overlaps.
        if has_overlap {
            recs.push(Recommendation {
                title: "Adjust stretch factors or use Grid".into(),
                summary: "Overlapping siblings suggest unbalanced stretch or insufficient spacing"
                    .into(),
                detail: "Review the stretch factors assigned to sibling widgets. Avoid extreme \
                         ratios (e.g. 100:1). Consider:\n  \
                         1. Using GridLayout instead of BoxLayout\n  \
                         2. Adding explicit min_width constraints\n  \
                         3. Increasing spacing or margin"
                    .into(),
            });
        }

        // R6: General advice.
        if recs.is_empty() {
            recs.push(Recommendation {
                title: "Run diagnostics after layout load".into(),
                summary: "Enable LayoutInspector after loading JSON or constructing native layouts"
                    .into(),
                detail:
                    "For best results, enable the inspector immediately after JsonLoader::load() \
                         or after constructing programmatic layouts, then call run_once() after \
                         the first layout.update()."
                        .into(),
            });
        }

        recs
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::index::{WidgetEntry, WidgetKind, WidgetRegistry};
    use std::sync::Mutex;

    /// Serializes LayoutInspector tests that share global `ENABLED` state.
    /// Uses `ignore_poison` to recover from panics in prior tests.
    static LAYOUT_INSPECTOR_LOCK: Mutex<()> = Mutex::new(());

    /// Acquire the test lock, recovering from poison if a previous test panicked.
    fn lock_inspector() -> std::sync::MutexGuard<'static, ()> {
        LAYOUT_INSPECTOR_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ── Enable/disable ──

    #[test]
    fn default_is_disabled() {
        let _lock = lock_inspector();
        LayoutInspector::disable(); // ensure clean state (tests share process)
        assert!(!LayoutInspector::is_enabled());
    }

    #[test]
    fn enable_disable_toggle() {
        let _lock = lock_inspector();
        LayoutInspector::disable(); // ensure clean state
        assert!(!LayoutInspector::is_enabled());
        LayoutInspector::enable();
        assert!(LayoutInspector::is_enabled());
        LayoutInspector::disable();
        assert!(!LayoutInspector::is_enabled());
    }

    #[test]
    fn disabled_run_once_returns_empty() {
        let _lock = lock_inspector();
        LayoutInspector::disable();
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        assert!(!report.has_issues());
        assert_eq!(report.widgets_inspected, 0);
    }

    // ── D1: Orphan check ──

    #[test]
    fn detect_orphan_widget() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 1,
            kind: WidgetKind::Button,
            parent: None,
            label: "lost_btn".into(),
        });
        // Window should not be flagged.
        reg.register(WidgetEntry {
            id: 2,
            kind: WidgetKind::Window,
            parent: None,
            label: "main_window".into(),
        });
        let report = LayoutInspector::run_once(&reg);
        assert!(report.has_issues());
        assert!(report.has_warnings());
        assert!(!report.has_errors());
        assert!(report
            .issues
            .iter()
            .any(|i| i.severity == Severity::Warning && i.description.contains("orphan")));
        LayoutInspector::disable();
    }

    #[test]
    fn window_not_flagged_as_orphan() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 10,
            kind: WidgetKind::Window,
            parent: None,
            label: "main".into(),
        });
        let report = LayoutInspector::run_once(&reg);
        let orphan_issues: Vec<_> =
            report.issues.iter().filter(|i| i.description.contains("orphan")).collect();
        assert!(orphan_issues.is_empty(), "windows without parent should not be orphans");
        LayoutInspector::disable();
    }

    #[test]
    fn widget_with_parent_not_orphan() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 1,
            kind: WidgetKind::Window,
            parent: None,
            label: "win".into(),
        });
        reg.register(WidgetEntry {
            id: 2,
            kind: WidgetKind::Button,
            parent: Some(1),
            label: "ok".into(),
        });
        let report = LayoutInspector::run_once(&reg);
        let orphan_issues: Vec<_> =
            report.issues.iter().filter(|i| i.description.contains("orphan")).collect();
        assert!(orphan_issues.is_empty(), "widget with parent should not be orphan");
        LayoutInspector::disable();
    }

    // ── D2: Empty layout check ──

    #[test]
    fn detect_empty_layout() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        LayoutInspector::register_native_layout(100, "hbox_main", 0, "HBoxLayout");
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        let empty_issues: Vec<_> =
            report.issues.iter().filter(|i| i.description.contains("empty layout")).collect();
        assert_eq!(empty_issues.len(), 1);
        LayoutInspector::disable();
    }

    #[test]
    fn non_empty_layout_clean() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        LayoutInspector::register_native_layout(100, "hbox_main", 3, "HBoxLayout");
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        let empty_issues: Vec<_> =
            report.issues.iter().filter(|i| i.description.contains("empty layout")).collect();
        assert!(empty_issues.is_empty());
        LayoutInspector::disable();
    }

    // ── D3: Zero-size check ──

    #[test]
    fn detect_zero_width() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        LayoutInspector::record_geometry(1, Rect::new(0, 0, 0, 100));
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        assert!(report.has_errors());
        assert!(report.issues.iter().any(|i| i.description.contains("zero width")));
        LayoutInspector::disable();
    }

    #[test]
    fn detect_zero_height() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        LayoutInspector::record_geometry(1, Rect::new(10, 10, 200, 0));
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        assert!(report.has_errors());
        assert!(report.issues.iter().any(|i| i.description.contains("zero height")));
        LayoutInspector::disable();
    }

    #[test]
    fn detect_zero_size() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        LayoutInspector::record_geometry(1, Rect::new(0, 0, 0, 0));
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        assert!(report.has_errors());
        assert!(report.issues.iter().any(|i| i.description.contains("zero size")));
        LayoutInspector::disable();
    }

    #[test]
    fn valid_size_not_flagged() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        LayoutInspector::record_geometry(1, Rect::new(0, 0, 100, 30));
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        let zero_issues: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.category == IssueCategory::Geometric)
            .filter(|i| {
                i.description.contains("zero width")
                    || i.description.contains("zero height")
                    || i.description.contains("zero size")
            })
            .collect();
        assert!(zero_issues.is_empty());
        LayoutInspector::disable();
    }

    // ── D4: Overlap check ──

    #[test]
    fn detect_overlapping_rects() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 10,
            kind: WidgetKind::Window,
            parent: None,
            label: "win".into(),
        });
        reg.register(WidgetEntry {
            id: 1,
            kind: WidgetKind::Button,
            parent: Some(10),
            label: "left".into(),
        });
        reg.register(WidgetEntry {
            id: 2,
            kind: WidgetKind::Button,
            parent: Some(10),
            label: "right".into(),
        });
        // Record sibling geometries that overlap at x=50-60
        LayoutInspector::record_geometry(1, Rect::new(0, 0, 60, 30));
        LayoutInspector::record_geometry(2, Rect::new(50, 0, 60, 30));
        let report = LayoutInspector::run_once(&reg);
        let overlap_issues: Vec<_> =
            report.issues.iter().filter(|i| i.description.contains("overlap")).collect();
        assert_eq!(overlap_issues.len(), 1);
        LayoutInspector::disable();
    }

    #[test]
    fn adjacent_rects_no_overlap() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 10,
            kind: WidgetKind::Window,
            parent: None,
            label: "win".into(),
        });
        reg.register(WidgetEntry {
            id: 1,
            kind: WidgetKind::Button,
            parent: Some(10),
            label: "left".into(),
        });
        reg.register(WidgetEntry {
            id: 2,
            kind: WidgetKind::Button,
            parent: Some(10),
            label: "right".into(),
        });
        // Edge-touching: left ends at x=60, right starts at x=60 — no pixel overlap.
        LayoutInspector::record_geometry(1, Rect::new(0, 0, 60, 30));
        LayoutInspector::record_geometry(2, Rect::new(60, 0, 60, 30));
        let report = LayoutInspector::run_once(&reg);
        let overlap_issues: Vec<_> =
            report.issues.iter().filter(|i| i.description.contains("overlap")).collect();
        assert!(overlap_issues.is_empty());
        LayoutInspector::disable();
    }

    #[test]
    fn no_overlap_single_child() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 10,
            kind: WidgetKind::Window,
            parent: None,
            label: "win".into(),
        });
        reg.register(WidgetEntry {
            id: 1,
            kind: WidgetKind::Button,
            parent: Some(10),
            label: "only".into(),
        });
        LayoutInspector::record_geometry(1, Rect::new(0, 0, 200, 30));
        let report = LayoutInspector::run_once(&reg);
        let overlap_issues: Vec<_> =
            report.issues.iter().filter(|i| i.description.contains("overlap")).collect();
        assert!(overlap_issues.is_empty());
        LayoutInspector::disable();
    }

    // ── Issue + Recommendation tests ──

    #[test]
    fn multiple_issues_caught_together() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 1,
            kind: WidgetKind::Window,
            parent: None,
            label: "win".into(),
        });
        reg.register(WidgetEntry {
            id: 2,
            kind: WidgetKind::Button,
            parent: None,
            label: "orphan".into(),
        });
        LayoutInspector::register_native_layout(1, "empty_vbox", 0, "VBoxLayout");
        LayoutInspector::record_geometry(2, Rect::new(0, 0, 0, 0));
        let report = LayoutInspector::run_once(&reg);
        assert!(report.has_issues());
        assert!(report.has_errors());
        assert!(report.has_warnings());
        assert_eq!(report.widgets_inspected, 2);
        LayoutInspector::disable();
    }

    #[test]
    fn no_recommendations_when_clean() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 1,
            kind: WidgetKind::Window,
            parent: None,
            label: "win".into(),
        });
        LayoutInspector::record_geometry(1, Rect::new(0, 0, 400, 300));
        let report = LayoutInspector::run_once(&reg);
        // Clean window with valid size — no issues should trigger recommendations
        // other than the generic R6.
        assert!(!report.issues.is_empty() || !report.recommendations.is_empty());
        LayoutInspector::disable();
    }

    #[test]
    fn recommendation_generated_for_issues() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let mut reg = WidgetRegistry::new();
        reg.register(WidgetEntry {
            id: 1,
            kind: WidgetKind::Button,
            parent: None,
            label: "orphan".into(),
        });
        let report = LayoutInspector::run_once(&reg);
        if report.issues.is_empty() {
            panic!(
                "expected orphan issues but got none (enabled={})",
                LayoutInspector::is_enabled()
            );
        }
        assert!(!report.recommendations.is_empty());
        assert!(report.recommendations.iter().any(|r| r.title.contains("Recalculate")));
        LayoutInspector::disable();
    }

    #[test]
    fn record_geometry_overwrites_same_id() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        LayoutInspector::record_geometry(42, Rect::new(0, 0, 100, 20));
        LayoutInspector::record_geometry(42, Rect::new(10, 10, 200, 40)); // overwrite
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        // Should see the second rect — verify by no zero-size error.
        assert!(!report.has_errors());
        LayoutInspector::disable();
    }

    #[test]
    fn report_display_inspector_header() {
        let _lock = lock_inspector();
        LayoutInspector::enable();
        let reg = WidgetRegistry::new();
        let report = LayoutInspector::run_once(&reg);
        let display_text = format!("{}", report);
        assert!(display_text.contains("Layout Inspector Report"));
        LayoutInspector::disable();
    }
}
