//! New widget types — popular modern UI controls and mobile-first widgets.
//!
//! This module contains widgets added as part of the BLUE11 improvement plan (R10).
//! These include popular UI patterns (Switch, SearchBox, Chip, Badge, etc.) and
//! mobile-first widgets (BottomSheet, NavigationDrawer, AppBar, etc.).
pub mod adaptive_scaffold;
pub mod app_bar;
pub mod avatar;
pub mod badge;
pub mod bottom_navigation_bar;
pub mod bottom_sheet;
pub mod carousel;
pub mod chip;
pub mod color_well;
pub mod cupertino;
pub mod divider;
pub mod empty_state;
pub mod fab;
pub mod ime_preedit;
pub mod masonry_layout;
pub mod mobile_date_picker;
pub mod navigation_drawer;
pub mod property_grid;
pub mod pull_to_refresh;
pub mod qr_code;
pub mod rating;
pub mod safe_area;
pub mod search_box;
pub mod skeleton_loader;
pub mod stepper;
pub mod switch;
pub mod tag_input;
pub mod wizard;
// Re-exports
pub use adaptive_scaffold::AdaptiveScaffold;
pub use app_bar::AppBar;
pub use avatar::Avatar;
pub use badge::Badge;
pub use bottom_navigation_bar::BottomNavigationBar;
pub use bottom_navigation_bar::NavItem;
pub use bottom_sheet::BottomSheet;
pub use carousel::Carousel;
pub use carousel::CarouselPage;
pub use chip::Chip;
pub use color_well::ColorWell;
pub use cupertino::CupertinoAlertDialog;
pub use cupertino::CupertinoSlider;
pub use cupertino::CupertinoSwitch;
pub use cupertino::MaterialNavigationRail;
pub use cupertino::MaterialSnackbar;
pub use cupertino::RailItem;
pub use divider::Divider;
pub use empty_state::EmptyState;
pub use fab::FAB;
pub use ime_preedit::ImePreedit;
pub use masonry_layout::{MasonryItem, MasonryLayout};
pub use mobile_date_picker::MobileDatePicker;
pub use navigation_drawer::NavigationDrawer;
pub use property_grid::{PropertyGrid, PropertyItem};
pub use pull_to_refresh::PullToRefresh;
pub use qr_code::QRCode;
pub use rating::Rating;
pub use safe_area::{SafeArea, SafeAreaInsets};
pub use search_box::SearchBox;
pub use skeleton_loader::SkeletonLoader;
pub use stepper::Stepper;
pub use switch::Switch;
pub use tag_input::TagInput;
pub use wizard::{WizardDialog, WizardStep};
