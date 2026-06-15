//! Platform accessibility bridges (macOS NSAccessibility, Windows UIAutomation, Linux AT-SPI).
//!
//! This module provides the foundation for OS-level accessibility integration.
//! Each platform backend can implement the `AccessibilityBridge` trait to expose
//! widget information to screen readers and other assistive technologies.
//!
//! Higher-level abstractions (`A11yProvider`, `A11yTree`) provide a unified
//! cross-platform accessibility node tree for screen reader navigation.

#[cfg(all(target_os = "macos", feature = "macos-legacy"))]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

use crate::compat::HashMap;
use crate::core::ObjectId;
use crate::widget::WidgetKind;

// ─── Cross‑platform A11y role enumeration ───────────────────────────────

/// Comprehensive accessibility role enumeration covering all standard UI
/// element types. Maps to platform-specific roles (NSAccessibilityRole on
/// macOS, UIA control types on Windows, AT-SPI roles on Linux).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum A11yRole {
    #[default]
    Unknown,
    /// Push button.
    Button,
    /// Static text label.
    Label,
    /// Editable text field.
    TextField,
    /// Check box (binary or tri-state).
    CheckBox,
    /// Radio button.
    RadioButton,
    /// Range slider.
    Slider,
    /// Progress indicator (determinate or indeterminate).
    ProgressBar,
    /// List control.
    List,
    /// Table (grid of rows and columns).
    Table,
    /// Image / graphic.
    Image,
    /// Hyperlink.
    Link,
    /// Heading (h1–h6).
    Heading,
    /// Paragraph / block of text.
    Paragraph,
    /// Generic grouping container.
    Group,
    /// Top-level window.
    Window,
    /// Dialog / modal overlay.
    Dialog,
    /// Menu bar.
    Menu,
    /// Menu item.
    MenuItem,
    /// Tab control.
    Tab,
    /// On/off switch.
    Switch,
    /// Alert / notification.
    Alert,
    /// Combo box (dropdown selection).
    ComboBox,
    /// Spin button (numeric stepper).
    SpinButton,
    /// Status bar.
    StatusBar,
    /// Tooltip.
    ToolTip,
    /// Tree control.
    Tree,
}

// ─── Accessibility state ────────────────────────────────────────────────

/// Full accessibility state for a single node in the accessibility tree.
#[derive(Debug, Clone, Default)]
pub struct A11yState {
    /// Semantic role of this node.
    pub role: A11yRole,
    /// Accessible name (label) for screen readers.
    pub label: String,
    /// Longer description / help text.
    pub description: String,
    /// Whether the element is enabled for interaction.
    pub enabled: bool,
    /// Whether the element currently has keyboard focus.
    pub focused: bool,
    /// Whether the element is selected (list items, table rows, etc.).
    pub selected: bool,
    /// Whether the element is expanded (tree nodes, disclosure triangles).
    pub expanded: bool,
    /// Current value (slider position, progress percent, text content).
    pub value: String,
    /// Child node IDs in traversal order.
    pub children: Vec<ObjectId>,
}

/// A single node in the accessibility tree, combining an [`A11yState`] with
/// its stable [`ObjectId`].
#[derive(Debug, Clone)]
pub struct A11yNode {
    /// Stable object identifier.
    pub id: ObjectId,
    /// Accessibility state payload.
    pub state: A11yState,
}

impl A11yNode {
    /// Construct a new accessibility node.
    pub fn new(id: ObjectId, state: A11yState) -> Self {
        Self { id, state }
    }
}

// ─── Accessibility tree ─────────────────────────────────────────────────

/// Manages a tree of [`A11yNode`]s for screen reader and assistive technology
/// navigation. Provides insertion, removal, update, query, and focus-tracking
/// operations.
#[derive(Debug, Clone)]
pub struct A11yTree {
    nodes: HashMap<ObjectId, A11yNode>,
    root_id: Option<ObjectId>,
    focus_order: Vec<ObjectId>,
    focus_index: usize,
}

impl Default for A11yTree {
    fn default() -> Self {
        Self::new()
    }
}

impl A11yTree {
    /// Create an empty accessibility tree.
    pub fn new() -> Self {
        Self { nodes: HashMap::new(), root_id: None, focus_order: Vec::new(), focus_index: 0 }
    }

    /// Register a new node in the tree. If the node's `id` already exists,
    /// it is overwritten. If this is the first node added, it becomes the root.
    pub fn register_node(&mut self, node: A11yNode) {
        let is_first = self.nodes.is_empty();
        self.nodes.insert(node.id, node.clone());
        if is_first {
            self.root_id = Some(node.id);
        }
        // Rebuild focus order whenever the tree changes.
        self.rebuild_focus_order();
    }

    /// Remove a node and all its descendants from the tree.
    pub fn unregister_node(&mut self, id: ObjectId) -> bool {
        if self.nodes.contains_key(&id) {
            // Collect descendant IDs.
            let descendants = self.collect_descendants(id);
            for did in &descendants {
                self.nodes.remove(did);
            }
            self.nodes.remove(&id);
            if self.root_id == Some(id) {
                self.root_id = self.nodes.keys().next().copied();
            }
            self.rebuild_focus_order();
            true
        } else {
            false
        }
    }

    /// Update the state of an existing node. Returns `true` if the node was found.
    pub fn update_node(&mut self, id: ObjectId, state: A11yState) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.state = state;
            self.rebuild_focus_order();
            true
        } else {
            false
        }
    }

    /// Find all nodes matching a given role.
    pub fn find_by_role(&self, role: A11yRole) -> Vec<&A11yNode> {
        self.nodes.values().filter(|n| n.state.role == role).collect()
    }

    /// Query nodes by a predicate on their state.
    pub fn query(&self, predicate: impl Fn(&A11yState) -> bool) -> Vec<&A11yNode> {
        self.nodes.values().filter(|n| predicate(&n.state)).collect()
    }

    /// Move focus to the next focusable node in traversal order.
    /// Returns the focused node's ID, or `None` if the tree is empty.
    pub fn focus_next(&mut self) -> Option<ObjectId> {
        if self.focus_order.is_empty() {
            return None;
        }
        self.focus_index = (self.focus_index + 1) % self.focus_order.len();
        let id = self.focus_order[self.focus_index];
        // Update the focused flag on the node.
        self.set_focused(id);
        Some(id)
    }

    /// Move focus to the previous focusable node in traversal order.
    /// Returns the focused node's ID, or `None` if the tree is empty.
    pub fn focus_previous(&mut self) -> Option<ObjectId> {
        if self.focus_order.is_empty() {
            return None;
        }
        if self.focus_index == 0 {
            self.focus_index = self.focus_order.len() - 1;
        } else {
            self.focus_index -= 1;
        }
        let id = self.focus_order[self.focus_index];
        self.set_focused(id);
        Some(id)
    }

    /// Get a reference to a node by its ID.
    pub fn get(&self, id: ObjectId) -> Option<&A11yNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable reference to a node by its ID.
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut A11yNode> {
        self.nodes.get_mut(&id)
    }

    /// Return the current root node ID, if any.
    pub fn root_id(&self) -> Option<ObjectId> {
        self.root_id
    }

    /// Return the number of nodes in the tree.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Iterate over all registered nodes.
    pub fn iter(&self) -> impl Iterator<Item = &A11yNode> {
        self.nodes.values()
    }

    // ── Internal helpers ──

    /// Collect all descendant IDs for a given parent (recursive).
    fn collect_descendants(&self, parent_id: ObjectId) -> Vec<ObjectId> {
        let mut result = Vec::new();
        if let Some(parent) = self.nodes.get(&parent_id) {
            for child_id in &parent.state.children {
                result.push(*child_id);
                result.extend(self.collect_descendants(*child_id));
            }
        }
        result
    }

    /// Rebuild the flat focus-traversal order from the tree hierarchy (DFS).
    fn rebuild_focus_order(&mut self) {
        self.focus_order.clear();
        if let Some(root_id) = self.root_id {
            let mut order = Vec::new();
            self.dfs_collect(root_id, &mut order);
            self.focus_order = order;
        }
        // Clamp the focus index.
        if !self.focus_order.is_empty() {
            self.focus_index = self.focus_index.min(self.focus_order.len() - 1);
        } else {
            self.focus_index = 0;
        }
    }

    /// Depth-first traversal collecting node IDs.
    fn dfs_collect(&self, id: ObjectId, out: &mut Vec<ObjectId>) {
        out.push(id);
        if let Some(node) = self.nodes.get(&id) {
            for child_id in &node.state.children {
                self.dfs_collect(*child_id, out);
            }
        }
    }

    /// Set the focused flag on one node and clear it on all others.
    fn set_focused(&mut self, target_id: ObjectId) {
        for node in self.nodes.values_mut() {
            node.state.focused = node.id == target_id;
        }
    }
}

// ─── Cross‑platform A11y provider trait ─────────────────────────────────

/// High-level provider for screen reader integration.
///
/// Implementations manage an [`A11yTree`], announce events to the platform's
/// native accessibility API, and coordinate with the [`AccessibilityBridge`]
/// for widget-level property sync.
pub trait A11yProvider: Send + Sync {
    /// Register a widget as an accessibility node. Returns the node's ID.
    fn register_widget(&mut self, id: ObjectId, state: A11yState);
    /// Remove a widget from the accessibility tree.
    fn unregister_widget(&mut self, id: ObjectId) -> bool;
    /// Update the state for an already-registered widget.
    fn update_widget_state(&mut self, id: ObjectId, state: A11yState) -> bool;
    /// Announce a message to the screen reader (e.g., "login successful").
    fn announce(&self, message: &str);
    /// Returns the current focus order as a slice of node IDs.
    fn focus_order(&self) -> Vec<ObjectId>;
    /// Move focus to the next element. Returns the newly focused node ID.
    fn focus_next(&mut self) -> Option<ObjectId>;
    /// Move focus to the previous element. Returns the newly focused node ID.
    fn focus_previous(&mut self) -> Option<ObjectId>;
    /// Return a reference to the internal accessibility tree.
    fn tree(&self) -> &A11yTree;
    /// Return a mutable reference to the internal accessibility tree.
    fn tree_mut(&mut self) -> &mut A11yTree;
}

// ─── Legacy role type (kept for backward compatibility) ──────────────────

/// Accessibility role types corresponding to platform-specific roles.
///
/// This is the older role enum kept for backward compatibility. New code
/// should prefer [`A11yRole`] for a more comprehensive set of roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibleRole {
    Button,
    CheckBox,
    ComboBox,
    Dialog,
    Group,
    Image,
    Label,
    Link,
    List,
    ListItem,
    Menu,
    MenuBar,
    MenuItem,
    ProgressBar,
    RadioButton,
    ScrollBar,
    Slider,
    SpinButton,
    Splitter,
    StaticText,
    Tab,
    TabGroup,
    Table,
    TextField,
    ToolBar,
    Tree,
    TreeItem,
    Window,
    Unknown,
}

impl From<A11yRole> for AccessibleRole {
    fn from(role: A11yRole) -> Self {
        match role {
            A11yRole::Button => AccessibleRole::Button,
            A11yRole::Label | A11yRole::Heading | A11yRole::Paragraph => AccessibleRole::StaticText,
            A11yRole::TextField => AccessibleRole::TextField,
            A11yRole::CheckBox => AccessibleRole::CheckBox,
            A11yRole::RadioButton => AccessibleRole::RadioButton,
            A11yRole::Slider => AccessibleRole::Slider,
            A11yRole::ProgressBar => AccessibleRole::ProgressBar,
            A11yRole::List => AccessibleRole::List,
            A11yRole::Table => AccessibleRole::Table,
            A11yRole::Image => AccessibleRole::Image,
            A11yRole::Link => AccessibleRole::Link,
            A11yRole::Group => AccessibleRole::Group,
            A11yRole::Window => AccessibleRole::Window,
            A11yRole::Dialog | A11yRole::Alert => AccessibleRole::Dialog,
            A11yRole::Menu => AccessibleRole::Menu,
            A11yRole::MenuItem => AccessibleRole::MenuItem,
            A11yRole::Tab => AccessibleRole::Tab,
            A11yRole::Switch => AccessibleRole::Button,
            A11yRole::ComboBox => AccessibleRole::ComboBox,
            A11yRole::SpinButton => AccessibleRole::SpinButton,
            A11yRole::StatusBar => AccessibleRole::Group,
            A11yRole::ToolTip => AccessibleRole::Group,
            A11yRole::Tree => AccessibleRole::Tree,
            A11yRole::Unknown => AccessibleRole::Unknown,
        }
    }
}

// ─── ARIA properties ────────────────────────────────────────────────────

/// ARIA property mapping for accessibility (BLUE11 R7.5).
#[derive(Debug, Clone, Default)]
pub struct AriaProperties {
    /// aria-label — overrides the accessible name.
    pub label: Option<String>,
    /// aria-describedby reference.
    pub described_by: Option<String>,
    /// aria-live region (polite, assertive, off).
    pub live_region: Option<String>,
    /// aria-atomic for live regions.
    pub atomic: bool,
    /// aria-busy state.
    pub busy: bool,
    /// Custom key-value ARIA attributes.
    pub custom: Vec<(String, String)>,
}

// ─── WidgetKind → AccessibleRole mapping ────────────────────────────────

#[cfg(not(feature = "mini"))]
impl From<WidgetKind> for AccessibleRole {
    fn from(kind: WidgetKind) -> Self {
        match kind {
            WidgetKind::Button | WidgetKind::ToggleButton | WidgetKind::ToolButton => {
                AccessibleRole::Button
            }
            WidgetKind::CheckBox | WidgetKind::CheckListBox => AccessibleRole::CheckBox,
            WidgetKind::ComboBox | WidgetKind::FontComboBox => AccessibleRole::ComboBox,
            WidgetKind::Dialog
            | WidgetKind::FileDialog
            | WidgetKind::ColorDialog
            | WidgetKind::FontDialog
            | WidgetKind::InputDialog
            | WidgetKind::ProgressDialog
            | WidgetKind::DirectoryDialog => AccessibleRole::Dialog,
            WidgetKind::Label => AccessibleRole::StaticText,
            WidgetKind::LineEdit | WidgetKind::TextEdit | WidgetKind::RichEdit => {
                AccessibleRole::TextField
            }
            WidgetKind::ListBox | WidgetKind::ListView => AccessibleRole::List,
            WidgetKind::MenuBar => AccessibleRole::MenuBar,
            WidgetKind::Menu | WidgetKind::ContextMenu => AccessibleRole::Menu,
            WidgetKind::MenuItem => AccessibleRole::MenuItem,
            WidgetKind::ProgressBar | WidgetKind::ActivityIndicator => AccessibleRole::ProgressBar,
            WidgetKind::RadioButton => AccessibleRole::RadioButton,
            WidgetKind::ScrollBar => AccessibleRole::ScrollBar,
            WidgetKind::Slider | WidgetKind::Dial => AccessibleRole::Slider,
            WidgetKind::SpinBox | WidgetKind::DoubleSpinBox => AccessibleRole::SpinButton,
            WidgetKind::TabWidget | WidgetKind::TabBar => AccessibleRole::TabGroup,
            WidgetKind::Table | WidgetKind::DataView => AccessibleRole::Table,
            WidgetKind::ToolBar => AccessibleRole::ToolBar,
            WidgetKind::TreeView => AccessibleRole::Tree,
            WidgetKind::Splitter => AccessibleRole::Splitter,
            WidgetKind::Window => AccessibleRole::Window,
            // BLUE11 new widgets
            WidgetKind::Switch => AccessibleRole::Button,
            WidgetKind::SearchBox => AccessibleRole::TextField,
            WidgetKind::Chip => AccessibleRole::Button,
            WidgetKind::Badge => AccessibleRole::Label,
            WidgetKind::SkeletonLoader => AccessibleRole::Label,
            WidgetKind::FAB => AccessibleRole::Button,
            WidgetKind::RefreshControl => AccessibleRole::Group,
            WidgetKind::BottomSheet => AccessibleRole::Group,
            WidgetKind::BottomNavigationBar => AccessibleRole::TabGroup,
            WidgetKind::NavigationDrawer => AccessibleRole::Group,
            WidgetKind::AppBar => AccessibleRole::Group,
            WidgetKind::MobileDatePicker => AccessibleRole::SpinButton,
            WidgetKind::Divider => AccessibleRole::Group,
            WidgetKind::Stepper => AccessibleRole::SpinButton,
            WidgetKind::Rating => AccessibleRole::Slider,
            WidgetKind::Avatar => AccessibleRole::Image,
            WidgetKind::EmptyState => AccessibleRole::Group,
            WidgetKind::Carousel => AccessibleRole::TabGroup,
            _ => AccessibleRole::Unknown,
        }
    }
}

#[cfg(feature = "mini")]
impl From<WidgetKind> for AccessibleRole {
    fn from(_kind: WidgetKind) -> Self {
        AccessibleRole::Unknown
    }
}

/// Map [`WidgetKind`] to the newer [`A11yRole`] enum.
#[cfg(not(feature = "mini"))]
impl From<WidgetKind> for A11yRole {
    fn from(kind: WidgetKind) -> Self {
        match kind {
            WidgetKind::Button
            | WidgetKind::ToggleButton
            | WidgetKind::ToolButton
            | WidgetKind::MenuButton
            | WidgetKind::FAB => A11yRole::Button,
            WidgetKind::CheckBox | WidgetKind::CheckListBox => A11yRole::CheckBox,
            WidgetKind::ComboBox
            | WidgetKind::FontComboBox
            | WidgetKind::EditableComboBox
            | WidgetKind::MultiSelectComboBox => A11yRole::ComboBox,
            WidgetKind::Dialog
            | WidgetKind::FileDialog
            | WidgetKind::ColorDialog
            | WidgetKind::FontDialog
            | WidgetKind::InputDialog
            | WidgetKind::ProgressDialog
            | WidgetKind::DirectoryDialog
            | WidgetKind::FindReplaceDialog
            | WidgetKind::CupertinoAlertDialog
            | WidgetKind::ModalBottomSheet => A11yRole::Dialog,
            WidgetKind::Label => A11yRole::Label,
            WidgetKind::LineEdit
            | WidgetKind::TextEdit
            | WidgetKind::RichEdit
            | WidgetKind::SearchBox
            | WidgetKind::SearchBar
            | WidgetKind::MaskedEdit
            | WidgetKind::AutoCompleteEdit
            | WidgetKind::FloatingLabel => A11yRole::TextField,
            WidgetKind::ListBox | WidgetKind::ListView => A11yRole::List,
            WidgetKind::MenuBar | WidgetKind::Menu => A11yRole::Menu,
            WidgetKind::MenuItem => A11yRole::MenuItem,
            WidgetKind::ProgressBar
            | WidgetKind::ActivityIndicator
            | WidgetKind::ProgressCircle => A11yRole::ProgressBar,
            WidgetKind::RadioButton => A11yRole::RadioButton,
            WidgetKind::Slider
            | WidgetKind::Dial
            | WidgetKind::CupertinoSlider
            | WidgetKind::RangeSlider => A11yRole::Slider,
            WidgetKind::SpinBox | WidgetKind::DoubleSpinBox | WidgetKind::Stepper => {
                A11yRole::SpinButton
            }
            WidgetKind::TabWidget | WidgetKind::TabBar | WidgetKind::TabView => A11yRole::Tab,
            WidgetKind::Table | WidgetKind::DataView => A11yRole::Table,
            WidgetKind::TreeView => A11yRole::Tree,
            WidgetKind::Window => A11yRole::Window,
            WidgetKind::Switch | WidgetKind::CupertinoSwitch => A11yRole::Switch,
            WidgetKind::Avatar | WidgetKind::Icon | WidgetKind::QRCode | WidgetKind::ColorWell => {
                A11yRole::Image
            }

            WidgetKind::Tooltip => A11yRole::ToolTip,
            WidgetKind::StatusBar => A11yRole::StatusBar,
            WidgetKind::Splitter
            | WidgetKind::Panel
            | WidgetKind::GroupBox
            | WidgetKind::NavigationStack
            | WidgetKind::Popover
            | WidgetKind::Chip
            | WidgetKind::Badge
            | WidgetKind::SkeletonLoader
            | WidgetKind::RefreshControl
            | WidgetKind::BottomSheet
            | WidgetKind::NavigationDrawer
            | WidgetKind::AppBar
            | WidgetKind::Divider
            | WidgetKind::EmptyState
            | WidgetKind::SafeArea
            | WidgetKind::SwipeToDismiss
            | WidgetKind::PropertiesPanel
            | WidgetKind::DropdownMenu
            | WidgetKind::SegmentedButton
            | WidgetKind::DockPanel
            | WidgetKind::MdiArea
            | WidgetKind::ScrollArea => A11yRole::Group,
            WidgetKind::DatePicker
            | WidgetKind::TimePicker
            | WidgetKind::DateTimePicker
            | WidgetKind::CupertinoDatePicker
            | WidgetKind::DateRangePicker
            | WidgetKind::MobileDatePicker => A11yRole::SpinButton,
            WidgetKind::LineChart
            | WidgetKind::BarChart
            | WidgetKind::PieChart
            | WidgetKind::Sparkline => A11yRole::Image,
            WidgetKind::Rating => A11yRole::Slider,
            WidgetKind::Carousel | WidgetKind::PagerPageView => A11yRole::Tab,
            _ => A11yRole::Unknown,
        }
    }
}

#[cfg(feature = "mini")]
impl From<WidgetKind> for A11yRole {
    fn from(_kind: WidgetKind) -> Self {
        A11yRole::Unknown
    }
}

// ─── Platform accessibility bridge trait ────────────────────────────────

/// Trait for platform-specific accessibility integration.
pub trait AccessibilityBridge: Send + Sync {
    /// Set the accessible name (label) for a widget.
    fn set_accessibility_name(&self, id: ObjectId, name: &str);
    /// Get the accessible name for a widget.
    fn accessibility_name(&self, id: ObjectId) -> Option<String>;
    /// Post a notification that a widget's accessible name changed.
    fn notify_name_changed(&self, id: ObjectId);
    /// Post a notification that a widget's value changed.
    fn notify_value_changed(&self, id: ObjectId);
    /// Post a notification that a widget's state changed (e.g., enabled/disabled).
    fn notify_state_changed(&self, id: ObjectId);
    /// Post a notification that focus moved to a widget.
    fn notify_focus_changed(&self, id: ObjectId);
    /// Set ARIA properties on a widget.
    /// The default implementation is a no-op. Platform-specific
    /// accessibility bridges should override this to expose ARIA
    /// properties to assistive technologies.
    fn set_aria_properties(&self, _id: ObjectId, _props: &AriaProperties) {}
}

// ─── Default A11yProvider implementation ────────────────────────────────

/// Default in-memory [`A11yProvider`] that maintains an [`A11yTree`] and logs
/// announcements. Platform backends can extend or wrap this implementation.
#[derive(Debug)]
pub struct DefaultA11yProvider {
    tree: A11yTree,
}

impl DefaultA11yProvider {
    /// Create a new default accessibility provider with an empty tree.
    pub fn new() -> Self {
        Self { tree: A11yTree::new() }
    }
}

impl Default for DefaultA11yProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl A11yProvider for DefaultA11yProvider {
    fn register_widget(&mut self, id: ObjectId, state: A11yState) {
        self.tree.register_node(A11yNode::new(id, state));
    }

    fn unregister_widget(&mut self, id: ObjectId) -> bool {
        self.tree.unregister_node(id)
    }

    fn update_widget_state(&mut self, id: ObjectId, state: A11yState) -> bool {
        self.tree.update_node(id, state)
    }

    fn announce(&self, message: &str) {
        log::info!("[A11y] Announce: {message}");
    }

    fn focus_order(&self) -> Vec<ObjectId> {
        self.tree.focus_order.clone()
    }

    fn focus_next(&mut self) -> Option<ObjectId> {
        self.tree.focus_next()
    }

    fn focus_previous(&mut self) -> Option<ObjectId> {
        self.tree.focus_previous()
    }

    fn tree(&self) -> &A11yTree {
        &self.tree
    }

    fn tree_mut(&mut self) -> &mut A11yTree {
        &mut self.tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get_node() {
        let mut tree = A11yTree::new();
        let id = 1u64;
        let state = A11yState {
            role: A11yRole::Button,
            label: "Submit".into(),
            enabled: true,
            ..Default::default()
        };
        tree.register_node(A11yNode::new(id, state.clone()));

        let node = tree.get(id).expect("node should exist");
        assert_eq!(node.state.role, A11yRole::Button);
        assert_eq!(node.state.label, "Submit");
        assert!(node.state.enabled);
    }

    #[test]
    fn test_unregister_node() {
        let mut tree = A11yTree::new();
        tree.register_node(A11yNode::new(1, A11yState::default()));
        assert!(tree.unregister_node(1));
        assert!(tree.get(1).is_none());
    }

    #[test]
    fn test_unregister_nonexistent_node() {
        let mut tree = A11yTree::new();
        assert!(!tree.unregister_node(42));
    }

    #[test]
    fn test_update_node() {
        let mut tree = A11yTree::new();
        let id = 1u64;
        tree.register_node(A11yNode::new(id, A11yState::default()));

        let updated = A11yState {
            label: "Updated".into(),
            role: A11yRole::TextField,
            enabled: false,
            ..Default::default()
        };
        assert!(tree.update_node(id, updated.clone()));
        let node = tree.get(id).expect("node should exist");
        assert_eq!(node.state.label, "Updated");
        assert!(!node.state.enabled);
    }

    #[test]
    fn test_find_by_role() {
        let mut tree = A11yTree::new();
        tree.register_node(A11yNode::new(
            1,
            A11yState { role: A11yRole::Button, ..Default::default() },
        ));
        tree.register_node(A11yNode::new(
            2,
            A11yState { role: A11yRole::Label, ..Default::default() },
        ));
        tree.register_node(A11yNode::new(
            3,
            A11yState { role: A11yRole::Button, ..Default::default() },
        ));

        let buttons = tree.find_by_role(A11yRole::Button);
        assert_eq!(buttons.len(), 2);
        let ids: std::collections::HashSet<ObjectId> = buttons.iter().map(|n| n.id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&3));
    }

    #[test]
    fn test_query() {
        let mut tree = A11yTree::new();
        tree.register_node(A11yNode::new(
            1,
            A11yState { label: "Alpha".into(), role: A11yRole::Button, ..Default::default() },
        ));
        tree.register_node(A11yNode::new(
            2,
            A11yState { label: "Beta".into(), role: A11yRole::Label, ..Default::default() },
        ));

        let enabled = tree.query(|s| s.label.starts_with('A'));
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, 1);
    }

    #[test]
    fn test_focus_next_and_previous() {
        let mut tree = A11yTree::new();
        // Register a proper tree with root 10 containing children 20, 30
        tree.register_node(A11yNode::new(
            10,
            A11yState { role: A11yRole::Group, children: vec![20, 30], ..Default::default() },
        ));
        tree.register_node(A11yNode::new(
            20,
            A11yState { role: A11yRole::Button, ..Default::default() },
        ));
        tree.register_node(A11yNode::new(
            30,
            A11yState { role: A11yRole::CheckBox, ..Default::default() },
        ));

        // focus_order should be [10, 20, 30] via DFS
        // Initial focus_index = 0, so focus_next -> index 1 -> 20
        assert_eq!(tree.focus_next(), Some(20));
        // Next -> index 2 -> 30
        assert_eq!(tree.focus_next(), Some(30));
        // Next wraps -> index 0 -> 10
        assert_eq!(tree.focus_next(), Some(10));

        // Previous wraps -> index 2 -> 30
        assert_eq!(tree.focus_previous(), Some(30));
        // Previous -> index 1 -> 20
        assert_eq!(tree.focus_previous(), Some(20));
    }

    #[test]
    fn test_focus_on_empty_tree() {
        let mut tree = A11yTree::new();
        assert!(tree.focus_next().is_none());
        assert!(tree.focus_previous().is_none());
    }

    #[test]
    fn test_focused_flag_set_correctly() {
        let mut tree = A11yTree::new();
        // Build a proper tree: root 1 with child 2
        tree.register_node(A11yNode::new(
            1,
            A11yState { role: A11yRole::Group, children: vec![2], ..Default::default() },
        ));
        tree.register_node(A11yNode::new(
            2,
            A11yState { role: A11yRole::Button, ..Default::default() },
        ));

        // focus_order = [1, 2]; focus_next moves to index 1 = 2
        tree.focus_next();
        assert!(tree.get(2).unwrap().state.focused);
        assert!(!tree.get(1).unwrap().state.focused);
    }

    #[test]
    fn test_role_conversion_widgetkind_to_a11yrole() {
        use A11yRole as R;
        use WidgetKind as K;

        assert_eq!(A11yRole::from(K::Button), R::Button);
        assert_eq!(A11yRole::from(K::CheckBox), R::CheckBox);
        assert_eq!(A11yRole::from(K::Label), R::Label);
        assert_eq!(A11yRole::from(K::LineEdit), R::TextField);
        assert_eq!(A11yRole::from(K::Slider), R::Slider);
        assert_eq!(A11yRole::from(K::Window), R::Window);
    }

    #[test]
    fn test_default_provider() {
        let mut provider = DefaultA11yProvider::new();
        let id = 42u64;
        provider.register_widget(
            id,
            A11yState {
                role: A11yRole::Button,
                label: "OK".into(),
                enabled: true,
                ..Default::default()
            },
        );

        assert_eq!(provider.tree().len(), 1);
        assert_eq!(provider.focus_next(), Some(id));
    }

    #[test]
    fn test_provider_announce() {
        let provider = DefaultA11yProvider::new();
        // Should not panic
        provider.announce("Hello, screen reader!");
    }
}
