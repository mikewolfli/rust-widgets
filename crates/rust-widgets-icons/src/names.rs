// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by `tools/gen_icon_names.py` from `tools/icon_tokens.txt` (curated) and
// `tools/icon_catalog.txt` (generated). Regenerate with:
//     python3 tools/gen_icon_names.py
//
// The `IconName` type: the enum, its token spellings, and the accessors the rest of the
// crate uses. The two declaration files are the single source of truth; the vendor map
// (`tools/vendor_material_symbols.py`) is cross-checked against them, so a token that
// cannot be drawn or a vendored icon nothing can name is a generation failure.
//
// The geometry itself lives in `data.rs` (opt-in `data` feature) and
// `fallback.rs` (always), both indexed by this enum's declaration order.

// `IconData` is defined once, in this crate's `lib.rs`, so a build without the `data`
// feature can still name the type; this module only borrows it for the two accessors
// that return it.
use crate::IconData;

/// Common icon names for use with the Icon widget.
///
/// This crate ships 4323 tokens: the 75 hand-curated spellings a control in this
/// crate names directly, and the 4248 names of the Material Symbols catalogue behind
/// them. With the opt-in `icons` feature on every variant resolves to a real Material
/// Symbols outline; with the feature off it resolves to generated fallback geometry. A
/// variant round-trips through [`IconName::as_str`] and [`IconName::from_name`], and the
/// integrity tests assert that two variants share a picture only where upstream itself
/// publishes one outline under two names ([`UPSTREAM_ALIASES`])).
///
/// Variants whose token starts with a digit take a `Num` prefix (`10k` -> `Num10k`),
/// because a Rust identifier cannot start with a digit; the token is unchanged.
#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    /// A tick, for confirmation or success.
    Check,
    /// Two crossing strokes inside a circle, for cancel or "no".
    Cross,
    /// A left-pointing arrow, for "back".
    ArrowLeft,
    /// A right-pointing arrow, for "next".
    ArrowRight,
    /// An upward arrow.
    ArrowUp,
    /// A downward arrow.
    ArrowDown,
    /// A five-pointed star, for favourites or ratings.
    Star,
    /// A heart, for likes or favourites.
    Heart,
    /// A gear, for configuration.
    Settings,
    /// A house, for the home view.
    Home,
    /// A magnifying glass, for search.
    Search,
    /// Three stacked bars, for a navigation menu.
    Menu,
    /// The X-shaped dismiss mark.
    Close,
    /// A plus sign, for adding.
    Plus,
    /// A minus sign, for removing.
    Minus,
    /// The letter "i" in a circle, for informational messages.
    Info,
    /// A triangle with an exclamation mark, for warnings.
    Warning,
    /// A circle with an exclamation mark, for errors.
    Error,
    /// A head-and-shoulders silhouette, for an account.
    User,
    /// An envelope, for messages.
    Mail,
    /// A bell, for notifications.
    Bell,
    /// A pencil, for editing.
    Edit,
    /// A waste bin, for deletion.
    Trash,
    /// A node-and-branches glyph, for sharing.
    Share,
    /// A circular arrow, for reloading.
    Refresh,
    /// Three horizontal dots, for an overflow menu.
    More,
    /// A funnel, for filtering.
    Filter,
    /// A closed padlock.
    Lock,
    /// An open padlock.
    Unlock,
    /// A downward arrow into a tray, for downloading.
    Download,
    /// An upward arrow out of a tray, for uploading.
    Upload,
    /// A left chevron, for collapsing or stepping back.
    ChevronLeft,
    /// A right chevron, for expanding or stepping forward.
    ChevronRight,
    /// An up chevron, for collapsing a disclosure.
    ChevronUp,
    /// A down chevron, for expanding a disclosure.
    ChevronDown,
    /// A vertical bar with a left chevron, for the first page.
    FirstPage,
    /// A vertical bar with a right chevron, for the last page.
    LastPage,
    /// A closed folder.
    Folder,
    /// An open folder.
    FolderOpen,
    /// A document sheet with ruled lines.
    File,
    /// A floppy disk, for saving.
    Save,
    /// Two stacked sheets, for copying.
    Copy,
    /// A printer, for printing.
    Print,
    /// A left-curving arrow, for undo.
    Undo,
    /// A right-curving arrow, for redo.
    Redo,
    /// A pair of scissors, for cutting.
    Cut,
    /// A clipboard, for pasting.
    Paste,
    /// A paperclip, for attaching a file.
    Attachment,
    /// Two chain links, for a hyperlink.
    Link,
    /// A filled circle with a tick, for a completed operation.
    Success,
    /// A question mark in a circle, for help.
    Help,
    /// A circle with a diagonal slash, for a disallowed action.
    Block,
    /// A clock face, for a scheduled time.
    Schedule,
    /// An hourglass, for a pending or in-progress state.
    Hourglass,
    /// A right-pointing triangle, for play.
    Play,
    /// Two vertical bars, for pause.
    Pause,
    /// A filled square, for stop.
    Stop,
    /// A triangle against a bar, for the next track.
    SkipNext,
    /// A speaker with sound waves, for unmuted audio.
    VolumeUp,
    /// A speaker with a cross, for muted audio.
    VolumeOff,
    /// A downward arrow over a bar, for sorting.
    Sort,
    /// Three ascending bars, for a chart.
    BarChart,
    /// A calendar page, for a date.
    Calendar,
    /// A grid of cells, for tabular data.
    Table,
    /// A speech bubble, for a conversation.
    Chat,
    /// A telephone handset, for a voice call.
    Call,
    /// A paper plane, for sending.
    Send,
    /// A bell with a cross, for muted notifications.
    NotificationsOff,
    /// A four-corner expand mark, for fullscreen.
    Fullscreen,
    /// A bug silhouette, for debug tooling and breakpoints.
    DebugBug,
    /// A component tile, for the component palette.
    DesignComponent,
    /// A lightning bolt, for event wires and triggers.
    DesignEvent,
    /// A laboratory flask, for tests.
    TestFlask,
    /// A terminal prompt, for developer tools.
    ToolsTerminal,
    /// A translate glyph, for localization tools.
    ToolsTranslate,
    /// Material Symbols `10k` (U+E951).
    Num10k,
    /// Material Symbols `10mp` (U+E952).
    Num10mp,
    /// Material Symbols `11mp` (U+E953).
    Num11mp,
    /// Material Symbols `123` (U+EB8D).
    Num123,
    /// Material Symbols `12mp` (U+E954).
    Num12mp,
    /// Material Symbols `13mp` (U+E955).
    Num13mp,
    /// Material Symbols `14mp` (U+E956).
    Num14mp,
    /// Material Symbols `15mp` (U+E957).
    Num15mp,
    /// Material Symbols `16mp` (U+E958).
    Num16mp,
    /// Material Symbols `17mp` (U+E959).
    Num17mp,
    /// Material Symbols `18_up_rating` (U+F8FD).
    Num18UpRating,
    /// Material Symbols `18mp` (U+E95A).
    Num18mp,
    /// Material Symbols `19mp` (U+E95B).
    Num19mp,
    /// Material Symbols `1k` (U+E95C).
    Num1k,
    /// Material Symbols `1k_plus` (U+E95D).
    Num1kPlus,
    /// Material Symbols `1x_mobiledata` (U+EFCD).
    Num1xMobiledata,
    /// Material Symbols `1x_mobiledata_badge` (U+F7F1).
    Num1xMobiledataBadge,
    /// Material Symbols `20mp` (U+E95E).
    Num20mp,
    /// Material Symbols `21mp` (U+E95F).
    Num21mp,
    /// Material Symbols `22mp` (U+E960).
    Num22mp,
    /// Material Symbols `23mp` (U+E961).
    Num23mp,
    /// Material Symbols `24fps_select` (U+F3F2).
    Num24fpsSelect,
    /// Material Symbols `24mp` (U+E962).
    Num24mp,
    /// Material Symbols `2d` (U+EF37).
    Num2d,
    /// Material Symbols `2d_2` (U+FFF0E).
    Num2d2,
    /// Material Symbols `2k` (U+E963).
    Num2k,
    /// Material Symbols `2k_plus` (U+E964).
    Num2kPlus,
    /// Material Symbols `2mp` (U+E965).
    Num2mp,
    /// Material Symbols `30fps` (U+EFCE).
    Num30fps,
    /// Material Symbols `30fps_select` (U+EFCF).
    Num30fpsSelect,
    /// Material Symbols `360` (U+E577).
    Num360,
    /// Material Symbols `3d` (U+ED38).
    Num3d,
    /// Material Symbols `3d_2` (U+FFF0F).
    Num3d2,
    /// Material Symbols `3d_rotation` (U+E84D).
    Num3dRotation,
    /// Material Symbols `3g_mobiledata` (U+EFD0).
    Num3gMobiledata,
    /// Material Symbols `3g_mobiledata_badge` (U+F7F0).
    Num3gMobiledataBadge,
    /// Material Symbols `3k` (U+E966).
    Num3k,
    /// Material Symbols `3k_plus` (U+E967).
    Num3kPlus,
    /// Material Symbols `3mp` (U+E968).
    Num3mp,
    /// Material Symbols `3p` (U+EFD1).
    Num3p,
    /// Material Symbols `4g_mobiledata` (U+EFD2).
    Num4gMobiledata,
    /// Material Symbols `4g_mobiledata_badge` (U+F7EF).
    Num4gMobiledataBadge,
    /// Material Symbols `4g_plus_mobiledata` (U+EFD3).
    Num4gPlusMobiledata,
    /// Material Symbols `4k` (U+E072).
    Num4k,
    /// Material Symbols `4k_plus` (U+E969).
    Num4kPlus,
    /// Material Symbols `4mp` (U+E96A).
    Num4mp,
    /// Material Symbols `50mp` (U+F6F3).
    Num50mp,
    /// Material Symbols `5g` (U+EF38).
    Num5g,
    /// Material Symbols `5g_mobiledata_badge` (U+F7EE).
    Num5gMobiledataBadge,
    /// Material Symbols `5k` (U+E96B).
    Num5k,
    /// Material Symbols `5k_plus` (U+E96C).
    Num5kPlus,
    /// Material Symbols `5mp` (U+E96D).
    Num5mp,
    /// Material Symbols `60fps` (U+EFD4).
    Num60fps,
    /// Material Symbols `60fps_select` (U+EFD5).
    Num60fpsSelect,
    /// Material Symbols `6_ft_apart` (U+F21E).
    Num6FtApart,
    /// Material Symbols `6k` (U+E96E).
    Num6k,
    /// Material Symbols `6k_plus` (U+E96F).
    Num6kPlus,
    /// Material Symbols `6mp` (U+E970).
    Num6mp,
    /// Material Symbols `7k` (U+E971).
    Num7k,
    /// Material Symbols `7k_plus` (U+E972).
    Num7kPlus,
    /// Material Symbols `7mp` (U+E973).
    Num7mp,
    /// Material Symbols `8k` (U+E974).
    Num8k,
    /// Material Symbols `8k_plus` (U+E975).
    Num8kPlus,
    /// Material Symbols `8mp` (U+E976).
    Num8mp,
    /// Material Symbols `9k` (U+E977).
    Num9k,
    /// Material Symbols `9k_plus` (U+E978).
    Num9kPlus,
    /// Material Symbols `9mp` (U+E979).
    Num9mp,
    /// Material Symbols `abc` (U+EB94).
    Abc,
    /// Material Symbols `ac_unit` (U+EB3B).
    AcUnit,
    /// Material Symbols `access_alarm` (U+E855).
    AccessAlarm,
    /// Material Symbols `access_alarms` (U+E855).
    AccessAlarms,
    /// Material Symbols `access_time` (U+EFD6).
    AccessTime,
    /// Material Symbols `access_time_filled` (U+EFD6).
    AccessTimeFilled,
    /// Material Symbols `accessibility` (U+E84E).
    Accessibility,
    /// Material Symbols `accessibility_new` (U+E92C).
    AccessibilityNew,
    /// Material Symbols `accessible` (U+E914).
    Accessible,
    /// Material Symbols `accessible_forward` (U+E934).
    AccessibleForward,
    /// Material Symbols `accessible_menu` (U+F34E).
    AccessibleMenu,
    /// Material Symbols `account_balance` (U+E84F).
    AccountBalance,
    /// Material Symbols `account_balance_wallet` (U+E850).
    AccountBalanceWallet,
    /// Material Symbols `account_box` (U+E851).
    AccountBox,
    /// Material Symbols `account_child` (U+E852).
    AccountChild,
    /// Material Symbols `account_child_invert` (U+E659).
    AccountChildInvert,
    /// Material Symbols `account_circle` (U+F20B).
    AccountCircle,
    /// Material Symbols `account_circle_filled` (U+F20B).
    AccountCircleFilled,
    /// Material Symbols `account_circle_off` (U+F7B3).
    AccountCircleOff,
    /// Material Symbols `account_tree` (U+E97A).
    AccountTree,
    /// Material Symbols `action_key` (U+F502).
    ActionKey,
    /// Material Symbols `activity_zone` (U+E1E6).
    ActivityZone,
    /// Material Symbols `acupuncture` (U+F2C4).
    Acupuncture,
    /// Material Symbols `acute` (U+E4CB).
    Acute,
    /// Material Symbols `ad` (U+E65A).
    Ad,
    /// Material Symbols `ad_group` (U+E65B).
    AdGroup,
    /// Material Symbols `ad_group_off` (U+EAE5).
    AdGroupOff,
    /// Material Symbols `ad_off` (U+F7B2).
    AdOff,
    /// Material Symbols `ad_units` (U+F2EB).
    AdUnits,
    /// Material Symbols `adaptive_audio_mic` (U+F4CC).
    AdaptiveAudioMic,
    /// Material Symbols `adaptive_audio_mic_off` (U+F4CB).
    AdaptiveAudioMicOff,
    /// Material Symbols `adb` (U+E60E).
    Adb,
    /// Material Symbols `add` (U+E145).
    Add,
    /// Material Symbols `add_2` (U+F3DD).
    Add2,
    /// Material Symbols `add_a_photo` (U+E439).
    AddAPhoto,
    /// Material Symbols `add_ad` (U+E72A).
    AddAd,
    /// Material Symbols `add_alarm` (U+E856).
    AddAlarm,
    /// Material Symbols `add_alert` (U+E003).
    AddAlert,
    /// Material Symbols `add_box` (U+E146).
    AddBox,
    /// Material Symbols `add_business` (U+E729).
    AddBusiness,
    /// Material Symbols `add_call` (U+F0B7).
    AddCall,
    /// Material Symbols `add_card` (U+EB86).
    AddCard,
    /// Material Symbols `add_chart` (U+EF3C).
    AddChart,
    /// Material Symbols `add_circle` (U+E990).
    AddCircle,
    /// Material Symbols `add_circle_outline` (U+E990).
    AddCircleOutline,
    /// Material Symbols `add_column_left` (U+F425).
    AddColumnLeft,
    /// Material Symbols `add_column_right` (U+F424).
    AddColumnRight,
    /// Material Symbols `add_comment` (U+E266).
    AddComment,
    /// Material Symbols `add_diamond` (U+F49C).
    AddDiamond,
    /// Material Symbols `add_home` (U+F8EB).
    AddHome,
    /// Material Symbols `add_home_work` (U+F8ED).
    AddHomeWork,
    /// Material Symbols `add_ic_call` (U+F0B7).
    AddIcCall,
    /// Material Symbols `add_link` (U+E178).
    AddLink,
    /// Material Symbols `add_location` (U+E567).
    AddLocation,
    /// Material Symbols `add_location_alt` (U+EF3A).
    AddLocationAlt,
    /// Material Symbols `add_moderator` (U+E97D).
    AddModerator,
    /// Material Symbols `add_notes` (U+E091).
    AddNotes,
    /// Material Symbols `add_photo_alternate` (U+E43E).
    AddPhotoAlternate,
    /// Material Symbols `add_reaction` (U+E1D3).
    AddReaction,
    /// Material Symbols `add_road` (U+EF3B).
    AddRoad,
    /// Material Symbols `add_row_above` (U+F423).
    AddRowAbove,
    /// Material Symbols `add_row_below` (U+F422).
    AddRowBelow,
    /// Material Symbols `add_shopping_cart` (U+E854).
    AddShoppingCart,
    /// Material Symbols `add_task` (U+F23A).
    AddTask,
    /// Material Symbols `add_to_drive` (U+E65C).
    AddToDrive,
    /// Material Symbols `add_to_home_screen` (U+F2B9).
    AddToHomeScreen,
    /// Material Symbols `add_to_photos` (U+E39D).
    AddToPhotos,
    /// Material Symbols `add_to_queue` (U+E05C).
    AddToQueue,
    /// Material Symbols `add_triangle` (U+F48E).
    AddTriangle,
    /// Material Symbols `addchart` (U+EF3C).
    Addchart,
    /// Material Symbols `adf_scanner` (U+EADA).
    AdfScanner,
    /// Material Symbols `adjust` (U+E39E).
    Adjust,
    /// Material Symbols `admin_meds` (U+E48D).
    AdminMeds,
    /// Material Symbols `admin_panel_settings` (U+EF3D).
    AdminPanelSettings,
    /// Material Symbols `ads_click` (U+E762).
    AdsClick,
    /// Material Symbols `agender` (U+F888).
    Agender,
    /// Material Symbols `agriculture` (U+EA79).
    Agriculture,
    /// Material Symbols `air` (U+EFD8).
    Air,
    /// Material Symbols `air_freshener` (U+E2CA).
    AirFreshener,
    /// Material Symbols `air_purifier` (U+E97E).
    AirPurifier,
    /// Material Symbols `air_purifier_gen` (U+E829).
    AirPurifierGen,
    /// Material Symbols `airline_seat_flat` (U+E630).
    AirlineSeatFlat,
    /// Material Symbols `airline_seat_flat_angled` (U+E631).
    AirlineSeatFlatAngled,
    /// Material Symbols `airline_seat_individual_suite` (U+E632).
    AirlineSeatIndividualSuite,
    /// Material Symbols `airline_seat_legroom_extra` (U+E633).
    AirlineSeatLegroomExtra,
    /// Material Symbols `airline_seat_legroom_normal` (U+E634).
    AirlineSeatLegroomNormal,
    /// Material Symbols `airline_seat_legroom_reduced` (U+E635).
    AirlineSeatLegroomReduced,
    /// Material Symbols `airline_seat_recline_extra` (U+E636).
    AirlineSeatReclineExtra,
    /// Material Symbols `airline_seat_recline_normal` (U+E637).
    AirlineSeatReclineNormal,
    /// Material Symbols `airline_stops` (U+E7D0).
    AirlineStops,
    /// Material Symbols `airlines` (U+E7CA).
    Airlines,
    /// Material Symbols `airplane_ticket` (U+EFD9).
    AirplaneTicket,
    /// Material Symbols `airplanemode_active` (U+E53D).
    AirplanemodeActive,
    /// Material Symbols `airplanemode_inactive` (U+E194).
    AirplanemodeInactive,
    /// Material Symbols `airplay` (U+E055).
    Airplay,
    /// Material Symbols `airport_shuttle` (U+EB3C).
    AirportShuttle,
    /// Material Symbols `airware` (U+F154).
    Airware,
    /// Material Symbols `airwave` (U+F154).
    Airwave,
    /// Material Symbols `alarm` (U+E855).
    Alarm,
    /// Material Symbols `alarm_add` (U+E856).
    AlarmAdd,
    /// Material Symbols `alarm_off` (U+E857).
    AlarmOff,
    /// Material Symbols `alarm_on` (U+E858).
    AlarmOn,
    /// Material Symbols `alarm_pause` (U+F35B).
    AlarmPause,
    /// Material Symbols `alarm_smart_wake` (U+F6B0).
    AlarmSmartWake,
    /// Material Symbols `album` (U+E019).
    Album,
    /// Material Symbols `align_center` (U+E356).
    AlignCenter,
    /// Material Symbols `align_end` (U+F797).
    AlignEnd,
    /// Material Symbols `align_flex_center` (U+F796).
    AlignFlexCenter,
    /// Material Symbols `align_flex_end` (U+F795).
    AlignFlexEnd,
    /// Material Symbols `align_flex_start` (U+F794).
    AlignFlexStart,
    /// Material Symbols `align_horizontal_center` (U+E00F).
    AlignHorizontalCenter,
    /// Material Symbols `align_horizontal_left` (U+E00D).
    AlignHorizontalLeft,
    /// Material Symbols `align_horizontal_right` (U+E010).
    AlignHorizontalRight,
    /// Material Symbols `align_items_stretch` (U+F793).
    AlignItemsStretch,
    /// Material Symbols `align_justify_center` (U+F792).
    AlignJustifyCenter,
    /// Material Symbols `align_justify_flex_end` (U+F791).
    AlignJustifyFlexEnd,
    /// Material Symbols `align_justify_flex_start` (U+F790).
    AlignJustifyFlexStart,
    /// Material Symbols `align_justify_space_around` (U+F78F).
    AlignJustifySpaceAround,
    /// Material Symbols `align_justify_space_between` (U+F78E).
    AlignJustifySpaceBetween,
    /// Material Symbols `align_justify_space_even` (U+F78D).
    AlignJustifySpaceEven,
    /// Material Symbols `align_justify_stretch` (U+F78C).
    AlignJustifyStretch,
    /// Material Symbols `align_self_stretch` (U+F78B).
    AlignSelfStretch,
    /// Material Symbols `align_space_around` (U+F78A).
    AlignSpaceAround,
    /// Material Symbols `align_space_between` (U+F789).
    AlignSpaceBetween,
    /// Material Symbols `align_space_even` (U+F788).
    AlignSpaceEven,
    /// Material Symbols `align_start` (U+F787).
    AlignStart,
    /// Material Symbols `align_stretch` (U+F786).
    AlignStretch,
    /// Material Symbols `align_vertical_bottom` (U+E015).
    AlignVerticalBottom,
    /// Material Symbols `align_vertical_center` (U+E011).
    AlignVerticalCenter,
    /// Material Symbols `align_vertical_top` (U+E00C).
    AlignVerticalTop,
    /// Material Symbols `all_inbox` (U+E97F).
    AllInbox,
    /// Material Symbols `all_inclusive` (U+EB3D).
    AllInclusive,
    /// Material Symbols `all_match` (U+E093).
    AllMatch,
    /// Material Symbols `all_out` (U+E90B).
    AllOut,
    /// Material Symbols `allergies` (U+E094).
    Allergies,
    /// Material Symbols `allergy` (U+E64E).
    Allergy,
    /// Material Symbols `alt_route` (U+F184).
    AltRoute,
    /// Material Symbols `alternate_email` (U+E0E6).
    AlternateEmail,
    /// Material Symbols `altitude` (U+F873).
    Altitude,
    /// Material Symbols `ambient_screen` (U+F6C4).
    AmbientScreen,
    /// Material Symbols `ambulance` (U+F803).
    Ambulance,
    /// Material Symbols `amend` (U+F802).
    Amend,
    /// Material Symbols `amp_stories` (U+EA13).
    AmpStories,
    /// Material Symbols `analytics` (U+EF3E).
    Analytics,
    /// Material Symbols `anchor` (U+F1CD).
    Anchor,
    /// Material Symbols `android` (U+E859).
    Android,
    /// Material Symbols `android_cell_4_bar` (U+EF06).
    AndroidCell4Bar,
    /// Material Symbols `android_cell_4_bar_alert` (U+EF09).
    AndroidCell4BarAlert,
    /// Material Symbols `android_cell_4_bar_off` (U+EF08).
    AndroidCell4BarOff,
    /// Material Symbols `android_cell_4_bar_plus` (U+EF07).
    AndroidCell4BarPlus,
    /// Material Symbols `android_cell_5_bar` (U+EF02).
    AndroidCell5Bar,
    /// Material Symbols `android_cell_5_bar_alert` (U+EF05).
    AndroidCell5BarAlert,
    /// Material Symbols `android_cell_5_bar_off` (U+EF04).
    AndroidCell5BarOff,
    /// Material Symbols `android_cell_5_bar_plus` (U+EF03).
    AndroidCell5BarPlus,
    /// Material Symbols `android_cell_dual_4_bar` (U+EF0D).
    AndroidCellDual4Bar,
    /// Material Symbols `android_cell_dual_4_bar_alert` (U+EF0F).
    AndroidCellDual4BarAlert,
    /// Material Symbols `android_cell_dual_4_bar_plus` (U+EF0E).
    AndroidCellDual4BarPlus,
    /// Material Symbols `android_cell_dual_5_bar` (U+EF0A).
    AndroidCellDual5Bar,
    /// Material Symbols `android_cell_dual_5_bar_alert` (U+EF0C).
    AndroidCellDual5BarAlert,
    /// Material Symbols `android_cell_dual_5_bar_plus` (U+EF0B).
    AndroidCellDual5BarPlus,
    /// Material Symbols `android_wifi_3_bar` (U+EF16).
    AndroidWifi3Bar,
    /// Material Symbols `android_wifi_3_bar_alert` (U+EF1B).
    AndroidWifi3BarAlert,
    /// Material Symbols `android_wifi_3_bar_lock` (U+EF1A).
    AndroidWifi3BarLock,
    /// Material Symbols `android_wifi_3_bar_off` (U+EF19).
    AndroidWifi3BarOff,
    /// Material Symbols `android_wifi_3_bar_plus` (U+EF18).
    AndroidWifi3BarPlus,
    /// Material Symbols `android_wifi_3_bar_question` (U+EF17).
    AndroidWifi3BarQuestion,
    /// Material Symbols `android_wifi_4_bar` (U+EF10).
    AndroidWifi4Bar,
    /// Material Symbols `android_wifi_4_bar_alert` (U+EF15).
    AndroidWifi4BarAlert,
    /// Material Symbols `android_wifi_4_bar_lock` (U+EF14).
    AndroidWifi4BarLock,
    /// Material Symbols `android_wifi_4_bar_off` (U+EF13).
    AndroidWifi4BarOff,
    /// Material Symbols `android_wifi_4_bar_plus` (U+EF12).
    AndroidWifi4BarPlus,
    /// Material Symbols `android_wifi_4_bar_question` (U+EF11).
    AndroidWifi4BarQuestion,
    /// Material Symbols `animated_images` (U+F49A).
    AnimatedImages,
    /// Material Symbols `animation` (U+E71C).
    Animation,
    /// Material Symbols `announcement` (U+E87F).
    Announcement,
    /// Material Symbols `antigravity` (U+FFFD2).
    Antigravity,
    /// Material Symbols `aod` (U+F2E6).
    Aod,
    /// Material Symbols `aod_tablet` (U+F89F).
    AodTablet,
    /// Material Symbols `aod_watch` (U+F6AC).
    AodWatch,
    /// Material Symbols `apartment` (U+EA40).
    Apartment,
    /// Material Symbols `api` (U+F1B7).
    Api,
    /// Material Symbols `apk_document` (U+F88E).
    ApkDocument,
    /// Material Symbols `apk_install` (U+F88F).
    ApkInstall,
    /// Material Symbols `app_badging` (U+F72F).
    AppBadging,
    /// Material Symbols `app_blocking` (U+F2E5).
    AppBlocking,
    /// Material Symbols `app_promo` (U+F2CD).
    AppPromo,
    /// Material Symbols `app_registration` (U+EF40).
    AppRegistration,
    /// Material Symbols `app_settings_alt` (U+F2D9).
    AppSettingsAlt,
    /// Material Symbols `app_shortcut` (U+F2DF).
    AppShortcut,
    /// Material Symbols `apparel` (U+EF7B).
    Apparel,
    /// Material Symbols `approval` (U+E982).
    Approval,
    /// Material Symbols `approval_delegation` (U+F84A).
    ApprovalDelegation,
    /// Material Symbols `approval_delegation_off` (U+F2C5).
    ApprovalDelegationOff,
    /// Material Symbols `apps` (U+E5C3).
    Apps,
    /// Material Symbols `apps_outage` (U+E7CC).
    AppsOutage,
    /// Material Symbols `apps_plus` (U+FFEA7).
    AppsPlus,
    /// Material Symbols `aq` (U+F55A).
    Aq,
    /// Material Symbols `aq_indoor` (U+F55B).
    AqIndoor,
    /// Material Symbols `ar_on_you` (U+EF7C).
    ArOnYou,
    /// Material Symbols `ar_stickers` (U+E983).
    ArStickers,
    /// Material Symbols `architecture` (U+EA3B).
    Architecture,
    /// Material Symbols `archive` (U+E149).
    Archive,
    /// Material Symbols `area_chart` (U+E770).
    AreaChart,
    /// Material Symbols `arming_countdown` (U+E78A).
    ArmingCountdown,
    /// Material Symbols `arrow_and_edge` (U+F5D7).
    ArrowAndEdge,
    /// Material Symbols `arrow_back` (U+E5C4).
    ArrowBack,
    /// Material Symbols `arrow_back_2` (U+F43A).
    ArrowBack2,
    /// Material Symbols `arrow_back_ios` (U+E5E0).
    ArrowBackIos,
    /// Material Symbols `arrow_back_ios_new` (U+E2EA).
    ArrowBackIosNew,
    /// Material Symbols `arrow_circle_down` (U+F181).
    ArrowCircleDown,
    /// Material Symbols `arrow_circle_left` (U+EAA7).
    ArrowCircleLeft,
    /// Material Symbols `arrow_circle_right` (U+EAAA).
    ArrowCircleRight,
    /// Material Symbols `arrow_circle_up` (U+F182).
    ArrowCircleUp,
    /// Material Symbols `arrow_cool_down` (U+F4B6).
    ArrowCoolDown,
    /// Material Symbols `arrow_downward` (U+E5DB).
    ArrowDownward,
    /// Material Symbols `arrow_downward_alt` (U+E984).
    ArrowDownwardAlt,
    /// Material Symbols `arrow_drop_down` (U+E5C5).
    ArrowDropDown,
    /// Material Symbols `arrow_drop_down_circle` (U+E5C6).
    ArrowDropDownCircle,
    /// Material Symbols `arrow_drop_up` (U+E5C7).
    ArrowDropUp,
    /// Material Symbols `arrow_forward` (U+E5C8).
    ArrowForward,
    /// Material Symbols `arrow_forward_ios` (U+E5E1).
    ArrowForwardIos,
    /// Material Symbols `arrow_insert` (U+F837).
    ArrowInsert,
    /// Material Symbols `arrow_left_alt` (U+EF7D).
    ArrowLeftAlt,
    /// Material Symbols `arrow_menu_close` (U+F3D3).
    ArrowMenuClose,
    /// Material Symbols `arrow_menu_open` (U+F3D2).
    ArrowMenuOpen,
    /// Material Symbols `arrow_or_edge` (U+F5D6).
    ArrowOrEdge,
    /// Material Symbols `arrow_outward` (U+F8CE).
    ArrowOutward,
    /// Material Symbols `arrow_range` (U+F69B).
    ArrowRange,
    /// Material Symbols `arrow_right_alt` (U+E941).
    ArrowRightAlt,
    /// Material Symbols `arrow_selector_tool` (U+F82F).
    ArrowSelectorTool,
    /// Material Symbols `arrow_shape_up` (U+EEF6).
    ArrowShapeUp,
    /// Material Symbols `arrow_shape_up_stack` (U+EEF7).
    ArrowShapeUpStack,
    /// Material Symbols `arrow_shape_up_stack_2` (U+EEF8).
    ArrowShapeUpStack2,
    /// Material Symbols `arrow_split` (U+EA04).
    ArrowSplit,
    /// Material Symbols `arrow_top_left` (U+F72E).
    ArrowTopLeft,
    /// Material Symbols `arrow_top_right` (U+F72D).
    ArrowTopRight,
    /// Material Symbols `arrow_upload_progress` (U+F3F4).
    ArrowUploadProgress,
    /// Material Symbols `arrow_upload_ready` (U+F3F5).
    ArrowUploadReady,
    /// Material Symbols `arrow_upward` (U+E5D8).
    ArrowUpward,
    /// Material Symbols `arrow_upward_alt` (U+E986).
    ArrowUpwardAlt,
    /// Material Symbols `arrow_warm_up` (U+F4B5).
    ArrowWarmUp,
    /// Material Symbols `arrows_input` (U+F394).
    ArrowsInput,
    /// Material Symbols `arrows_left_right_circle` (U+EEE4).
    ArrowsLeftRightCircle,
    /// Material Symbols `arrows_more_down` (U+F8AB).
    ArrowsMoreDown,
    /// Material Symbols `arrows_more_up` (U+F8AC).
    ArrowsMoreUp,
    /// Material Symbols `arrows_output` (U+F393).
    ArrowsOutput,
    /// Material Symbols `arrows_outward` (U+F72C).
    ArrowsOutward,
    /// Material Symbols `arrows_up_down_circle` (U+EEE3).
    ArrowsUpDownCircle,
    /// Material Symbols `art_track` (U+E060).
    ArtTrack,
    /// Material Symbols `article` (U+EF42).
    Article,
    /// Material Symbols `article_person` (U+F368).
    ArticlePerson,
    /// Material Symbols `article_shortcut` (U+F587).
    ArticleShortcut,
    /// Material Symbols `artist` (U+E01A).
    Artist,
    /// Material Symbols `aspect_ratio` (U+E85B).
    AspectRatio,
    /// Material Symbols `assessment` (U+F0CC).
    Assessment,
    /// Material Symbols `assignment` (U+E85D).
    Assignment,
    /// Material Symbols `assignment_add` (U+F848).
    AssignmentAdd,
    /// Material Symbols `assignment_globe` (U+EEEC).
    AssignmentGlobe,
    /// Material Symbols `assignment_ind` (U+E85E).
    AssignmentInd,
    /// Material Symbols `assignment_late` (U+E85F).
    AssignmentLate,
    /// Material Symbols `assignment_return` (U+E860).
    AssignmentReturn,
    /// Material Symbols `assignment_returned` (U+E861).
    AssignmentReturned,
    /// Material Symbols `assignment_turned_in` (U+E862).
    AssignmentTurnedIn,
    /// Material Symbols `assist_walker` (U+F8D5).
    AssistWalker,
    /// Material Symbols `assistant` (U+E39F).
    Assistant,
    /// Material Symbols `assistant_device` (U+E987).
    AssistantDevice,
    /// Material Symbols `assistant_direction` (U+E988).
    AssistantDirection,
    /// Material Symbols `assistant_navigation` (U+E989).
    AssistantNavigation,
    /// Material Symbols `assistant_on_hub` (U+F6C1).
    AssistantOnHub,
    /// Material Symbols `assistant_photo` (U+F0C6).
    AssistantPhoto,
    /// Material Symbols `assured_workload` (U+EB6F).
    AssuredWorkload,
    /// Material Symbols `asterisk` (U+F525).
    Asterisk,
    /// Material Symbols `astrophotography_auto` (U+F1D9).
    AstrophotographyAuto,
    /// Material Symbols `astrophotography_off` (U+F1DA).
    AstrophotographyOff,
    /// Material Symbols `atm` (U+E573).
    Atm,
    /// Material Symbols `atr` (U+EBC7).
    Atr,
    /// Material Symbols `attach_email` (U+EA5E).
    AttachEmail,
    /// Material Symbols `attach_file` (U+E226).
    AttachFile,
    /// Material Symbols `attach_file_add` (U+F841).
    AttachFileAdd,
    /// Material Symbols `attach_file_off` (U+F4D9).
    AttachFileOff,
    /// Material Symbols `attach_money` (U+E227).
    AttachMoney,
    /// Material Symbols `attractions` (U+EA52).
    Attractions,
    /// Material Symbols `attribution` (U+EFDB).
    Attribution,
    /// Material Symbols `audio_capture` (U+FFF03).
    AudioCapture,
    /// Material Symbols `audio_description` (U+F58C).
    AudioDescription,
    /// Material Symbols `audio_file` (U+EB82).
    AudioFile,
    /// Material Symbols `audio_video_receiver` (U+F5D3).
    AudioVideoReceiver,
    /// Material Symbols `audiotrack` (U+E405).
    Audiotrack,
    /// Material Symbols `auto_activity_zone` (U+F8AD).
    AutoActivityZone,
    /// Material Symbols `auto_awesome` (U+E65F).
    AutoAwesome,
    /// Material Symbols `auto_awesome_mosaic` (U+E660).
    AutoAwesomeMosaic,
    /// Material Symbols `auto_awesome_motion` (U+E661).
    AutoAwesomeMotion,
    /// Material Symbols `auto_delete` (U+EA4C).
    AutoDelete,
    /// Material Symbols `auto_detect_voice` (U+F83E).
    AutoDetectVoice,
    /// Material Symbols `auto_draw_solid` (U+E98A).
    AutoDrawSolid,
    /// Material Symbols `auto_fix` (U+E663).
    AutoFix,
    /// Material Symbols `auto_fix_high` (U+E663).
    AutoFixHigh,
    /// Material Symbols `auto_fix_normal` (U+E664).
    AutoFixNormal,
    /// Material Symbols `auto_fix_off` (U+E665).
    AutoFixOff,
    /// Material Symbols `auto_graph` (U+E4FB).
    AutoGraph,
    /// Material Symbols `auto_label` (U+F6BE).
    AutoLabel,
    /// Material Symbols `auto_meeting_room` (U+F6BF).
    AutoMeetingRoom,
    /// Material Symbols `auto_mode` (U+EC20).
    AutoMode,
    /// Material Symbols `auto_read_pause` (U+F219).
    AutoReadPause,
    /// Material Symbols `auto_read_play` (U+F216).
    AutoReadPlay,
    /// Material Symbols `auto_schedule` (U+E214).
    AutoSchedule,
    /// Material Symbols `auto_stories` (U+E666).
    AutoStories,
    /// Material Symbols `auto_stories_off` (U+F267).
    AutoStoriesOff,
    /// Material Symbols `auto_timer` (U+EF7F).
    AutoTimer,
    /// Material Symbols `auto_towing` (U+E71E).
    AutoTowing,
    /// Material Symbols `auto_transmission` (U+F53F).
    AutoTransmission,
    /// Material Symbols `auto_videocam` (U+F6C0).
    AutoVideocam,
    /// Material Symbols `autofps_select` (U+EFDC).
    AutofpsSelect,
    /// Material Symbols `automation` (U+F421).
    Automation,
    /// Material Symbols `autopause` (U+F6B6).
    Autopause,
    /// Material Symbols `autopay` (U+F84B).
    Autopay,
    /// Material Symbols `autoplay` (U+F6B5).
    Autoplay,
    /// Material Symbols `autorenew` (U+E863).
    Autorenew,
    /// Material Symbols `autostop` (U+F682).
    Autostop,
    /// Material Symbols `av1` (U+F4B0).
    Av1,
    /// Material Symbols `av_timer` (U+E01B).
    AvTimer,
    /// Material Symbols `avc` (U+F4AF).
    Avc,
    /// Material Symbols `avg_pace` (U+F6BB).
    AvgPace,
    /// Material Symbols `avg_time` (U+F813).
    AvgTime,
    /// Material Symbols `avocado_bean` (U+FFFA7).
    AvocadoBean,
    /// Material Symbols `award_meal` (U+F241).
    AwardMeal,
    /// Material Symbols `award_star` (U+F612).
    AwardStar,
    /// Material Symbols `azm` (U+F6EC).
    Azm,
    /// Material Symbols `b_circle` (U+EEE2).
    BCircle,
    /// Material Symbols `baby_changing_station` (U+F19B).
    BabyChangingStation,
    /// Material Symbols `back_hand` (U+E764).
    BackHand,
    /// Material Symbols `back_to_tab` (U+F72B).
    BackToTab,
    /// Material Symbols `background_dot_large` (U+F79E).
    BackgroundDotLarge,
    /// Material Symbols `background_dot_small` (U+F514).
    BackgroundDotSmall,
    /// Material Symbols `background_grid_small` (U+F79D).
    BackgroundGridSmall,
    /// Material Symbols `background_replace` (U+F20A).
    BackgroundReplace,
    /// Material Symbols `backlight_high` (U+F7ED).
    BacklightHigh,
    /// Material Symbols `backlight_high_off` (U+F4EF).
    BacklightHighOff,
    /// Material Symbols `backlight_low` (U+F7EC).
    BacklightLow,
    /// Material Symbols `backpack` (U+F19C).
    Backpack,
    /// Material Symbols `backspace` (U+E14A).
    Backspace,
    /// Material Symbols `backup` (U+E864).
    Backup,
    /// Material Symbols `backup_table` (U+EF43).
    BackupTable,
    /// Material Symbols `badge` (U+EA67).
    Badge,
    /// Material Symbols `badge_critical_battery` (U+F156).
    BadgeCriticalBattery,
    /// Material Symbols `badminton` (U+F2A8).
    Badminton,
    /// Material Symbols `bakery_dining` (U+EA53).
    BakeryDining,
    /// Material Symbols `balance` (U+EAF6).
    Balance,
    /// Material Symbols `balcony` (U+E58F).
    Balcony,
    /// Material Symbols `ballot` (U+E172).
    Ballot,
    /// Material Symbols `bar_chart_4_bars` (U+F681).
    BarChart4Bars,
    /// Material Symbols `bar_chart_off` (U+F411).
    BarChartOff,
    /// Material Symbols `barcode` (U+E70B).
    Barcode,
    /// Material Symbols `barcode_reader` (U+F85C).
    BarcodeReader,
    /// Material Symbols `barcode_scanner` (U+E70C).
    BarcodeScanner,
    /// Material Symbols `barefoot` (U+F871).
    Barefoot,
    /// Material Symbols `batch_prediction` (U+F0F5).
    BatchPrediction,
    /// Material Symbols `bath_bedrock` (U+F286).
    BathBedrock,
    /// Material Symbols `bath_outdoor` (U+F6FB).
    BathOutdoor,
    /// Material Symbols `bath_private` (U+F6FA).
    BathPrivate,
    /// Material Symbols `bath_public_large` (U+F6F9).
    BathPublicLarge,
    /// Material Symbols `bath_soak` (U+F2A0).
    BathSoak,
    /// Material Symbols `bathroom` (U+EFDD).
    Bathroom,
    /// Material Symbols `bathtub` (U+EA41).
    Bathtub,
    /// Material Symbols `battery_0_bar` (U+EBDC).
    Battery0Bar,
    /// Material Symbols `battery_1_bar` (U+F09C).
    Battery1Bar,
    /// Material Symbols `battery_20` (U+F09C).
    Battery20,
    /// Material Symbols `battery_2_bar` (U+F09D).
    Battery2Bar,
    /// Material Symbols `battery_30` (U+F09D).
    Battery30,
    /// Material Symbols `battery_3_bar` (U+F09E).
    Battery3Bar,
    /// Material Symbols `battery_4_bar` (U+F09F).
    Battery4Bar,
    /// Material Symbols `battery_50` (U+F09E).
    Battery50,
    /// Material Symbols `battery_5_bar` (U+F0A0).
    Battery5Bar,
    /// Material Symbols `battery_60` (U+F09F).
    Battery60,
    /// Material Symbols `battery_6_bar` (U+F0A1).
    Battery6Bar,
    /// Material Symbols `battery_80` (U+F0A0).
    Battery80,
    /// Material Symbols `battery_90` (U+F0A1).
    Battery90,
    /// Material Symbols `battery_alert` (U+E19C).
    BatteryAlert,
    /// Material Symbols `battery_android_0` (U+F30D).
    BatteryAndroid0,
    /// Material Symbols `battery_android_1` (U+F30C).
    BatteryAndroid1,
    /// Material Symbols `battery_android_2` (U+F30B).
    BatteryAndroid2,
    /// Material Symbols `battery_android_3` (U+F30A).
    BatteryAndroid3,
    /// Material Symbols `battery_android_4` (U+F309).
    BatteryAndroid4,
    /// Material Symbols `battery_android_5` (U+F308).
    BatteryAndroid5,
    /// Material Symbols `battery_android_6` (U+F307).
    BatteryAndroid6,
    /// Material Symbols `battery_android_alert` (U+F306).
    BatteryAndroidAlert,
    /// Material Symbols `battery_android_bolt` (U+F305).
    BatteryAndroidBolt,
    /// Material Symbols `battery_android_frame_1` (U+F257).
    BatteryAndroidFrame1,
    /// Material Symbols `battery_android_frame_2` (U+F256).
    BatteryAndroidFrame2,
    /// Material Symbols `battery_android_frame_3` (U+F255).
    BatteryAndroidFrame3,
    /// Material Symbols `battery_android_frame_4` (U+F254).
    BatteryAndroidFrame4,
    /// Material Symbols `battery_android_frame_5` (U+F253).
    BatteryAndroidFrame5,
    /// Material Symbols `battery_android_frame_6` (U+F252).
    BatteryAndroidFrame6,
    /// Material Symbols `battery_android_frame_alert` (U+F251).
    BatteryAndroidFrameAlert,
    /// Material Symbols `battery_android_frame_bolt` (U+F250).
    BatteryAndroidFrameBolt,
    /// Material Symbols `battery_android_frame_full` (U+F24F).
    BatteryAndroidFrameFull,
    /// Material Symbols `battery_android_frame_plus` (U+F24E).
    BatteryAndroidFramePlus,
    /// Material Symbols `battery_android_frame_question` (U+F24D).
    BatteryAndroidFrameQuestion,
    /// Material Symbols `battery_android_frame_share` (U+F24C).
    BatteryAndroidFrameShare,
    /// Material Symbols `battery_android_frame_shield` (U+F24B).
    BatteryAndroidFrameShield,
    /// Material Symbols `battery_android_full` (U+F304).
    BatteryAndroidFull,
    /// Material Symbols `battery_android_plus` (U+F303).
    BatteryAndroidPlus,
    /// Material Symbols `battery_android_question` (U+F302).
    BatteryAndroidQuestion,
    /// Material Symbols `battery_android_share` (U+F301).
    BatteryAndroidShare,
    /// Material Symbols `battery_android_shield` (U+F300).
    BatteryAndroidShield,
    /// Material Symbols `battery_change` (U+F7EB).
    BatteryChange,
    /// Material Symbols `battery_charging_20` (U+F0A2).
    BatteryCharging20,
    /// Material Symbols `battery_charging_20_2` (U+FFF3E).
    BatteryCharging202,
    /// Material Symbols `battery_charging_30` (U+F0A3).
    BatteryCharging30,
    /// Material Symbols `battery_charging_30_2` (U+FFF3D).
    BatteryCharging302,
    /// Material Symbols `battery_charging_50` (U+F0A4).
    BatteryCharging50,
    /// Material Symbols `battery_charging_50_2` (U+FFF3C).
    BatteryCharging502,
    /// Material Symbols `battery_charging_60` (U+F0A5).
    BatteryCharging60,
    /// Material Symbols `battery_charging_60_2` (U+FFF3B).
    BatteryCharging602,
    /// Material Symbols `battery_charging_80` (U+F0A6).
    BatteryCharging80,
    /// Material Symbols `battery_charging_80_2` (U+FFF3A).
    BatteryCharging802,
    /// Material Symbols `battery_charging_90` (U+F0A7).
    BatteryCharging90,
    /// Material Symbols `battery_charging_full` (U+E1A3).
    BatteryChargingFull,
    /// Material Symbols `battery_charging_full_2` (U+FFF39).
    BatteryChargingFull2,
    /// Material Symbols `battery_error` (U+F7EA).
    BatteryError,
    /// Material Symbols `battery_full` (U+E1A5).
    BatteryFull,
    /// Material Symbols `battery_full_alt` (U+F13B).
    BatteryFullAlt,
    /// Material Symbols `battery_horiz_000` (U+F8AE).
    BatteryHoriz000,
    /// Material Symbols `battery_horiz_050` (U+F8AF).
    BatteryHoriz050,
    /// Material Symbols `battery_horiz_075` (U+F8B0).
    BatteryHoriz075,
    /// Material Symbols `battery_low` (U+F155).
    BatteryLow,
    /// Material Symbols `battery_plus` (U+F7E9).
    BatteryPlus,
    /// Material Symbols `battery_profile` (U+E206).
    BatteryProfile,
    /// Material Symbols `battery_saver` (U+EFDE).
    BatterySaver,
    /// Material Symbols `battery_share` (U+F67E).
    BatteryShare,
    /// Material Symbols `battery_status_good` (U+F67D).
    BatteryStatusGood,
    /// Material Symbols `battery_std` (U+E1A5).
    BatteryStd,
    /// Material Symbols `battery_unknown` (U+E1A6).
    BatteryUnknown,
    /// Material Symbols `battery_vert_005` (U+F8B1).
    BatteryVert005,
    /// Material Symbols `battery_vert_020` (U+F8B2).
    BatteryVert020,
    /// Material Symbols `battery_vert_050` (U+F8B3).
    BatteryVert050,
    /// Material Symbols `battery_very_low` (U+F156).
    BatteryVeryLow,
    /// Material Symbols `beach_access` (U+EB3E).
    BeachAccess,
    /// Material Symbols `bed` (U+EFDF).
    Bed,
    /// Material Symbols `bedroom_baby` (U+EFE0).
    BedroomBaby,
    /// Material Symbols `bedroom_child` (U+EFE1).
    BedroomChild,
    /// Material Symbols `bedroom_parent` (U+EFE2).
    BedroomParent,
    /// Material Symbols `bedtime` (U+F159).
    Bedtime,
    /// Material Symbols `bedtime_off` (U+EB76).
    BedtimeOff,
    /// Material Symbols `beenhere` (U+E52D).
    Beenhere,
    /// Material Symbols `beer_meal` (U+F285).
    BeerMeal,
    /// Material Symbols `bento` (U+F1F4).
    Bento,
    /// Material Symbols `bia` (U+F6EB).
    Bia,
    /// Material Symbols `bid_landscape` (U+E667).
    BidLandscape,
    /// Material Symbols `bid_landscape_disabled` (U+EF81).
    BidLandscapeDisabled,
    /// Material Symbols `bigtop_updates` (U+E669).
    BigtopUpdates,
    /// Material Symbols `bike_dock` (U+F47B).
    BikeDock,
    /// Material Symbols `bike_lane` (U+F47A).
    BikeLane,
    /// Material Symbols `bike_scooter` (U+EF45).
    BikeScooter,
    /// Material Symbols `biotech` (U+EA3A).
    Biotech,
    /// Material Symbols `blanket` (U+E828).
    Blanket,
    /// Material Symbols `blender` (U+EFE3).
    Blender,
    /// Material Symbols `blind` (U+F8D6).
    Blind,
    /// Material Symbols `blinds` (U+E286).
    Blinds,
    /// Material Symbols `blinds_2` (U+FFF78).
    Blinds2,
    /// Material Symbols `blinds_2_closed` (U+FFF79).
    Blinds2Closed,
    /// Material Symbols `blinds_closed` (U+EC1F).
    BlindsClosed,
    /// Material Symbols `blood_pressure` (U+E097).
    BloodPressure,
    /// Material Symbols `bloodtype` (U+EFE4).
    Bloodtype,
    /// Material Symbols `bluetooth` (U+E1A7).
    Bluetooth,
    /// Material Symbols `bluetooth_audio` (U+E60F).
    BluetoothAudio,
    /// Material Symbols `bluetooth_connected` (U+E1A8).
    BluetoothConnected,
    /// Material Symbols `bluetooth_disabled` (U+E1A9).
    BluetoothDisabled,
    /// Material Symbols `bluetooth_drive` (U+EFE5).
    BluetoothDrive,
    /// Material Symbols `bluetooth_searching` (U+E60F).
    BluetoothSearching,
    /// Material Symbols `blur_circular` (U+E3A2).
    BlurCircular,
    /// Material Symbols `blur_linear` (U+E3A3).
    BlurLinear,
    /// Material Symbols `blur_medium` (U+E84C).
    BlurMedium,
    /// Material Symbols `blur_off` (U+E3A4).
    BlurOff,
    /// Material Symbols `blur_on` (U+E3A5).
    BlurOn,
    /// Material Symbols `blur_short` (U+E8CF).
    BlurShort,
    /// Material Symbols `boat_bus` (U+F36D).
    BoatBus,
    /// Material Symbols `boat_railway` (U+F36C).
    BoatRailway,
    /// Material Symbols `body_fat` (U+E098).
    BodyFat,
    /// Material Symbols `body_system` (U+E099).
    BodySystem,
    /// Material Symbols `bolt` (U+EA0B).
    Bolt,
    /// Material Symbols `bolt_boost` (U+FFF6A).
    BoltBoost,
    /// Material Symbols `bomb` (U+F568).
    Bomb,
    /// Material Symbols `book` (U+E86E).
    Book,
    /// Material Symbols `book_2` (U+F53E).
    Book2,
    /// Material Symbols `book_3` (U+F53D).
    Book3,
    /// Material Symbols `book_4` (U+F53C).
    Book4,
    /// Material Symbols `book_5` (U+F53B).
    Book5,
    /// Material Symbols `book_6` (U+F3DF).
    Book6,
    /// Material Symbols `book_online` (U+F2E4).
    BookOnline,
    /// Material Symbols `book_ribbon` (U+F3E7).
    BookRibbon,
    /// Material Symbols `bookmark` (U+E8E7).
    Bookmark,
    /// Material Symbols `bookmark_add` (U+E598).
    BookmarkAdd,
    /// Material Symbols `bookmark_added` (U+E599).
    BookmarkAdded,
    /// Material Symbols `bookmark_bag` (U+F410).
    BookmarkBag,
    /// Material Symbols `bookmark_border` (U+E8E7).
    BookmarkBorder,
    /// Material Symbols `bookmark_check` (U+F457).
    BookmarkCheck,
    /// Material Symbols `bookmark_flag` (U+F456).
    BookmarkFlag,
    /// Material Symbols `bookmark_heart` (U+F455).
    BookmarkHeart,
    /// Material Symbols `bookmark_manager` (U+F7B1).
    BookmarkManager,
    /// Material Symbols `bookmark_remove` (U+E59A).
    BookmarkRemove,
    /// Material Symbols `bookmark_stacks` (U+EEE8).
    BookmarkStacks,
    /// Material Symbols `bookmark_star` (U+F454).
    BookmarkStar,
    /// Material Symbols `bookmarks` (U+E98B).
    Bookmarks,
    /// Material Symbols `books_movies_and_music` (U+EF82).
    BooksMoviesAndMusic,
    /// Material Symbols `border_all` (U+E228).
    BorderAll,
    /// Material Symbols `border_bottom` (U+E229).
    BorderBottom,
    /// Material Symbols `border_clear` (U+E22A).
    BorderClear,
    /// Material Symbols `border_color` (U+E22B).
    BorderColor,
    /// Material Symbols `border_horizontal` (U+E22C).
    BorderHorizontal,
    /// Material Symbols `border_inner` (U+E22D).
    BorderInner,
    /// Material Symbols `border_left` (U+E22E).
    BorderLeft,
    /// Material Symbols `border_outer` (U+E22F).
    BorderOuter,
    /// Material Symbols `border_right` (U+E230).
    BorderRight,
    /// Material Symbols `border_style` (U+E231).
    BorderStyle,
    /// Material Symbols `border_top` (U+E232).
    BorderTop,
    /// Material Symbols `border_vertical` (U+E233).
    BorderVertical,
    /// Material Symbols `borg` (U+F40D).
    Borg,
    /// Material Symbols `bottom_app_bar` (U+E730).
    BottomAppBar,
    /// Material Symbols `bottom_drawer` (U+E72D).
    BottomDrawer,
    /// Material Symbols `bottom_navigation` (U+E98C).
    BottomNavigation,
    /// Material Symbols `bottom_panel_close` (U+F72A).
    BottomPanelClose,
    /// Material Symbols `bottom_panel_open` (U+F729).
    BottomPanelOpen,
    /// Material Symbols `bottom_right_click` (U+F684).
    BottomRightClick,
    /// Material Symbols `bottom_sheets` (U+E98D).
    BottomSheets,
    /// Material Symbols `box` (U+F5A4).
    Box,
    /// Material Symbols `box_add` (U+F5A5).
    BoxAdd,
    /// Material Symbols `box_edit` (U+F5A6).
    BoxEdit,
    /// Material Symbols `boy` (U+EB67).
    Boy,
    /// Material Symbols `brand_awareness` (U+E98E).
    BrandAwareness,
    /// Material Symbols `brand_family` (U+F4F1).
    BrandFamily,
    /// Material Symbols `branding_watermark` (U+E06B).
    BrandingWatermark,
    /// Material Symbols `breakfast_dining` (U+EA54).
    BreakfastDining,
    /// Material Symbols `breaking_news` (U+EA08).
    BreakingNews,
    /// Material Symbols `breaking_news_alt_1` (U+F0BA).
    BreakingNewsAlt1,
    /// Material Symbols `breastfeeding` (U+F856).
    Breastfeeding,
    /// Material Symbols `brick` (U+F388).
    Brick,
    /// Material Symbols `briefcase_meal` (U+F246).
    BriefcaseMeal,
    /// Material Symbols `brightness_1` (U+E3FA).
    Brightness1,
    /// Material Symbols `brightness_2` (U+F036).
    Brightness2,
    /// Material Symbols `brightness_3` (U+E3A8).
    Brightness3,
    /// Material Symbols `brightness_4` (U+E3A9).
    Brightness4,
    /// Material Symbols `brightness_5` (U+E3AA).
    Brightness5,
    /// Material Symbols `brightness_6` (U+E3AB).
    Brightness6,
    /// Material Symbols `brightness_7` (U+E3AC).
    Brightness7,
    /// Material Symbols `brightness_alert` (U+F5CF).
    BrightnessAlert,
    /// Material Symbols `brightness_auto` (U+E1AB).
    BrightnessAuto,
    /// Material Symbols `brightness_empty` (U+F7E8).
    BrightnessEmpty,
    /// Material Symbols `brightness_high` (U+E1AC).
    BrightnessHigh,
    /// Material Symbols `brightness_low` (U+E1AD).
    BrightnessLow,
    /// Material Symbols `brightness_medium` (U+E1AE).
    BrightnessMedium,
    /// Material Symbols `bring_your_own_ip` (U+E016).
    BringYourOwnIp,
    /// Material Symbols `broadcast_on_home` (U+F8F8).
    BroadcastOnHome,
    /// Material Symbols `broadcast_on_personal` (U+F8F9).
    BroadcastOnPersonal,
    /// Material Symbols `broken_image` (U+E3AD).
    BrokenImage,
    /// Material Symbols `browse` (U+EB13).
    Browse,
    /// Material Symbols `browse_activity` (U+F8A5).
    BrowseActivity,
    /// Material Symbols `browse_gallery` (U+EBD1).
    BrowseGallery,
    /// Material Symbols `browser_not_supported` (U+EF47).
    BrowserNotSupported,
    /// Material Symbols `browser_updated` (U+E7CF).
    BrowserUpdated,
    /// Material Symbols `brunch_dining` (U+EA73).
    BrunchDining,
    /// Material Symbols `brush` (U+E3AE).
    Brush,
    /// Material Symbols `bubble` (U+EF83).
    Bubble,
    /// Material Symbols `bubble_chart` (U+E6DD).
    BubbleChart,
    /// Material Symbols `bubbles` (U+F64E).
    Bubbles,
    /// Material Symbols `bucket_check` (U+EF2A).
    BucketCheck,
    /// Material Symbols `bug_report` (U+E868).
    BugReport,
    /// Material Symbols `build` (U+F8CD).
    Build,
    /// Material Symbols `build_circle` (U+EF48).
    BuildCircle,
    /// Material Symbols `bullet_chart` (U+FFEC7).
    BulletChart,
    /// Material Symbols `bungalow` (U+E591).
    Bungalow,
    /// Material Symbols `burst_mode` (U+E43C).
    BurstMode,
    /// Material Symbols `bus_alert` (U+E98F).
    BusAlert,
    /// Material Symbols `bus_map_pin` (U+FFFA2).
    BusMapPin,
    /// Material Symbols `bus_railway` (U+F36B).
    BusRailway,
    /// Material Symbols `business` (U+E7EE).
    Business,
    /// Material Symbols `business_center` (U+EB3F).
    BusinessCenter,
    /// Material Symbols `business_chip` (U+F84C).
    BusinessChip,
    /// Material Symbols `business_messages` (U+EF84).
    BusinessMessages,
    /// Material Symbols `buttons_alt` (U+E72F).
    ButtonsAlt,
    /// Material Symbols `cabin` (U+E589).
    Cabin,
    /// Material Symbols `cable` (U+EFE6).
    Cable,
    /// Material Symbols `cable_car` (U+F479).
    CableCar,
    /// Material Symbols `cached` (U+E86A).
    Cached,
    /// Material Symbols `cadence` (U+F4B4).
    Cadence,
    /// Material Symbols `cake` (U+E7E9).
    Cake,
    /// Material Symbols `cake_add` (U+F85B).
    CakeAdd,
    /// Material Symbols `calculate` (U+EA5F).
    Calculate,
    /// Material Symbols `calendar_add_on` (U+EF85).
    CalendarAddOn,
    /// Material Symbols `calendar_apps_script` (U+F0BB).
    CalendarAppsScript,
    /// Material Symbols `calendar_check` (U+F243).
    CalendarCheck,
    /// Material Symbols `calendar_clock` (U+F540).
    CalendarClock,
    /// Material Symbols `calendar_lock` (U+F242).
    CalendarLock,
    /// Material Symbols `calendar_meal` (U+F296).
    CalendarMeal,
    /// Material Symbols `calendar_meal_2` (U+F240).
    CalendarMeal2,
    /// Material Symbols `calendar_month` (U+EBCC).
    CalendarMonth,
    /// Material Symbols `calendar_today` (U+E935).
    CalendarToday,
    /// Material Symbols `calendar_view_day` (U+E936).
    CalendarViewDay,
    /// Material Symbols `calendar_view_month` (U+EFE7).
    CalendarViewMonth,
    /// Material Symbols `calendar_view_week` (U+EFE8).
    CalendarViewWeek,
    /// Material Symbols `call_end` (U+F0BC).
    CallEnd,
    /// Material Symbols `call_end_alt` (U+F0BC).
    CallEndAlt,
    /// Material Symbols `call_log` (U+E08E).
    CallLog,
    /// Material Symbols `call_made` (U+E0B2).
    CallMade,
    /// Material Symbols `call_merge` (U+E0B3).
    CallMerge,
    /// Material Symbols `call_missed` (U+E0B4).
    CallMissed,
    /// Material Symbols `call_missed_outgoing` (U+E0E4).
    CallMissedOutgoing,
    /// Material Symbols `call_quality` (U+F652).
    CallQuality,
    /// Material Symbols `call_received` (U+E0B5).
    CallReceived,
    /// Material Symbols `call_split` (U+E0B6).
    CallSplit,
    /// Material Symbols `call_to_action` (U+E06C).
    CallToAction,
    /// Material Symbols `camera` (U+E3AF).
    Camera,
    /// Material Symbols `camera_alt` (U+E412).
    CameraAlt,
    /// Material Symbols `camera_enhance` (U+E8FC).
    CameraEnhance,
    /// Material Symbols `camera_front` (U+F2C9).
    CameraFront,
    /// Material Symbols `camera_indoor` (U+EFE9).
    CameraIndoor,
    /// Material Symbols `camera_outdoor` (U+EFEA).
    CameraOutdoor,
    /// Material Symbols `camera_rear` (U+F2C8).
    CameraRear,
    /// Material Symbols `camera_roll` (U+E3B3).
    CameraRoll,
    /// Material Symbols `camera_video` (U+F7A6).
    CameraVideo,
    /// Material Symbols `cameraswitch` (U+EFEB).
    Cameraswitch,
    /// Material Symbols `campaign` (U+EF49).
    Campaign,
    /// Material Symbols `camping` (U+F8A2).
    Camping,
    /// Material Symbols `cancel` (U+E888).
    Cancel,
    /// Material Symbols `cancel_presentation` (U+E0E9).
    CancelPresentation,
    /// Material Symbols `cancel_schedule_send` (U+EA39).
    CancelScheduleSend,
    /// Material Symbols `candle` (U+F588).
    Candle,
    /// Material Symbols `candlestick_chart` (U+EAD4).
    CandlestickChart,
    /// Material Symbols `cannabis` (U+F2F3).
    Cannabis,
    /// Material Symbols `captive_portal` (U+F728).
    CaptivePortal,
    /// Material Symbols `capture` (U+F727).
    Capture,
    /// Material Symbols `car_crash` (U+EBF2).
    CarCrash,
    /// Material Symbols `car_defrost_left` (U+F344).
    CarDefrostLeft,
    /// Material Symbols `car_defrost_low_left` (U+F343).
    CarDefrostLowLeft,
    /// Material Symbols `car_defrost_low_right` (U+F342).
    CarDefrostLowRight,
    /// Material Symbols `car_defrost_mid_left` (U+F278).
    CarDefrostMidLeft,
    /// Material Symbols `car_defrost_mid_low_left` (U+F341).
    CarDefrostMidLowLeft,
    /// Material Symbols `car_defrost_mid_low_right` (U+F277).
    CarDefrostMidLowRight,
    /// Material Symbols `car_defrost_mid_right` (U+F340).
    CarDefrostMidRight,
    /// Material Symbols `car_defrost_right` (U+F33F).
    CarDefrostRight,
    /// Material Symbols `car_fan_low_left` (U+F33E).
    CarFanLowLeft,
    /// Material Symbols `car_fan_low_mid_left` (U+F33D).
    CarFanLowMidLeft,
    /// Material Symbols `car_fan_low_right` (U+F33C).
    CarFanLowRight,
    /// Material Symbols `car_fan_mid_left` (U+F33B).
    CarFanMidLeft,
    /// Material Symbols `car_fan_mid_low_right` (U+F33A).
    CarFanMidLowRight,
    /// Material Symbols `car_fan_mid_right` (U+F339).
    CarFanMidRight,
    /// Material Symbols `car_fan_recirculate` (U+F338).
    CarFanRecirculate,
    /// Material Symbols `car_fan_recirculate_2` (U+FFF40).
    CarFanRecirculate2,
    /// Material Symbols `car_gear` (U+F337).
    CarGear,
    /// Material Symbols `car_lock` (U+F336).
    CarLock,
    /// Material Symbols `car_mirror_heat` (U+F335).
    CarMirrorHeat,
    /// Material Symbols `car_rental` (U+EA55).
    CarRental,
    /// Material Symbols `car_repair` (U+EA56).
    CarRepair,
    /// Material Symbols `car_seat_off` (U+FFEBE).
    CarSeatOff,
    /// Material Symbols `car_tag` (U+F4E3).
    CarTag,
    /// Material Symbols `card_giftcard` (U+E8F6).
    CardGiftcard,
    /// Material Symbols `card_membership` (U+E8F7).
    CardMembership,
    /// Material Symbols `card_travel` (U+E8F8).
    CardTravel,
    /// Material Symbols `cardio_load` (U+F4B9).
    CardioLoad,
    /// Material Symbols `cardiology` (U+E09C).
    Cardiology,
    /// Material Symbols `cards` (U+E991).
    Cards,
    /// Material Symbols `cards_stack` (U+F38F).
    CardsStack,
    /// Material Symbols `cards_star` (U+F375).
    CardsStar,
    /// Material Symbols `carpenter` (U+F1F8).
    Carpenter,
    /// Material Symbols `carry_on_bag` (U+EB08).
    CarryOnBag,
    /// Material Symbols `carry_on_bag_checked` (U+EB0B).
    CarryOnBagChecked,
    /// Material Symbols `carry_on_bag_inactive` (U+EB0A).
    CarryOnBagInactive,
    /// Material Symbols `carry_on_bag_question` (U+EB09).
    CarryOnBagQuestion,
    /// Material Symbols `cases` (U+E992).
    Cases,
    /// Material Symbols `casino` (U+EB40).
    Casino,
    /// Material Symbols `cast` (U+E307).
    Cast,
    /// Material Symbols `cast_connected` (U+E308).
    CastConnected,
    /// Material Symbols `cast_for_education` (U+EFEC).
    CastForEducation,
    /// Material Symbols `cast_pause` (U+F5F0).
    CastPause,
    /// Material Symbols `cast_warning` (U+F5EF).
    CastWarning,
    /// Material Symbols `castle` (U+EAB1).
    Castle,
    /// Material Symbols `category` (U+E72C).
    Category,
    /// Material Symbols `category_search` (U+F437).
    CategorySearch,
    /// Material Symbols `celebration` (U+EA65).
    Celebration,
    /// Material Symbols `cell_merge` (U+F82E).
    CellMerge,
    /// Material Symbols `cell_tower` (U+EBBA).
    CellTower,
    /// Material Symbols `cell_wifi` (U+E0EC).
    CellWifi,
    /// Material Symbols `center_focus_strong` (U+E3B4).
    CenterFocusStrong,
    /// Material Symbols `center_focus_weak` (U+E3B5).
    CenterFocusWeak,
    /// Material Symbols `chair` (U+EFED).
    Chair,
    /// Material Symbols `chair_alt` (U+EFEE).
    ChairAlt,
    /// Material Symbols `chair_counter` (U+F29F).
    ChairCounter,
    /// Material Symbols `chair_fireplace` (U+F29E).
    ChairFireplace,
    /// Material Symbols `chair_umbrella` (U+F29D).
    ChairUmbrella,
    /// Material Symbols `chalet` (U+E585).
    Chalet,
    /// Material Symbols `change_circle` (U+E2E7).
    ChangeCircle,
    /// Material Symbols `change_history` (U+E86B).
    ChangeHistory,
    /// Material Symbols `charger` (U+E2AE).
    Charger,
    /// Material Symbols `charging_station` (U+F2E3).
    ChargingStation,
    /// Material Symbols `chart_data` (U+E473).
    ChartData,
    /// Material Symbols `chat_add_on` (U+F0F3).
    ChatAddOn,
    /// Material Symbols `chat_apps_script` (U+F0BD).
    ChatAppsScript,
    /// Material Symbols `chat_bubble` (U+E0CB).
    ChatBubble,
    /// Material Symbols `chat_bubble_off` (U+FFFBB).
    ChatBubbleOff,
    /// Material Symbols `chat_bubble_outline` (U+E0CB).
    ChatBubbleOutline,
    /// Material Symbols `chat_dashed` (U+EEED).
    ChatDashed,
    /// Material Symbols `chat_display` (U+FFEAC).
    ChatDisplay,
    /// Material Symbols `chat_error` (U+F7AC).
    ChatError,
    /// Material Symbols `chat_info` (U+F52B).
    ChatInfo,
    /// Material Symbols `chat_paste_go` (U+F6BD).
    ChatPasteGo,
    /// Material Symbols `chat_paste_go_2` (U+F3CB).
    ChatPasteGo2,
    /// Material Symbols `check_alert` (U+FFF85).
    CheckAlert,
    /// Material Symbols `check_box` (U+E9DE).
    CheckBox,
    /// Material Symbols `check_box_outline_blank` (U+E835).
    CheckBoxOutlineBlank,
    /// Material Symbols `check_circle` (U+F0BE).
    CheckCircle,
    /// Material Symbols `check_circle_filled` (U+F0BE).
    CheckCircleFilled,
    /// Material Symbols `check_circle_outline` (U+F0BE).
    CheckCircleOutline,
    /// Material Symbols `check_circle_unread` (U+F27E).
    CheckCircleUnread,
    /// Material Symbols `check_in_out` (U+F6F6).
    CheckInOut,
    /// Material Symbols `check_indeterminate_small` (U+F88A).
    CheckIndeterminateSmall,
    /// Material Symbols `check_small` (U+F88B).
    CheckSmall,
    /// Material Symbols `checkbook` (U+E70D).
    Checkbook,
    /// Material Symbols `checked_bag` (U+EB0C).
    CheckedBag,
    /// Material Symbols `checked_bag_question` (U+EB0D).
    CheckedBagQuestion,
    /// Material Symbols `checklist` (U+E6B1).
    Checklist,
    /// Material Symbols `checklist_rtl` (U+E6B3).
    ChecklistRtl,
    /// Material Symbols `checkroom` (U+F19E).
    Checkroom,
    /// Material Symbols `cheer` (U+F6A8).
    Cheer,
    /// Material Symbols `chef_hat` (U+F357).
    ChefHat,
    /// Material Symbols `chess` (U+F5E7).
    Chess,
    /// Material Symbols `chess_bishop` (U+F261).
    ChessBishop,
    /// Material Symbols `chess_bishop_2` (U+F262).
    ChessBishop2,
    /// Material Symbols `chess_king` (U+F25F).
    ChessKing,
    /// Material Symbols `chess_king_2` (U+F260).
    ChessKing2,
    /// Material Symbols `chess_knight` (U+F25E).
    ChessKnight,
    /// Material Symbols `chess_pawn` (U+F3B6).
    ChessPawn,
    /// Material Symbols `chess_pawn_2` (U+F25D).
    ChessPawn2,
    /// Material Symbols `chess_queen` (U+F25C).
    ChessQueen,
    /// Material Symbols `chess_rook` (U+F25B).
    ChessRook,
    /// Material Symbols `chevron_backward` (U+F46B).
    ChevronBackward,
    /// Material Symbols `chevron_forward` (U+F46A).
    ChevronForward,
    /// Material Symbols `chevron_line_up` (U+EEC3).
    ChevronLineUp,
    /// Material Symbols `child_care` (U+EB41).
    ChildCare,
    /// Material Symbols `child_friendly` (U+EF80).
    ChildFriendly,
    /// Material Symbols `child_hat` (U+EF30).
    ChildHat,
    /// Material Symbols `chip_extraction` (U+F821).
    ChipExtraction,
    /// Material Symbols `chips` (U+E993).
    Chips,
    /// Material Symbols `chrome_reader_mode` (U+E86D).
    ChromeReaderMode,
    /// Material Symbols `chromecast_2` (U+F17B).
    Chromecast2,
    /// Material Symbols `chromecast_device` (U+E83C).
    ChromecastDevice,
    /// Material Symbols `chronic` (U+EBB2).
    Chronic,
    /// Material Symbols `church` (U+EAAE).
    Church,
    /// Material Symbols `cinematic_blur` (U+F853).
    CinematicBlur,
    /// Material Symbols `circle` (U+EF4A).
    Circle,
    /// Material Symbols `circle_circle` (U+EEE1).
    CircleCircle,
    /// Material Symbols `circle_notifications` (U+E994).
    CircleNotifications,
    /// Material Symbols `circles` (U+E7EA).
    Circles,
    /// Material Symbols `circles_ext` (U+E7EC).
    CirclesExt,
    /// Material Symbols `clarify` (U+F0BF).
    Clarify,
    /// Material Symbols `class` (U+E86E).
    Class,
    /// Material Symbols `clean_hands` (U+F21F).
    CleanHands,
    /// Material Symbols `cleaning` (U+E995).
    Cleaning,
    /// Material Symbols `cleaning_bucket` (U+F8B4).
    CleaningBucket,
    /// Material Symbols `cleaning_services` (U+F0FF).
    CleaningServices,
    /// Material Symbols `clear` (U+E5CD).
    Clear,
    /// Material Symbols `clear_all` (U+E0B8).
    ClearAll,
    /// Material Symbols `clear_day` (U+F157).
    ClearDay,
    /// Material Symbols `clear_night` (U+F159).
    ClearNight,
    /// Material Symbols `climate_mini_split` (U+F8B5).
    ClimateMiniSplit,
    /// Material Symbols `clinical_notes` (U+E09E).
    ClinicalNotes,
    /// Material Symbols `clock_arrow_down` (U+F382).
    ClockArrowDown,
    /// Material Symbols `clock_arrow_up` (U+F381).
    ClockArrowUp,
    /// Material Symbols `clock_loader_10` (U+F726).
    ClockLoader10,
    /// Material Symbols `clock_loader_20` (U+F725).
    ClockLoader20,
    /// Material Symbols `clock_loader_40` (U+F724).
    ClockLoader40,
    /// Material Symbols `clock_loader_60` (U+F723).
    ClockLoader60,
    /// Material Symbols `clock_loader_80` (U+F722).
    ClockLoader80,
    /// Material Symbols `clock_loader_90` (U+F721).
    ClockLoader90,
    /// Material Symbols `close_fullscreen` (U+F1CF).
    CloseFullscreen,
    /// Material Symbols `close_small` (U+F508).
    CloseSmall,
    /// Material Symbols `closed_caption` (U+E996).
    ClosedCaption,
    /// Material Symbols `closed_caption_add` (U+F4AE).
    ClosedCaptionAdd,
    /// Material Symbols `closed_caption_disabled` (U+F1DC).
    ClosedCaptionDisabled,
    /// Material Symbols `closed_caption_display` (U+FFEAB).
    ClosedCaptionDisplay,
    /// Material Symbols `closed_caption_off` (U+E996).
    ClosedCaptionOff,
    /// Material Symbols `cloud` (U+F15C).
    Cloud,
    /// Material Symbols `cloud_alert` (U+F3CC).
    CloudAlert,
    /// Material Symbols `cloud_circle` (U+E2BE).
    CloudCircle,
    /// Material Symbols `cloud_done` (U+E2BF).
    CloudDone,
    /// Material Symbols `cloud_download` (U+E2C0).
    CloudDownload,
    /// Material Symbols `cloud_lock` (U+F386).
    CloudLock,
    /// Material Symbols `cloud_off` (U+E2C1).
    CloudOff,
    /// Material Symbols `cloud_queue` (U+F15C).
    CloudQueue,
    /// Material Symbols `cloud_sync` (U+EB5A).
    CloudSync,
    /// Material Symbols `cloud_upload` (U+E2C3).
    CloudUpload,
    /// Material Symbols `cloudy` (U+F15C).
    Cloudy,
    /// Material Symbols `cloudy_filled` (U+F15C).
    CloudyFilled,
    /// Material Symbols `cloudy_snowing` (U+E810).
    CloudySnowing,
    /// Material Symbols `co2` (U+E7B0).
    Co2,
    /// Material Symbols `co_present` (U+EAF0).
    CoPresent,
    /// Material Symbols `code` (U+E86F).
    Code,
    /// Material Symbols `code_blocks` (U+F84D).
    CodeBlocks,
    /// Material Symbols `code_off` (U+E4F3).
    CodeOff,
    /// Material Symbols `code_xml` (U+FFF8B).
    CodeXml,
    /// Material Symbols `coffee` (U+EFEF).
    Coffee,
    /// Material Symbols `coffee_maker` (U+EFF0).
    CoffeeMaker,
    /// Material Symbols `cognition` (U+E09F).
    Cognition,
    /// Material Symbols `cognition_2` (U+F3B5).
    Cognition2,
    /// Material Symbols `collapse_all` (U+E944).
    CollapseAll,
    /// Material Symbols `collapse_content` (U+F507).
    CollapseContent,
    /// Material Symbols `collections` (U+E3D3).
    Collections,
    /// Material Symbols `collections_bookmark` (U+E431).
    CollectionsBookmark,
    /// Material Symbols `color_lens` (U+E40A).
    ColorLens,
    /// Material Symbols `colorize` (U+E3B8).
    Colorize,
    /// Material Symbols `colors` (U+E997).
    Colors,
    /// Material Symbols `combine_columns` (U+F420).
    CombineColumns,
    /// Material Symbols `comedy_mask` (U+F4D6).
    ComedyMask,
    /// Material Symbols `comic_bubble` (U+F5DD).
    ComicBubble,
    /// Material Symbols `comment` (U+E24C).
    Comment,
    /// Material Symbols `comment_bank` (U+EA4E).
    CommentBank,
    /// Material Symbols `comments_disabled` (U+E7A2).
    CommentsDisabled,
    /// Material Symbols `commit` (U+EAF5).
    Commit,
    /// Material Symbols `communication` (U+E27C).
    Communication,
    /// Material Symbols `communities` (U+EB16).
    Communities,
    /// Material Symbols `communities_filled` (U+EB16).
    CommunitiesFilled,
    /// Material Symbols `commute` (U+E940).
    Commute,
    /// Material Symbols `compare` (U+E3B9).
    Compare,
    /// Material Symbols `compare_arrows` (U+E915).
    CompareArrows,
    /// Material Symbols `compass_calibration` (U+E57C).
    CompassCalibration,
    /// Material Symbols `component_exchange` (U+F1E7).
    ComponentExchange,
    /// Material Symbols `compost` (U+E761).
    Compost,
    /// Material Symbols `compress` (U+E94D).
    Compress,
    /// Material Symbols `computer` (U+E31E).
    Computer,
    /// Material Symbols `computer_arrow_up` (U+F2F7).
    ComputerArrowUp,
    /// Material Symbols `computer_cancel` (U+F2F6).
    ComputerCancel,
    /// Material Symbols `computer_sound` (U+EEB4).
    ComputerSound,
    /// Material Symbols `concierge` (U+F561).
    Concierge,
    /// Material Symbols `conditions` (U+E0A0).
    Conditions,
    /// Material Symbols `confirmation_number` (U+E638).
    ConfirmationNumber,
    /// Material Symbols `congenital` (U+E0A1).
    Congenital,
    /// Material Symbols `connect_without_contact` (U+F223).
    ConnectWithoutContact,
    /// Material Symbols `connected_tv` (U+E998).
    ConnectedTv,
    /// Material Symbols `connecting_airports` (U+E7C9).
    ConnectingAirports,
    /// Material Symbols `construction` (U+EA3C).
    Construction,
    /// Material Symbols `contact_emergency` (U+F8D1).
    ContactEmergency,
    /// Material Symbols `contact_mail` (U+E0D0).
    ContactMail,
    /// Material Symbols `contact_page` (U+F22E).
    ContactPage,
    /// Material Symbols `contact_phone` (U+F0C0).
    ContactPhone,
    /// Material Symbols `contact_phone_filled` (U+F0C0).
    ContactPhoneFilled,
    /// Material Symbols `contact_support` (U+E94C).
    ContactSupport,
    /// Material Symbols `contactless` (U+EA71).
    Contactless,
    /// Material Symbols `contactless_off` (U+F858).
    ContactlessOff,
    /// Material Symbols `contacts` (U+E0BA).
    Contacts,
    /// Material Symbols `contacts_product` (U+E999).
    ContactsProduct,
    /// Material Symbols `content_copy` (U+E14D).
    ContentCopy,
    /// Material Symbols `content_cut` (U+E14E).
    ContentCut,
    /// Material Symbols `content_paste` (U+E14F).
    ContentPaste,
    /// Material Symbols `content_paste_go` (U+EA8E).
    ContentPasteGo,
    /// Material Symbols `content_paste_off` (U+E4F8).
    ContentPasteOff,
    /// Material Symbols `content_paste_search` (U+EA9B).
    ContentPasteSearch,
    /// Material Symbols `contextual_token` (U+F486).
    ContextualToken,
    /// Material Symbols `contextual_token_add` (U+F485).
    ContextualTokenAdd,
    /// Material Symbols `contract` (U+F5A0).
    Contract,
    /// Material Symbols `contract_delete` (U+F5A2).
    ContractDelete,
    /// Material Symbols `contract_edit` (U+F5A1).
    ContractEdit,
    /// Material Symbols `contrast` (U+EB37).
    Contrast,
    /// Material Symbols `contrast_circle` (U+F49F).
    ContrastCircle,
    /// Material Symbols `contrast_rtl_off` (U+EC72).
    ContrastRtlOff,
    /// Material Symbols `contrast_square` (U+F4A0).
    ContrastSquare,
    /// Material Symbols `control_camera` (U+E074).
    ControlCamera,
    /// Material Symbols `control_point` (U+E990).
    ControlPoint,
    /// Material Symbols `control_point_duplicate` (U+E3BB).
    ControlPointDuplicate,
    /// Material Symbols `controller_gen` (U+E83D).
    ControllerGen,
    /// Material Symbols `conversation` (U+EF2F).
    Conversation,
    /// Material Symbols `conversion_path` (U+F0C1).
    ConversionPath,
    /// Material Symbols `conversion_path_off` (U+F7B4).
    ConversionPathOff,
    /// Material Symbols `convert_to_text` (U+F41F).
    ConvertToText,
    /// Material Symbols `conveyor_belt` (U+F867).
    ConveyorBelt,
    /// Material Symbols `cookie` (U+EAAC).
    Cookie,
    /// Material Symbols `cookie_off` (U+F79A).
    CookieOff,
    /// Material Symbols `cooking` (U+E2B6).
    Cooking,
    /// Material Symbols `cool_to_dry` (U+E276).
    CoolToDry,
    /// Material Symbols `copy_all` (U+E2EC).
    CopyAll,
    /// Material Symbols `copyright` (U+E90C).
    Copyright,
    /// Material Symbols `coronavirus` (U+F221).
    Coronavirus,
    /// Material Symbols `corporate_fare` (U+F1D0).
    CorporateFare,
    /// Material Symbols `cottage` (U+E587).
    Cottage,
    /// Material Symbols `counter_0` (U+F785).
    Counter0,
    /// Material Symbols `counter_1` (U+F784).
    Counter1,
    /// Material Symbols `counter_2` (U+F783).
    Counter2,
    /// Material Symbols `counter_3` (U+F782).
    Counter3,
    /// Material Symbols `counter_4` (U+F781).
    Counter4,
    /// Material Symbols `counter_5` (U+F780).
    Counter5,
    /// Material Symbols `counter_6` (U+F77F).
    Counter6,
    /// Material Symbols `counter_7` (U+F77E).
    Counter7,
    /// Material Symbols `counter_8` (U+F77D).
    Counter8,
    /// Material Symbols `counter_9` (U+F77C).
    Counter9,
    /// Material Symbols `countertops` (U+F1F7).
    Countertops,
    /// Material Symbols `create` (U+F097).
    Create,
    /// Material Symbols `create_new_folder` (U+E2CC).
    CreateNewFolder,
    /// Material Symbols `credit_card` (U+E8A1).
    CreditCard,
    /// Material Symbols `credit_card_clock` (U+F438).
    CreditCardClock,
    /// Material Symbols `credit_card_gear` (U+F52D).
    CreditCardGear,
    /// Material Symbols `credit_card_heart` (U+F52C).
    CreditCardHeart,
    /// Material Symbols `credit_card_off` (U+E4F4).
    CreditCardOff,
    /// Material Symbols `credit_score` (U+EFF1).
    CreditScore,
    /// Material Symbols `crib` (U+E588).
    Crib,
    /// Material Symbols `crisis_alert` (U+EBE9).
    CrisisAlert,
    /// Material Symbols `crop` (U+E3BE).
    Crop,
    /// Material Symbols `crop_16_9` (U+E3BC).
    Crop169,
    /// Material Symbols `crop_21_9` (U+FFF0A).
    Crop219,
    /// Material Symbols `crop_2_3` (U+FFF0B).
    Crop23,
    /// Material Symbols `crop_3_2` (U+E3BD).
    Crop32,
    /// Material Symbols `crop_5_4` (U+E3BF).
    Crop54,
    /// Material Symbols `crop_7_5` (U+E3C0).
    Crop75,
    /// Material Symbols `crop_9_16` (U+F549).
    Crop916,
    /// Material Symbols `crop_din` (U+E3C6).
    CropDin,
    /// Material Symbols `crop_free` (U+E3C2).
    CropFree,
    /// Material Symbols `crop_landscape` (U+E3C3).
    CropLandscape,
    /// Material Symbols `crop_original` (U+E3F4).
    CropOriginal,
    /// Material Symbols `crop_portrait` (U+E3C5).
    CropPortrait,
    /// Material Symbols `crop_rotate` (U+E437).
    CropRotate,
    /// Material Symbols `crop_square` (U+E3C6).
    CropSquare,
    /// Material Symbols `crossword` (U+F5E5).
    Crossword,
    /// Material Symbols `crowdsource` (U+EB18).
    Crowdsource,
    /// Material Symbols `crown` (U+ECB3).
    Crown,
    /// Material Symbols `cruelty_free` (U+E799).
    CrueltyFree,
    /// Material Symbols `css` (U+EB93).
    Css,
    /// Material Symbols `csv` (U+E6CF).
    Csv,
    /// Material Symbols `currency_bitcoin` (U+EBC5).
    CurrencyBitcoin,
    /// Material Symbols `currency_exchange` (U+EB70).
    CurrencyExchange,
    /// Material Symbols `currency_franc` (U+EAFA).
    CurrencyFranc,
    /// Material Symbols `currency_lira` (U+EAEF).
    CurrencyLira,
    /// Material Symbols `currency_pound` (U+EAF1).
    CurrencyPound,
    /// Material Symbols `currency_ruble` (U+EAEC).
    CurrencyRuble,
    /// Material Symbols `currency_rupee` (U+EAF7).
    CurrencyRupee,
    /// Material Symbols `currency_rupee_circle` (U+F460).
    CurrencyRupeeCircle,
    /// Material Symbols `currency_yen` (U+EAFB).
    CurrencyYen,
    /// Material Symbols `currency_yuan` (U+EAF9).
    CurrencyYuan,
    /// Material Symbols `curtains` (U+EC1E).
    Curtains,
    /// Material Symbols `curtains_closed` (U+EC1D).
    CurtainsClosed,
    /// Material Symbols `custom_typography` (U+E732).
    CustomTypography,
    /// Material Symbols `cycle` (U+F854).
    Cycle,
    /// Material Symbols `cyclone` (U+EBD5).
    Cyclone,
    /// Material Symbols `dangerous` (U+E99A).
    Dangerous,
    /// Material Symbols `dark_mode` (U+E51C).
    DarkMode,
    /// Material Symbols `dashboard` (U+E871).
    Dashboard,
    /// Material Symbols `dashboard_2` (U+F3EA).
    Dashboard2,
    /// Material Symbols `dashboard_2_add` (U+FFEE9).
    Dashboard2Add,
    /// Material Symbols `dashboard_2_edit` (U+FFFD7).
    Dashboard2Edit,
    /// Material Symbols `dashboard_2_gear` (U+FFFD6).
    Dashboard2Gear,
    /// Material Symbols `dashboard_customize` (U+E99B).
    DashboardCustomize,
    /// Material Symbols `data_alert` (U+F7F6).
    DataAlert,
    /// Material Symbols `data_array` (U+EAD1).
    DataArray,
    /// Material Symbols `data_check` (U+F7F2).
    DataCheck,
    /// Material Symbols `data_exploration` (U+E76F).
    DataExploration,
    /// Material Symbols `data_info_alert` (U+F7F5).
    DataInfoAlert,
    /// Material Symbols `data_loss_prevention` (U+E2DC).
    DataLossPrevention,
    /// Material Symbols `data_object` (U+EAD3).
    DataObject,
    /// Material Symbols `data_saver_off` (U+EFF2).
    DataSaverOff,
    /// Material Symbols `data_saver_on` (U+EFF3).
    DataSaverOn,
    /// Material Symbols `data_table` (U+E99C).
    DataTable,
    /// Material Symbols `data_thresholding` (U+EB9F).
    DataThresholding,
    /// Material Symbols `data_usage` (U+EFF2).
    DataUsage,
    /// Material Symbols `database` (U+F20E).
    Database,
    /// Material Symbols `database_off` (U+F414).
    DatabaseOff,
    /// Material Symbols `database_search` (U+F38E).
    DatabaseSearch,
    /// Material Symbols `database_upload` (U+F3DC).
    DatabaseUpload,
    /// Material Symbols `dataset` (U+F8EE).
    Dataset,
    /// Material Symbols `dataset_linked` (U+F8EF).
    DatasetLinked,
    /// Material Symbols `date_range` (U+E916).
    DateRange,
    /// Material Symbols `deblur` (U+EB77).
    Deblur,
    /// Material Symbols `deceased` (U+E0A5).
    Deceased,
    /// Material Symbols `decimal_decrease` (U+F82D).
    DecimalDecrease,
    /// Material Symbols `decimal_increase` (U+F82C).
    DecimalIncrease,
    /// Material Symbols `deck` (U+EA42).
    Deck,
    /// Material Symbols `dehaze` (U+E3C7).
    Dehaze,
    /// Material Symbols `delete` (U+E92E).
    Delete,
    /// Material Symbols `delete_forever` (U+E92B).
    DeleteForever,
    /// Material Symbols `delete_history` (U+F518).
    DeleteHistory,
    /// Material Symbols `delete_outline` (U+E92E).
    DeleteOutline,
    /// Material Symbols `delete_sweep` (U+E16C).
    DeleteSweep,
    /// Material Symbols `delivery_dining` (U+EB28).
    DeliveryDining,
    /// Material Symbols `delivery_truck_bolt` (U+F3A2).
    DeliveryTruckBolt,
    /// Material Symbols `delivery_truck_speed` (U+F3A1).
    DeliveryTruckSpeed,
    /// Material Symbols `demography` (U+E489).
    Demography,
    /// Material Symbols `density_large` (U+EBA9).
    DensityLarge,
    /// Material Symbols `density_medium` (U+EB9E).
    DensityMedium,
    /// Material Symbols `density_small` (U+EBA8).
    DensitySmall,
    /// Material Symbols `dentistry` (U+E0A6).
    Dentistry,
    /// Material Symbols `departure_board` (U+E576).
    DepartureBoard,
    /// Material Symbols `deployed_code` (U+F720).
    DeployedCode,
    /// Material Symbols `deployed_code_account` (U+F51B).
    DeployedCodeAccount,
    /// Material Symbols `deployed_code_alert` (U+F5F2).
    DeployedCodeAlert,
    /// Material Symbols `deployed_code_history` (U+F5F3).
    DeployedCodeHistory,
    /// Material Symbols `deployed_code_update` (U+F5F4).
    DeployedCodeUpdate,
    /// Material Symbols `dermatology` (U+E0A7).
    Dermatology,
    /// Material Symbols `description` (U+E873).
    Description,
    /// Material Symbols `deselect` (U+EBB6).
    Deselect,
    /// Material Symbols `design_services` (U+F10A).
    DesignServices,
    /// Material Symbols `desk` (U+F8F4).
    Desk,
    /// Material Symbols `deskphone` (U+F7FA).
    Deskphone,
    /// Material Symbols `desktop_access_disabled` (U+E99D).
    DesktopAccessDisabled,
    /// Material Symbols `desktop_cloud` (U+F3DB).
    DesktopCloud,
    /// Material Symbols `desktop_cloud_stack` (U+F3BE).
    DesktopCloudStack,
    /// Material Symbols `desktop_landscape` (U+F45E).
    DesktopLandscape,
    /// Material Symbols `desktop_landscape_add` (U+F439).
    DesktopLandscapeAdd,
    /// Material Symbols `desktop_mac` (U+E30B).
    DesktopMac,
    /// Material Symbols `desktop_portrait` (U+F45D).
    DesktopPortrait,
    /// Material Symbols `desktop_windows` (U+E30C).
    DesktopWindows,
    /// Material Symbols `destruction` (U+F585).
    Destruction,
    /// Material Symbols `details` (U+E3C8).
    Details,
    /// Material Symbols `detection_and_zone` (U+E29F).
    DetectionAndZone,
    /// Material Symbols `detection_and_zone_off` (U+EEBF).
    DetectionAndZoneOff,
    /// Material Symbols `detector` (U+E282).
    Detector,
    /// Material Symbols `detector_alarm` (U+E1F7).
    DetectorAlarm,
    /// Material Symbols `detector_battery` (U+E204).
    DetectorBattery,
    /// Material Symbols `detector_co` (U+E2AF).
    DetectorCo,
    /// Material Symbols `detector_offline` (U+E223).
    DetectorOffline,
    /// Material Symbols `detector_smoke` (U+E285).
    DetectorSmoke,
    /// Material Symbols `detector_status` (U+E1E8).
    DetectorStatus,
    /// Material Symbols `developer_board` (U+E30D).
    DeveloperBoard,
    /// Material Symbols `developer_board_off` (U+E4FF).
    DeveloperBoardOff,
    /// Material Symbols `developer_guide` (U+E99E).
    DeveloperGuide,
    /// Material Symbols `developer_mode` (U+F2E2).
    DeveloperMode,
    /// Material Symbols `developer_mode_tv` (U+E874).
    DeveloperModeTv,
    /// Material Symbols `device_band` (U+F2F5).
    DeviceBand,
    /// Material Symbols `device_hub` (U+E335).
    DeviceHub,
    /// Material Symbols `device_reset` (U+E8B3).
    DeviceReset,
    /// Material Symbols `device_swoosh_star` (U+FFEB8).
    DeviceSwooshStar,
    /// Material Symbols `device_thermostat` (U+E1FF).
    DeviceThermostat,
    /// Material Symbols `device_unknown` (U+F2E1).
    DeviceUnknown,
    /// Material Symbols `devices` (U+E326).
    Devices,
    /// Material Symbols `devices_fold` (U+EBDE).
    DevicesFold,
    /// Material Symbols `devices_fold_2` (U+F406).
    DevicesFold2,
    /// Material Symbols `devices_off` (U+F7A5).
    DevicesOff,
    /// Material Symbols `devices_other` (U+E337).
    DevicesOther,
    /// Material Symbols `devices_wearables` (U+F6AB).
    DevicesWearables,
    /// Material Symbols `dew_point` (U+F879).
    DewPoint,
    /// Material Symbols `diagnosis` (U+E0A8).
    Diagnosis,
    /// Material Symbols `diagonal_line` (U+F41E).
    DiagonalLine,
    /// Material Symbols `dialer_sip` (U+E0BB).
    DialerSip,
    /// Material Symbols `dialogs` (U+E99F).
    Dialogs,
    /// Material Symbols `dialpad` (U+E0BC).
    Dialpad,
    /// Material Symbols `diamond` (U+EAD5).
    Diamond,
    /// Material Symbols `diamond_shine` (U+F2B2).
    DiamondShine,
    /// Material Symbols `dictionary` (U+F539).
    Dictionary,
    /// Material Symbols `difference` (U+EB7D).
    Difference,
    /// Material Symbols `digital_out_of_home` (U+F1DE).
    DigitalOutOfHome,
    /// Material Symbols `digital_wellbeing` (U+EF86).
    DigitalWellbeing,
    /// Material Symbols `dine_heart` (U+F29C).
    DineHeart,
    /// Material Symbols `dine_in` (U+F295).
    DineIn,
    /// Material Symbols `dine_lamp` (U+F29B).
    DineLamp,
    /// Material Symbols `dining` (U+EFF4).
    Dining,
    /// Material Symbols `dinner_dining` (U+EA57).
    DinnerDining,
    /// Material Symbols `directions` (U+E52E).
    Directions,
    /// Material Symbols `directions_alt` (U+F880).
    DirectionsAlt,
    /// Material Symbols `directions_alt_off` (U+F881).
    DirectionsAltOff,
    /// Material Symbols `directions_bike` (U+E52F).
    DirectionsBike,
    /// Material Symbols `directions_boat` (U+EFF5).
    DirectionsBoat,
    /// Material Symbols `directions_boat_filled` (U+EFF5).
    DirectionsBoatFilled,
    /// Material Symbols `directions_bus` (U+EFF6).
    DirectionsBus,
    /// Material Symbols `directions_bus_filled` (U+EFF6).
    DirectionsBusFilled,
    /// Material Symbols `directions_car` (U+EFF7).
    DirectionsCar,
    /// Material Symbols `directions_car_filled` (U+EFF7).
    DirectionsCarFilled,
    /// Material Symbols `directions_off` (U+F10F).
    DirectionsOff,
    /// Material Symbols `directions_railway` (U+EFF8).
    DirectionsRailway,
    /// Material Symbols `directions_railway_2` (U+F462).
    DirectionsRailway2,
    /// Material Symbols `directions_railway_filled` (U+EFF8).
    DirectionsRailwayFilled,
    /// Material Symbols `directions_run` (U+E566).
    DirectionsRun,
    /// Material Symbols `directions_subway` (U+EFFA).
    DirectionsSubway,
    /// Material Symbols `directions_subway_filled` (U+EFFA).
    DirectionsSubwayFilled,
    /// Material Symbols `directions_transit` (U+EFFA).
    DirectionsTransit,
    /// Material Symbols `directions_transit_filled` (U+EFFA).
    DirectionsTransitFilled,
    /// Material Symbols `directions_walk` (U+E536).
    DirectionsWalk,
    /// Material Symbols `directory_sync` (U+E394).
    DirectorySync,
    /// Material Symbols `dirty_lens` (U+EF4B).
    DirtyLens,
    /// Material Symbols `disabled_by_default` (U+F230).
    DisabledByDefault,
    /// Material Symbols `disabled_visible` (U+E76E).
    DisabledVisible,
    /// Material Symbols `disc_full` (U+E610).
    DiscFull,
    /// Material Symbols `discover_tune` (U+E018).
    DiscoverTune,
    /// Material Symbols `dishwasher` (U+E9A0).
    Dishwasher,
    /// Material Symbols `dishwasher_gen` (U+E832).
    DishwasherGen,
    /// Material Symbols `display_add` (U+FFED2).
    DisplayAdd,
    /// Material Symbols `display_external_input` (U+F7E7).
    DisplayExternalInput,
    /// Material Symbols `display_group` (U+FFEAA).
    DisplayGroup,
    /// Material Symbols `display_settings` (U+EB97).
    DisplaySettings,
    /// Material Symbols `distance` (U+F6EA).
    Distance,
    /// Material Symbols `diversity_1` (U+F8D7).
    Diversity1,
    /// Material Symbols `diversity_2` (U+F8D8).
    Diversity2,
    /// Material Symbols `diversity_3` (U+F8D9).
    Diversity3,
    /// Material Symbols `diversity_4` (U+F857).
    Diversity4,
    /// Material Symbols `dns` (U+E875).
    Dns,
    /// Material Symbols `do_disturb` (U+F08C).
    DoDisturb,
    /// Material Symbols `do_disturb_alt` (U+F08D).
    DoDisturbAlt,
    /// Material Symbols `do_disturb_off` (U+F08E).
    DoDisturbOff,
    /// Material Symbols `do_disturb_on` (U+F08F).
    DoDisturbOn,
    /// Material Symbols `do_not_disturb` (U+F08D).
    DoNotDisturb,
    /// Material Symbols `do_not_disturb_alt` (U+F08C).
    DoNotDisturbAlt,
    /// Material Symbols `do_not_disturb_off` (U+F08E).
    DoNotDisturbOff,
    /// Material Symbols `do_not_disturb_on` (U+F08F).
    DoNotDisturbOn,
    /// Material Symbols `do_not_disturb_on_total_silence` (U+EFFB).
    DoNotDisturbOnTotalSilence,
    /// Material Symbols `do_not_step` (U+F19F).
    DoNotStep,
    /// Material Symbols `do_not_touch` (U+F1B0).
    DoNotTouch,
    /// Material Symbols `dock` (U+F2E0).
    Dock,
    /// Material Symbols `dock_to_bottom` (U+F7E6).
    DockToBottom,
    /// Material Symbols `dock_to_left` (U+F7E5).
    DockToLeft,
    /// Material Symbols `dock_to_right` (U+F7E4).
    DockToRight,
    /// Material Symbols `docs` (U+EA7D).
    Docs,
    /// Material Symbols `docs_add_on` (U+F0C2).
    DocsAddOn,
    /// Material Symbols `docs_apps_script` (U+F0C3).
    DocsAppsScript,
    /// Material Symbols `document_scanner` (U+E5FA).
    DocumentScanner,
    /// Material Symbols `document_search` (U+F385).
    DocumentSearch,
    /// Material Symbols `document_share` (U+FFEA6).
    DocumentShare,
    /// Material Symbols `domain` (U+E7EE).
    Domain,
    /// Material Symbols `domain_add` (U+EB62).
    DomainAdd,
    /// Material Symbols `domain_disabled` (U+E0EF).
    DomainDisabled,
    /// Material Symbols `domain_disabled_check` (U+FFEC6).
    DomainDisabledCheck,
    /// Material Symbols `domain_verification` (U+EF4C).
    DomainVerification,
    /// Material Symbols `domain_verification_off` (U+F7B0).
    DomainVerificationOff,
    /// Material Symbols `domino_mask` (U+F5E4).
    DominoMask,
    /// Material Symbols `done` (U+E876).
    Done,
    /// Material Symbols `done_all` (U+E877).
    DoneAll,
    /// Material Symbols `done_outline` (U+E92F).
    DoneOutline,
    /// Material Symbols `donut_large` (U+E917).
    DonutLarge,
    /// Material Symbols `donut_small` (U+E918).
    DonutSmall,
    /// Material Symbols `door_back` (U+EFFC).
    DoorBack,
    /// Material Symbols `door_front` (U+EFFD).
    DoorFront,
    /// Material Symbols `door_open` (U+E77C).
    DoorOpen,
    /// Material Symbols `door_sensor` (U+E28A).
    DoorSensor,
    /// Material Symbols `door_sliding` (U+EFFE).
    DoorSliding,
    /// Material Symbols `doorbell` (U+EFFF).
    Doorbell,
    /// Material Symbols `doorbell_3p` (U+E1E7).
    Doorbell3p,
    /// Material Symbols `doorbell_chime` (U+E1F3).
    DoorbellChime,
    /// Material Symbols `double_arrow` (U+EA50).
    DoubleArrow,
    /// Material Symbols `downhill_skiing` (U+E509).
    DownhillSkiing,
    /// Material Symbols `download_2` (U+F523).
    Download2,
    /// Material Symbols `download_done` (U+F091).
    DownloadDone,
    /// Material Symbols `download_for_offline` (U+F000).
    DownloadForOffline,
    /// Material Symbols `downloading` (U+F001).
    Downloading,
    /// Material Symbols `draft` (U+E66D).
    Draft,
    /// Material Symbols `draft_orders` (U+E7B3).
    DraftOrders,
    /// Material Symbols `drafts` (U+E151).
    Drafts,
    /// Material Symbols `drag_click` (U+F71F).
    DragClick,
    /// Material Symbols `drag_handle` (U+E25D).
    DragHandle,
    /// Material Symbols `drag_indicator` (U+E945).
    DragIndicator,
    /// Material Symbols `drag_pan` (U+F71E).
    DragPan,
    /// Material Symbols `draw` (U+E746).
    Draw,
    /// Material Symbols `draw_abstract` (U+F7F8).
    DrawAbstract,
    /// Material Symbols `draw_collage` (U+F7F7).
    DrawCollage,
    /// Material Symbols `drawing_recognition` (U+EB00).
    DrawingRecognition,
    /// Material Symbols `dresser` (U+E210).
    Dresser,
    /// Material Symbols `drive_eta` (U+EFF7).
    DriveEta,
    /// Material Symbols `drive_export` (U+F41D).
    DriveExport,
    /// Material Symbols `drive_file_move` (U+E9A1).
    DriveFileMove,
    /// Material Symbols `drive_file_move_outline` (U+E9A1).
    DriveFileMoveOutline,
    /// Material Symbols `drive_file_move_rtl` (U+E9A1).
    DriveFileMoveRtl,
    /// Material Symbols `drive_file_rename` (U+E676).
    DriveFileRename,
    /// Material Symbols `drive_file_rename_outline` (U+E9A2).
    DriveFileRenameOutline,
    /// Material Symbols `drive_folder_upload` (U+E9A3).
    DriveFolderUpload,
    /// Material Symbols `drone` (U+F25A).
    Drone,
    /// Material Symbols `drone_2` (U+F259).
    Drone2,
    /// Material Symbols `dropdown` (U+E9A4).
    Dropdown,
    /// Material Symbols `dropdown_menu` (U+FFEF0).
    DropdownMenu,
    /// Material Symbols `dropper_eye` (U+F351).
    DropperEye,
    /// Material Symbols `dry` (U+F1B3).
    Dry,
    /// Material Symbols `dry_cleaning` (U+EA58).
    DryCleaning,
    /// Material Symbols `dual_screen` (U+F6CF).
    DualScreen,
    /// Material Symbols `duo` (U+E9A5).
    Duo,
    /// Material Symbols `dvr` (U+E1B2).
    Dvr,
    /// Material Symbols `dynamic_feed` (U+EA14).
    DynamicFeed,
    /// Material Symbols `dynamic_form` (U+F1BF).
    DynamicForm,
    /// Material Symbols `e911_avatar` (U+F11A).
    E911Avatar,
    /// Material Symbols `e911_emergency` (U+F119).
    E911Emergency,
    /// Material Symbols `e_mobiledata` (U+F002).
    EMobiledata,
    /// Material Symbols `e_mobiledata_badge` (U+F7E3).
    EMobiledataBadge,
    /// Material Symbols `ear_sound` (U+F356).
    EarSound,
    /// Material Symbols `earbud_case` (U+F327).
    EarbudCase,
    /// Material Symbols `earbud_left` (U+F326).
    EarbudLeft,
    /// Material Symbols `earbud_right` (U+F325).
    EarbudRight,
    /// Material Symbols `earbuds` (U+F003).
    Earbuds,
    /// Material Symbols `earbuds_2` (U+F324).
    Earbuds2,
    /// Material Symbols `earbuds_battery` (U+F004).
    EarbudsBattery,
    /// Material Symbols `early_on` (U+E2BA).
    EarlyOn,
    /// Material Symbols `earthquake` (U+F64F).
    Earthquake,
    /// Material Symbols `east` (U+F1DF).
    East,
    /// Material Symbols `ecg` (U+F80F).
    Ecg,
    /// Material Symbols `ecg_heart` (U+F6E9).
    EcgHeart,
    /// Material Symbols `eco` (U+EA35).
    Eco,
    /// Material Symbols `eda` (U+F6E8).
    Eda,
    /// Material Symbols `edgesensor_high` (U+F2EF).
    EdgesensorHigh,
    /// Material Symbols `edgesensor_low` (U+F2EE).
    EdgesensorLow,
    /// Material Symbols `edit_arrow_down` (U+F380).
    EditArrowDown,
    /// Material Symbols `edit_arrow_up` (U+F37F).
    EditArrowUp,
    /// Material Symbols `edit_attributes` (U+E578).
    EditAttributes,
    /// Material Symbols `edit_audio` (U+F42D).
    EditAudio,
    /// Material Symbols `edit_calendar` (U+E742).
    EditCalendar,
    /// Material Symbols `edit_document` (U+F88C).
    EditDocument,
    /// Material Symbols `edit_line` (U+FFEA5).
    EditLine,
    /// Material Symbols `edit_location` (U+E568).
    EditLocation,
    /// Material Symbols `edit_location_alt` (U+E1C5).
    EditLocationAlt,
    /// Material Symbols `edit_note` (U+E745).
    EditNote,
    /// Material Symbols `edit_notifications` (U+E525).
    EditNotifications,
    /// Material Symbols `edit_off` (U+E950).
    EditOff,
    /// Material Symbols `edit_road` (U+EF4D).
    EditRoad,
    /// Material Symbols `edit_square` (U+F88D).
    EditSquare,
    /// Material Symbols `editor_choice` (U+F528).
    EditorChoice,
    /// Material Symbols `egg` (U+EACC).
    Egg,
    /// Material Symbols `egg_alt` (U+EAC8).
    EggAlt,
    /// Material Symbols `eject` (U+E8FB).
    Eject,
    /// Material Symbols `elderly` (U+F21A).
    Elderly,
    /// Material Symbols `elderly_woman` (U+EB69).
    ElderlyWoman,
    /// Material Symbols `electric_bike` (U+EB1B).
    ElectricBike,
    /// Material Symbols `electric_bolt` (U+EC1C).
    ElectricBolt,
    /// Material Symbols `electric_car` (U+EB1C).
    ElectricCar,
    /// Material Symbols `electric_meter` (U+EC1B).
    ElectricMeter,
    /// Material Symbols `electric_moped` (U+EB1D).
    ElectricMoped,
    /// Material Symbols `electric_rickshaw` (U+EB1E).
    ElectricRickshaw,
    /// Material Symbols `electric_scooter` (U+EB1F).
    ElectricScooter,
    /// Material Symbols `electrical_services` (U+F102).
    ElectricalServices,
    /// Material Symbols `elevation` (U+F6E7).
    Elevation,
    /// Material Symbols `elevator` (U+F1A0).
    Elevator,
    /// Material Symbols `email` (U+E159).
    Email,
    /// Material Symbols `emergency` (U+E1EB).
    Emergency,
    /// Material Symbols `emergency_heat` (U+F15D).
    EmergencyHeat,
    /// Material Symbols `emergency_heat_2` (U+F4E5).
    EmergencyHeat2,
    /// Material Symbols `emergency_home` (U+E82A).
    EmergencyHome,
    /// Material Symbols `emergency_recording` (U+EBF4).
    EmergencyRecording,
    /// Material Symbols `emergency_share` (U+EBF6).
    EmergencyShare,
    /// Material Symbols `emergency_share_off` (U+F59E).
    EmergencyShareOff,
    /// Material Symbols `emoji_emotions` (U+EA22).
    EmojiEmotions,
    /// Material Symbols `emoji_events` (U+EA23).
    EmojiEvents,
    /// Material Symbols `emoji_flags` (U+F0C6).
    EmojiFlags,
    /// Material Symbols `emoji_food_beverage` (U+EA1B).
    EmojiFoodBeverage,
    /// Material Symbols `emoji_language` (U+F4CD).
    EmojiLanguage,
    /// Material Symbols `emoji_nature` (U+EA1C).
    EmojiNature,
    /// Material Symbols `emoji_objects` (U+EA24).
    EmojiObjects,
    /// Material Symbols `emoji_people` (U+EA1D).
    EmojiPeople,
    /// Material Symbols `emoji_symbols` (U+EA1E).
    EmojiSymbols,
    /// Material Symbols `emoji_transportation` (U+EA1F).
    EmojiTransportation,
    /// Material Symbols `emoticon` (U+E5F3).
    Emoticon,
    /// Material Symbols `empty_dashboard` (U+F844).
    EmptyDashboard,
    /// Material Symbols `enable` (U+F188).
    Enable,
    /// Material Symbols `encrypted` (U+E593).
    Encrypted,
    /// Material Symbols `encrypted_add` (U+F429).
    EncryptedAdd,
    /// Material Symbols `encrypted_add_circle` (U+F42A).
    EncryptedAddCircle,
    /// Material Symbols `encrypted_minus_circle` (U+F428).
    EncryptedMinusCircle,
    /// Material Symbols `encrypted_off` (U+F427).
    EncryptedOff,
    /// Material Symbols `endocrinology` (U+E0A9).
    Endocrinology,
    /// Material Symbols `energy` (U+E9A6).
    Energy,
    /// Material Symbols `energy_program_saving` (U+F15F).
    EnergyProgramSaving,
    /// Material Symbols `energy_program_time_used` (U+F161).
    EnergyProgramTimeUsed,
    /// Material Symbols `energy_savings_leaf` (U+EC1A).
    EnergySavingsLeaf,
    /// Material Symbols `engineering` (U+EA3D).
    Engineering,
    /// Material Symbols `enhanced_encryption` (U+E63F).
    EnhancedEncryption,
    /// Material Symbols `ent` (U+E0AA).
    Ent,
    /// Material Symbols `enterprise` (U+E70E).
    Enterprise,
    /// Material Symbols `enterprise_off` (U+EB4D).
    EnterpriseOff,
    /// Material Symbols `equal` (U+F77B).
    Equal,
    /// Material Symbols `equalizer` (U+E01D).
    Equalizer,
    /// Material Symbols `eraser_size_1` (U+F3FC).
    EraserSize1,
    /// Material Symbols `eraser_size_2` (U+F3FB).
    EraserSize2,
    /// Material Symbols `eraser_size_3` (U+F3FA).
    EraserSize3,
    /// Material Symbols `eraser_size_4` (U+F3F9).
    EraserSize4,
    /// Material Symbols `eraser_size_5` (U+F3F8).
    EraserSize5,
    /// Material Symbols `error_circle_rounded` (U+F8B6).
    ErrorCircleRounded,
    /// Material Symbols `error_med` (U+E49B).
    ErrorMed,
    /// Material Symbols `error_outline` (U+F8B6).
    ErrorOutline,
    /// Material Symbols `escalator` (U+F1A1).
    Escalator,
    /// Material Symbols `escalator_warning` (U+F1AC).
    EscalatorWarning,
    /// Material Symbols `euro` (U+EA15).
    Euro,
    /// Material Symbols `euro_symbol` (U+E926).
    EuroSymbol,
    /// Material Symbols `ev_charger` (U+E56D).
    EvCharger,
    /// Material Symbols `ev_mobiledata_badge` (U+F7E2).
    EvMobiledataBadge,
    /// Material Symbols `ev_shadow` (U+EF8F).
    EvShadow,
    /// Material Symbols `ev_shadow_add` (U+F580).
    EvShadowAdd,
    /// Material Symbols `ev_shadow_minus` (U+F57F).
    EvShadowMinus,
    /// Material Symbols `ev_station` (U+E56D).
    EvStation,
    /// Material Symbols `event` (U+E878).
    Event,
    /// Material Symbols `event_available` (U+E614).
    EventAvailable,
    /// Material Symbols `event_busy` (U+E615).
    EventBusy,
    /// Material Symbols `event_list` (U+F683).
    EventList,
    /// Material Symbols `event_note` (U+E616).
    EventNote,
    /// Material Symbols `event_repeat` (U+EB7B).
    EventRepeat,
    /// Material Symbols `event_seat` (U+E903).
    EventSeat,
    /// Material Symbols `event_upcoming` (U+F238).
    EventUpcoming,
    /// Material Symbols `exclamation` (U+F22F).
    Exclamation,
    /// Material Symbols `exercise` (U+F6E6).
    Exercise,
    /// Material Symbols `exit_to_app` (U+E879).
    ExitToApp,
    /// Material Symbols `expand` (U+E94F).
    Expand,
    /// Material Symbols `expand_all` (U+E946).
    ExpandAll,
    /// Material Symbols `expand_circle_down` (U+E7CD).
    ExpandCircleDown,
    /// Material Symbols `expand_circle_right` (U+F591).
    ExpandCircleRight,
    /// Material Symbols `expand_circle_up` (U+F5D2).
    ExpandCircleUp,
    /// Material Symbols `expand_content` (U+F830).
    ExpandContent,
    /// Material Symbols `expand_less` (U+E5CE).
    ExpandLess,
    /// Material Symbols `expand_more` (U+E5CF).
    ExpandMore,
    /// Material Symbols `expansion_panels` (U+EF90).
    ExpansionPanels,
    /// Material Symbols `expension_panels` (U+EF90).
    ExpensionPanels,
    /// Material Symbols `experiment` (U+E686).
    Experiment,
    /// Material Symbols `explicit` (U+E01E).
    Explicit,
    /// Material Symbols `explore` (U+E87A).
    Explore,
    /// Material Symbols `explore_nearby` (U+E538).
    ExploreNearby,
    /// Material Symbols `explore_off` (U+E9A8).
    ExploreOff,
    /// Material Symbols `explosion` (U+F685).
    Explosion,
    /// Material Symbols `export_notes` (U+E0AC).
    ExportNotes,
    /// Material Symbols `exposure` (U+E3F6).
    Exposure,
    /// Material Symbols `exposure_neg_1` (U+E3CB).
    ExposureNeg1,
    /// Material Symbols `exposure_neg_2` (U+E3CC).
    ExposureNeg2,
    /// Material Symbols `exposure_plus_1` (U+E800).
    ExposurePlus1,
    /// Material Symbols `exposure_plus_2` (U+E3CE).
    ExposurePlus2,
    /// Material Symbols `exposure_zero` (U+E3CF).
    ExposureZero,
    /// Material Symbols `extension` (U+E87B).
    Extension,
    /// Material Symbols `extension_off` (U+E4F5).
    ExtensionOff,
    /// Material Symbols `eye_tracking` (U+F4C9).
    EyeTracking,
    /// Material Symbols `eyebrow` (U+EEB3).
    Eyebrow,
    /// Material Symbols `eyeglasses` (U+F6EE).
    Eyeglasses,
    /// Material Symbols `eyeglasses_2` (U+F2C7).
    Eyeglasses2,
    /// Material Symbols `eyeglasses_2_sound` (U+F265).
    Eyeglasses2Sound,
    /// Material Symbols `eyeglasses_3` (U+FFEF1).
    Eyeglasses3,
    /// Material Symbols `face` (U+F008).
    Face,
    /// Material Symbols `face_2` (U+F8DA).
    Face2,
    /// Material Symbols `face_3` (U+F8DB).
    Face3,
    /// Material Symbols `face_4` (U+F8DC).
    Face4,
    /// Material Symbols `face_5` (U+F8DD).
    Face5,
    /// Material Symbols `face_6` (U+F8DE).
    Face6,
    /// Material Symbols `face_down` (U+F402).
    FaceDown,
    /// Material Symbols `face_left` (U+F401).
    FaceLeft,
    /// Material Symbols `face_nod` (U+F400).
    FaceNod,
    /// Material Symbols `face_retouching_natural` (U+EF4E).
    FaceRetouchingNatural,
    /// Material Symbols `face_retouching_off` (U+F007).
    FaceRetouchingOff,
    /// Material Symbols `face_right` (U+F3FF).
    FaceRight,
    /// Material Symbols `face_shake` (U+F3FE).
    FaceShake,
    /// Material Symbols `face_unlock` (U+F008).
    FaceUnlock,
    /// Material Symbols `face_up` (U+F3FD).
    FaceUp,
    /// Material Symbols `fact_check` (U+F0C5).
    FactCheck,
    /// Material Symbols `factory` (U+EBBC).
    Factory,
    /// Material Symbols `falling` (U+F60D).
    Falling,
    /// Material Symbols `familiar_face_and_zone` (U+E21C).
    FamiliarFaceAndZone,
    /// Material Symbols `family_group` (U+EEF2).
    FamilyGroup,
    /// Material Symbols `family_history` (U+E0AD).
    FamilyHistory,
    /// Material Symbols `family_home` (U+EB26).
    FamilyHome,
    /// Material Symbols `family_link` (U+EB19).
    FamilyLink,
    /// Material Symbols `family_restroom` (U+F1A2).
    FamilyRestroom,
    /// Material Symbols `family_star` (U+F527).
    FamilyStar,
    /// Material Symbols `fan_focus` (U+F334).
    FanFocus,
    /// Material Symbols `fan_indirect` (U+F333).
    FanIndirect,
    /// Material Symbols `farsight_digital` (U+F559).
    FarsightDigital,
    /// Material Symbols `fast_forward` (U+E01F).
    FastForward,
    /// Material Symbols `fast_rewind` (U+E020).
    FastRewind,
    /// Material Symbols `fastfood` (U+E57A).
    Fastfood,
    /// Material Symbols `faucet` (U+E278).
    Faucet,
    /// Material Symbols `favorite` (U+E87E).
    Favorite,
    /// Material Symbols `favorite_border` (U+E87E).
    FavoriteBorder,
    /// Material Symbols `fax` (U+EAD8).
    Fax,
    /// Material Symbols `feature_search` (U+E9A9).
    FeatureSearch,
    /// Material Symbols `featured_play_list` (U+E06D).
    FeaturedPlayList,
    /// Material Symbols `featured_seasonal_and_gifts` (U+EF91).
    FeaturedSeasonalAndGifts,
    /// Material Symbols `featured_video` (U+E06E).
    FeaturedVideo,
    /// Material Symbols `feed` (U+F009).
    Feed,
    /// Material Symbols `feedback` (U+E87F).
    Feedback,
    /// Material Symbols `female` (U+E590).
    Female,
    /// Material Symbols `femur` (U+F891).
    Femur,
    /// Material Symbols `femur_alt` (U+F892).
    FemurAlt,
    /// Material Symbols `fence` (U+F1F6).
    Fence,
    /// Material Symbols `fertile` (U+F6E5).
    Fertile,
    /// Material Symbols `festival` (U+EA68).
    Festival,
    /// Material Symbols `fiber_dvr` (U+E05D).
    FiberDvr,
    /// Material Symbols `fiber_manual_record` (U+E061).
    FiberManualRecord,
    /// Material Symbols `fiber_new` (U+E05E).
    FiberNew,
    /// Material Symbols `fiber_pin` (U+E06A).
    FiberPin,
    /// Material Symbols `fiber_smart_record` (U+E062).
    FiberSmartRecord,
    /// Material Symbols `file_copy` (U+E173).
    FileCopy,
    /// Material Symbols `file_copy_off` (U+F4D8).
    FileCopyOff,
    /// Material Symbols `file_download` (U+F090).
    FileDownload,
    /// Material Symbols `file_download_done` (U+F091).
    FileDownloadDone,
    /// Material Symbols `file_download_off` (U+E4FE).
    FileDownloadOff,
    /// Material Symbols `file_export` (U+F3B2).
    FileExport,
    /// Material Symbols `file_json` (U+F3BB).
    FileJson,
    /// Material Symbols `file_map` (U+E2C5).
    FileMap,
    /// Material Symbols `file_map_stack` (U+F3E2).
    FileMapStack,
    /// Material Symbols `file_open` (U+EAF3).
    FileOpen,
    /// Material Symbols `file_png` (U+F3BC).
    FilePng,
    /// Material Symbols `file_present` (U+EA0E).
    FilePresent,
    /// Material Symbols `file_save` (U+F17F).
    FileSave,
    /// Material Symbols `file_save_off` (U+E505).
    FileSaveOff,
    /// Material Symbols `file_upload` (U+F09B).
    FileUpload,
    /// Material Symbols `file_upload_off` (U+F886).
    FileUploadOff,
    /// Material Symbols `files` (U+EA85).
    Files,
    /// Material Symbols `filter_1` (U+E3D0).
    Filter1,
    /// Material Symbols `filter_2` (U+E3D1).
    Filter2,
    /// Material Symbols `filter_3` (U+E3D2).
    Filter3,
    /// Material Symbols `filter_4` (U+E3D4).
    Filter4,
    /// Material Symbols `filter_5` (U+E3D5).
    Filter5,
    /// Material Symbols `filter_6` (U+E3D6).
    Filter6,
    /// Material Symbols `filter_7` (U+E3D7).
    Filter7,
    /// Material Symbols `filter_8` (U+E3D8).
    Filter8,
    /// Material Symbols `filter_9` (U+E3D9).
    Filter9,
    /// Material Symbols `filter_9_plus` (U+E3DA).
    Filter9Plus,
    /// Material Symbols `filter_alt` (U+EF4F).
    FilterAlt,
    /// Material Symbols `filter_alt_off` (U+EB32).
    FilterAltOff,
    /// Material Symbols `filter_arrow_right` (U+F3D1).
    FilterArrowRight,
    /// Material Symbols `filter_b_and_w` (U+E3DB).
    FilterBAndW,
    /// Material Symbols `filter_cancel` (U+FFEA4).
    FilterCancel,
    /// Material Symbols `filter_center_focus` (U+E3DC).
    FilterCenterFocus,
    /// Material Symbols `filter_drama` (U+E3DD).
    FilterDrama,
    /// Material Symbols `filter_frames` (U+E3DE).
    FilterFrames,
    /// Material Symbols `filter_hdr` (U+E3DF).
    FilterHdr,
    /// Material Symbols `filter_list` (U+E152).
    FilterList,
    /// Material Symbols `filter_list_alt` (U+E94E).
    FilterListAlt,
    /// Material Symbols `filter_list_off` (U+EB57).
    FilterListOff,
    /// Material Symbols `filter_none` (U+E3E0).
    FilterNone,
    /// Material Symbols `filter_plus` (U+FFEA3).
    FilterPlus,
    /// Material Symbols `filter_retrolux` (U+E3E1).
    FilterRetrolux,
    /// Material Symbols `filter_tilt_shift` (U+E3E2).
    FilterTiltShift,
    /// Material Symbols `filter_vintage` (U+E3E3).
    FilterVintage,
    /// Material Symbols `finance` (U+E6BF).
    Finance,
    /// Material Symbols `finance_chip` (U+F84E).
    FinanceChip,
    /// Material Symbols `finance_mode` (U+EF92).
    FinanceMode,
    /// Material Symbols `find_in_page` (U+E880).
    FindInPage,
    /// Material Symbols `find_replace` (U+E881).
    FindReplace,
    /// Material Symbols `fingerprint` (U+E90D).
    Fingerprint,
    /// Material Symbols `fingerprint_off` (U+F49D).
    FingerprintOff,
    /// Material Symbols `fire_check` (U+FFFA8).
    FireCheck,
    /// Material Symbols `fire_extinguisher` (U+F1D8).
    FireExtinguisher,
    /// Material Symbols `fire_hydrant` (U+F1A3).
    FireHydrant,
    /// Material Symbols `fire_truck` (U+F8F2).
    FireTruck,
    /// Material Symbols `fireplace` (U+EA43).
    Fireplace,
    /// Material Symbols `fit_page` (U+F77A).
    FitPage,
    /// Material Symbols `fit_page_height` (U+F397).
    FitPageHeight,
    /// Material Symbols `fit_page_width` (U+F396).
    FitPageWidth,
    /// Material Symbols `fit_screen` (U+EA10).
    FitScreen,
    /// Material Symbols `fit_width` (U+F779).
    FitWidth,
    /// Material Symbols `fitness_center` (U+EB43).
    FitnessCenter,
    /// Material Symbols `fitness_tracker` (U+F463).
    FitnessTracker,
    /// Material Symbols `fitness_trackers` (U+EEF1).
    FitnessTrackers,
    /// Material Symbols `flag` (U+F0C6).
    Flag,
    /// Material Symbols `flag_2` (U+F40F).
    Flag2,
    /// Material Symbols `flag_check` (U+F3D8).
    FlagCheck,
    /// Material Symbols `flag_circle` (U+EAF8).
    FlagCircle,
    /// Material Symbols `flag_filled` (U+F0C6).
    FlagFilled,
    /// Material Symbols `flaky` (U+EF50).
    Flaky,
    /// Material Symbols `flare` (U+E3E4).
    Flare,
    /// Material Symbols `flash_auto` (U+E3E5).
    FlashAuto,
    /// Material Symbols `flash_off` (U+E3E6).
    FlashOff,
    /// Material Symbols `flash_on` (U+E3E7).
    FlashOn,
    /// Material Symbols `flashlight_off` (U+F00A).
    FlashlightOff,
    /// Material Symbols `flashlight_on` (U+F00B).
    FlashlightOn,
    /// Material Symbols `flatware` (U+F00C).
    Flatware,
    /// Material Symbols `flex_direction` (U+F778).
    FlexDirection,
    /// Material Symbols `flex_no_wrap` (U+F777).
    FlexNoWrap,
    /// Material Symbols `flex_wrap` (U+F776).
    FlexWrap,
    /// Material Symbols `flight` (U+E539).
    Flight,
    /// Material Symbols `flight_class` (U+E7CB).
    FlightClass,
    /// Material Symbols `flight_land` (U+E904).
    FlightLand,
    /// Material Symbols `flight_takeoff` (U+E905).
    FlightTakeoff,
    /// Material Symbols `flights_and_hotels` (U+E9AB).
    FlightsAndHotels,
    /// Material Symbols `flightsmode` (U+EF93).
    Flightsmode,
    /// Material Symbols `flip` (U+E3E8).
    Flip,
    /// Material Symbols `flip_camera_android` (U+EA37).
    FlipCameraAndroid,
    /// Material Symbols `flip_camera_ios` (U+EA38).
    FlipCameraIos,
    /// Material Symbols `flip_to_back` (U+E882).
    FlipToBack,
    /// Material Symbols `flip_to_front` (U+E883).
    FlipToFront,
    /// Material Symbols `float_landscape_2` (U+F45C).
    FloatLandscape2,
    /// Material Symbols `float_portrait_2` (U+F45B).
    FloatPortrait2,
    /// Material Symbols `flood` (U+EBE6).
    Flood,
    /// Material Symbols `floor` (U+F6E4).
    Floor,
    /// Material Symbols `floor_lamp` (U+E21E).
    FloorLamp,
    /// Material Symbols `flourescent` (U+F07D).
    Flourescent,
    /// Material Symbols `flowchart` (U+F38D).
    Flowchart,
    /// Material Symbols `flowsheet` (U+E0AE).
    Flowsheet,
    /// Material Symbols `fluid` (U+E483).
    Fluid,
    /// Material Symbols `fluid_balance` (U+F80D).
    FluidBalance,
    /// Material Symbols `fluid_med` (U+F80C).
    FluidMed,
    /// Material Symbols `fluorescent` (U+F07D).
    Fluorescent,
    /// Material Symbols `flutter` (U+F1DD).
    Flutter,
    /// Material Symbols `flutter_dash` (U+E00B).
    FlutterDash,
    /// Material Symbols `flyover` (U+F478).
    Flyover,
    /// Material Symbols `fmd_bad` (U+F00E).
    FmdBad,
    /// Material Symbols `fmd_good` (U+F1DB).
    FmdGood,
    /// Material Symbols `foggy` (U+E818).
    Foggy,
    /// Material Symbols `folded_hands` (U+F5ED).
    FoldedHands,
    /// Material Symbols `folder_check` (U+F3D7).
    FolderCheck,
    /// Material Symbols `folder_check_2` (U+F3D6).
    FolderCheck2,
    /// Material Symbols `folder_code` (U+F3C8).
    FolderCode,
    /// Material Symbols `folder_copy` (U+EBBD).
    FolderCopy,
    /// Material Symbols `folder_data` (U+F586).
    FolderData,
    /// Material Symbols `folder_delete` (U+EB34).
    FolderDelete,
    /// Material Symbols `folder_eye` (U+F3D5).
    FolderEye,
    /// Material Symbols `folder_info` (U+F395).
    FolderInfo,
    /// Material Symbols `folder_limited` (U+F4E4).
    FolderLimited,
    /// Material Symbols `folder_managed` (U+F775).
    FolderManaged,
    /// Material Symbols `folder_match` (U+F3D4).
    FolderMatch,
    /// Material Symbols `folder_off` (U+EB83).
    FolderOff,
    /// Material Symbols `folder_shared` (U+E2C9).
    FolderShared,
    /// Material Symbols `folder_special` (U+E617).
    FolderSpecial,
    /// Material Symbols `folder_supervised` (U+F774).
    FolderSupervised,
    /// Material Symbols `folder_zip` (U+EB2C).
    FolderZip,
    /// Material Symbols `follow_the_signs` (U+F222).
    FollowTheSigns,
    /// Material Symbols `font_download` (U+E167).
    FontDownload,
    /// Material Symbols `font_download_off` (U+E4F9).
    FontDownloadOff,
    /// Material Symbols `food_bank` (U+F1F2).
    FoodBank,
    /// Material Symbols `foot_bones` (U+F893).
    FootBones,
    /// Material Symbols `footprint` (U+F87D).
    Footprint,
    /// Material Symbols `for_you` (U+E9AC).
    ForYou,
    /// Material Symbols `forest` (U+EA99).
    Forest,
    /// Material Symbols `fork_chart` (U+FFFA6).
    ForkChart,
    /// Material Symbols `fork_left` (U+EBA0).
    ForkLeft,
    /// Material Symbols `fork_right` (U+EBAC).
    ForkRight,
    /// Material Symbols `fork_spoon` (U+F3E4).
    ForkSpoon,
    /// Material Symbols `forklift` (U+F868).
    Forklift,
    /// Material Symbols `format_align_center` (U+E234).
    FormatAlignCenter,
    /// Material Symbols `format_align_justify` (U+E235).
    FormatAlignJustify,
    /// Material Symbols `format_align_left` (U+E236).
    FormatAlignLeft,
    /// Material Symbols `format_align_right` (U+E237).
    FormatAlignRight,
    /// Material Symbols `format_bold` (U+E238).
    FormatBold,
    /// Material Symbols `format_clear` (U+E239).
    FormatClear,
    /// Material Symbols `format_color_fill` (U+E23A).
    FormatColorFill,
    /// Material Symbols `format_color_reset` (U+E23B).
    FormatColorReset,
    /// Material Symbols `format_color_text` (U+E23C).
    FormatColorText,
    /// Material Symbols `format_h1` (U+F85D).
    FormatH1,
    /// Material Symbols `format_h2` (U+F85E).
    FormatH2,
    /// Material Symbols `format_h3` (U+F85F).
    FormatH3,
    /// Material Symbols `format_h4` (U+F860).
    FormatH4,
    /// Material Symbols `format_h5` (U+F861).
    FormatH5,
    /// Material Symbols `format_h6` (U+F862).
    FormatH6,
    /// Material Symbols `format_image_back` (U+EEB0).
    FormatImageBack,
    /// Material Symbols `format_image_break_left` (U+EEAF).
    FormatImageBreakLeft,
    /// Material Symbols `format_image_break_right` (U+EEAE).
    FormatImageBreakRight,
    /// Material Symbols `format_image_front` (U+EEAD).
    FormatImageFront,
    /// Material Symbols `format_image_inline_left` (U+EEAC).
    FormatImageInlineLeft,
    /// Material Symbols `format_image_inline_right` (U+FFFFD).
    FormatImageInlineRight,
    /// Material Symbols `format_image_left` (U+F863).
    FormatImageLeft,
    /// Material Symbols `format_image_right` (U+F864).
    FormatImageRight,
    /// Material Symbols `format_indent_decrease` (U+E23D).
    FormatIndentDecrease,
    /// Material Symbols `format_indent_increase` (U+E23E).
    FormatIndentIncrease,
    /// Material Symbols `format_ink_highlighter` (U+F82B).
    FormatInkHighlighter,
    /// Material Symbols `format_italic` (U+E23F).
    FormatItalic,
    /// Material Symbols `format_letter_spacing` (U+F773).
    FormatLetterSpacing,
    /// Material Symbols `format_letter_spacing_2` (U+F618).
    FormatLetterSpacing2,
    /// Material Symbols `format_letter_spacing_standard` (U+F617).
    FormatLetterSpacingStandard,
    /// Material Symbols `format_letter_spacing_wide` (U+F616).
    FormatLetterSpacingWide,
    /// Material Symbols `format_letter_spacing_wider` (U+F615).
    FormatLetterSpacingWider,
    /// Material Symbols `format_line_spacing` (U+E240).
    FormatLineSpacing,
    /// Material Symbols `format_list_bulleted` (U+E241).
    FormatListBulleted,
    /// Material Symbols `format_list_bulleted_add` (U+F849).
    FormatListBulletedAdd,
    /// Material Symbols `format_list_numbered` (U+E242).
    FormatListNumbered,
    /// Material Symbols `format_list_numbered_rtl` (U+E267).
    FormatListNumberedRtl,
    /// Material Symbols `format_overline` (U+EB65).
    FormatOverline,
    /// Material Symbols `format_paint` (U+E243).
    FormatPaint,
    /// Material Symbols `format_paint_off` (U+FFF97).
    FormatPaintOff,
    /// Material Symbols `format_paragraph` (U+F865).
    FormatParagraph,
    /// Material Symbols `format_quote` (U+E244).
    FormatQuote,
    /// Material Symbols `format_quote_off` (U+F413).
    FormatQuoteOff,
    /// Material Symbols `format_shapes` (U+E25E).
    FormatShapes,
    /// Material Symbols `format_size` (U+E245).
    FormatSize,
    /// Material Symbols `format_strikethrough` (U+E246).
    FormatStrikethrough,
    /// Material Symbols `format_text_clip` (U+F82A).
    FormatTextClip,
    /// Material Symbols `format_text_overflow` (U+F829).
    FormatTextOverflow,
    /// Material Symbols `format_text_wrap` (U+F828).
    FormatTextWrap,
    /// Material Symbols `format_textdirection_l_to_r` (U+E247).
    FormatTextdirectionLToR,
    /// Material Symbols `format_textdirection_r_to_l` (U+E248).
    FormatTextdirectionRToL,
    /// Material Symbols `format_textdirection_vertical` (U+F4B8).
    FormatTextdirectionVertical,
    /// Material Symbols `format_underlined` (U+E249).
    FormatUnderlined,
    /// Material Symbols `format_underlined_squiggle` (U+F885).
    FormatUnderlinedSquiggle,
    /// Material Symbols `forms_add_on` (U+F0C7).
    FormsAddOn,
    /// Material Symbols `forms_apps_script` (U+F0C8).
    FormsAppsScript,
    /// Material Symbols `fort` (U+EAAD).
    Fort,
    /// Material Symbols `forum` (U+E8AF).
    Forum,
    /// Material Symbols `forward` (U+F57A).
    Forward,
    /// Material Symbols `forward_10` (U+E056).
    Forward10,
    /// Material Symbols `forward_30` (U+E057).
    Forward30,
    /// Material Symbols `forward_5` (U+E058).
    Forward5,
    /// Material Symbols `forward_circle` (U+F6F5).
    ForwardCircle,
    /// Material Symbols `forward_media` (U+F6F4).
    ForwardMedia,
    /// Material Symbols `forward_to_inbox` (U+F187).
    ForwardToInbox,
    /// Material Symbols `foundation` (U+F200).
    Foundation,
    /// Material Symbols `fragrance` (U+F345).
    Fragrance,
    /// Material Symbols `frame_bug` (U+EEEF).
    FrameBug,
    /// Material Symbols `frame_exclamation` (U+EEEE).
    FrameExclamation,
    /// Material Symbols `frame_inspect` (U+F772).
    FrameInspect,
    /// Material Symbols `frame_person` (U+F8A6).
    FramePerson,
    /// Material Symbols `frame_person_mic` (U+F4D5).
    FramePersonMic,
    /// Material Symbols `frame_person_off` (U+F7D1).
    FramePersonOff,
    /// Material Symbols `frame_reload` (U+F771).
    FrameReload,
    /// Material Symbols `frame_source` (U+F770).
    FrameSource,
    /// Material Symbols `free_breakfast` (U+EB44).
    FreeBreakfast,
    /// Material Symbols `free_cancellation` (U+E748).
    FreeCancellation,
    /// Material Symbols `front_hand` (U+E769).
    FrontHand,
    /// Material Symbols `front_loader` (U+F869).
    FrontLoader,
    /// Material Symbols `full_coverage` (U+EB12).
    FullCoverage,
    /// Material Symbols `full_hd` (U+F58B).
    FullHd,
    /// Material Symbols `full_stacked_bar_chart` (U+F212).
    FullStackedBarChart,
    /// Material Symbols `fullscreen_exit` (U+E5D1).
    FullscreenExit,
    /// Material Symbols `fullscreen_portrait` (U+F45A).
    FullscreenPortrait,
    /// Material Symbols `function` (U+F866).
    Function,
    /// Material Symbols `function_search` (U+FFEA2).
    FunctionSearch,
    /// Material Symbols `functions` (U+E24A).
    Functions,
    /// Material Symbols `funicular` (U+F477).
    Funicular,
    /// Material Symbols `g_mobiledata` (U+F010).
    GMobiledata,
    /// Material Symbols `g_mobiledata_badge` (U+F7E1).
    GMobiledataBadge,
    /// Material Symbols `g_translate` (U+E927).
    GTranslate,
    /// Material Symbols `gallery_thumbnail` (U+F86F).
    GalleryThumbnail,
    /// Material Symbols `game_bumper_left` (U+EEE0).
    GameBumperLeft,
    /// Material Symbols `game_bumper_right` (U+EEDF).
    GameBumperRight,
    /// Material Symbols `game_button_l` (U+EEDE).
    GameButtonL,
    /// Material Symbols `game_button_l1` (U+EEDD).
    GameButtonL1,
    /// Material Symbols `game_button_l2` (U+EEDC).
    GameButtonL2,
    /// Material Symbols `game_button_r` (U+EEDB).
    GameButtonR,
    /// Material Symbols `game_button_r1` (U+EEDA).
    GameButtonR1,
    /// Material Symbols `game_button_r2` (U+EED9).
    GameButtonR2,
    /// Material Symbols `game_button_zl` (U+EED8).
    GameButtonZl,
    /// Material Symbols `game_button_zr` (U+EED7).
    GameButtonZr,
    /// Material Symbols `game_stick_l3` (U+EED6).
    GameStickL3,
    /// Material Symbols `game_stick_left` (U+EED5).
    GameStickLeft,
    /// Material Symbols `game_stick_r3` (U+EED4).
    GameStickR3,
    /// Material Symbols `game_stick_right` (U+EED3).
    GameStickRight,
    /// Material Symbols `game_trigger_left` (U+EED2).
    GameTriggerLeft,
    /// Material Symbols `game_trigger_right` (U+EED1).
    GameTriggerRight,
    /// Material Symbols `gamepad` (U+E30F).
    Gamepad,
    /// Material Symbols `gamepad_circle_down` (U+EED0).
    GamepadCircleDown,
    /// Material Symbols `gamepad_circle_left` (U+EECF).
    GamepadCircleLeft,
    /// Material Symbols `gamepad_circle_right` (U+EECE).
    GamepadCircleRight,
    /// Material Symbols `gamepad_circle_up` (U+EECD).
    GamepadCircleUp,
    /// Material Symbols `gamepad_down` (U+EECC).
    GamepadDown,
    /// Material Symbols `gamepad_left` (U+EECB).
    GamepadLeft,
    /// Material Symbols `gamepad_right` (U+EECA).
    GamepadRight,
    /// Material Symbols `gamepad_up` (U+EEC9).
    GamepadUp,
    /// Material Symbols `games` (U+E30F).
    Games,
    /// Material Symbols `garage` (U+F011).
    Garage,
    /// Material Symbols `garage_check` (U+F28D).
    GarageCheck,
    /// Material Symbols `garage_door` (U+E714).
    GarageDoor,
    /// Material Symbols `garage_door_open` (U+FFF77).
    GarageDoorOpen,
    /// Material Symbols `garage_home` (U+E82D).
    GarageHome,
    /// Material Symbols `garage_money` (U+F28C).
    GarageMoney,
    /// Material Symbols `garden_cart` (U+F8A9).
    GardenCart,
    /// Material Symbols `gas_meter` (U+EC19).
    GasMeter,
    /// Material Symbols `gastroenterology` (U+E0F1).
    Gastroenterology,
    /// Material Symbols `gate` (U+E277).
    Gate,
    /// Material Symbols `gavel` (U+E90E).
    Gavel,
    /// Material Symbols `general_device` (U+E6DE).
    GeneralDevice,
    /// Material Symbols `generating_tokens` (U+E749).
    GeneratingTokens,
    /// Material Symbols `genetics` (U+E0F3).
    Genetics,
    /// Material Symbols `genres` (U+E6EE).
    Genres,
    /// Material Symbols `gesture` (U+E155).
    Gesture,
    /// Material Symbols `gesture_select` (U+F657).
    GestureSelect,
    /// Material Symbols `get_app` (U+F090).
    GetApp,
    /// Material Symbols `gif` (U+E908).
    Gif,
    /// Material Symbols `gif_2` (U+F40E).
    Gif2,
    /// Material Symbols `gif_box` (U+E7A3).
    GifBox,
    /// Material Symbols `girl` (U+EB68).
    Girl,
    /// Material Symbols `gite` (U+E58B).
    Gite,
    /// Material Symbols `glass_cup` (U+F6E3).
    GlassCup,
    /// Material Symbols `globe` (U+E64C).
    Globe,
    /// Material Symbols `globe_2_cancel` (U+FFFB7).
    Globe2Cancel,
    /// Material Symbols `globe_2_question` (U+FFFB6).
    Globe2Question,
    /// Material Symbols `globe_asia` (U+F799).
    GlobeAsia,
    /// Material Symbols `globe_book` (U+F3C9).
    GlobeBook,
    /// Material Symbols `globe_clock` (U+FFED1).
    GlobeClock,
    /// Material Symbols `globe_location_pin` (U+F35D).
    GlobeLocationPin,
    /// Material Symbols `globe_uk` (U+F798).
    GlobeUk,
    /// Material Symbols `glucose` (U+E4A0).
    Glucose,
    /// Material Symbols `glyphs` (U+F8A3).
    Glyphs,
    /// Material Symbols `go_to_line` (U+F71D).
    GoToLine,
    /// Material Symbols `golf_course` (U+EB45).
    GolfCourse,
    /// Material Symbols `gondola_lift` (U+F476).
    GondolaLift,
    /// Material Symbols `google_home_devices` (U+E715).
    GoogleHomeDevices,
    /// Material Symbols `google_plus_reshare` (U+F57A).
    GooglePlusReshare,
    /// Material Symbols `google_tv_remote` (U+F5DB).
    GoogleTvRemote,
    /// Material Symbols `google_wifi` (U+F579).
    GoogleWifi,
    /// Material Symbols `gpp_bad` (U+F012).
    GppBad,
    /// Material Symbols `gpp_good` (U+F013).
    GppGood,
    /// Material Symbols `gpp_maybe` (U+F014).
    GppMaybe,
    /// Material Symbols `gps_fixed` (U+E55C).
    GpsFixed,
    /// Material Symbols `gps_not_fixed` (U+E1B7).
    GpsNotFixed,
    /// Material Symbols `gps_off` (U+E1B6).
    GpsOff,
    /// Material Symbols `grade` (U+F09A).
    Grade,
    /// Material Symbols `gradient` (U+E3E9).
    Gradient,
    /// Material Symbols `grading` (U+EA4F).
    Grading,
    /// Material Symbols `grain` (U+E3EA).
    Grain,
    /// Material Symbols `graph_1` (U+F3A0).
    Graph1,
    /// Material Symbols `graph_2` (U+F39F).
    Graph2,
    /// Material Symbols `graph_3` (U+F39E).
    Graph3,
    /// Material Symbols `graph_4` (U+F39D).
    Graph4,
    /// Material Symbols `graph_5` (U+F39C).
    Graph5,
    /// Material Symbols `graph_6` (U+F39B).
    Graph6,
    /// Material Symbols `graph_7` (U+F346).
    Graph7,
    /// Material Symbols `graph_8` (U+FFFEC).
    Graph8,
    /// Material Symbols `graphic_eq` (U+E1B8).
    GraphicEq,
    /// Material Symbols `graphic_eq_off` (U+FFF98).
    GraphicEqOff,
    /// Material Symbols `grass` (U+F205).
    Grass,
    /// Material Symbols `grid_3x3` (U+F015).
    Grid3x3,
    /// Material Symbols `grid_3x3_off` (U+F67C).
    Grid3x3Off,
    /// Material Symbols `grid_4x4` (U+F016).
    Grid4x4,
    /// Material Symbols `grid_goldenratio` (U+F017).
    GridGoldenratio,
    /// Material Symbols `grid_guides` (U+F76F).
    GridGuides,
    /// Material Symbols `grid_layout_side` (U+FFF8D).
    GridLayoutSide,
    /// Material Symbols `grid_off` (U+E3EB).
    GridOff,
    /// Material Symbols `grid_on` (U+E3EC).
    GridOn,
    /// Material Symbols `grid_view` (U+E9B0).
    GridView,
    /// Material Symbols `grocery` (U+EF97).
    Grocery,
    /// Material Symbols `group` (U+EA21).
    Group,
    /// Material Symbols `group_add` (U+E7F0).
    GroupAdd,
    /// Material Symbols `group_eye` (U+FFEA1).
    GroupEye,
    /// Material Symbols `group_off` (U+E747).
    GroupOff,
    /// Material Symbols `group_remove` (U+E7AD).
    GroupRemove,
    /// Material Symbols `group_search` (U+F3CE).
    GroupSearch,
    /// Material Symbols `group_work` (U+E886).
    GroupWork,
    /// Material Symbols `grouped_bar_chart` (U+F211).
    GroupedBarChart,
    /// Material Symbols `groups` (U+F233).
    Groups,
    /// Material Symbols `groups_2` (U+F8DF).
    Groups2,
    /// Material Symbols `groups_3` (U+F8E0).
    Groups3,
    /// Material Symbols `guardian` (U+F4C1).
    Guardian,
    /// Material Symbols `gynecology` (U+E0F4).
    Gynecology,
    /// Material Symbols `h_mobiledata` (U+F018).
    HMobiledata,
    /// Material Symbols `h_mobiledata_badge` (U+F7E0).
    HMobiledataBadge,
    /// Material Symbols `h_plus_mobiledata` (U+F019).
    HPlusMobiledata,
    /// Material Symbols `h_plus_mobiledata_badge` (U+F7DF).
    HPlusMobiledataBadge,
    /// Material Symbols `hail` (U+E9B1).
    Hail,
    /// Material Symbols `hallway` (U+E6F8).
    Hallway,
    /// Material Symbols `hanami_dango` (U+F23F).
    HanamiDango,
    /// Material Symbols `hand_bones` (U+F894).
    HandBones,
    /// Material Symbols `hand_gesture` (U+EF9C).
    HandGesture,
    /// Material Symbols `hand_gesture_off` (U+F3F3).
    HandGestureOff,
    /// Material Symbols `hand_meal` (U+F294).
    HandMeal,
    /// Material Symbols `hand_package` (U+F293).
    HandPackage,
    /// Material Symbols `handheld_controller` (U+F4C6).
    HandheldController,
    /// Material Symbols `handshake` (U+EBCB).
    Handshake,
    /// Material Symbols `handwriting_recognition` (U+EB02).
    HandwritingRecognition,
    /// Material Symbols `handyman` (U+F10B).
    Handyman,
    /// Material Symbols `hangout_video` (U+E0C1).
    HangoutVideo,
    /// Material Symbols `hangout_video_off` (U+E0C2).
    HangoutVideoOff,
    /// Material Symbols `hard_disk` (U+F3DA).
    HardDisk,
    /// Material Symbols `hard_drive` (U+F80E).
    HardDrive,
    /// Material Symbols `hard_drive_2` (U+F7A4).
    HardDrive2,
    /// Material Symbols `hardware` (U+EA59).
    Hardware,
    /// Material Symbols `hd` (U+E052).
    Hd,
    /// Material Symbols `hdr_auto` (U+F01A).
    HdrAuto,
    /// Material Symbols `hdr_auto_select` (U+F01B).
    HdrAutoSelect,
    /// Material Symbols `hdr_enhanced_select` (U+EF51).
    HdrEnhancedSelect,
    /// Material Symbols `hdr_off` (U+E3ED).
    HdrOff,
    /// Material Symbols `hdr_off_select` (U+F01C).
    HdrOffSelect,
    /// Material Symbols `hdr_on` (U+E3EE).
    HdrOn,
    /// Material Symbols `hdr_on_select` (U+F01D).
    HdrOnSelect,
    /// Material Symbols `hdr_plus` (U+F01E).
    HdrPlus,
    /// Material Symbols `hdr_plus_off` (U+E3EF).
    HdrPlusOff,
    /// Material Symbols `hdr_strong` (U+E3F1).
    HdrStrong,
    /// Material Symbols `hdr_weak` (U+E3F2).
    HdrWeak,
    /// Material Symbols `head_mounted_device` (U+F4C5).
    HeadMountedDevice,
    /// Material Symbols `headphones` (U+F01F).
    Headphones,
    /// Material Symbols `headphones_battery` (U+F020).
    HeadphonesBattery,
    /// Material Symbols `headset` (U+F01F).
    Headset,
    /// Material Symbols `headset_mic` (U+E311).
    HeadsetMic,
    /// Material Symbols `headset_off` (U+E33A).
    HeadsetOff,
    /// Material Symbols `healing` (U+E3F3).
    Healing,
    /// Material Symbols `health_and_beauty` (U+EF9D).
    HealthAndBeauty,
    /// Material Symbols `health_and_safety` (U+E1D5).
    HealthAndSafety,
    /// Material Symbols `health_cross` (U+F2C3).
    HealthCross,
    /// Material Symbols `health_metrics` (U+F6E2).
    HealthMetrics,
    /// Material Symbols `heap_snapshot_large` (U+F76E).
    HeapSnapshotLarge,
    /// Material Symbols `heap_snapshot_multiple` (U+F76D).
    HeapSnapshotMultiple,
    /// Material Symbols `heap_snapshot_thumbnail` (U+F76C).
    HeapSnapshotThumbnail,
    /// Material Symbols `hearing` (U+E023).
    Hearing,
    /// Material Symbols `hearing_aid` (U+F464).
    HearingAid,
    /// Material Symbols `hearing_aid_disabled` (U+F3B0).
    HearingAidDisabled,
    /// Material Symbols `hearing_aid_disabled_left` (U+F2EC).
    HearingAidDisabledLeft,
    /// Material Symbols `hearing_aid_left` (U+F2ED).
    HearingAidLeft,
    /// Material Symbols `hearing_disabled` (U+F104).
    HearingDisabled,
    /// Material Symbols `heart_broken` (U+EAC2).
    HeartBroken,
    /// Material Symbols `heart_check` (U+F60A).
    HeartCheck,
    /// Material Symbols `heart_minus` (U+F883).
    HeartMinus,
    /// Material Symbols `heart_plus` (U+F884).
    HeartPlus,
    /// Material Symbols `heart_smile` (U+F292).
    HeartSmile,
    /// Material Symbols `heat` (U+F537).
    Heat,
    /// Material Symbols `heat_pump` (U+EC18).
    HeatPump,
    /// Material Symbols `heat_pump_balance` (U+E27E).
    HeatPumpBalance,
    /// Material Symbols `height` (U+EA16).
    Height,
    /// Material Symbols `helicopter` (U+F60C).
    Helicopter,
    /// Material Symbols `help_center` (U+F1C0).
    HelpCenter,
    /// Material Symbols `help_clinic` (U+F810).
    HelpClinic,
    /// Material Symbols `help_outline` (U+E8FD).
    HelpOutline,
    /// Material Symbols `hematology` (U+E0F6).
    Hematology,
    /// Material Symbols `hevc` (U+F021).
    Hevc,
    /// Material Symbols `hexagon` (U+EB39).
    Hexagon,
    /// Material Symbols `hide` (U+EF9E).
    Hide,
    /// Material Symbols `hide_image` (U+F022).
    HideImage,
    /// Material Symbols `hide_source` (U+F023).
    HideSource,
    /// Material Symbols `high_chair` (U+F29A).
    HighChair,
    /// Material Symbols `high_density` (U+F79C).
    HighDensity,
    /// Material Symbols `high_quality` (U+E024).
    HighQuality,
    /// Material Symbols `high_quality_off` (U+FFED6).
    HighQualityOff,
    /// Material Symbols `high_res` (U+F54B).
    HighRes,
    /// Material Symbols `highlight` (U+E25F).
    Highlight,
    /// Material Symbols `highlight_alt` (U+EF52).
    HighlightAlt,
    /// Material Symbols `highlight_keyboard_focus` (U+F510).
    HighlightKeyboardFocus,
    /// Material Symbols `highlight_mouse_cursor` (U+F511).
    HighlightMouseCursor,
    /// Material Symbols `highlight_off` (U+E888).
    HighlightOff,
    /// Material Symbols `highlight_text_cursor` (U+F512).
    HighlightTextCursor,
    /// Material Symbols `highlighter_size_1` (U+F76B).
    HighlighterSize1,
    /// Material Symbols `highlighter_size_2` (U+F76A).
    HighlighterSize2,
    /// Material Symbols `highlighter_size_3` (U+F769).
    HighlighterSize3,
    /// Material Symbols `highlighter_size_4` (U+F768).
    HighlighterSize4,
    /// Material Symbols `highlighter_size_5` (U+F767).
    HighlighterSize5,
    /// Material Symbols `hiking` (U+E50A).
    Hiking,
    /// Material Symbols `history` (U+E8B3).
    History,
    /// Material Symbols `history_2` (U+F3E6).
    History2,
    /// Material Symbols `history_edu` (U+EA3E).
    HistoryEdu,
    /// Material Symbols `history_off` (U+F4DA).
    HistoryOff,
    /// Material Symbols `history_toggle_off` (U+F17D).
    HistoryToggleOff,
    /// Material Symbols `hive` (U+EAA6).
    Hive,
    /// Material Symbols `hls` (U+EB8A).
    Hls,
    /// Material Symbols `hls_off` (U+EB8C).
    HlsOff,
    /// Material Symbols `holiday_village` (U+E58A).
    HolidayVillage,
    /// Material Symbols `home_and_garden` (U+EF9F).
    HomeAndGarden,
    /// Material Symbols `home_app_logo` (U+E295).
    HomeAppLogo,
    /// Material Symbols `home_filled` (U+E9B2).
    HomeFilled,
    /// Material Symbols `home_health` (U+E4B9).
    HomeHealth,
    /// Material Symbols `home_improvement_and_tools` (U+EFA0).
    HomeImprovementAndTools,
    /// Material Symbols `home_iot_device` (U+E283).
    HomeIotDevice,
    /// Material Symbols `home_max` (U+F024).
    HomeMax,
    /// Material Symbols `home_max_dots` (U+E849).
    HomeMaxDots,
    /// Material Symbols `home_mini` (U+F025).
    HomeMini,
    /// Material Symbols `home_pin` (U+F14D).
    HomePin,
    /// Material Symbols `home_repair_service` (U+F100).
    HomeRepairService,
    /// Material Symbols `home_speaker` (U+F11C).
    HomeSpeaker,
    /// Material Symbols `home_storage` (U+F86C).
    HomeStorage,
    /// Material Symbols `home_storage_gear` (U+FFF7E).
    HomeStorageGear,
    /// Material Symbols `home_work` (U+F030).
    HomeWork,
    /// Material Symbols `horizontal_align_center` (U+FFF9C).
    HorizontalAlignCenter,
    /// Material Symbols `horizontal_align_left` (U+FFF9B).
    HorizontalAlignLeft,
    /// Material Symbols `horizontal_align_right` (U+FFF9A).
    HorizontalAlignRight,
    /// Material Symbols `horizontal_distribute` (U+E014).
    HorizontalDistribute,
    /// Material Symbols `horizontal_rule` (U+F108).
    HorizontalRule,
    /// Material Symbols `horizontal_split` (U+E947).
    HorizontalSplit,
    /// Material Symbols `host` (U+F3D9).
    Host,
    /// Material Symbols `hot_tub` (U+EB46).
    HotTub,
    /// Material Symbols `hotel` (U+E549).
    Hotel,
    /// Material Symbols `hotel_class` (U+E743).
    HotelClass,
    /// Material Symbols `hourglass_arrow_down` (U+F37E).
    HourglassArrowDown,
    /// Material Symbols `hourglass_arrow_up` (U+F37D).
    HourglassArrowUp,
    /// Material Symbols `hourglass_bottom` (U+EA5C).
    HourglassBottom,
    /// Material Symbols `hourglass_check` (U+FFFED).
    HourglassCheck,
    /// Material Symbols `hourglass_disabled` (U+EF53).
    HourglassDisabled,
    /// Material Symbols `hourglass_empty` (U+E88B).
    HourglassEmpty,
    /// Material Symbols `hourglass_full` (U+E88C).
    HourglassFull,
    /// Material Symbols `hourglass_pause` (U+F38C).
    HourglassPause,
    /// Material Symbols `hourglass_top` (U+EA5B).
    HourglassTop,
    /// Material Symbols `house` (U+EA44).
    House,
    /// Material Symbols `house_siding` (U+F202).
    HouseSiding,
    /// Material Symbols `house_with_shield` (U+E786).
    HouseWithShield,
    /// Material Symbols `houseboat` (U+E584).
    Houseboat,
    /// Material Symbols `household_supplies` (U+EFA1).
    HouseholdSupplies,
    /// Material Symbols `hov` (U+F475).
    Hov,
    /// Material Symbols `how_to_reg` (U+E174).
    HowToReg,
    /// Material Symbols `how_to_vote` (U+E175).
    HowToVote,
    /// Material Symbols `hr_resting` (U+F6BA).
    HrResting,
    /// Material Symbols `html` (U+EB7E).
    Html,
    /// Material Symbols `http` (U+E902).
    Http,
    /// Material Symbols `https` (U+E899).
    Https,
    /// Material Symbols `hub` (U+E9F4).
    Hub,
    /// Material Symbols `humerus` (U+F895).
    Humerus,
    /// Material Symbols `humerus_alt` (U+F896).
    HumerusAlt,
    /// Material Symbols `humidity_high` (U+F163).
    HumidityHigh,
    /// Material Symbols `humidity_indoor` (U+F558).
    HumidityIndoor,
    /// Material Symbols `humidity_low` (U+F164).
    HumidityLow,
    /// Material Symbols `humidity_mid` (U+F165).
    HumidityMid,
    /// Material Symbols `humidity_percentage` (U+F87E).
    HumidityPercentage,
    /// Material Symbols `hvac` (U+F10E).
    Hvac,
    /// Material Symbols `hvac_max_defrost` (U+F332).
    HvacMaxDefrost,
    /// Material Symbols `ice_skating` (U+E50B).
    IceSkating,
    /// Material Symbols `icecream` (U+EA69).
    Icecream,
    /// Material Symbols `id_card` (U+F4CA).
    IdCard,
    /// Material Symbols `id_card_2` (U+FFEEA).
    IdCard2,
    /// Material Symbols `identity_aware_proxy` (U+E2DD).
    IdentityAwareProxy,
    /// Material Symbols `identity_platform` (U+EBB7).
    IdentityPlatform,
    /// Material Symbols `ifl` (U+E025).
    Ifl,
    /// Material Symbols `iframe` (U+F71B).
    Iframe,
    /// Material Symbols `iframe_off` (U+F71C).
    IframeOff,
    /// Material Symbols `image` (U+E3F4).
    Image,
    /// Material Symbols `image_arrow_up` (U+F317).
    ImageArrowUp,
    /// Material Symbols `image_aspect_ratio` (U+E6A6).
    ImageAspectRatio,
    /// Material Symbols `image_inset` (U+F247).
    ImageInset,
    /// Material Symbols `image_not_supported` (U+F116).
    ImageNotSupported,
    /// Material Symbols `image_search` (U+E43F).
    ImageSearch,
    /// Material Symbols `imagesearch_roller` (U+E9B4).
    ImagesearchRoller,
    /// Material Symbols `imagesmode` (U+EFA2).
    Imagesmode,
    /// Material Symbols `immunology` (U+E0FB).
    Immunology,
    /// Material Symbols `import_contacts` (U+E0E0).
    ImportContacts,
    /// Material Symbols `import_export` (U+E8D5).
    ImportExport,
    /// Material Symbols `import_spark` (U+FFEA9).
    ImportSpark,
    /// Material Symbols `important_devices` (U+E912).
    ImportantDevices,
    /// Material Symbols `in_home_mode` (U+E833).
    InHomeMode,
    /// Material Symbols `inactive_order` (U+E0FC).
    InactiveOrder,
    /// Material Symbols `inbox` (U+E156).
    Inbox,
    /// Material Symbols `inbox_customize` (U+F859).
    InboxCustomize,
    /// Material Symbols `inbox_text` (U+F399).
    InboxText,
    /// Material Symbols `inbox_text_asterisk` (U+F360).
    InboxTextAsterisk,
    /// Material Symbols `inbox_text_person` (U+F35E).
    InboxTextPerson,
    /// Material Symbols `inbox_text_share` (U+F35C).
    InboxTextShare,
    /// Material Symbols `incomplete_circle` (U+E79B).
    IncompleteCircle,
    /// Material Symbols `indeterminate_check_box` (U+E909).
    IndeterminateCheckBox,
    /// Material Symbols `indeterminate_question_box` (U+F56D).
    IndeterminateQuestionBox,
    /// Material Symbols `info_i` (U+F59B).
    InfoI,
    /// Material Symbols `infrared` (U+F87C).
    Infrared,
    /// Material Symbols `ink_eraser` (U+E6D0).
    InkEraser,
    /// Material Symbols `ink_eraser_off` (U+E7E3).
    InkEraserOff,
    /// Material Symbols `ink_highlighter` (U+E6D1).
    InkHighlighter,
    /// Material Symbols `ink_highlighter_move` (U+F524).
    InkHighlighterMove,
    /// Material Symbols `ink_highlighter_off` (U+FFF14).
    InkHighlighterOff,
    /// Material Symbols `ink_marker` (U+E6D2).
    InkMarker,
    /// Material Symbols `ink_pen` (U+E6D3).
    InkPen,
    /// Material Symbols `ink_selection` (U+EF52).
    InkSelection,
    /// Material Symbols `inpatient` (U+E0FE).
    Inpatient,
    /// Material Symbols `input` (U+E890).
    Input,
    /// Material Symbols `input_circle` (U+F71A).
    InputCircle,
    /// Material Symbols `insert_chart` (U+F0CC).
    InsertChart,
    /// Material Symbols `insert_chart_filled` (U+F0CC).
    InsertChartFilled,
    /// Material Symbols `insert_chart_outlined` (U+F0CC).
    InsertChartOutlined,
    /// Material Symbols `insert_comment` (U+E24C).
    InsertComment,
    /// Material Symbols `insert_drive_file` (U+E66D).
    InsertDriveFile,
    /// Material Symbols `insert_emoticon` (U+EA22).
    InsertEmoticon,
    /// Material Symbols `insert_invitation` (U+E878).
    InsertInvitation,
    /// Material Symbols `insert_link` (U+E250).
    InsertLink,
    /// Material Symbols `insert_page_break` (U+EACA).
    InsertPageBreak,
    /// Material Symbols `insert_photo` (U+E3F4).
    InsertPhoto,
    /// Material Symbols `insert_text` (U+F827).
    InsertText,
    /// Material Symbols `insights` (U+F092).
    Insights,
    /// Material Symbols `install_desktop` (U+EB71).
    InstallDesktop,
    /// Material Symbols `install_mobile` (U+F2CD).
    InstallMobile,
    /// Material Symbols `instant_mix` (U+E026).
    InstantMix,
    /// Material Symbols `integration_instructions` (U+EF54).
    IntegrationInstructions,
    /// Material Symbols `interactive_space` (U+F7FF).
    InteractiveSpace,
    /// Material Symbols `interests` (U+E7C8).
    Interests,
    /// Material Symbols `interpreter_mode` (U+E83B).
    InterpreterMode,
    /// Material Symbols `inventory` (U+E179).
    Inventory,
    /// Material Symbols `inventory_2` (U+E1A1).
    Inventory2,
    /// Material Symbols `invert_colors` (U+E891).
    InvertColors,
    /// Material Symbols `invert_colors_off` (U+E0C4).
    InvertColorsOff,
    /// Material Symbols `ios` (U+E027).
    Ios,
    /// Material Symbols `ios_share` (U+E6B8).
    IosShare,
    /// Material Symbols `iron` (U+E583).
    Iron,
    /// Material Symbols `iso` (U+E3F6).
    Iso,
    /// Material Symbols `jamboard_kiosk` (U+E9B5).
    JamboardKiosk,
    /// Material Symbols `japanese_curry` (U+F284).
    JapaneseCurry,
    /// Material Symbols `japanese_flag` (U+F283).
    JapaneseFlag,
    /// Material Symbols `javascript` (U+EB7C).
    Javascript,
    /// Material Symbols `jewelry` (U+FFEDB).
    Jewelry,
    /// Material Symbols `join` (U+F84F).
    Join,
    /// Material Symbols `join_full` (U+F84F).
    JoinFull,
    /// Material Symbols `join_inner` (U+EAF4).
    JoinInner,
    /// Material Symbols `join_left` (U+EAF2).
    JoinLeft,
    /// Material Symbols `join_right` (U+EAEA).
    JoinRight,
    /// Material Symbols `joystick` (U+F5EE).
    Joystick,
    /// Material Symbols `jump_to_element` (U+F719).
    JumpToElement,
    /// Material Symbols `kanji_alcohol` (U+F23E).
    KanjiAlcohol,
    /// Material Symbols `kayaking` (U+E50C).
    Kayaking,
    /// Material Symbols `kebab_dining` (U+E842).
    KebabDining,
    /// Material Symbols `keep` (U+F027).
    Keep,
    /// Material Symbols `keep_off` (U+E6F9).
    KeepOff,
    /// Material Symbols `keep_pin` (U+F027).
    KeepPin,
    /// Material Symbols `keep_public` (U+F56F).
    KeepPublic,
    /// Material Symbols `kettle` (U+E2B9).
    Kettle,
    /// Material Symbols `key` (U+E73C).
    Key,
    /// Material Symbols `key_off` (U+EB84).
    KeyOff,
    /// Material Symbols `key_vertical` (U+F51A).
    KeyVertical,
    /// Material Symbols `key_visualizer` (U+F199).
    KeyVisualizer,
    /// Material Symbols `keyboard` (U+E312).
    Keyboard,
    /// Material Symbols `keyboard_alt` (U+F028).
    KeyboardAlt,
    /// Material Symbols `keyboard_arrow_down` (U+E313).
    KeyboardArrowDown,
    /// Material Symbols `keyboard_arrow_left` (U+E314).
    KeyboardArrowLeft,
    /// Material Symbols `keyboard_arrow_right` (U+E315).
    KeyboardArrowRight,
    /// Material Symbols `keyboard_arrow_up` (U+E316).
    KeyboardArrowUp,
    /// Material Symbols `keyboard_backspace` (U+E317).
    KeyboardBackspace,
    /// Material Symbols `keyboard_capslock` (U+E318).
    KeyboardCapslock,
    /// Material Symbols `keyboard_capslock_badge` (U+F7DE).
    KeyboardCapslockBadge,
    /// Material Symbols `keyboard_command_key` (U+EAE7).
    KeyboardCommandKey,
    /// Material Symbols `keyboard_control_key` (U+EAE6).
    KeyboardControlKey,
    /// Material Symbols `keyboard_double_arrow_down` (U+EAD0).
    KeyboardDoubleArrowDown,
    /// Material Symbols `keyboard_double_arrow_left` (U+EAC3).
    KeyboardDoubleArrowLeft,
    /// Material Symbols `keyboard_double_arrow_right` (U+EAC9).
    KeyboardDoubleArrowRight,
    /// Material Symbols `keyboard_double_arrow_up` (U+EACF).
    KeyboardDoubleArrowUp,
    /// Material Symbols `keyboard_external_input` (U+F7DD).
    KeyboardExternalInput,
    /// Material Symbols `keyboard_full` (U+F7DC).
    KeyboardFull,
    /// Material Symbols `keyboard_hide` (U+E31A).
    KeyboardHide,
    /// Material Symbols `keyboard_keys` (U+F67B).
    KeyboardKeys,
    /// Material Symbols `keyboard_lock` (U+F492).
    KeyboardLock,
    /// Material Symbols `keyboard_lock_off` (U+F491).
    KeyboardLockOff,
    /// Material Symbols `keyboard_off` (U+F67A).
    KeyboardOff,
    /// Material Symbols `keyboard_onscreen` (U+F7DB).
    KeyboardOnscreen,
    /// Material Symbols `keyboard_option_key` (U+EAE8).
    KeyboardOptionKey,
    /// Material Symbols `keyboard_previous_language` (U+F7DA).
    KeyboardPreviousLanguage,
    /// Material Symbols `keyboard_return` (U+E31B).
    KeyboardReturn,
    /// Material Symbols `keyboard_tab` (U+E31C).
    KeyboardTab,
    /// Material Symbols `keyboard_tab_rtl` (U+EC73).
    KeyboardTabRtl,
    /// Material Symbols `keyboard_voice` (U+E31D).
    KeyboardVoice,
    /// Material Symbols `kid_star` (U+F526).
    KidStar,
    /// Material Symbols `king_bed` (U+EA45).
    KingBed,
    /// Material Symbols `kitchen` (U+EB47).
    Kitchen,
    /// Material Symbols `kitesurfing` (U+E50D).
    Kitesurfing,
    /// Material Symbols `lab_panel` (U+E103).
    LabPanel,
    /// Material Symbols `lab_profile` (U+E104).
    LabProfile,
    /// Material Symbols `lab_research` (U+F80B).
    LabResearch,
    /// Material Symbols `label` (U+E893).
    Label,
    /// Material Symbols `label_important` (U+E948).
    LabelImportant,
    /// Material Symbols `label_important_outline` (U+E948).
    LabelImportantOutline,
    /// Material Symbols `label_off` (U+E9B6).
    LabelOff,
    /// Material Symbols `label_outline` (U+E893).
    LabelOutline,
    /// Material Symbols `labs` (U+E105).
    Labs,
    /// Material Symbols `lan` (U+EB2F).
    Lan,
    /// Material Symbols `landscape` (U+E564).
    Landscape,
    /// Material Symbols `landscape_2` (U+F4C4).
    Landscape2,
    /// Material Symbols `landscape_2_edit` (U+F310).
    Landscape2Edit,
    /// Material Symbols `landscape_2_off` (U+F4C3).
    Landscape2Off,
    /// Material Symbols `landslide` (U+EBD7).
    Landslide,
    /// Material Symbols `language` (U+EA07).
    Language,
    /// Material Symbols `language_chinese_array` (U+F766).
    LanguageChineseArray,
    /// Material Symbols `language_chinese_cangjie` (U+F765).
    LanguageChineseCangjie,
    /// Material Symbols `language_chinese_dayi` (U+F764).
    LanguageChineseDayi,
    /// Material Symbols `language_chinese_pinyin` (U+F763).
    LanguageChinesePinyin,
    /// Material Symbols `language_chinese_quick` (U+F762).
    LanguageChineseQuick,
    /// Material Symbols `language_chinese_wubi` (U+F761).
    LanguageChineseWubi,
    /// Material Symbols `language_french` (U+F760).
    LanguageFrench,
    /// Material Symbols `language_gb_english` (U+F75F).
    LanguageGbEnglish,
    /// Material Symbols `language_international` (U+F75E).
    LanguageInternational,
    /// Material Symbols `language_japanese_kana` (U+F513).
    LanguageJapaneseKana,
    /// Material Symbols `language_korean_latin` (U+F75D).
    LanguageKoreanLatin,
    /// Material Symbols `language_pinyin` (U+F75C).
    LanguagePinyin,
    /// Material Symbols `language_spanish` (U+F5E9).
    LanguageSpanish,
    /// Material Symbols `language_us` (U+F759).
    LanguageUs,
    /// Material Symbols `language_us_colemak` (U+F75B).
    LanguageUsColemak,
    /// Material Symbols `language_us_dvorak` (U+F75A).
    LanguageUsDvorak,
    /// Material Symbols `laps` (U+F6B9).
    Laps,
    /// Material Symbols `laptop` (U+E31E).
    Laptop,
    /// Material Symbols `laptop_car` (U+F3CD).
    LaptopCar,
    /// Material Symbols `laptop_chromebook` (U+E31F).
    LaptopChromebook,
    /// Material Symbols `laptop_mac` (U+E320).
    LaptopMac,
    /// Material Symbols `laptop_windows` (U+E321).
    LaptopWindows,
    /// Material Symbols `lasso_select` (U+EB03).
    LassoSelect,
    /// Material Symbols `launch` (U+E89E).
    Launch,
    /// Material Symbols `laundry` (U+E2A8).
    Laundry,
    /// Material Symbols `layers` (U+E53B).
    Layers,
    /// Material Symbols `layers_clear` (U+E53C).
    LayersClear,
    /// Material Symbols `lda` (U+E106).
    Lda,
    /// Material Symbols `leaderboard` (U+F20C).
    Leaderboard,
    /// Material Symbols `leak_add` (U+E3F8).
    LeakAdd,
    /// Material Symbols `leak_remove` (U+E3F9).
    LeakRemove,
    /// Material Symbols `left_click` (U+F718).
    LeftClick,
    /// Material Symbols `left_panel_close` (U+F717).
    LeftPanelClose,
    /// Material Symbols `left_panel_open` (U+F716).
    LeftPanelOpen,
    /// Material Symbols `legend_toggle` (U+F11B).
    LegendToggle,
    /// Material Symbols `lens` (U+E3FA).
    Lens,
    /// Material Symbols `lens_blur` (U+F029).
    LensBlur,
    /// Material Symbols `letter_switch` (U+F758).
    LetterSwitch,
    /// Material Symbols `library_add` (U+E03C).
    LibraryAdd,
    /// Material Symbols `library_add_check` (U+E9B7).
    LibraryAddCheck,
    /// Material Symbols `library_books` (U+E02F).
    LibraryBooks,
    /// Material Symbols `library_music` (U+E030).
    LibraryMusic,
    /// Material Symbols `license` (U+EB04).
    License,
    /// Material Symbols `lift_to_talk` (U+EFA3).
    LiftToTalk,
    /// Material Symbols `light` (U+F02A).
    Light,
    /// Material Symbols `light_group` (U+E28B).
    LightGroup,
    /// Material Symbols `light_group_2` (U+FFF76).
    LightGroup2,
    /// Material Symbols `light_mode` (U+E518).
    LightMode,
    /// Material Symbols `light_mode_auto` (U+FFF00).
    LightModeAuto,
    /// Material Symbols `light_off` (U+E9B8).
    LightOff,
    /// Material Symbols `lightbulb` (U+E90F).
    Lightbulb,
    /// Material Symbols `lightbulb_2` (U+F3E3).
    Lightbulb2,
    /// Material Symbols `lightbulb_circle` (U+EBFE).
    LightbulbCircle,
    /// Material Symbols `lightbulb_outline` (U+E90F).
    LightbulbOutline,
    /// Material Symbols `lightning_stand` (U+EFA4).
    LightningStand,
    /// Material Symbols `lightstrip` (U+FFF75).
    Lightstrip,
    /// Material Symbols `line_axis` (U+EA9A).
    LineAxis,
    /// Material Symbols `line_curve` (U+F757).
    LineCurve,
    /// Material Symbols `line_end` (U+F826).
    LineEnd,
    /// Material Symbols `line_end_arrow` (U+F81D).
    LineEndArrow,
    /// Material Symbols `line_end_arrow_notch` (U+F81C).
    LineEndArrowNotch,
    /// Material Symbols `line_end_circle` (U+F81B).
    LineEndCircle,
    /// Material Symbols `line_end_diamond` (U+F81A).
    LineEndDiamond,
    /// Material Symbols `line_end_square` (U+F819).
    LineEndSquare,
    /// Material Symbols `line_start` (U+F825).
    LineStart,
    /// Material Symbols `line_start_arrow` (U+F818).
    LineStartArrow,
    /// Material Symbols `line_start_arrow_notch` (U+F817).
    LineStartArrowNotch,
    /// Material Symbols `line_start_circle` (U+F816).
    LineStartCircle,
    /// Material Symbols `line_start_diamond` (U+F815).
    LineStartDiamond,
    /// Material Symbols `line_start_square` (U+F814).
    LineStartSquare,
    /// Material Symbols `line_style` (U+E919).
    LineStyle,
    /// Material Symbols `line_weight` (U+E91A).
    LineWeight,
    /// Material Symbols `linear_scale` (U+E260).
    LinearScale,
    /// Material Symbols `link_2` (U+FFFB5).
    Link2,
    /// Material Symbols `link_off` (U+E16F).
    LinkOff,
    /// Material Symbols `linked_camera` (U+E438).
    LinkedCamera,
    /// Material Symbols `linked_services` (U+F535).
    LinkedServices,
    /// Material Symbols `lips` (U+EEB2).
    Lips,
    /// Material Symbols `liquor` (U+EA60).
    Liquor,
    /// Material Symbols `list` (U+E896).
    List,
    /// Material Symbols `list_2` (U+FFECA).
    List2,
    /// Material Symbols `list_alt` (U+E0EE).
    ListAlt,
    /// Material Symbols `list_alt_add` (U+F756).
    ListAltAdd,
    /// Material Symbols `list_alt_check` (U+F3DE).
    ListAltCheck,
    /// Material Symbols `list_arrow` (U+FFF33).
    ListArrow,
    /// Material Symbols `lists` (U+E9B9).
    Lists,
    /// Material Symbols `live_help` (U+E0C6).
    LiveHelp,
    /// Material Symbols `live_tv` (U+E63A).
    LiveTv,
    /// Material Symbols `living` (U+F02B).
    Living,
    /// Material Symbols `local_activity` (U+E553).
    LocalActivity,
    /// Material Symbols `local_airport` (U+E53D).
    LocalAirport,
    /// Material Symbols `local_atm` (U+E53E).
    LocalAtm,
    /// Material Symbols `local_bar` (U+E540).
    LocalBar,
    /// Material Symbols `local_cafe` (U+EB44).
    LocalCafe,
    /// Material Symbols `local_car_wash` (U+E542).
    LocalCarWash,
    /// Material Symbols `local_convenience_store` (U+E543).
    LocalConvenienceStore,
    /// Material Symbols `local_dining` (U+E561).
    LocalDining,
    /// Material Symbols `local_drink` (U+E544).
    LocalDrink,
    /// Material Symbols `local_fire_department` (U+EF55).
    LocalFireDepartment,
    /// Material Symbols `local_florist` (U+E545).
    LocalFlorist,
    /// Material Symbols `local_gas_station` (U+E546).
    LocalGasStation,
    /// Material Symbols `local_grocery_store` (U+E8CC).
    LocalGroceryStore,
    /// Material Symbols `local_hospital` (U+E548).
    LocalHospital,
    /// Material Symbols `local_hotel` (U+E549).
    LocalHotel,
    /// Material Symbols `local_laundry_service` (U+E54A).
    LocalLaundryService,
    /// Material Symbols `local_library` (U+E54B).
    LocalLibrary,
    /// Material Symbols `local_mall` (U+E54C).
    LocalMall,
    /// Material Symbols `local_movies` (U+E8DA).
    LocalMovies,
    /// Material Symbols `local_offer` (U+F05B).
    LocalOffer,
    /// Material Symbols `local_parking` (U+E54F).
    LocalParking,
    /// Material Symbols `local_pharmacy` (U+E550).
    LocalPharmacy,
    /// Material Symbols `local_phone` (U+F0D4).
    LocalPhone,
    /// Material Symbols `local_pizza` (U+E552).
    LocalPizza,
    /// Material Symbols `local_play` (U+E553).
    LocalPlay,
    /// Material Symbols `local_police` (U+EF56).
    LocalPolice,
    /// Material Symbols `local_post_office` (U+E554).
    LocalPostOffice,
    /// Material Symbols `local_printshop` (U+E8AD).
    LocalPrintshop,
    /// Material Symbols `local_see` (U+E557).
    LocalSee,
    /// Material Symbols `local_shipping` (U+E558).
    LocalShipping,
    /// Material Symbols `local_taxi` (U+E559).
    LocalTaxi,
    /// Material Symbols `location_automation` (U+F14F).
    LocationAutomation,
    /// Material Symbols `location_away` (U+F150).
    LocationAway,
    /// Material Symbols `location_chip` (U+F850).
    LocationChip,
    /// Material Symbols `location_city` (U+E7F1).
    LocationCity,
    /// Material Symbols `location_disabled` (U+E1B6).
    LocationDisabled,
    /// Material Symbols `location_home` (U+F152).
    LocationHome,
    /// Material Symbols `location_off` (U+E0C7).
    LocationOff,
    /// Material Symbols `location_on` (U+F1DB).
    LocationOn,
    /// Material Symbols `location_pin` (U+F1DB).
    LocationPin,
    /// Material Symbols `location_searching` (U+E1B7).
    LocationSearching,
    /// Material Symbols `locator_tag` (U+F8C1).
    LocatorTag,
    /// Material Symbols `lock_clock` (U+EF57).
    LockClock,
    /// Material Symbols `lock_open` (U+E898).
    LockOpen,
    /// Material Symbols `lock_open_circle` (U+F361).
    LockOpenCircle,
    /// Material Symbols `lock_open_right` (U+F656).
    LockOpenRight,
    /// Material Symbols `lock_outline` (U+E899).
    LockOutline,
    /// Material Symbols `lock_person` (U+F8F3).
    LockPerson,
    /// Material Symbols `lock_reset` (U+EADE).
    LockReset,
    /// Material Symbols `login` (U+EA77).
    Login,
    /// Material Symbols `logo_dev` (U+EAD6).
    LogoDev,
    /// Material Symbols `logout` (U+E9BA).
    Logout,
    /// Material Symbols `looks` (U+E3FC).
    Looks,
    /// Material Symbols `looks_3` (U+E3FB).
    Looks3,
    /// Material Symbols `looks_4` (U+E3FD).
    Looks4,
    /// Material Symbols `looks_5` (U+E3FE).
    Looks5,
    /// Material Symbols `looks_6` (U+E3FF).
    Looks6,
    /// Material Symbols `looks_one` (U+E400).
    LooksOne,
    /// Material Symbols `looks_two` (U+E401).
    LooksTwo,
    /// Material Symbols `loop` (U+E863).
    Loop,
    /// Material Symbols `loupe` (U+E402).
    Loupe,
    /// Material Symbols `low_density` (U+F79B).
    LowDensity,
    /// Material Symbols `low_priority` (U+E16D).
    LowPriority,
    /// Material Symbols `lowercase` (U+F48A).
    Lowercase,
    /// Material Symbols `loyalty` (U+E89A).
    Loyalty,
    /// Material Symbols `lte_mobiledata` (U+F02C).
    LteMobiledata,
    /// Material Symbols `lte_mobiledata_badge` (U+F7D9).
    LteMobiledataBadge,
    /// Material Symbols `lte_plus_mobiledata` (U+F02D).
    LtePlusMobiledata,
    /// Material Symbols `lte_plus_mobiledata_badge` (U+F7D8).
    LtePlusMobiledataBadge,
    /// Material Symbols `luggage` (U+F235).
    Luggage,
    /// Material Symbols `lunch_dining` (U+EA61).
    LunchDining,
    /// Material Symbols `lyrics` (U+EC0B).
    Lyrics,
    /// Material Symbols `macro_auto` (U+F6F2).
    MacroAuto,
    /// Material Symbols `macro_off` (U+F8D2).
    MacroOff,
    /// Material Symbols `magic_button` (U+F136).
    MagicButton,
    /// Material Symbols `magic_exchange` (U+F7F4).
    MagicExchange,
    /// Material Symbols `magic_tether` (U+F7D7).
    MagicTether,
    /// Material Symbols `magnification_large` (U+F83D).
    MagnificationLarge,
    /// Material Symbols `magnification_small` (U+F83C).
    MagnificationSmall,
    /// Material Symbols `magnify_docked` (U+F7D6).
    MagnifyDocked,
    /// Material Symbols `magnify_fullscreen` (U+F7D5).
    MagnifyFullscreen,
    /// Material Symbols `mail_asterisk` (U+EEF4).
    MailAsterisk,
    /// Material Symbols `mail_lock` (U+EC0A).
    MailLock,
    /// Material Symbols `mail_off` (U+F48B).
    MailOff,
    /// Material Symbols `mail_outline` (U+E159).
    MailOutline,
    /// Material Symbols `mail_shield` (U+F249).
    MailShield,
    /// Material Symbols `male` (U+E58E).
    Male,
    /// Material Symbols `man` (U+E4EB).
    Man,
    /// Material Symbols `man_2` (U+F8E1).
    Man2,
    /// Material Symbols `man_3` (U+F8E2).
    Man3,
    /// Material Symbols `man_4` (U+F8E3).
    Man4,
    /// Material Symbols `manage_accounts` (U+F02E).
    ManageAccounts,
    /// Material Symbols `manage_history` (U+EBE7).
    ManageHistory,
    /// Material Symbols `manage_search` (U+F02F).
    ManageSearch,
    /// Material Symbols `manga` (U+F5E3).
    Manga,
    /// Material Symbols `manufacturing` (U+E726).
    Manufacturing,
    /// Material Symbols `map` (U+E55B).
    Map,
    /// Material Symbols `map_pin_heart` (U+F298).
    MapPinHeart,
    /// Material Symbols `map_pin_review` (U+F297).
    MapPinReview,
    /// Material Symbols `map_search` (U+F3CA).
    MapSearch,
    /// Material Symbols `maps_home_work` (U+F030).
    MapsHomeWork,
    /// Material Symbols `maps_ugc` (U+EF58).
    MapsUgc,
    /// Material Symbols `margin` (U+E9BB).
    Margin,
    /// Material Symbols `mark_as_unread` (U+E9BC).
    MarkAsUnread,
    /// Material Symbols `mark_chat_read` (U+F18B).
    MarkChatRead,
    /// Material Symbols `mark_chat_unread` (U+F189).
    MarkChatUnread,
    /// Material Symbols `mark_email_read` (U+F18C).
    MarkEmailRead,
    /// Material Symbols `mark_email_unread` (U+F18A).
    MarkEmailUnread,
    /// Material Symbols `mark_unread_chat_alt` (U+EB9D).
    MarkUnreadChatAlt,
    /// Material Symbols `markdown` (U+F552).
    Markdown,
    /// Material Symbols `markdown_convert` (U+FFEA0).
    MarkdownConvert,
    /// Material Symbols `markdown_copy` (U+F553).
    MarkdownCopy,
    /// Material Symbols `markdown_document` (U+FFE9F).
    MarkdownDocument,
    /// Material Symbols `markdown_paste` (U+F554).
    MarkdownPaste,
    /// Material Symbols `markdown_spark` (U+FFEA8).
    MarkdownSpark,
    /// Material Symbols `markunread` (U+E159).
    Markunread,
    /// Material Symbols `markunread_mailbox` (U+E89B).
    MarkunreadMailbox,
    /// Material Symbols `masked_transitions` (U+E72E).
    MaskedTransitions,
    /// Material Symbols `masked_transitions_add` (U+F42B).
    MaskedTransitionsAdd,
    /// Material Symbols `masks` (U+F218).
    Masks,
    /// Material Symbols `massage` (U+F2C2).
    Massage,
    /// Material Symbols `match_case` (U+F6F1).
    MatchCase,
    /// Material Symbols `match_case_off` (U+F36F).
    MatchCaseOff,
    /// Material Symbols `match_word` (U+F6F0).
    MatchWord,
    /// Material Symbols `matter` (U+E907).
    Matter,
    /// Material Symbols `maximize` (U+E930).
    Maximize,
    /// Material Symbols `meal_dinner` (U+F23D).
    MealDinner,
    /// Material Symbols `meal_lunch` (U+F23C).
    MealLunch,
    /// Material Symbols `measuring_tape` (U+F6AF).
    MeasuringTape,
    /// Material Symbols `media_bluetooth_off` (U+F031).
    MediaBluetoothOff,
    /// Material Symbols `media_bluetooth_on` (U+F032).
    MediaBluetoothOn,
    /// Material Symbols `media_link` (U+F83F).
    MediaLink,
    /// Material Symbols `media_output` (U+F4F2).
    MediaOutput,
    /// Material Symbols `media_output_off` (U+F4F3).
    MediaOutputOff,
    /// Material Symbols `mediation` (U+EFA7).
    Mediation,
    /// Material Symbols `medical_information` (U+EBED).
    MedicalInformation,
    /// Material Symbols `medical_mask` (U+F80A).
    MedicalMask,
    /// Material Symbols `medical_services` (U+F109).
    MedicalServices,
    /// Material Symbols `medication` (U+F033).
    Medication,
    /// Material Symbols `medication_liquid` (U+EA87).
    MedicationLiquid,
    /// Material Symbols `meeting_room` (U+EB4F).
    MeetingRoom,
    /// Material Symbols `memory` (U+E322).
    Memory,
    /// Material Symbols `memory_alt` (U+F7A3).
    MemoryAlt,
    /// Material Symbols `menstrual_health` (U+F6E1).
    MenstrualHealth,
    /// Material Symbols `menu_book` (U+EA19).
    MenuBook,
    /// Material Symbols `menu_book_2` (U+F291).
    MenuBook2,
    /// Material Symbols `menu_open` (U+E9BD).
    MenuOpen,
    /// Material Symbols `merge` (U+EB98).
    Merge,
    /// Material Symbols `merge_type` (U+E252).
    MergeType,
    /// Material Symbols `message` (U+E0C9).
    Message,
    /// Material Symbols `metabolism` (U+E10B).
    Metabolism,
    /// Material Symbols `metro` (U+F474).
    Metro,
    /// Material Symbols `mfg_nest_yale_lock` (U+F11D).
    MfgNestYaleLock,
    /// Material Symbols `mic` (U+E31D).
    Mic,
    /// Material Symbols `mic_alert` (U+F392).
    MicAlert,
    /// Material Symbols `mic_double` (U+F5D1).
    MicDouble,
    /// Material Symbols `mic_external_off` (U+EF59).
    MicExternalOff,
    /// Material Symbols `mic_external_on` (U+EF5A).
    MicExternalOn,
    /// Material Symbols `mic_gear` (U+EEBA).
    MicGear,
    /// Material Symbols `mic_none` (U+E31D).
    MicNone,
    /// Material Symbols `mic_off` (U+E02B).
    MicOff,
    /// Material Symbols `microbiology` (U+E10C).
    Microbiology,
    /// Material Symbols `microwave` (U+F204).
    Microwave,
    /// Material Symbols `microwave_gen` (U+E847).
    MicrowaveGen,
    /// Material Symbols `military_tech` (U+EA3F).
    MilitaryTech,
    /// Material Symbols `mimo` (U+E9BE).
    Mimo,
    /// Material Symbols `mimo_disconnect` (U+E9BF).
    MimoDisconnect,
    /// Material Symbols `mindfulness` (U+F6E0).
    Mindfulness,
    /// Material Symbols `minimize` (U+E931).
    Minimize,
    /// Material Symbols `minor_crash` (U+EBF1).
    MinorCrash,
    /// Material Symbols `mintmark` (U+EFA9).
    Mintmark,
    /// Material Symbols `missed_video_call` (U+F0CE).
    MissedVideoCall,
    /// Material Symbols `missed_video_call_filled` (U+F0CE).
    MissedVideoCallFilled,
    /// Material Symbols `missing_controller` (U+E701).
    MissingController,
    /// Material Symbols `mist` (U+E188).
    Mist,
    /// Material Symbols `mitre` (U+F547).
    Mitre,
    /// Material Symbols `mixture_med` (U+E4C8).
    MixtureMed,
    /// Material Symbols `mms` (U+E618).
    Mms,
    /// Material Symbols `mobile` (U+E7BA).
    Mobile,
    /// Material Symbols `mobile_2` (U+F2DB).
    Mobile2,
    /// Material Symbols `mobile_3` (U+F2DA).
    Mobile3,
    /// Material Symbols `mobile_alert` (U+F2D3).
    MobileAlert,
    /// Material Symbols `mobile_arrow_down` (U+F2CD).
    MobileArrowDown,
    /// Material Symbols `mobile_arrow_right` (U+F2D2).
    MobileArrowRight,
    /// Material Symbols `mobile_arrow_up_right` (U+F2B9).
    MobileArrowUpRight,
    /// Material Symbols `mobile_block` (U+F2E5).
    MobileBlock,
    /// Material Symbols `mobile_camera` (U+F44E).
    MobileCamera,
    /// Material Symbols `mobile_camera_front` (U+F2C9).
    MobileCameraFront,
    /// Material Symbols `mobile_camera_rear` (U+F2C8).
    MobileCameraRear,
    /// Material Symbols `mobile_cancel` (U+F2EA).
    MobileCancel,
    /// Material Symbols `mobile_cast` (U+F2CC).
    MobileCast,
    /// Material Symbols `mobile_charge` (U+F2E3).
    MobileCharge,
    /// Material Symbols `mobile_chat` (U+F79F).
    MobileChat,
    /// Material Symbols `mobile_check` (U+F073).
    MobileCheck,
    /// Material Symbols `mobile_code` (U+F2E2).
    MobileCode,
    /// Material Symbols `mobile_dock` (U+F2E0).
    MobileDock,
    /// Material Symbols `mobile_dots` (U+F2D0).
    MobileDots,
    /// Material Symbols `mobile_friendly` (U+F073).
    MobileFriendly,
    /// Material Symbols `mobile_gear` (U+F2D9).
    MobileGear,
    /// Material Symbols `mobile_hand` (U+F323).
    MobileHand,
    /// Material Symbols `mobile_hand_left` (U+F313).
    MobileHandLeft,
    /// Material Symbols `mobile_hand_left_off` (U+F312).
    MobileHandLeftOff,
    /// Material Symbols `mobile_hand_off` (U+F314).
    MobileHandOff,
    /// Material Symbols `mobile_info` (U+F2DC).
    MobileInfo,
    /// Material Symbols `mobile_landscape` (U+ED3E).
    MobileLandscape,
    /// Material Symbols `mobile_layout` (U+F2BF).
    MobileLayout,
    /// Material Symbols `mobile_lock_landscape` (U+F2D8).
    MobileLockLandscape,
    /// Material Symbols `mobile_lock_portrait` (U+F2BE).
    MobileLockPortrait,
    /// Material Symbols `mobile_loupe` (U+F322).
    MobileLoupe,
    /// Material Symbols `mobile_menu` (U+F2D1).
    MobileMenu,
    /// Material Symbols `mobile_off` (U+E201).
    MobileOff,
    /// Material Symbols `mobile_question` (U+F2E1).
    MobileQuestion,
    /// Material Symbols `mobile_rotate` (U+F2D5).
    MobileRotate,
    /// Material Symbols `mobile_rotate_lock` (U+F2D6).
    MobileRotateLock,
    /// Material Symbols `mobile_screen_share` (U+F2DF).
    MobileScreenShare,
    /// Material Symbols `mobile_screensaver` (U+F321).
    MobileScreensaver,
    /// Material Symbols `mobile_sensor_hi` (U+F2EF).
    MobileSensorHi,
    /// Material Symbols `mobile_sensor_lo` (U+F2EE).
    MobileSensorLo,
    /// Material Symbols `mobile_share` (U+F2DF).
    MobileShare,
    /// Material Symbols `mobile_share_stack` (U+F2DE).
    MobileShareStack,
    /// Material Symbols `mobile_sound` (U+F2E8).
    MobileSound,
    /// Material Symbols `mobile_sound_2` (U+F318).
    MobileSound2,
    /// Material Symbols `mobile_sound_off` (U+F7AA).
    MobileSoundOff,
    /// Material Symbols `mobile_speaker` (U+F320).
    MobileSpeaker,
    /// Material Symbols `mobile_tap` (U+FFEB2).
    MobileTap,
    /// Material Symbols `mobile_text` (U+F2EB).
    MobileText,
    /// Material Symbols `mobile_text_2` (U+F2E6).
    MobileText2,
    /// Material Symbols `mobile_theft` (U+F2A9).
    MobileTheft,
    /// Material Symbols `mobile_ticket` (U+F2E4).
    MobileTicket,
    /// Material Symbols `mobile_unlock` (U+EEEA).
    MobileUnlock,
    /// Material Symbols `mobile_vibrate` (U+F2CB).
    MobileVibrate,
    /// Material Symbols `mobile_wrench` (U+F2B0).
    MobileWrench,
    /// Material Symbols `mobiledata_arrows` (U+FFFA3).
    MobiledataArrows,
    /// Material Symbols `mobiledata_off` (U+F034).
    MobiledataOff,
    /// Material Symbols `mode` (U+F097).
    Mode,
    /// Material Symbols `mode_comment` (U+E253).
    ModeComment,
    /// Material Symbols `mode_cool` (U+F166).
    ModeCool,
    /// Material Symbols `mode_cool_off` (U+F167).
    ModeCoolOff,
    /// Material Symbols `mode_dual` (U+F557).
    ModeDual,
    /// Material Symbols `mode_edit` (U+F097).
    ModeEdit,
    /// Material Symbols `mode_edit_outline` (U+F097).
    ModeEditOutline,
    /// Material Symbols `mode_fan` (U+F168).
    ModeFan,
    /// Material Symbols `mode_fan_2` (U+FFFD0).
    ModeFan2,
    /// Material Symbols `mode_fan_off` (U+EC17).
    ModeFanOff,
    /// Material Symbols `mode_heat` (U+F16A).
    ModeHeat,
    /// Material Symbols `mode_heat_cool` (U+F16B).
    ModeHeatCool,
    /// Material Symbols `mode_heat_off` (U+F16D).
    ModeHeatOff,
    /// Material Symbols `mode_night` (U+F036).
    ModeNight,
    /// Material Symbols `mode_of_travel` (U+E7CE).
    ModeOfTravel,
    /// Material Symbols `mode_off_on` (U+F16F).
    ModeOffOn,
    /// Material Symbols `mode_standby` (U+F037).
    ModeStandby,
    /// Material Symbols `model_training` (U+F0CF).
    ModelTraining,
    /// Material Symbols `modeling` (U+F3AA).
    Modeling,
    /// Material Symbols `monetization_on` (U+E263).
    MonetizationOn,
    /// Material Symbols `money` (U+E57D).
    Money,
    /// Material Symbols `money_bag` (U+F3EE).
    MoneyBag,
    /// Material Symbols `money_off` (U+F038).
    MoneyOff,
    /// Material Symbols `money_off_csred` (U+F038).
    MoneyOffCsred,
    /// Material Symbols `money_range` (U+F245).
    MoneyRange,
    /// Material Symbols `monitor` (U+EF5B).
    Monitor,
    /// Material Symbols `monitor_heart` (U+EAA2).
    MonitorHeart,
    /// Material Symbols `monitor_weight` (U+F039).
    MonitorWeight,
    /// Material Symbols `monitor_weight_gain` (U+F6DF).
    MonitorWeightGain,
    /// Material Symbols `monitor_weight_loss` (U+F6DE).
    MonitorWeightLoss,
    /// Material Symbols `monitoring` (U+F190).
    Monitoring,
    /// Material Symbols `monochrome_photos` (U+E403).
    MonochromePhotos,
    /// Material Symbols `monorail` (U+F473).
    Monorail,
    /// Material Symbols `mood` (U+EA22).
    Mood,
    /// Material Symbols `mood_bad` (U+E7F3).
    MoodBad,
    /// Material Symbols `mood_heart` (U+FFFB4).
    MoodHeart,
    /// Material Symbols `moon_stars` (U+F34F).
    MoonStars,
    /// Material Symbols `mop` (U+E28D).
    Mop,
    /// Material Symbols `moped` (U+EB28).
    Moped,
    /// Material Symbols `moped_package` (U+F28B).
    MopedPackage,
    /// Material Symbols `more_down` (U+F196).
    MoreDown,
    /// Material Symbols `more_horiz` (U+E5D3).
    MoreHoriz,
    /// Material Symbols `more_time` (U+EA5D).
    MoreTime,
    /// Material Symbols `more_up` (U+F197).
    MoreUp,
    /// Material Symbols `more_vert` (U+E5D4).
    MoreVert,
    /// Material Symbols `mosque` (U+EAB2).
    Mosque,
    /// Material Symbols `motion_blur` (U+F0D0).
    MotionBlur,
    /// Material Symbols `motion_mode` (U+F842).
    MotionMode,
    /// Material Symbols `motion_photos_auto` (U+F03A).
    MotionPhotosAuto,
    /// Material Symbols `motion_photos_off` (U+E9C0).
    MotionPhotosOff,
    /// Material Symbols `motion_photos_on` (U+E9C1).
    MotionPhotosOn,
    /// Material Symbols `motion_photos_pause` (U+F227).
    MotionPhotosPause,
    /// Material Symbols `motion_photos_paused` (U+F227).
    MotionPhotosPaused,
    /// Material Symbols `motion_play` (U+F40B).
    MotionPlay,
    /// Material Symbols `motion_sensor_active` (U+E792).
    MotionSensorActive,
    /// Material Symbols `motion_sensor_alert` (U+E784).
    MotionSensorAlert,
    /// Material Symbols `motion_sensor_idle` (U+E783).
    MotionSensorIdle,
    /// Material Symbols `motion_sensor_urgent` (U+E78E).
    MotionSensorUrgent,
    /// Material Symbols `motorcycle` (U+E91B).
    Motorcycle,
    /// Material Symbols `mountain_flag` (U+F5E2).
    MountainFlag,
    /// Material Symbols `mountain_steam` (U+F282).
    MountainSteam,
    /// Material Symbols `mouse` (U+E323).
    Mouse,
    /// Material Symbols `mouse_lock` (U+F490).
    MouseLock,
    /// Material Symbols `mouse_lock_off` (U+F48F).
    MouseLockOff,
    /// Material Symbols `move` (U+E740).
    Move,
    /// Material Symbols `move_down` (U+EB61).
    MoveDown,
    /// Material Symbols `move_group` (U+F715).
    MoveGroup,
    /// Material Symbols `move_item` (U+F1FF).
    MoveItem,
    /// Material Symbols `move_location` (U+E741).
    MoveLocation,
    /// Material Symbols `move_selection_down` (U+F714).
    MoveSelectionDown,
    /// Material Symbols `move_selection_left` (U+F713).
    MoveSelectionLeft,
    /// Material Symbols `move_selection_right` (U+F712).
    MoveSelectionRight,
    /// Material Symbols `move_selection_up` (U+F711).
    MoveSelectionUp,
    /// Material Symbols `move_to_inbox` (U+E168).
    MoveToInbox,
    /// Material Symbols `move_up` (U+EB64).
    MoveUp,
    /// Material Symbols `moved_location` (U+E594).
    MovedLocation,
    /// Material Symbols `movie` (U+E404).
    Movie,
    /// Material Symbols `movie_creation` (U+E404).
    MovieCreation,
    /// Material Symbols `movie_edit` (U+F840).
    MovieEdit,
    /// Material Symbols `movie_edit_off` (U+FFF7D).
    MovieEditOff,
    /// Material Symbols `movie_filter` (U+E43A).
    MovieFilter,
    /// Material Symbols `movie_info` (U+E02D).
    MovieInfo,
    /// Material Symbols `movie_off` (U+F499).
    MovieOff,
    /// Material Symbols `movie_speaker` (U+F2A3).
    MovieSpeaker,
    /// Material Symbols `moving` (U+E501).
    Moving,
    /// Material Symbols `moving_beds` (U+E73D).
    MovingBeds,
    /// Material Symbols `moving_ministry` (U+E73E).
    MovingMinistry,
    /// Material Symbols `mp` (U+E9C3).
    Mp,
    /// Material Symbols `multicooker` (U+E293).
    Multicooker,
    /// Material Symbols `multiline_chart` (U+E6DF).
    MultilineChart,
    /// Material Symbols `multimodal_hand_eye` (U+F41B).
    MultimodalHandEye,
    /// Material Symbols `multiple_airports` (U+EFAB).
    MultipleAirports,
    /// Material Symbols `multiple_stop` (U+F1B9).
    MultipleStop,
    /// Material Symbols `museum` (U+EA36).
    Museum,
    /// Material Symbols `music_cast` (U+EB1A).
    MusicCast,
    /// Material Symbols `music_history` (U+F2C1).
    MusicHistory,
    /// Material Symbols `music_note` (U+E405).
    MusicNote,
    /// Material Symbols `music_note_2` (U+FFFD8).
    MusicNote2,
    /// Material Symbols `music_note_add` (U+F391).
    MusicNoteAdd,
    /// Material Symbols `music_off` (U+E440).
    MusicOff,
    /// Material Symbols `music_video` (U+E063).
    MusicVideo,
    /// Material Symbols `my_location` (U+E55C).
    MyLocation,
    /// Material Symbols `mystery` (U+F5E1).
    Mystery,
    /// Material Symbols `nat` (U+EF5C).
    Nat,
    /// Material Symbols `nature` (U+E406).
    Nature,
    /// Material Symbols `nature_people` (U+E407).
    NaturePeople,
    /// Material Symbols `navigate_before` (U+E5CB).
    NavigateBefore,
    /// Material Symbols `navigate_next` (U+E5CC).
    NavigateNext,
    /// Material Symbols `navigation` (U+E55D).
    Navigation,
    /// Material Symbols `near_me` (U+E569).
    NearMe,
    /// Material Symbols `near_me_disabled` (U+F1EF).
    NearMeDisabled,
    /// Material Symbols `nearby` (U+E6B7).
    Nearby,
    /// Material Symbols `nearby_error` (U+F03B).
    NearbyError,
    /// Material Symbols `nearby_off` (U+F03C).
    NearbyOff,
    /// Material Symbols `nephrology` (U+E10D).
    Nephrology,
    /// Material Symbols `nest_audio` (U+EBBF).
    NestAudio,
    /// Material Symbols `nest_cam_floodlight` (U+F8B7).
    NestCamFloodlight,
    /// Material Symbols `nest_cam_indoor` (U+F11E).
    NestCamIndoor,
    /// Material Symbols `nest_cam_iq` (U+F11F).
    NestCamIq,
    /// Material Symbols `nest_cam_iq_outdoor` (U+F120).
    NestCamIqOutdoor,
    /// Material Symbols `nest_cam_magnet_mount` (U+F8B8).
    NestCamMagnetMount,
    /// Material Symbols `nest_cam_outdoor` (U+F121).
    NestCamOutdoor,
    /// Material Symbols `nest_cam_stand` (U+F8B9).
    NestCamStand,
    /// Material Symbols `nest_cam_wall_mount` (U+F8BA).
    NestCamWallMount,
    /// Material Symbols `nest_cam_wired_stand` (U+EC16).
    NestCamWiredStand,
    /// Material Symbols `nest_clock_farsight_analog` (U+F8BB).
    NestClockFarsightAnalog,
    /// Material Symbols `nest_clock_farsight_digital` (U+F8BC).
    NestClockFarsightDigital,
    /// Material Symbols `nest_connect` (U+F122).
    NestConnect,
    /// Material Symbols `nest_detect` (U+F123).
    NestDetect,
    /// Material Symbols `nest_display` (U+F124).
    NestDisplay,
    /// Material Symbols `nest_display_max` (U+F125).
    NestDisplayMax,
    /// Material Symbols `nest_doorbell_visitor` (U+F8BD).
    NestDoorbellVisitor,
    /// Material Symbols `nest_eco_leaf` (U+F8BE).
    NestEcoLeaf,
    /// Material Symbols `nest_farsight_cool` (U+F27D).
    NestFarsightCool,
    /// Material Symbols `nest_farsight_dual` (U+F27C).
    NestFarsightDual,
    /// Material Symbols `nest_farsight_eco` (U+F27B).
    NestFarsightEco,
    /// Material Symbols `nest_farsight_heat` (U+F27A).
    NestFarsightHeat,
    /// Material Symbols `nest_farsight_seasonal` (U+F279).
    NestFarsightSeasonal,
    /// Material Symbols `nest_farsight_weather` (U+F8BF).
    NestFarsightWeather,
    /// Material Symbols `nest_found_savings` (U+F8C0).
    NestFoundSavings,
    /// Material Symbols `nest_gale_wifi` (U+F579).
    NestGaleWifi,
    /// Material Symbols `nest_heat_link_e` (U+F126).
    NestHeatLinkE,
    /// Material Symbols `nest_heat_link_gen_3` (U+F127).
    NestHeatLinkGen3,
    /// Material Symbols `nest_hello_doorbell` (U+E82C).
    NestHelloDoorbell,
    /// Material Symbols `nest_locator_tag` (U+F8C1).
    NestLocatorTag,
    /// Material Symbols `nest_mini` (U+E789).
    NestMini,
    /// Material Symbols `nest_multi_room` (U+F8C2).
    NestMultiRoom,
    /// Material Symbols `nest_protect` (U+E68E).
    NestProtect,
    /// Material Symbols `nest_remote` (U+F5DB).
    NestRemote,
    /// Material Symbols `nest_remote_comfort_sensor` (U+F12A).
    NestRemoteComfortSensor,
    /// Material Symbols `nest_secure_alarm` (U+F12B).
    NestSecureAlarm,
    /// Material Symbols `nest_sunblock` (U+F8C3).
    NestSunblock,
    /// Material Symbols `nest_tag` (U+F8C1).
    NestTag,
    /// Material Symbols `nest_thermostat` (U+E68F).
    NestThermostat,
    /// Material Symbols `nest_thermostat_e_eu` (U+F12D).
    NestThermostatEEu,
    /// Material Symbols `nest_thermostat_gen_3` (U+F12E).
    NestThermostatGen3,
    /// Material Symbols `nest_thermostat_sensor` (U+F12F).
    NestThermostatSensor,
    /// Material Symbols `nest_thermostat_sensor_eu` (U+F130).
    NestThermostatSensorEu,
    /// Material Symbols `nest_thermostat_zirconium_eu` (U+F131).
    NestThermostatZirconiumEu,
    /// Material Symbols `nest_true_radiant` (U+F8C4).
    NestTrueRadiant,
    /// Material Symbols `nest_wake_on_approach` (U+F8C5).
    NestWakeOnApproach,
    /// Material Symbols `nest_wake_on_press` (U+F8C6).
    NestWakeOnPress,
    /// Material Symbols `nest_wifi_gale` (U+F132).
    NestWifiGale,
    /// Material Symbols `nest_wifi_mistral` (U+F133).
    NestWifiMistral,
    /// Material Symbols `nest_wifi_point` (U+F134).
    NestWifiPoint,
    /// Material Symbols `nest_wifi_point_vento` (U+F134).
    NestWifiPointVento,
    /// Material Symbols `nest_wifi_pro` (U+F56B).
    NestWifiPro,
    /// Material Symbols `nest_wifi_pro_2` (U+F56A).
    NestWifiPro2,
    /// Material Symbols `nest_wifi_router` (U+F133).
    NestWifiRouter,
    /// Material Symbols `network_cell` (U+E1B9).
    NetworkCell,
    /// Material Symbols `network_check` (U+E640).
    NetworkCheck,
    /// Material Symbols `network_intel_node` (U+F371).
    NetworkIntelNode,
    /// Material Symbols `network_intelligence` (U+EFAC).
    NetworkIntelligence,
    /// Material Symbols `network_intelligence_history` (U+F5F6).
    NetworkIntelligenceHistory,
    /// Material Symbols `network_intelligence_update` (U+F5F5).
    NetworkIntelligenceUpdate,
    /// Material Symbols `network_locked` (U+E61A).
    NetworkLocked,
    /// Material Symbols `network_manage` (U+F7AB).
    NetworkManage,
    /// Material Symbols `network_node` (U+F56E).
    NetworkNode,
    /// Material Symbols `network_ping` (U+EBCA).
    NetworkPing,
    /// Material Symbols `network_wifi` (U+E1BA).
    NetworkWifi,
    /// Material Symbols `network_wifi_1_bar` (U+EBE4).
    NetworkWifi1Bar,
    /// Material Symbols `network_wifi_1_bar_locked` (U+F58F).
    NetworkWifi1BarLocked,
    /// Material Symbols `network_wifi_2_bar` (U+EBD6).
    NetworkWifi2Bar,
    /// Material Symbols `network_wifi_2_bar_locked` (U+F58E).
    NetworkWifi2BarLocked,
    /// Material Symbols `network_wifi_3_bar` (U+EBE1).
    NetworkWifi3Bar,
    /// Material Symbols `network_wifi_3_bar_locked` (U+F58D).
    NetworkWifi3BarLocked,
    /// Material Symbols `network_wifi_locked` (U+F532).
    NetworkWifiLocked,
    /// Material Symbols `neurology` (U+E10E).
    Neurology,
    /// Material Symbols `new_label` (U+E609).
    NewLabel,
    /// Material Symbols `new_releases` (U+EF76).
    NewReleases,
    /// Material Symbols `new_window` (U+F710).
    NewWindow,
    /// Material Symbols `news` (U+E032).
    News,
    /// Material Symbols `newsmode` (U+EFAD).
    Newsmode,
    /// Material Symbols `newspaper` (U+EB81).
    Newspaper,
    /// Material Symbols `newsstand` (U+E9C4).
    Newsstand,
    /// Material Symbols `next_plan` (U+EF5D).
    NextPlan,
    /// Material Symbols `next_week` (U+E16A).
    NextWeek,
    /// Material Symbols `nfc` (U+E1BB).
    Nfc,
    /// Material Symbols `nfc_off` (U+F369).
    NfcOff,
    /// Material Symbols `night_shelter` (U+F1F1).
    NightShelter,
    /// Material Symbols `night_sight_auto` (U+F1D7).
    NightSightAuto,
    /// Material Symbols `night_sight_auto_off` (U+F1F9).
    NightSightAutoOff,
    /// Material Symbols `night_sight_max` (U+F6C3).
    NightSightMax,
    /// Material Symbols `nightlife` (U+EA62).
    Nightlife,
    /// Material Symbols `nightlight` (U+F03D).
    Nightlight,
    /// Material Symbols `nightlight_round` (U+F03D).
    NightlightRound,
    /// Material Symbols `nights_stay` (U+F174).
    NightsStay,
    /// Material Symbols `no_accounts` (U+F03E).
    NoAccounts,
    /// Material Symbols `no_adult_content` (U+F8FE).
    NoAdultContent,
    /// Material Symbols `no_backpack` (U+F237).
    NoBackpack,
    /// Material Symbols `no_crash` (U+EBF0).
    NoCrash,
    /// Material Symbols `no_drinks` (U+F1A5).
    NoDrinks,
    /// Material Symbols `no_encryption` (U+F03F).
    NoEncryption,
    /// Material Symbols `no_encryption_gmailerrorred` (U+F03F).
    NoEncryptionGmailerrorred,
    /// Material Symbols `no_flash` (U+F1A6).
    NoFlash,
    /// Material Symbols `no_food` (U+F1A7).
    NoFood,
    /// Material Symbols `no_luggage` (U+F23B).
    NoLuggage,
    /// Material Symbols `no_meals` (U+F1D6).
    NoMeals,
    /// Material Symbols `no_meeting_room` (U+EB4E).
    NoMeetingRoom,
    /// Material Symbols `no_photography` (U+F1A8).
    NoPhotography,
    /// Material Symbols `no_sim` (U+E1CE).
    NoSim,
    /// Material Symbols `no_sound` (U+E710).
    NoSound,
    /// Material Symbols `no_stroller` (U+F1AF).
    NoStroller,
    /// Material Symbols `no_transfer` (U+F1D5).
    NoTransfer,
    /// Material Symbols `noise_aware` (U+EBEC).
    NoiseAware,
    /// Material Symbols `noise_control_off` (U+EBF3).
    NoiseControlOff,
    /// Material Symbols `noise_control_on` (U+F8A8).
    NoiseControlOn,
    /// Material Symbols `nordic_walking` (U+E50E).
    NordicWalking,
    /// Material Symbols `north` (U+F1E0).
    North,
    /// Material Symbols `north_east` (U+F1E1).
    NorthEast,
    /// Material Symbols `north_west` (U+F1E2).
    NorthWest,
    /// Material Symbols `not_accessible` (U+F0FE).
    NotAccessible,
    /// Material Symbols `not_accessible_forward` (U+F54A).
    NotAccessibleForward,
    /// Material Symbols `not_interested` (U+F08C).
    NotInterested,
    /// Material Symbols `not_listed_location` (U+E575).
    NotListedLocation,
    /// Material Symbols `not_started` (U+F0D1).
    NotStarted,
    /// Material Symbols `note` (U+E66D).
    Note,
    /// Material Symbols `note_add` (U+E89C).
    NoteAdd,
    /// Material Symbols `note_alt` (U+F040).
    NoteAlt,
    /// Material Symbols `note_stack` (U+F562).
    NoteStack,
    /// Material Symbols `note_stack_add` (U+F563).
    NoteStackAdd,
    /// Material Symbols `notes` (U+E26C).
    Notes,
    /// Material Symbols `notification_add` (U+E399).
    NotificationAdd,
    /// Material Symbols `notification_audio` (U+EEC1).
    NotificationAudio,
    /// Material Symbols `notification_audio_off` (U+EEC0).
    NotificationAudioOff,
    /// Material Symbols `notification_important` (U+E004).
    NotificationImportant,
    /// Material Symbols `notification_multiple` (U+E6C2).
    NotificationMultiple,
    /// Material Symbols `notification_settings` (U+F367).
    NotificationSettings,
    /// Material Symbols `notification_sound` (U+F353).
    NotificationSound,
    /// Material Symbols `notifications` (U+E7F5).
    Notifications,
    /// Material Symbols `notifications_active` (U+E7F7).
    NotificationsActive,
    /// Material Symbols `notifications_none` (U+E7F5).
    NotificationsNone,
    /// Material Symbols `notifications_paused` (U+E7F8).
    NotificationsPaused,
    /// Material Symbols `notifications_unread` (U+F4FE).
    NotificationsUnread,
    /// Material Symbols `numbers` (U+EAC7).
    Numbers,
    /// Material Symbols `nutrition` (U+E110).
    Nutrition,
    /// Material Symbols `ods` (U+E6E8).
    Ods,
    /// Material Symbols `odt` (U+E6E9).
    Odt,
    /// Material Symbols `offline_bolt` (U+E932).
    OfflineBolt,
    /// Material Symbols `offline_pin` (U+E90A).
    OfflinePin,
    /// Material Symbols `offline_pin_off` (U+F4D0).
    OfflinePinOff,
    /// Material Symbols `offline_share` (U+F2DE).
    OfflineShare,
    /// Material Symbols `oil_barrel` (U+EC15).
    OilBarrel,
    /// Material Symbols `okonomiyaki` (U+F281).
    Okonomiyaki,
    /// Material Symbols `on_device_training` (U+EBFD).
    OnDeviceTraining,
    /// Material Symbols `on_hub_device` (U+E6C3).
    OnHubDevice,
    /// Material Symbols `oncology` (U+E114).
    Oncology,
    /// Material Symbols `ondemand_video` (U+E63A).
    OndemandVideo,
    /// Material Symbols `online_prediction` (U+F0EB).
    OnlinePrediction,
    /// Material Symbols `onsen` (U+F6F8).
    Onsen,
    /// Material Symbols `opacity` (U+E91C).
    Opacity,
    /// Material Symbols `open_in_browser` (U+E89D).
    OpenInBrowser,
    /// Material Symbols `open_in_full` (U+F1CE).
    OpenInFull,
    /// Material Symbols `open_in_new` (U+E89E).
    OpenInNew,
    /// Material Symbols `open_in_new_down` (U+F70F).
    OpenInNewDown,
    /// Material Symbols `open_in_new_off` (U+E4F6).
    OpenInNewOff,
    /// Material Symbols `open_in_phone` (U+F2D2).
    OpenInPhone,
    /// Material Symbols `open_jam` (U+EFAE).
    OpenJam,
    /// Material Symbols `open_run` (U+F4B7).
    OpenRun,
    /// Material Symbols `open_with` (U+E89F).
    OpenWith,
    /// Material Symbols `ophthalmology` (U+E115).
    Ophthalmology,
    /// Material Symbols `oral_disease` (U+E116).
    OralDisease,
    /// Material Symbols `orbit` (U+F426).
    Orbit,
    /// Material Symbols `order_approve` (U+F812).
    OrderApprove,
    /// Material Symbols `order_play` (U+F811).
    OrderPlay,
    /// Material Symbols `orders` (U+EB14).
    Orders,
    /// Material Symbols `orthopedics` (U+F897).
    Orthopedics,
    /// Material Symbols `other_admission` (U+E47B).
    OtherAdmission,
    /// Material Symbols `other_houses` (U+E58C).
    OtherHouses,
    /// Material Symbols `outbound` (U+E1CA).
    Outbound,
    /// Material Symbols `outbox` (U+EF5F).
    Outbox,
    /// Material Symbols `outbox_alt` (U+EB17).
    OutboxAlt,
    /// Material Symbols `outdoor_garden` (U+E205).
    OutdoorGarden,
    /// Material Symbols `outdoor_grill` (U+EA47).
    OutdoorGrill,
    /// Material Symbols `outgoing_mail` (U+F0D2).
    OutgoingMail,
    /// Material Symbols `outlet` (U+F1D4).
    Outlet,
    /// Material Symbols `outlined_flag` (U+F0C6).
    OutlinedFlag,
    /// Material Symbols `outpatient` (U+E118).
    Outpatient,
    /// Material Symbols `outpatient_med` (U+E119).
    OutpatientMed,
    /// Material Symbols `output` (U+EBBE).
    Output,
    /// Material Symbols `output_circle` (U+F70E).
    OutputCircle,
    /// Material Symbols `oven` (U+E9C7).
    Oven,
    /// Material Symbols `oven_gen` (U+E843).
    OvenGen,
    /// Material Symbols `overview` (U+E4A7).
    Overview,
    /// Material Symbols `overview_key` (U+F7D4).
    OverviewKey,
    /// Material Symbols `owl` (U+F3B4).
    Owl,
    /// Material Symbols `oxygen_saturation` (U+E4DE).
    OxygenSaturation,
    /// Material Symbols `p2p` (U+F52A).
    P2p,
    /// Material Symbols `pace` (U+F6B8).
    Pace,
    /// Material Symbols `pacemaker` (U+E656).
    Pacemaker,
    /// Material Symbols `package` (U+E48F).
    Package,
    /// Material Symbols `package_2` (U+F569).
    Package2,
    /// Material Symbols `padding` (U+E9C8).
    Padding,
    /// Material Symbols `padel` (U+F2A7).
    Padel,
    /// Material Symbols `page_control` (U+E731).
    PageControl,
    /// Material Symbols `page_footer` (U+F383).
    PageFooter,
    /// Material Symbols `page_header` (U+F384).
    PageHeader,
    /// Material Symbols `page_info` (U+F614).
    PageInfo,
    /// Material Symbols `page_menu_ios` (U+EEFB).
    PageMenuIos,
    /// Material Symbols `pageless` (U+F509).
    Pageless,
    /// Material Symbols `pages` (U+E7F9).
    Pages,
    /// Material Symbols `pageview` (U+E8A0).
    Pageview,
    /// Material Symbols `paid` (U+F041).
    Paid,
    /// Material Symbols `palette` (U+E40A).
    Palette,
    /// Material Symbols `pallet` (U+F86A).
    Pallet,
    /// Material Symbols `pan_tool` (U+E925).
    PanTool,
    /// Material Symbols `pan_tool_alt` (U+EBB9).
    PanToolAlt,
    /// Material Symbols `pan_zoom` (U+F655).
    PanZoom,
    /// Material Symbols `panorama` (U+E691).
    Panorama,
    /// Material Symbols `panorama_fish_eye` (U+E40C).
    PanoramaFishEye,
    /// Material Symbols `panorama_horizontal` (U+E40D).
    PanoramaHorizontal,
    /// Material Symbols `panorama_photosphere` (U+E9C9).
    PanoramaPhotosphere,
    /// Material Symbols `panorama_vertical` (U+E40E).
    PanoramaVertical,
    /// Material Symbols `panorama_wide_angle` (U+E40F).
    PanoramaWideAngle,
    /// Material Symbols `paragliding` (U+E50F).
    Paragliding,
    /// Material Symbols `parent_child_dining` (U+F22D).
    ParentChildDining,
    /// Material Symbols `park` (U+EA63).
    Park,
    /// Material Symbols `parking_meter` (U+F28A).
    ParkingMeter,
    /// Material Symbols `parking_sign` (U+F289).
    ParkingSign,
    /// Material Symbols `parking_valet` (U+F288).
    ParkingValet,
    /// Material Symbols `partly_cloudy_day` (U+F172).
    PartlyCloudyDay,
    /// Material Symbols `partly_cloudy_night` (U+F174).
    PartlyCloudyNight,
    /// Material Symbols `partner_exchange` (U+F7F9).
    PartnerExchange,
    /// Material Symbols `partner_heart` (U+EF2E).
    PartnerHeart,
    /// Material Symbols `partner_reports` (U+EFAF).
    PartnerReports,
    /// Material Symbols `party_mode` (U+E7FA).
    PartyMode,
    /// Material Symbols `passkey` (U+F87F).
    Passkey,
    /// Material Symbols `passport` (U+EEC4).
    Passport,
    /// Material Symbols `password` (U+F042).
    Password,
    /// Material Symbols `password_2` (U+F4A9).
    Password2,
    /// Material Symbols `password_2_off` (U+F4A8).
    Password2Off,
    /// Material Symbols `patient_list` (U+E653).
    PatientList,
    /// Material Symbols `pattern` (U+F043).
    Pattern,
    /// Material Symbols `pause_circle` (U+E1A2).
    PauseCircle,
    /// Material Symbols `pause_circle_filled` (U+E1A2).
    PauseCircleFilled,
    /// Material Symbols `pause_circle_outline` (U+E1A2).
    PauseCircleOutline,
    /// Material Symbols `pause_presentation` (U+E0EA).
    PausePresentation,
    /// Material Symbols `payment` (U+E8A1).
    Payment,
    /// Material Symbols `payment_arrow_down` (U+F2C0).
    PaymentArrowDown,
    /// Material Symbols `payment_card` (U+F2A1).
    PaymentCard,
    /// Material Symbols `payments` (U+EF63).
    Payments,
    /// Material Symbols `pedal_bike` (U+EB29).
    PedalBike,
    /// Material Symbols `pediatrics` (U+E11D).
    Pediatrics,
    /// Material Symbols `pen_size_1` (U+F755).
    PenSize1,
    /// Material Symbols `pen_size_2` (U+F754).
    PenSize2,
    /// Material Symbols `pen_size_3` (U+F753).
    PenSize3,
    /// Material Symbols `pen_size_4` (U+F752).
    PenSize4,
    /// Material Symbols `pen_size_5` (U+F751).
    PenSize5,
    /// Material Symbols `pending` (U+EF64).
    Pending,
    /// Material Symbols `pending_actions` (U+F1BB).
    PendingActions,
    /// Material Symbols `pentagon` (U+EB50).
    Pentagon,
    /// Material Symbols `people` (U+EA21).
    People,
    /// Material Symbols `people_alt` (U+EA21).
    PeopleAlt,
    /// Material Symbols `people_outline` (U+EA21).
    PeopleOutline,
    /// Material Symbols `people_size_decrease` (U+FFEB1).
    PeopleSizeDecrease,
    /// Material Symbols `people_size_increase` (U+FFEB0).
    PeopleSizeIncrease,
    /// Material Symbols `percent` (U+EB58).
    Percent,
    /// Material Symbols `percent_discount` (U+F244).
    PercentDiscount,
    /// Material Symbols `performance_max` (U+E51A).
    PerformanceMax,
    /// Material Symbols `pergola` (U+E203).
    Pergola,
    /// Material Symbols `perm_camera_mic` (U+E8A2).
    PermCameraMic,
    /// Material Symbols `perm_contact_calendar` (U+E8A3).
    PermContactCalendar,
    /// Material Symbols `perm_data_setting` (U+E8A4).
    PermDataSetting,
    /// Material Symbols `perm_device_information` (U+F2DC).
    PermDeviceInformation,
    /// Material Symbols `perm_identity` (U+F0D3).
    PermIdentity,
    /// Material Symbols `perm_media` (U+E8A7).
    PermMedia,
    /// Material Symbols `perm_phone_msg` (U+E8A8).
    PermPhoneMsg,
    /// Material Symbols `perm_scan_wifi` (U+E8A9).
    PermScanWifi,
    /// Material Symbols `person` (U+F0D3).
    Person,
    /// Material Symbols `person_2` (U+F8E4).
    Person2,
    /// Material Symbols `person_3` (U+F8E5).
    Person3,
    /// Material Symbols `person_4` (U+F8E6).
    Person4,
    /// Material Symbols `person_add` (U+EA4D).
    PersonAdd,
    /// Material Symbols `person_add_alt` (U+EA4D).
    PersonAddAlt,
    /// Material Symbols `person_add_disabled` (U+E9CB).
    PersonAddDisabled,
    /// Material Symbols `person_alert` (U+F567).
    PersonAlert,
    /// Material Symbols `person_apron` (U+F5A3).
    PersonApron,
    /// Material Symbols `person_book` (U+F5E8).
    PersonBook,
    /// Material Symbols `person_cancel` (U+F566).
    PersonCancel,
    /// Material Symbols `person_celebrate` (U+F7FE).
    PersonCelebrate,
    /// Material Symbols `person_check` (U+F565).
    PersonCheck,
    /// Material Symbols `person_edit` (U+F4FA).
    PersonEdit,
    /// Material Symbols `person_filled` (U+F0D3).
    PersonFilled,
    /// Material Symbols `person_heart` (U+F290).
    PersonHeart,
    /// Material Symbols `person_off` (U+E510).
    PersonOff,
    /// Material Symbols `person_outline` (U+F0D3).
    PersonOutline,
    /// Material Symbols `person_pin` (U+E55A).
    PersonPin,
    /// Material Symbols `person_pin_circle` (U+E56A).
    PersonPinCircle,
    /// Material Symbols `person_play` (U+F7FD).
    PersonPlay,
    /// Material Symbols `person_raised_hand` (U+F59A).
    PersonRaisedHand,
    /// Material Symbols `person_remove` (U+EF66).
    PersonRemove,
    /// Material Symbols `person_search` (U+F106).
    PersonSearch,
    /// Material Symbols `person_shield` (U+E384).
    PersonShield,
    /// Material Symbols `person_text` (U+EEBD).
    PersonText,
    /// Material Symbols `personal_bag` (U+EB0E).
    PersonalBag,
    /// Material Symbols `personal_bag_off` (U+EB0F).
    PersonalBagOff,
    /// Material Symbols `personal_bag_question` (U+EB10).
    PersonalBagQuestion,
    /// Material Symbols `personal_injury` (U+E6DA).
    PersonalInjury,
    /// Material Symbols `personal_places` (U+E703).
    PersonalPlaces,
    /// Material Symbols `personal_video` (U+E63B).
    PersonalVideo,
    /// Material Symbols `pest_control` (U+F0FA).
    PestControl,
    /// Material Symbols `pest_control_rodent` (U+F0FD).
    PestControlRodent,
    /// Material Symbols `pet_supplies` (U+EFB1).
    PetSupplies,
    /// Material Symbols `pets` (U+E91D).
    Pets,
    /// Material Symbols `phishing` (U+EAD7).
    Phishing,
    /// Material Symbols `phone` (U+F0D4).
    Phone,
    /// Material Symbols `phone_alt` (U+F0D4).
    PhoneAlt,
    /// Material Symbols `phone_android` (U+F2DB).
    PhoneAndroid,
    /// Material Symbols `phone_bluetooth_speaker` (U+E61B).
    PhoneBluetoothSpeaker,
    /// Material Symbols `phone_callback` (U+E649).
    PhoneCallback,
    /// Material Symbols `phone_cancel` (U+FFF9D).
    PhoneCancel,
    /// Material Symbols `phone_disabled` (U+E9CC).
    PhoneDisabled,
    /// Material Symbols `phone_enabled` (U+E9CD).
    PhoneEnabled,
    /// Material Symbols `phone_forwarded` (U+E61C).
    PhoneForwarded,
    /// Material Symbols `phone_in_talk` (U+E61D).
    PhoneInTalk,
    /// Material Symbols `phone_iphone` (U+F2DA).
    PhoneIphone,
    /// Material Symbols `phone_locked` (U+E61E).
    PhoneLocked,
    /// Material Symbols `phone_missed` (U+E61F).
    PhoneMissed,
    /// Material Symbols `phone_paused` (U+E620).
    PhonePaused,
    /// Material Symbols `phonelink` (U+E326).
    Phonelink,
    /// Material Symbols `phonelink_erase` (U+F2EA).
    PhonelinkErase,
    /// Material Symbols `phonelink_lock` (U+F2BE).
    PhonelinkLock,
    /// Material Symbols `phonelink_off` (U+F7A5).
    PhonelinkOff,
    /// Material Symbols `phonelink_ring` (U+F2E8).
    PhonelinkRing,
    /// Material Symbols `phonelink_ring_off` (U+F7AA).
    PhonelinkRingOff,
    /// Material Symbols `phonelink_setup` (U+F2D9).
    PhonelinkSetup,
    /// Material Symbols `photo` (U+E693).
    Photo,
    /// Material Symbols `photo_album` (U+E411).
    PhotoAlbum,
    /// Material Symbols `photo_auto_merge` (U+F530).
    PhotoAutoMerge,
    /// Material Symbols `photo_camera` (U+E412).
    PhotoCamera,
    /// Material Symbols `photo_camera_back` (U+EF68).
    PhotoCameraBack,
    /// Material Symbols `photo_camera_front` (U+EF69).
    PhotoCameraFront,
    /// Material Symbols `photo_filter` (U+E43B).
    PhotoFilter,
    /// Material Symbols `photo_frame` (U+F0D9).
    PhotoFrame,
    /// Material Symbols `photo_library` (U+E413).
    PhotoLibrary,
    /// Material Symbols `photo_prints` (U+EFB2).
    PhotoPrints,
    /// Material Symbols `photo_size_select_actual` (U+E693).
    PhotoSizeSelectActual,
    /// Material Symbols `photo_size_select_large` (U+E433).
    PhotoSizeSelectLarge,
    /// Material Symbols `photo_size_select_small` (U+E434).
    PhotoSizeSelectSmall,
    /// Material Symbols `php` (U+EB8F).
    Php,
    /// Material Symbols `physical_therapy` (U+E11E).
    PhysicalTherapy,
    /// Material Symbols `piano` (U+E521).
    Piano,
    /// Material Symbols `piano_off` (U+E520).
    PianoOff,
    /// Material Symbols `pickleball` (U+F2A6).
    Pickleball,
    /// Material Symbols `picture_as_pdf` (U+E415).
    PictureAsPdf,
    /// Material Symbols `picture_in_picture` (U+E8AA).
    PictureInPicture,
    /// Material Symbols `picture_in_picture_alt` (U+E911).
    PictureInPictureAlt,
    /// Material Symbols `picture_in_picture_center` (U+F550).
    PictureInPictureCenter,
    /// Material Symbols `picture_in_picture_large` (U+F54F).
    PictureInPictureLarge,
    /// Material Symbols `picture_in_picture_medium` (U+F54E).
    PictureInPictureMedium,
    /// Material Symbols `picture_in_picture_mobile` (U+F517).
    PictureInPictureMobile,
    /// Material Symbols `picture_in_picture_off` (U+F52F).
    PictureInPictureOff,
    /// Material Symbols `picture_in_picture_small` (U+F54D).
    PictureInPictureSmall,
    /// Material Symbols `pie_chart` (U+F0DA).
    PieChart,
    /// Material Symbols `pie_chart_filled` (U+F0DA).
    PieChartFilled,
    /// Material Symbols `pie_chart_outline` (U+F0DA).
    PieChartOutline,
    /// Material Symbols `pie_chart_outlined` (U+F0DA).
    PieChartOutlined,
    /// Material Symbols `pill` (U+E11F).
    Pill,
    /// Material Symbols `pill_off` (U+F809).
    PillOff,
    /// Material Symbols `pin` (U+F045).
    Pin,
    /// Material Symbols `pin_drop` (U+E55E).
    PinDrop,
    /// Material Symbols `pin_end` (U+E767).
    PinEnd,
    /// Material Symbols `pin_history` (U+FFF2E).
    PinHistory,
    /// Material Symbols `pin_invoke` (U+E763).
    PinInvoke,
    /// Material Symbols `pin_road` (U+FFF2D).
    PinRoad,
    /// Material Symbols `pin_road_2` (U+FFEEB).
    PinRoad2,
    /// Material Symbols `pinboard` (U+F3AB).
    Pinboard,
    /// Material Symbols `pinboard_unread` (U+F3AC).
    PinboardUnread,
    /// Material Symbols `pinch` (U+EB38).
    Pinch,
    /// Material Symbols `pinch_zoom_in` (U+F1FA).
    PinchZoomIn,
    /// Material Symbols `pinch_zoom_out` (U+F1FB).
    PinchZoomOut,
    /// Material Symbols `pip` (U+F64D).
    Pip,
    /// Material Symbols `pip_exit` (U+F70D).
    PipExit,
    /// Material Symbols `pivot_table_chart` (U+E9CE).
    PivotTableChart,
    /// Material Symbols `place` (U+F1DB).
    Place,
    /// Material Symbols `place_item` (U+F1F0).
    PlaceItem,
    /// Material Symbols `plagiarism` (U+EA5A).
    Plagiarism,
    /// Material Symbols `plane_contrails` (U+F2AC).
    PlaneContrails,
    /// Material Symbols `planet` (U+F387).
    Planet,
    /// Material Symbols `planner_banner_ad_pt` (U+E692).
    PlannerBannerAdPt,
    /// Material Symbols `planner_review` (U+E694).
    PlannerReview,
    /// Material Symbols `play_arrow` (U+E037).
    PlayArrow,
    /// Material Symbols `play_circle` (U+E1C4).
    PlayCircle,
    /// Material Symbols `play_disabled` (U+EF6A).
    PlayDisabled,
    /// Material Symbols `play_for_work` (U+E906).
    PlayForWork,
    /// Material Symbols `play_lesson` (U+F047).
    PlayLesson,
    /// Material Symbols `play_music` (U+E6EE).
    PlayMusic,
    /// Material Symbols `play_pause` (U+F137).
    PlayPause,
    /// Material Symbols `play_shapes` (U+F7FC).
    PlayShapes,
    /// Material Symbols `playground` (U+F28E).
    Playground,
    /// Material Symbols `playground_2` (U+F28F).
    Playground2,
    /// Material Symbols `playing_cards` (U+F5DC).
    PlayingCards,
    /// Material Symbols `playlist_add` (U+E03B).
    PlaylistAdd,
    /// Material Symbols `playlist_add_check` (U+E065).
    PlaylistAddCheck,
    /// Material Symbols `playlist_add_check_circle` (U+E7E6).
    PlaylistAddCheckCircle,
    /// Material Symbols `playlist_add_circle` (U+E7E5).
    PlaylistAddCircle,
    /// Material Symbols `playlist_play` (U+E05F).
    PlaylistPlay,
    /// Material Symbols `playlist_remove` (U+EB80).
    PlaylistRemove,
    /// Material Symbols `plug_connect` (U+F35A).
    PlugConnect,
    /// Material Symbols `plumbing` (U+F107).
    Plumbing,
    /// Material Symbols `plus_one` (U+E800).
    PlusOne,
    /// Material Symbols `podcasts` (U+F048).
    Podcasts,
    /// Material Symbols `podiatry` (U+E120).
    Podiatry,
    /// Material Symbols `podium` (U+F7FB).
    Podium,
    /// Material Symbols `point_of_sale` (U+F17E).
    PointOfSale,
    /// Material Symbols `point_scan` (U+F70C).
    PointScan,
    /// Material Symbols `poker_chip` (U+F49B).
    PokerChip,
    /// Material Symbols `policy` (U+EA17).
    Policy,
    /// Material Symbols `policy_alert` (U+F407).
    PolicyAlert,
    /// Material Symbols `poll` (U+F0CC).
    Poll,
    /// Material Symbols `polyline` (U+EBBB).
    Polyline,
    /// Material Symbols `polymer` (U+E8AB).
    Polymer,
    /// Material Symbols `pool` (U+EB48).
    Pool,
    /// Material Symbols `portable_wifi_off` (U+F087).
    PortableWifiOff,
    /// Material Symbols `portrait` (U+E851).
    Portrait,
    /// Material Symbols `position_bottom_left` (U+F70B).
    PositionBottomLeft,
    /// Material Symbols `position_bottom_right` (U+F70A).
    PositionBottomRight,
    /// Material Symbols `position_top_right` (U+F709).
    PositionTopRight,
    /// Material Symbols `post` (U+E705).
    Post,
    /// Material Symbols `post_add` (U+EA20).
    PostAdd,
    /// Material Symbols `potted_plant` (U+F8AA).
    PottedPlant,
    /// Material Symbols `power` (U+E63C).
    Power,
    /// Material Symbols `power_input` (U+E336).
    PowerInput,
    /// Material Symbols `power_off` (U+E646).
    PowerOff,
    /// Material Symbols `power_rounded` (U+F8C7).
    PowerRounded,
    /// Material Symbols `power_settings_circle` (U+F418).
    PowerSettingsCircle,
    /// Material Symbols `power_settings_new` (U+F8C7).
    PowerSettingsNew,
    /// Material Symbols `prayer_times` (U+F838).
    PrayerTimes,
    /// Material Symbols `precision_manufacturing` (U+F049).
    PrecisionManufacturing,
    /// Material Symbols `pregnancy` (U+F5F1).
    Pregnancy,
    /// Material Symbols `pregnant_woman` (U+F5F1).
    PregnantWoman,
    /// Material Symbols `preliminary` (U+E7D8).
    Preliminary,
    /// Material Symbols `prescriptions` (U+E121).
    Prescriptions,
    /// Material Symbols `present_to_all` (U+E0DF).
    PresentToAll,
    /// Material Symbols `preview` (U+F1C5).
    Preview,
    /// Material Symbols `preview_off` (U+F7AF).
    PreviewOff,
    /// Material Symbols `price_change` (U+F04A).
    PriceChange,
    /// Material Symbols `price_check` (U+F04B).
    PriceCheck,
    /// Material Symbols `print_add` (U+F7A2).
    PrintAdd,
    /// Material Symbols `print_connect` (U+F7A1).
    PrintConnect,
    /// Material Symbols `print_disabled` (U+E9CF).
    PrintDisabled,
    /// Material Symbols `print_error` (U+F7A0).
    PrintError,
    /// Material Symbols `print_lock` (U+F651).
    PrintLock,
    /// Material Symbols `priority` (U+EFB4).
    Priority,
    /// Material Symbols `priority_high` (U+E645).
    PriorityHigh,
    /// Material Symbols `privacy` (U+F148).
    Privacy,
    /// Material Symbols `privacy_tip` (U+F0DC).
    PrivacyTip,
    /// Material Symbols `private_connectivity` (U+E744).
    PrivateConnectivity,
    /// Material Symbols `problem` (U+E122).
    Problem,
    /// Material Symbols `procedure` (U+E651).
    Procedure,
    /// Material Symbols `process_chart` (U+F855).
    ProcessChart,
    /// Material Symbols `production_quantity_limits` (U+E1D1).
    ProductionQuantityLimits,
    /// Material Symbols `productivity` (U+E296).
    Productivity,
    /// Material Symbols `progress_activity` (U+E9D0).
    ProgressActivity,
    /// Material Symbols `prompt_suggestion` (U+F4F6).
    PromptSuggestion,
    /// Material Symbols `propane` (U+EC14).
    Propane,
    /// Material Symbols `propane_tank` (U+EC13).
    PropaneTank,
    /// Material Symbols `psychiatry` (U+E123).
    Psychiatry,
    /// Material Symbols `psychology` (U+EA4A).
    Psychology,
    /// Material Symbols `psychology_alt` (U+F8EA).
    PsychologyAlt,
    /// Material Symbols `public` (U+E80B).
    Public,
    /// Material Symbols `public_off` (U+F1CA).
    PublicOff,
    /// Material Symbols `publish` (U+E255).
    Publish,
    /// Material Symbols `published_with_changes` (U+F232).
    PublishedWithChanges,
    /// Material Symbols `pulmonology` (U+E124).
    Pulmonology,
    /// Material Symbols `pulse_alert` (U+F501).
    PulseAlert,
    /// Material Symbols `punch_clock` (U+EAA8).
    PunchClock,
    /// Material Symbols `push_pin` (U+F10D).
    PushPin,
    /// Material Symbols `qr_code` (U+EF6B).
    QrCode,
    /// Material Symbols `qr_code_2` (U+E00A).
    QrCode2,
    /// Material Symbols `qr_code_2_add` (U+F658).
    QrCode2Add,
    /// Material Symbols `qr_code_scanner` (U+F206).
    QrCodeScanner,
    /// Material Symbols `query_builder` (U+EFD6).
    QueryBuilder,
    /// Material Symbols `query_stats` (U+E4FC).
    QueryStats,
    /// Material Symbols `question_answer` (U+E8AF).
    QuestionAnswer,
    /// Material Symbols `question_exchange` (U+F7F3).
    QuestionExchange,
    /// Material Symbols `question_mark` (U+EB8B).
    QuestionMark,
    /// Material Symbols `queue` (U+E03C).
    Queue,
    /// Material Symbols `queue_music` (U+E03D).
    QueueMusic,
    /// Material Symbols `queue_play_next` (U+E066).
    QueuePlayNext,
    /// Material Symbols `quick_phrases` (U+E7D1).
    QuickPhrases,
    /// Material Symbols `quick_reference` (U+E46E).
    QuickReference,
    /// Material Symbols `quick_reference_all` (U+F801).
    QuickReferenceAll,
    /// Material Symbols `quick_reorder` (U+EB15).
    QuickReorder,
    /// Material Symbols `quickreply` (U+EF6C).
    Quickreply,
    /// Material Symbols `quiet_time` (U+F159).
    QuietTime,
    /// Material Symbols `quiet_time_active` (U+EB76).
    QuietTimeActive,
    /// Material Symbols `quiz` (U+F04C).
    Quiz,
    /// Material Symbols `r_mobiledata` (U+F04D).
    RMobiledata,
    /// Material Symbols `radar` (U+F04E).
    Radar,
    /// Material Symbols `radio` (U+E03E).
    Radio,
    /// Material Symbols `radio_button_checked` (U+E837).
    RadioButtonChecked,
    /// Material Symbols `radio_button_partial` (U+F560).
    RadioButtonPartial,
    /// Material Symbols `radio_button_unchecked` (U+E836).
    RadioButtonUnchecked,
    /// Material Symbols `radiology` (U+E125).
    Radiology,
    /// Material Symbols `railway_alert` (U+E9D1).
    RailwayAlert,
    /// Material Symbols `railway_alert_2` (U+F461).
    RailwayAlert2,
    /// Material Symbols `rainy` (U+F176).
    Rainy,
    /// Material Symbols `rainy_heavy` (U+F61F).
    RainyHeavy,
    /// Material Symbols `rainy_light` (U+F61E).
    RainyLight,
    /// Material Symbols `rainy_snow` (U+F61D).
    RainySnow,
    /// Material Symbols `ramen_dining` (U+EA64).
    RamenDining,
    /// Material Symbols `ramp_left` (U+EB9C).
    RampLeft,
    /// Material Symbols `ramp_right` (U+EB96).
    RampRight,
    /// Material Symbols `range_hood` (U+E1EA).
    RangeHood,
    /// Material Symbols `rate_review` (U+E560).
    RateReview,
    /// Material Symbols `rate_review_rtl` (U+E706).
    RateReviewRtl,
    /// Material Symbols `raven` (U+F555).
    Raven,
    /// Material Symbols `raw_off` (U+F04F).
    RawOff,
    /// Material Symbols `raw_on` (U+F050).
    RawOn,
    /// Material Symbols `read_more` (U+EF6D).
    ReadMore,
    /// Material Symbols `readiness_score` (U+F6DD).
    ReadinessScore,
    /// Material Symbols `real_estate_agent` (U+E73A).
    RealEstateAgent,
    /// Material Symbols `rear_camera` (U+F6C2).
    RearCamera,
    /// Material Symbols `rebase` (U+F845).
    Rebase,
    /// Material Symbols `rebase_edit` (U+F846).
    RebaseEdit,
    /// Material Symbols `receipt` (U+E8B0).
    Receipt,
    /// Material Symbols `receipt_long` (U+EF6E).
    ReceiptLong,
    /// Material Symbols `receipt_long_off` (U+F40A).
    ReceiptLongOff,
    /// Material Symbols `recent_actors` (U+E03F).
    RecentActors,
    /// Material Symbols `recent_patient` (U+F808).
    RecentPatient,
    /// Material Symbols `recenter` (U+F4C0).
    Recenter,
    /// Material Symbols `recommend` (U+E9D2).
    Recommend,
    /// Material Symbols `record_voice_over` (U+E91F).
    RecordVoiceOver,
    /// Material Symbols `rectangle` (U+EB54).
    Rectangle,
    /// Material Symbols `rectangle_add` (U+EEC8).
    RectangleAdd,
    /// Material Symbols `recycling` (U+E760).
    Recycling,
    /// Material Symbols `redeem` (U+E8F6).
    Redeem,
    /// Material Symbols `reduce_capacity` (U+F21C).
    ReduceCapacity,
    /// Material Symbols `regular_expression` (U+F750).
    RegularExpression,
    /// Material Symbols `relax` (U+F6DC).
    Relax,
    /// Material Symbols `release_alert` (U+F654).
    ReleaseAlert,
    /// Material Symbols `remember_me` (U+F051).
    RememberMe,
    /// Material Symbols `reminder` (U+E6C6).
    Reminder,
    /// Material Symbols `reminders_alt` (U+E6C6).
    RemindersAlt,
    /// Material Symbols `remote_gen` (U+E83E).
    RemoteGen,
    /// Material Symbols `remove` (U+E15B).
    Remove,
    /// Material Symbols `remove_circle` (U+F08F).
    RemoveCircle,
    /// Material Symbols `remove_circle_outline` (U+F08F).
    RemoveCircleOutline,
    /// Material Symbols `remove_done` (U+E9D3).
    RemoveDone,
    /// Material Symbols `remove_from_queue` (U+E067).
    RemoveFromQueue,
    /// Material Symbols `remove_moderator` (U+E9D4).
    RemoveModerator,
    /// Material Symbols `remove_red_eye` (U+E8F4).
    RemoveRedEye,
    /// Material Symbols `remove_road` (U+EBFC).
    RemoveRoad,
    /// Material Symbols `remove_selection` (U+E9D5).
    RemoveSelection,
    /// Material Symbols `remove_shopping_cart` (U+E928).
    RemoveShoppingCart,
    /// Material Symbols `reopen_window` (U+F708).
    ReopenWindow,
    /// Material Symbols `reorder` (U+E8FE).
    Reorder,
    /// Material Symbols `repartition` (U+F8E8).
    Repartition,
    /// Material Symbols `repeat` (U+E040).
    Repeat,
    /// Material Symbols `repeat_on` (U+E9D6).
    RepeatOn,
    /// Material Symbols `repeat_one` (U+E041).
    RepeatOne,
    /// Material Symbols `repeat_one_on` (U+E9D7).
    RepeatOneOn,
    /// Material Symbols `replace_audio` (U+F451).
    ReplaceAudio,
    /// Material Symbols `replace_image` (U+F450).
    ReplaceImage,
    /// Material Symbols `replace_video` (U+F44F).
    ReplaceVideo,
    /// Material Symbols `replay` (U+E042).
    Replay,
    /// Material Symbols `replay_10` (U+E059).
    Replay10,
    /// Material Symbols `replay_30` (U+E05A).
    Replay30,
    /// Material Symbols `replay_5` (U+E05B).
    Replay5,
    /// Material Symbols `replay_circle_filled` (U+E9D8).
    ReplayCircleFilled,
    /// Material Symbols `reply` (U+E15E).
    Reply,
    /// Material Symbols `reply_all` (U+E15F).
    ReplyAll,
    /// Material Symbols `report` (U+F052).
    Report,
    /// Material Symbols `report_gmailerrorred` (U+F052).
    ReportGmailerrorred,
    /// Material Symbols `report_off` (U+E170).
    ReportOff,
    /// Material Symbols `report_problem` (U+F083).
    ReportProblem,
    /// Material Symbols `request_page` (U+F22C).
    RequestPage,
    /// Material Symbols `request_quote` (U+F1B6).
    RequestQuote,
    /// Material Symbols `reset_brightness` (U+F482).
    ResetBrightness,
    /// Material Symbols `reset_colors` (U+FFEE2).
    ResetColors,
    /// Material Symbols `reset_exposure` (U+F266).
    ResetExposure,
    /// Material Symbols `reset_focus` (U+F481).
    ResetFocus,
    /// Material Symbols `reset_image` (U+F824).
    ResetImage,
    /// Material Symbols `reset_iso` (U+F480).
    ResetIso,
    /// Material Symbols `reset_settings` (U+F47F).
    ResetSettings,
    /// Material Symbols `reset_shadow` (U+F47E).
    ResetShadow,
    /// Material Symbols `reset_shutter_speed` (U+F47D).
    ResetShutterSpeed,
    /// Material Symbols `reset_tv` (U+E9D9).
    ResetTv,
    /// Material Symbols `reset_white_balance` (U+F47C).
    ResetWhiteBalance,
    /// Material Symbols `reset_wrench` (U+F56C).
    ResetWrench,
    /// Material Symbols `resize` (U+F707).
    Resize,
    /// Material Symbols `resize_window` (U+FFF99).
    ResizeWindow,
    /// Material Symbols `respiratory_rate` (U+E127).
    RespiratoryRate,
    /// Material Symbols `responsive_layout` (U+E9DA).
    ResponsiveLayout,
    /// Material Symbols `rest_area` (U+F22A).
    RestArea,
    /// Material Symbols `restart_alt` (U+F053).
    RestartAlt,
    /// Material Symbols `restaurant` (U+E56C).
    Restaurant,
    /// Material Symbols `restaurant_menu` (U+E561).
    RestaurantMenu,
    /// Material Symbols `restore` (U+E8B3).
    Restore,
    /// Material Symbols `restore_from_trash` (U+E938).
    RestoreFromTrash,
    /// Material Symbols `restore_page` (U+E929).
    RestorePage,
    /// Material Symbols `resume` (U+F7D0).
    Resume,
    /// Material Symbols `reviews` (U+F07C).
    Reviews,
    /// Material Symbols `rewarded_ads` (U+EFB6).
    RewardedAds,
    /// Material Symbols `rheumatology` (U+E128).
    Rheumatology,
    /// Material Symbols `rib_cage` (U+F898).
    RibCage,
    /// Material Symbols `rice_bowl` (U+F1F5).
    RiceBowl,
    /// Material Symbols `right_click` (U+F706).
    RightClick,
    /// Material Symbols `right_panel_close` (U+F705).
    RightPanelClose,
    /// Material Symbols `right_panel_open` (U+F704).
    RightPanelOpen,
    /// Material Symbols `ring_volume` (U+F0DD).
    RingVolume,
    /// Material Symbols `ring_volume_filled` (U+F0DD).
    RingVolumeFilled,
    /// Material Symbols `ripples` (U+E9DB).
    Ripples,
    /// Material Symbols `road` (U+F472).
    Road,
    /// Material Symbols `robot` (U+F882).
    Robot,
    /// Material Symbols `robot_2` (U+F5D0).
    Robot2,
    /// Material Symbols `rocket` (U+EBA5).
    Rocket,
    /// Material Symbols `rocket_launch` (U+EB9B).
    RocketLaunch,
    /// Material Symbols `roller_shades` (U+EC12).
    RollerShades,
    /// Material Symbols `roller_shades_closed` (U+EC11).
    RollerShadesClosed,
    /// Material Symbols `roller_skating` (U+EBCD).
    RollerSkating,
    /// Material Symbols `roofing` (U+F201).
    Roofing,
    /// Material Symbols `room` (U+F1DB).
    Room,
    /// Material Symbols `room_preferences` (U+F1B8).
    RoomPreferences,
    /// Material Symbols `room_service` (U+EB49).
    RoomService,
    /// Material Symbols `rotate_90_degrees_ccw` (U+E418).
    Rotate90DegreesCcw,
    /// Material Symbols `rotate_90_degrees_cw` (U+EAAB).
    Rotate90DegreesCw,
    /// Material Symbols `rotate_auto` (U+F417).
    RotateAuto,
    /// Material Symbols `rotate_left` (U+E419).
    RotateLeft,
    /// Material Symbols `rotate_right` (U+E41A).
    RotateRight,
    /// Material Symbols `roundabout_left` (U+EB99).
    RoundaboutLeft,
    /// Material Symbols `roundabout_right` (U+EBA3).
    RoundaboutRight,
    /// Material Symbols `rounded_corner` (U+E920).
    RoundedCorner,
    /// Material Symbols `route` (U+EACD).
    Route,
    /// Material Symbols `router` (U+E328).
    Router,
    /// Material Symbols `router_off` (U+F2F4).
    RouterOff,
    /// Material Symbols `routine` (U+E20C).
    Routine,
    /// Material Symbols `rowing` (U+E921).
    Rowing,
    /// Material Symbols `rss_feed` (U+E0E5).
    RssFeed,
    /// Material Symbols `rsvp` (U+F055).
    Rsvp,
    /// Material Symbols `rtt` (U+E9AD).
    Rtt,
    /// Material Symbols `rubric` (U+EB27).
    Rubric,
    /// Material Symbols `rule` (U+F1C2).
    Rule,
    /// Material Symbols `rule_folder` (U+F1C9).
    RuleFolder,
    /// Material Symbols `rule_settings` (U+F64C).
    RuleSettings,
    /// Material Symbols `run_circle` (U+EF6F).
    RunCircle,
    /// Material Symbols `running_with_errors` (U+E51D).
    RunningWithErrors,
    /// Material Symbols `rv_hookup` (U+E642).
    RvHookup,
    /// Material Symbols `safety_check` (U+EBEF).
    SafetyCheck,
    /// Material Symbols `safety_check_off` (U+F59D).
    SafetyCheckOff,
    /// Material Symbols `safety_divider` (U+E1CC).
    SafetyDivider,
    /// Material Symbols `sailing` (U+E502).
    Sailing,
    /// Material Symbols `salinity` (U+F876).
    Salinity,
    /// Material Symbols `sanitizer` (U+F21D).
    Sanitizer,
    /// Material Symbols `satellite` (U+E562).
    Satellite,
    /// Material Symbols `satellite_alt` (U+EB3A).
    SatelliteAlt,
    /// Material Symbols `sauna` (U+F6F7).
    Sauna,
    /// Material Symbols `save_alt` (U+F090).
    SaveAlt,
    /// Material Symbols `save_as` (U+EB60).
    SaveAs,
    /// Material Symbols `save_clock` (U+F398).
    SaveClock,
    /// Material Symbols `saved_search` (U+EA11).
    SavedSearch,
    /// Material Symbols `savings` (U+E2EB).
    Savings,
    /// Material Symbols `scale` (U+EB5F).
    Scale,
    /// Material Symbols `scan` (U+F74E).
    Scan,
    /// Material Symbols `scan_delete` (U+F74F).
    ScanDelete,
    /// Material Symbols `scanner` (U+E329).
    Scanner,
    /// Material Symbols `scatter_plot` (U+E268).
    ScatterPlot,
    /// Material Symbols `scene` (U+E2A7).
    Scene,
    /// Material Symbols `schedule_send` (U+EA0A).
    ScheduleSend,
    /// Material Symbols `schema` (U+E4FD).
    Schema,
    /// Material Symbols `school` (U+E80C).
    School,
    /// Material Symbols `science` (U+EA4B).
    Science,
    /// Material Symbols `science_off` (U+F542).
    ScienceOff,
    /// Material Symbols `scooter` (U+F471).
    Scooter,
    /// Material Symbols `score` (U+E269).
    Score,
    /// Material Symbols `scoreboard` (U+EBD0).
    Scoreboard,
    /// Material Symbols `screen_lock_landscape` (U+F2D8).
    ScreenLockLandscape,
    /// Material Symbols `screen_lock_portrait` (U+F2BE).
    ScreenLockPortrait,
    /// Material Symbols `screen_lock_rotation` (U+F2D6).
    ScreenLockRotation,
    /// Material Symbols `screen_record` (U+F679).
    ScreenRecord,
    /// Material Symbols `screen_rotation` (U+F2D5).
    ScreenRotation,
    /// Material Symbols `screen_rotation_alt` (U+EBEE).
    ScreenRotationAlt,
    /// Material Symbols `screen_rotation_up` (U+F678).
    ScreenRotationUp,
    /// Material Symbols `screen_search_desktop` (U+EF70).
    ScreenSearchDesktop,
    /// Material Symbols `screen_share` (U+E0E2).
    ScreenShare,
    /// Material Symbols `screenshot` (U+F056).
    Screenshot,
    /// Material Symbols `screenshot_frame` (U+F677).
    ScreenshotFrame,
    /// Material Symbols `screenshot_frame_2` (U+F374).
    ScreenshotFrame2,
    /// Material Symbols `screenshot_keyboard` (U+F7D3).
    ScreenshotKeyboard,
    /// Material Symbols `screenshot_monitor` (U+EC08).
    ScreenshotMonitor,
    /// Material Symbols `screenshot_region` (U+F7D2).
    ScreenshotRegion,
    /// Material Symbols `screenshot_tablet` (U+F697).
    ScreenshotTablet,
    /// Material Symbols `script` (U+F45F).
    Script,
    /// Material Symbols `scrollable_header` (U+E9DC).
    ScrollableHeader,
    /// Material Symbols `scuba_diving` (U+EBCE).
    ScubaDiving,
    /// Material Symbols `sd` (U+E9DD).
    Sd,
    /// Material Symbols `sd_card` (U+E623).
    SdCard,
    /// Material Symbols `sd_card_alert` (U+F057).
    SdCardAlert,
    /// Material Symbols `sd_storage` (U+E623).
    SdStorage,
    /// Material Symbols `sdk` (U+E720).
    Sdk,
    /// Material Symbols `search_activity` (U+F3E5).
    SearchActivity,
    /// Material Symbols `search_check` (U+F800).
    SearchCheck,
    /// Material Symbols `search_check_2` (U+F469).
    SearchCheck2,
    /// Material Symbols `search_gear` (U+EEFA).
    SearchGear,
    /// Material Symbols `search_hands_free` (U+E696).
    SearchHandsFree,
    /// Material Symbols `search_insights` (U+F4BC).
    SearchInsights,
    /// Material Symbols `search_off` (U+EA76).
    SearchOff,
    /// Material Symbols `seat_cool_left` (U+F331).
    SeatCoolLeft,
    /// Material Symbols `seat_cool_right` (U+F330).
    SeatCoolRight,
    /// Material Symbols `seat_heat_left` (U+F32F).
    SeatHeatLeft,
    /// Material Symbols `seat_heat_right` (U+F32E).
    SeatHeatRight,
    /// Material Symbols `seat_read` (U+FFEDA).
    SeatRead,
    /// Material Symbols `seat_vent_left` (U+F32D).
    SeatVentLeft,
    /// Material Symbols `seat_vent_right` (U+F32C).
    SeatVentRight,
    /// Material Symbols `seat_window` (U+FFEE1).
    SeatWindow,
    /// Material Symbols `security` (U+E32A).
    Security,
    /// Material Symbols `security_key` (U+F503).
    SecurityKey,
    /// Material Symbols `security_update` (U+F2CD).
    SecurityUpdate,
    /// Material Symbols `security_update_good` (U+F073).
    SecurityUpdateGood,
    /// Material Symbols `security_update_warning` (U+F2D3).
    SecurityUpdateWarning,
    /// Material Symbols `segment` (U+E94B).
    Segment,
    /// Material Symbols `select` (U+F74D).
    Select,
    /// Material Symbols `select_all` (U+E162).
    SelectAll,
    /// Material Symbols `select_check_box` (U+F1FE).
    SelectCheckBox,
    /// Material Symbols `select_to_speak` (U+F7CF).
    SelectToSpeak,
    /// Material Symbols `select_window` (U+E6FA).
    SelectWindow,
    /// Material Symbols `select_window_2` (U+F4C8).
    SelectWindow2,
    /// Material Symbols `select_window_off` (U+E506).
    SelectWindowOff,
    /// Material Symbols `self_care` (U+F86D).
    SelfCare,
    /// Material Symbols `self_improvement` (U+EA78).
    SelfImprovement,
    /// Material Symbols `sell` (U+F05B).
    Sell,
    /// Material Symbols `sell_cloud` (U+FFF7B).
    SellCloud,
    /// Material Symbols `send_and_archive` (U+EA0C).
    SendAndArchive,
    /// Material Symbols `send_money` (U+E8B7).
    SendMoney,
    /// Material Symbols `send_time_extension` (U+EADB).
    SendTimeExtension,
    /// Material Symbols `send_to_mobile` (U+F2D2).
    SendToMobile,
    /// Material Symbols `sensor_door` (U+F1B5).
    SensorDoor,
    /// Material Symbols `sensor_occupied` (U+EC10).
    SensorOccupied,
    /// Material Symbols `sensor_window` (U+F1B4).
    SensorWindow,
    /// Material Symbols `sensors` (U+E51E).
    Sensors,
    /// Material Symbols `sensors_krx` (U+F556).
    SensorsKrx,
    /// Material Symbols `sensors_krx_off` (U+F515).
    SensorsKrxOff,
    /// Material Symbols `sensors_off` (U+E51F).
    SensorsOff,
    /// Material Symbols `sentiment_calm` (U+F6A7).
    SentimentCalm,
    /// Material Symbols `sentiment_content` (U+F6A6).
    SentimentContent,
    /// Material Symbols `sentiment_dissatisfied` (U+E811).
    SentimentDissatisfied,
    /// Material Symbols `sentiment_excited` (U+F6A5).
    SentimentExcited,
    /// Material Symbols `sentiment_extremely_dissatisfied` (U+F194).
    SentimentExtremelyDissatisfied,
    /// Material Symbols `sentiment_frustrated` (U+F6A4).
    SentimentFrustrated,
    /// Material Symbols `sentiment_neutral` (U+E812).
    SentimentNeutral,
    /// Material Symbols `sentiment_sad` (U+F6A3).
    SentimentSad,
    /// Material Symbols `sentiment_satisfied` (U+E813).
    SentimentSatisfied,
    /// Material Symbols `sentiment_satisfied_alt` (U+E813).
    SentimentSatisfiedAlt,
    /// Material Symbols `sentiment_stressed` (U+F6A2).
    SentimentStressed,
    /// Material Symbols `sentiment_very_dissatisfied` (U+E814).
    SentimentVeryDissatisfied,
    /// Material Symbols `sentiment_very_satisfied` (U+E815).
    SentimentVerySatisfied,
    /// Material Symbols `sentiment_worried` (U+F6A1).
    SentimentWorried,
    /// Material Symbols `serif` (U+F4AC).
    Serif,
    /// Material Symbols `server_person` (U+F3BD).
    ServerPerson,
    /// Material Symbols `service_toolbox` (U+E717).
    ServiceToolbox,
    /// Material Symbols `set_meal` (U+F1EA).
    SetMeal,
    /// Material Symbols `settings_accessibility` (U+F05D).
    SettingsAccessibility,
    /// Material Symbols `settings_account_box` (U+F835).
    SettingsAccountBox,
    /// Material Symbols `settings_alert` (U+F143).
    SettingsAlert,
    /// Material Symbols `settings_applications` (U+E8B9).
    SettingsApplications,
    /// Material Symbols `settings_b_roll` (U+F625).
    SettingsBRoll,
    /// Material Symbols `settings_backup_restore` (U+E8BA).
    SettingsBackupRestore,
    /// Material Symbols `settings_bluetooth` (U+E8BB).
    SettingsBluetooth,
    /// Material Symbols `settings_brightness` (U+E8BD).
    SettingsBrightness,
    /// Material Symbols `settings_cell` (U+F2D1).
    SettingsCell,
    /// Material Symbols `settings_cinematic_blur` (U+F624).
    SettingsCinematicBlur,
    /// Material Symbols `settings_ethernet` (U+E8BE).
    SettingsEthernet,
    /// Material Symbols `settings_heart` (U+F522).
    SettingsHeart,
    /// Material Symbols `settings_input_antenna` (U+E8BF).
    SettingsInputAntenna,
    /// Material Symbols `settings_input_component` (U+E8C1).
    SettingsInputComponent,
    /// Material Symbols `settings_input_composite` (U+E8C1).
    SettingsInputComposite,
    /// Material Symbols `settings_input_hdmi` (U+E8C2).
    SettingsInputHdmi,
    /// Material Symbols `settings_input_svideo` (U+E8C3).
    SettingsInputSvideo,
    /// Material Symbols `settings_motion_mode` (U+F833).
    SettingsMotionMode,
    /// Material Symbols `settings_night_sight` (U+F832).
    SettingsNightSight,
    /// Material Symbols `settings_overscan` (U+E8C4).
    SettingsOverscan,
    /// Material Symbols `settings_panorama` (U+F831).
    SettingsPanorama,
    /// Material Symbols `settings_phone` (U+E8C5).
    SettingsPhone,
    /// Material Symbols `settings_photo_camera` (U+F834).
    SettingsPhotoCamera,
    /// Material Symbols `settings_power` (U+E8C6).
    SettingsPower,
    /// Material Symbols `settings_remote` (U+E8C7).
    SettingsRemote,
    /// Material Symbols `settings_screen` (U+FFEDF).
    SettingsScreen,
    /// Material Symbols `settings_seating` (U+EF2D).
    SettingsSeating,
    /// Material Symbols `settings_slow_motion` (U+F623).
    SettingsSlowMotion,
    /// Material Symbols `settings_suggest` (U+F05E).
    SettingsSuggest,
    /// Material Symbols `settings_system_daydream` (U+E1C3).
    SettingsSystemDaydream,
    /// Material Symbols `settings_timelapse` (U+F622).
    SettingsTimelapse,
    /// Material Symbols `settings_video_camera` (U+F621).
    SettingsVideoCamera,
    /// Material Symbols `settings_voice` (U+E8C8).
    SettingsVoice,
    /// Material Symbols `settop_component` (U+E2AC).
    SettopComponent,
    /// Material Symbols `severe_cold` (U+EBD3).
    SevereCold,
    /// Material Symbols `shades` (U+FFF73).
    Shades,
    /// Material Symbols `shades_closed` (U+FFF74).
    ShadesClosed,
    /// Material Symbols `shadow` (U+E9DF).
    Shadow,
    /// Material Symbols `shadow_add` (U+F584).
    ShadowAdd,
    /// Material Symbols `shadow_minus` (U+F583).
    ShadowMinus,
    /// Material Symbols `shape_line` (U+F8D3).
    ShapeLine,
    /// Material Symbols `shape_recognition` (U+EB01).
    ShapeRecognition,
    /// Material Symbols `shapes` (U+E602).
    Shapes,
    /// Material Symbols `share_eta` (U+E5F7).
    ShareEta,
    /// Material Symbols `share_location` (U+F05F).
    ShareLocation,
    /// Material Symbols `share_off` (U+F6CB).
    ShareOff,
    /// Material Symbols `share_reviews` (U+F8A4).
    ShareReviews,
    /// Material Symbols `share_windows` (U+F613).
    ShareWindows,
    /// Material Symbols `shaved_ice` (U+F225).
    ShavedIce,
    /// Material Symbols `sheets_column_swap` (U+FFE9D).
    SheetsColumnSwap,
    /// Material Symbols `sheets_rtl` (U+F823).
    SheetsRtl,
    /// Material Symbols `shelf_auto_hide` (U+F703).
    ShelfAutoHide,
    /// Material Symbols `shelf_position` (U+F702).
    ShelfPosition,
    /// Material Symbols `shelves` (U+F86E).
    Shelves,
    /// Material Symbols `shield` (U+E9E0).
    Shield,
    /// Material Symbols `shield_card` (U+FFF30).
    ShieldCard,
    /// Material Symbols `shield_lock` (U+F686).
    ShieldLock,
    /// Material Symbols `shield_locked` (U+F592).
    ShieldLocked,
    /// Material Symbols `shield_moon` (U+EAA9).
    ShieldMoon,
    /// Material Symbols `shield_person` (U+F650).
    ShieldPerson,
    /// Material Symbols `shield_question` (U+F529).
    ShieldQuestion,
    /// Material Symbols `shield_radar` (U+FFF2F).
    ShieldRadar,
    /// Material Symbols `shield_toggle` (U+F2AD).
    ShieldToggle,
    /// Material Symbols `shield_watch` (U+F30F).
    ShieldWatch,
    /// Material Symbols `shield_with_heart` (U+E78F).
    ShieldWithHeart,
    /// Material Symbols `shield_with_house` (U+E78D).
    ShieldWithHouse,
    /// Material Symbols `shift` (U+E5F2).
    Shift,
    /// Material Symbols `shift_lock` (U+F7AE).
    ShiftLock,
    /// Material Symbols `shift_lock_off` (U+F483).
    ShiftLockOff,
    /// Material Symbols `shoe_cleats` (U+FFFB3).
    ShoeCleats,
    /// Material Symbols `shop` (U+E8C9).
    Shop,
    /// Material Symbols `shop_2` (U+E8CA).
    Shop2,
    /// Material Symbols `shop_two` (U+E8CA).
    ShopTwo,
    /// Material Symbols `shopping_bag` (U+F1CC).
    ShoppingBag,
    /// Material Symbols `shopping_bag_speed` (U+F39A).
    ShoppingBagSpeed,
    /// Material Symbols `shopping_basket` (U+E8CB).
    ShoppingBasket,
    /// Material Symbols `shopping_cart` (U+E8CC).
    ShoppingCart,
    /// Material Symbols `shopping_cart_checkout` (U+EB88).
    ShoppingCartCheckout,
    /// Material Symbols `shopping_cart_off` (U+F4F7).
    ShoppingCartOff,
    /// Material Symbols `shoppingmode` (U+EFB7).
    Shoppingmode,
    /// Material Symbols `short_stay` (U+E4D0).
    ShortStay,
    /// Material Symbols `short_text` (U+E261).
    ShortText,
    /// Material Symbols `shortcut` (U+F57A).
    Shortcut,
    /// Material Symbols `show_chart` (U+E6E1).
    ShowChart,
    /// Material Symbols `shower` (U+F061).
    Shower,
    /// Material Symbols `shuffle` (U+E043).
    Shuffle,
    /// Material Symbols `shuffle_on` (U+E9E1).
    ShuffleOn,
    /// Material Symbols `shutter_speed` (U+E43D).
    ShutterSpeed,
    /// Material Symbols `shutter_speed_add` (U+F57E).
    ShutterSpeedAdd,
    /// Material Symbols `shutter_speed_minus` (U+F57D).
    ShutterSpeedMinus,
    /// Material Symbols `sick` (U+F220).
    Sick,
    /// Material Symbols `side_navigation` (U+E9E2).
    SideNavigation,
    /// Material Symbols `sign_language` (U+EBE5).
    SignLanguage,
    /// Material Symbols `sign_language_2` (U+F258).
    SignLanguage2,
    /// Material Symbols `sign_language_off` (U+FFEE4).
    SignLanguageOff,
    /// Material Symbols `signal_cellular_0_bar` (U+F0A8).
    SignalCellular0Bar,
    /// Material Symbols `signal_cellular_1_bar` (U+F0A9).
    SignalCellular1Bar,
    /// Material Symbols `signal_cellular_2_bar` (U+F0AA).
    SignalCellular2Bar,
    /// Material Symbols `signal_cellular_3_bar` (U+F0AB).
    SignalCellular3Bar,
    /// Material Symbols `signal_cellular_4_bar` (U+E1C8).
    SignalCellular4Bar,
    /// Material Symbols `signal_cellular_add` (U+F7A9).
    SignalCellularAdd,
    /// Material Symbols `signal_cellular_alt` (U+E202).
    SignalCellularAlt,
    /// Material Symbols `signal_cellular_alt_1_bar` (U+EBDF).
    SignalCellularAlt1Bar,
    /// Material Symbols `signal_cellular_alt_2_bar` (U+EBE3).
    SignalCellularAlt2Bar,
    /// Material Symbols `signal_cellular_alt_off` (U+FFF8A).
    SignalCellularAltOff,
    /// Material Symbols `signal_cellular_connected_no_internet_0_bar` (U+F0AC).
    SignalCellularConnectedNoInternet0Bar,
    /// Material Symbols `signal_cellular_connected_no_internet_4_bar` (U+E1CD).
    SignalCellularConnectedNoInternet4Bar,
    /// Material Symbols `signal_cellular_no_sim` (U+E1CE).
    SignalCellularNoSim,
    /// Material Symbols `signal_cellular_nodata` (U+F062).
    SignalCellularNodata,
    /// Material Symbols `signal_cellular_null` (U+E1CF).
    SignalCellularNull,
    /// Material Symbols `signal_cellular_off` (U+E1D0).
    SignalCellularOff,
    /// Material Symbols `signal_cellular_pause` (U+F5A7).
    SignalCellularPause,
    /// Material Symbols `signal_disconnected` (U+F239).
    SignalDisconnected,
    /// Material Symbols `signal_wifi_0_bar` (U+F0B0).
    SignalWifi0Bar,
    /// Material Symbols `signal_wifi_4_bar` (U+F065).
    SignalWifi4Bar,
    /// Material Symbols `signal_wifi_4_bar_lock` (U+E1E1).
    SignalWifi4BarLock,
    /// Material Symbols `signal_wifi_bad` (U+F064).
    SignalWifiBad,
    /// Material Symbols `signal_wifi_connected_no_internet_4` (U+F064).
    SignalWifiConnectedNoInternet4,
    /// Material Symbols `signal_wifi_off` (U+E1DA).
    SignalWifiOff,
    /// Material Symbols `signal_wifi_statusbar_4_bar` (U+F065).
    SignalWifiStatusbar4Bar,
    /// Material Symbols `signal_wifi_statusbar_not_connected` (U+F0EF).
    SignalWifiStatusbarNotConnected,
    /// Material Symbols `signal_wifi_statusbar_null` (U+F067).
    SignalWifiStatusbarNull,
    /// Material Symbols `signature` (U+F74C).
    Signature,
    /// Material Symbols `signpost` (U+EB91).
    Signpost,
    /// Material Symbols `sim_card` (U+E32B).
    SimCard,
    /// Material Symbols `sim_card_alert` (U+F057).
    SimCardAlert,
    /// Material Symbols `sim_card_download` (U+F068).
    SimCardDownload,
    /// Material Symbols `sim_card_lock` (U+FFEC1).
    SimCardLock,
    /// Material Symbols `simulation` (U+F3E1).
    Simulation,
    /// Material Symbols `single_arrow` (U+FFED9).
    SingleArrow,
    /// Material Symbols `single_bed` (U+EA48).
    SingleBed,
    /// Material Symbols `sip` (U+F069).
    Sip,
    /// Material Symbols `siren` (U+F3A7).
    Siren,
    /// Material Symbols `siren_check` (U+F3A6).
    SirenCheck,
    /// Material Symbols `siren_open` (U+F3A5).
    SirenOpen,
    /// Material Symbols `siren_question` (U+F3A4).
    SirenQuestion,
    /// Material Symbols `skateboarding` (U+E511).
    Skateboarding,
    /// Material Symbols `skeleton` (U+F899).
    Skeleton,
    /// Material Symbols `skillet` (U+F543).
    Skillet,
    /// Material Symbols `skillet_cooktop` (U+F544).
    SkilletCooktop,
    /// Material Symbols `skip_previous` (U+E045).
    SkipPrevious,
    /// Material Symbols `skull` (U+F89A).
    Skull,
    /// Material Symbols `skull_list` (U+F370).
    SkullList,
    /// Material Symbols `slab_serif` (U+F4AB).
    SlabSerif,
    /// Material Symbols `sledding` (U+E512).
    Sledding,
    /// Material Symbols `sleep` (U+E213).
    Sleep,
    /// Material Symbols `sleep_score` (U+F6B7).
    SleepScore,
    /// Material Symbols `slide_library` (U+F822).
    SlideLibrary,
    /// Material Symbols `sliders` (U+E9E3).
    Sliders,
    /// Material Symbols `slideshow` (U+E41B).
    Slideshow,
    /// Material Symbols `slow_motion_video` (U+E068).
    SlowMotionVideo,
    /// Material Symbols `smart_button` (U+F1C1).
    SmartButton,
    /// Material Symbols `smart_card_reader` (U+F4A5).
    SmartCardReader,
    /// Material Symbols `smart_card_reader_off` (U+F4A6).
    SmartCardReaderOff,
    /// Material Symbols `smart_display` (U+F06A).
    SmartDisplay,
    /// Material Symbols `smart_outlet` (U+E844).
    SmartOutlet,
    /// Material Symbols `smart_screen` (U+F2D0).
    SmartScreen,
    /// Material Symbols `smart_toy` (U+F06C).
    SmartToy,
    /// Material Symbols `smartphone` (U+E7BA).
    Smartphone,
    /// Material Symbols `smartphone_camera` (U+F44E).
    SmartphoneCamera,
    /// Material Symbols `smb_share` (U+F74B).
    SmbShare,
    /// Material Symbols `smoke_free` (U+EB4A).
    SmokeFree,
    /// Material Symbols `smoking_rooms` (U+EB4B).
    SmokingRooms,
    /// Material Symbols `sms` (U+E625).
    Sms,
    /// Material Symbols `sms_failed` (U+E87F).
    SmsFailed,
    /// Material Symbols `snail` (U+FFEDE).
    Snail,
    /// Material Symbols `snippet_folder` (U+F1C7).
    SnippetFolder,
    /// Material Symbols `snooze` (U+E046).
    Snooze,
    /// Material Symbols `snowboarding` (U+E513).
    Snowboarding,
    /// Material Symbols `snowflake` (U+ED5B).
    Snowflake,
    /// Material Symbols `snowing` (U+E80F).
    Snowing,
    /// Material Symbols `snowing_heavy` (U+F61C).
    SnowingHeavy,
    /// Material Symbols `snowmobile` (U+E503).
    Snowmobile,
    /// Material Symbols `snowshoeing` (U+E514).
    Snowshoeing,
    /// Material Symbols `soap` (U+F1B2).
    Soap,
    /// Material Symbols `soba` (U+EF36).
    Soba,
    /// Material Symbols `social_distance` (U+E1CB).
    SocialDistance,
    /// Material Symbols `social_leaderboard` (U+F6A0).
    SocialLeaderboard,
    /// Material Symbols `solar_power` (U+EC0F).
    SolarPower,
    /// Material Symbols `solo_dining` (U+EF35).
    SoloDining,
    /// Material Symbols `sort_by_alpha` (U+E053).
    SortByAlpha,
    /// Material Symbols `sos` (U+EBF7).
    Sos,
    /// Material Symbols `sound_detection_dog_barking` (U+F149).
    SoundDetectionDogBarking,
    /// Material Symbols `sound_detection_glass_break` (U+F14A).
    SoundDetectionGlassBreak,
    /// Material Symbols `sound_detection_loud_sound` (U+F14B).
    SoundDetectionLoudSound,
    /// Material Symbols `sound_sampler` (U+F6B4).
    SoundSampler,
    /// Material Symbols `soundbar` (U+FFF72).
    Soundbar,
    /// Material Symbols `soup_kitchen` (U+E7D3).
    SoupKitchen,
    /// Material Symbols `source` (U+F1C8).
    Source,
    /// Material Symbols `source_environment` (U+E527).
    SourceEnvironment,
    /// Material Symbols `source_notes` (U+E12D).
    SourceNotes,
    /// Material Symbols `south` (U+F1E3).
    South,
    /// Material Symbols `south_america` (U+E7E4).
    SouthAmerica,
    /// Material Symbols `south_east` (U+F1E4).
    SouthEast,
    /// Material Symbols `south_west` (U+F1E5).
    SouthWest,
    /// Material Symbols `spa` (U+EB4C).
    Spa,
    /// Material Symbols `space_bar` (U+E256).
    SpaceBar,
    /// Material Symbols `space_dashboard` (U+E66B).
    SpaceDashboard,
    /// Material Symbols `space_dashboard_2` (U+FFF8C).
    SpaceDashboard2,
    /// Material Symbols `spatial_audio` (U+EBEB).
    SpatialAudio,
    /// Material Symbols `spatial_audio_off` (U+EBE8).
    SpatialAudioOff,
    /// Material Symbols `spatial_gallery` (U+FFEB6).
    SpatialGallery,
    /// Material Symbols `spatial_speaker` (U+F4CF).
    SpatialSpeaker,
    /// Material Symbols `spatial_tracking` (U+EBEA).
    SpatialTracking,
    /// Material Symbols `speaker` (U+E32D).
    Speaker,
    /// Material Symbols `speaker_2` (U+FFF71).
    Speaker2,
    /// Material Symbols `speaker_3` (U+FFEB7).
    Speaker3,
    /// Material Symbols `speaker_group` (U+E32E).
    SpeakerGroup,
    /// Material Symbols `speaker_notes` (U+E8CD).
    SpeakerNotes,
    /// Material Symbols `speaker_notes_off` (U+E92A).
    SpeakerNotesOff,
    /// Material Symbols `speaker_phone` (U+E0D2).
    SpeakerPhone,
    /// Material Symbols `special_character` (U+F74A).
    SpecialCharacter,
    /// Material Symbols `specific_gravity` (U+F872).
    SpecificGravity,
    /// Material Symbols `speech_to_text` (U+F8A7).
    SpeechToText,
    /// Material Symbols `speech_to_text_2` (U+FFEC9).
    SpeechToText2,
    /// Material Symbols `speed` (U+E9E4).
    Speed,
    /// Material Symbols `speed_0_25` (U+F4D4).
    Speed025,
    /// Material Symbols `speed_0_2x` (U+F498).
    Speed02x,
    /// Material Symbols `speed_0_5` (U+F4E2).
    Speed05,
    /// Material Symbols `speed_0_5x` (U+F497).
    Speed05x,
    /// Material Symbols `speed_0_75` (U+F4D3).
    Speed075,
    /// Material Symbols `speed_0_7x` (U+F496).
    Speed07x,
    /// Material Symbols `speed_1_2` (U+F4E1).
    Speed12,
    /// Material Symbols `speed_1_25` (U+F4D2).
    Speed125,
    /// Material Symbols `speed_1_2x` (U+F495).
    Speed12x,
    /// Material Symbols `speed_1_5` (U+F4E0).
    Speed15,
    /// Material Symbols `speed_1_5x` (U+F494).
    Speed15x,
    /// Material Symbols `speed_1_75` (U+F4D1).
    Speed175,
    /// Material Symbols `speed_1_7x` (U+F493).
    Speed17x,
    /// Material Symbols `speed_2` (U+FFF38).
    Speed2,
    /// Material Symbols `speed_2x` (U+F4EB).
    Speed2x,
    /// Material Symbols `speed_3` (U+FFF37).
    Speed3,
    /// Material Symbols `speed_4` (U+FFF36).
    Speed4,
    /// Material Symbols `speed_camera` (U+F470).
    SpeedCamera,
    /// Material Symbols `spellcheck` (U+E8CE).
    Spellcheck,
    /// Material Symbols `split_scene` (U+F3BF).
    SplitScene,
    /// Material Symbols `split_scene_2` (U+FFEF7).
    SplitScene2,
    /// Material Symbols `split_scene_down` (U+F2FF).
    SplitSceneDown,
    /// Material Symbols `split_scene_left` (U+F2FE).
    SplitSceneLeft,
    /// Material Symbols `split_scene_right` (U+F2FD).
    SplitSceneRight,
    /// Material Symbols `split_scene_up` (U+F2FC).
    SplitSceneUp,
    /// Material Symbols `splitscreen` (U+F06D).
    Splitscreen,
    /// Material Symbols `splitscreen_add` (U+F4FD).
    SplitscreenAdd,
    /// Material Symbols `splitscreen_bottom` (U+F676).
    SplitscreenBottom,
    /// Material Symbols `splitscreen_landscape` (U+F459).
    SplitscreenLandscape,
    /// Material Symbols `splitscreen_landscape_add` (U+FFFBA).
    SplitscreenLandscapeAdd,
    /// Material Symbols `splitscreen_left` (U+F675).
    SplitscreenLeft,
    /// Material Symbols `splitscreen_portrait` (U+F458).
    SplitscreenPortrait,
    /// Material Symbols `splitscreen_right` (U+F674).
    SplitscreenRight,
    /// Material Symbols `splitscreen_top` (U+F673).
    SplitscreenTop,
    /// Material Symbols `splitscreen_vertical_add` (U+F4FC).
    SplitscreenVerticalAdd,
    /// Material Symbols `spo2` (U+F6DB).
    Spo2,
    /// Material Symbols `spoke` (U+E9A7).
    Spoke,
    /// Material Symbols `sports` (U+EA30).
    Sports,
    /// Material Symbols `sports_and_outdoors` (U+EFB8).
    SportsAndOutdoors,
    /// Material Symbols `sports_bar` (U+F1F3).
    SportsBar,
    /// Material Symbols `sports_baseball` (U+EA51).
    SportsBaseball,
    /// Material Symbols `sports_basketball` (U+EA26).
    SportsBasketball,
    /// Material Symbols `sports_cricket` (U+EA27).
    SportsCricket,
    /// Material Symbols `sports_esports` (U+EA28).
    SportsEsports,
    /// Material Symbols `sports_football` (U+EA29).
    SportsFootball,
    /// Material Symbols `sports_golf` (U+EA2A).
    SportsGolf,
    /// Material Symbols `sports_gymnastics` (U+EBC4).
    SportsGymnastics,
    /// Material Symbols `sports_handball` (U+EA33).
    SportsHandball,
    /// Material Symbols `sports_hockey` (U+EA2B).
    SportsHockey,
    /// Material Symbols `sports_kabaddi` (U+EA34).
    SportsKabaddi,
    /// Material Symbols `sports_martial_arts` (U+EAE9).
    SportsMartialArts,
    /// Material Symbols `sports_mma` (U+EA2C).
    SportsMma,
    /// Material Symbols `sports_motorsports` (U+EA2D).
    SportsMotorsports,
    /// Material Symbols `sports_rugby` (U+EA2E).
    SportsRugby,
    /// Material Symbols `sports_score` (U+F06E).
    SportsScore,
    /// Material Symbols `sports_soccer` (U+EA2F).
    SportsSoccer,
    /// Material Symbols `sports_tennis` (U+EA32).
    SportsTennis,
    /// Material Symbols `sports_volleyball` (U+EA31).
    SportsVolleyball,
    /// Material Symbols `sprinkler` (U+E29A).
    Sprinkler,
    /// Material Symbols `sprint` (U+F81F).
    Sprint,
    /// Material Symbols `sql` (U+FFF95).
    Sql,
    /// Material Symbols `square` (U+EB36).
    Square,
    /// Material Symbols `square_circle` (U+EEC7).
    SquareCircle,
    /// Material Symbols `square_dot` (U+F3B3).
    SquareDot,
    /// Material Symbols `square_foot` (U+EA49).
    SquareFoot,
    /// Material Symbols `ssid_chart` (U+EB66).
    SsidChart,
    /// Material Symbols `stack` (U+F609).
    Stack,
    /// Material Symbols `stack_group` (U+F359).
    StackGroup,
    /// Material Symbols `stack_hexagon` (U+F41C).
    StackHexagon,
    /// Material Symbols `stack_off` (U+F608).
    StackOff,
    /// Material Symbols `stack_star` (U+F607).
    StackStar,
    /// Material Symbols `stacked_bar_chart` (U+E9E6).
    StackedBarChart,
    /// Material Symbols `stacked_email` (U+E6C7).
    StackedEmail,
    /// Material Symbols `stacked_inbox` (U+E6C9).
    StackedInbox,
    /// Material Symbols `stacked_line_chart` (U+F22B).
    StackedLineChart,
    /// Material Symbols `stacks` (U+F500).
    Stacks,
    /// Material Symbols `stadia_controller` (U+F135).
    StadiaController,
    /// Material Symbols `stadium` (U+EB90).
    Stadium,
    /// Material Symbols `stairs` (U+F1A9).
    Stairs,
    /// Material Symbols `stairs_2` (U+F46C).
    Stairs2,
    /// Material Symbols `star_border` (U+F09A).
    StarBorder,
    /// Material Symbols `star_border_purple500` (U+F09A).
    StarBorderPurple500,
    /// Material Symbols `star_half` (U+E839).
    StarHalf,
    /// Material Symbols `star_outline` (U+F09A).
    StarOutline,
    /// Material Symbols `star_purple500` (U+F09A).
    StarPurple500,
    /// Material Symbols `star_rate` (U+F0EC).
    StarRate,
    /// Material Symbols `star_rate_half` (U+EC45).
    StarRateHalf,
    /// Material Symbols `star_shine` (U+F31D).
    StarShine,
    /// Material Symbols `stars` (U+E8D0).
    Stars,
    /// Material Symbols `stars_2` (U+F31C).
    Stars2,
    /// Material Symbols `start` (U+E089).
    Start,
    /// Material Symbols `stat_0` (U+E697).
    Stat0,
    /// Material Symbols `stat_1` (U+E698).
    Stat1,
    /// Material Symbols `stat_2` (U+E699).
    Stat2,
    /// Material Symbols `stat_3` (U+E69A).
    Stat3,
    /// Material Symbols `stat_minus_1` (U+E69B).
    StatMinus1,
    /// Material Symbols `stat_minus_2` (U+E69C).
    StatMinus2,
    /// Material Symbols `stat_minus_3` (U+E69D).
    StatMinus3,
    /// Material Symbols `stay_current_landscape` (U+ED3E).
    StayCurrentLandscape,
    /// Material Symbols `stay_current_portrait` (U+E7BA).
    StayCurrentPortrait,
    /// Material Symbols `stay_primary_landscape` (U+ED3E).
    StayPrimaryLandscape,
    /// Material Symbols `stay_primary_portrait` (U+F2D3).
    StayPrimaryPortrait,
    /// Material Symbols `steering_wheel_cool` (U+FFEBD).
    SteeringWheelCool,
    /// Material Symbols `steering_wheel_heat` (U+F32B).
    SteeringWheelHeat,
    /// Material Symbols `step` (U+F6FE).
    Step,
    /// Material Symbols `step_into` (U+F701).
    StepInto,
    /// Material Symbols `step_out` (U+F700).
    StepOut,
    /// Material Symbols `step_over` (U+F6FF).
    StepOver,
    /// Material Symbols `steppers` (U+E9E7).
    Steppers,
    /// Material Symbols `steps` (U+F6DA).
    Steps,
    /// Material Symbols `stethoscope` (U+F805).
    Stethoscope,
    /// Material Symbols `stethoscope_arrow` (U+F807).
    StethoscopeArrow,
    /// Material Symbols `stethoscope_check` (U+F806).
    StethoscopeCheck,
    /// Material Symbols `sticker` (U+E707).
    Sticker,
    /// Material Symbols `sticker_add` (U+EEC2).
    StickerAdd,
    /// Material Symbols `sticky_note` (U+E9E8).
    StickyNote,
    /// Material Symbols `sticky_note_2` (U+F1FC).
    StickyNote2,
    /// Material Symbols `stock_media` (U+F570).
    StockMedia,
    /// Material Symbols `stockpot` (U+F545).
    Stockpot,
    /// Material Symbols `stop_circle` (U+EF71).
    StopCircle,
    /// Material Symbols `stop_screen_share` (U+E0E3).
    StopScreenShare,
    /// Material Symbols `storage` (U+E1DB).
    Storage,
    /// Material Symbols `store` (U+E8D1).
    Store,
    /// Material Symbols `store_mall_directory` (U+E8D1).
    StoreMallDirectory,
    /// Material Symbols `storefront` (U+EA12).
    Storefront,
    /// Material Symbols `storm` (U+F070).
    Storm,
    /// Material Symbols `straight` (U+EB95).
    Straight,
    /// Material Symbols `straighten` (U+E41C).
    Straighten,
    /// Material Symbols `strategy` (U+F5DF).
    Strategy,
    /// Material Symbols `stream` (U+E9E9).
    Stream,
    /// Material Symbols `stream_apps` (U+F79F).
    StreamApps,
    /// Material Symbols `streetview` (U+E56E).
    Streetview,
    /// Material Symbols `stress_management` (U+F6D9).
    StressManagement,
    /// Material Symbols `strikethrough_s` (U+E257).
    StrikethroughS,
    /// Material Symbols `stroke_full` (U+F749).
    StrokeFull,
    /// Material Symbols `stroke_partial` (U+F748).
    StrokePartial,
    /// Material Symbols `stroller` (U+F1AE).
    Stroller,
    /// Material Symbols `style` (U+E41D).
    Style,
    /// Material Symbols `styler` (U+E273).
    Styler,
    /// Material Symbols `stylus` (U+F604).
    Stylus,
    /// Material Symbols `stylus_brush` (U+F366).
    StylusBrush,
    /// Material Symbols `stylus_fountain_pen` (U+F365).
    StylusFountainPen,
    /// Material Symbols `stylus_highlighter` (U+F364).
    StylusHighlighter,
    /// Material Symbols `stylus_laser_pointer` (U+F747).
    StylusLaserPointer,
    /// Material Symbols `stylus_note` (U+F603).
    StylusNote,
    /// Material Symbols `stylus_pen` (U+F363).
    StylusPen,
    /// Material Symbols `stylus_pencil` (U+F362).
    StylusPencil,
    /// Material Symbols `subdirectory_arrow_left` (U+E5D9).
    SubdirectoryArrowLeft,
    /// Material Symbols `subdirectory_arrow_right` (U+E5DA).
    SubdirectoryArrowRight,
    /// Material Symbols `subheader` (U+E9EA).
    Subheader,
    /// Material Symbols `subject` (U+E8D2).
    Subject,
    /// Material Symbols `subscript` (U+F111).
    Subscript,
    /// Material Symbols `subscriptions` (U+E064).
    Subscriptions,
    /// Material Symbols `subtitles` (U+E048).
    Subtitles,
    /// Material Symbols `subtitles_gear` (U+F355).
    SubtitlesGear,
    /// Material Symbols `subtitles_off` (U+EF72).
    SubtitlesOff,
    /// Material Symbols `subway` (U+E56F).
    Subway,
    /// Material Symbols `subway_walk` (U+F287).
    SubwayWalk,
    /// Material Symbols `subwoofer` (U+FFF70).
    Subwoofer,
    /// Material Symbols `summarize` (U+F071).
    Summarize,
    /// Material Symbols `sunny` (U+E81A).
    Sunny,
    /// Material Symbols `sunny_snowing` (U+E819).
    SunnySnowing,
    /// Material Symbols `superscript` (U+F112).
    Superscript,
    /// Material Symbols `supervised_user_circle` (U+E939).
    SupervisedUserCircle,
    /// Material Symbols `supervised_user_circle_off` (U+F60E).
    SupervisedUserCircleOff,
    /// Material Symbols `supervisor_account` (U+E8D3).
    SupervisorAccount,
    /// Material Symbols `support` (U+EF73).
    Support,
    /// Material Symbols `support_agent` (U+F0E2).
    SupportAgent,
    /// Material Symbols `surfing` (U+E515).
    Surfing,
    /// Material Symbols `surgical` (U+E131).
    Surgical,
    /// Material Symbols `surround_sound` (U+E049).
    SurroundSound,
    /// Material Symbols `swap_calls` (U+E0D7).
    SwapCalls,
    /// Material Symbols `swap_driving_apps` (U+E69E).
    SwapDrivingApps,
    /// Material Symbols `swap_driving_apps_wheel` (U+E69F).
    SwapDrivingAppsWheel,
    /// Material Symbols `swap_horiz` (U+E8D4).
    SwapHoriz,
    /// Material Symbols `swap_horizontal_circle` (U+E933).
    SwapHorizontalCircle,
    /// Material Symbols `swap_vert` (U+E8D5).
    SwapVert,
    /// Material Symbols `swap_vertical_circle` (U+E8D6).
    SwapVerticalCircle,
    /// Material Symbols `sweep` (U+E6AC).
    Sweep,
    /// Material Symbols `swipe` (U+E9EC).
    Swipe,
    /// Material Symbols `swipe_down` (U+EB53).
    SwipeDown,
    /// Material Symbols `swipe_down_alt` (U+EB30).
    SwipeDownAlt,
    /// Material Symbols `swipe_left` (U+EB59).
    SwipeLeft,
    /// Material Symbols `swipe_left_2` (U+FFF94).
    SwipeLeft2,
    /// Material Symbols `swipe_left_alt` (U+EB33).
    SwipeLeftAlt,
    /// Material Symbols `swipe_right` (U+EB52).
    SwipeRight,
    /// Material Symbols `swipe_right_2` (U+FFF93).
    SwipeRight2,
    /// Material Symbols `swipe_right_alt` (U+EB56).
    SwipeRightAlt,
    /// Material Symbols `swipe_up` (U+EB2E).
    SwipeUp,
    /// Material Symbols `swipe_up_alt` (U+EB35).
    SwipeUpAlt,
    /// Material Symbols `swipe_vertical` (U+EB51).
    SwipeVertical,
    /// Material Symbols `switch` (U+E1F4).
    Switch,
    /// Material Symbols `switch_access` (U+F6FD).
    SwitchAccess,
    /// Material Symbols `switch_access_2` (U+F506).
    SwitchAccess2,
    /// Material Symbols `switch_access_3` (U+F34D).
    SwitchAccess3,
    /// Material Symbols `switch_access_shortcut` (U+E7E1).
    SwitchAccessShortcut,
    /// Material Symbols `switch_access_shortcut_add` (U+E7E2).
    SwitchAccessShortcutAdd,
    /// Material Symbols `switch_account` (U+E9ED).
    SwitchAccount,
    /// Material Symbols `switch_camera` (U+E41E).
    SwitchCamera,
    /// Material Symbols `switch_left` (U+F1D1).
    SwitchLeft,
    /// Material Symbols `switch_off` (U+FFF6F).
    SwitchOff,
    /// Material Symbols `switch_right` (U+F1D2).
    SwitchRight,
    /// Material Symbols `switch_video` (U+E41F).
    SwitchVideo,
    /// Material Symbols `switches` (U+E733).
    Switches,
    /// Material Symbols `sword_rose` (U+F5DE).
    SwordRose,
    /// Material Symbols `swords` (U+F889).
    Swords,
    /// Material Symbols `symptoms` (U+E132).
    Symptoms,
    /// Material Symbols `synagogue` (U+EAB0).
    Synagogue,
    /// Material Symbols `sync` (U+E627).
    Sync,
    /// Material Symbols `sync_alt` (U+EA18).
    SyncAlt,
    /// Material Symbols `sync_arrow_down` (U+F37C).
    SyncArrowDown,
    /// Material Symbols `sync_arrow_up` (U+F37B).
    SyncArrowUp,
    /// Material Symbols `sync_desktop` (U+F41A).
    SyncDesktop,
    /// Material Symbols `sync_disabled` (U+E628).
    SyncDisabled,
    /// Material Symbols `sync_lock` (U+EAEE).
    SyncLock,
    /// Material Symbols `sync_problem` (U+E629).
    SyncProblem,
    /// Material Symbols `sync_saved_locally` (U+F820).
    SyncSavedLocally,
    /// Material Symbols `sync_saved_locally_off` (U+F264).
    SyncSavedLocallyOff,
    /// Material Symbols `syringe` (U+E133).
    Syringe,
    /// Material Symbols `system_security_update` (U+F2CD).
    SystemSecurityUpdate,
    /// Material Symbols `system_security_update_good` (U+F073).
    SystemSecurityUpdateGood,
    /// Material Symbols `system_security_update_warning` (U+F2D3).
    SystemSecurityUpdateWarning,
    /// Material Symbols `system_update` (U+F2CD).
    SystemUpdate,
    /// Material Symbols `system_update_alt` (U+E8D7).
    SystemUpdateAlt,
    /// Material Symbols `tab` (U+E8D8).
    Tab,
    /// Material Symbols `tab_close` (U+F745).
    TabClose,
    /// Material Symbols `tab_close_inactive` (U+F3D0).
    TabCloseInactive,
    /// Material Symbols `tab_close_right` (U+F746).
    TabCloseRight,
    /// Material Symbols `tab_duplicate` (U+F744).
    TabDuplicate,
    /// Material Symbols `tab_group` (U+F743).
    TabGroup,
    /// Material Symbols `tab_inactive` (U+F43B).
    TabInactive,
    /// Material Symbols `tab_move` (U+F742).
    TabMove,
    /// Material Symbols `tab_new_right` (U+F741).
    TabNewRight,
    /// Material Symbols `tab_recent` (U+F740).
    TabRecent,
    /// Material Symbols `tab_search` (U+F2F2).
    TabSearch,
    /// Material Symbols `tab_unselected` (U+E8D9).
    TabUnselected,
    /// Material Symbols `table_bar` (U+EAD2).
    TableBar,
    /// Material Symbols `table_chart` (U+E265).
    TableChart,
    /// Material Symbols `table_chart_view` (U+F6EF).
    TableChartView,
    /// Material Symbols `table_convert` (U+F3C7).
    TableConvert,
    /// Material Symbols `table_edit` (U+F3C6).
    TableEdit,
    /// Material Symbols `table_eye` (U+F466).
    TableEye,
    /// Material Symbols `table_lamp` (U+E1F2).
    TableLamp,
    /// Material Symbols `table_large` (U+F299).
    TableLarge,
    /// Material Symbols `table_restaurant` (U+EAC6).
    TableRestaurant,
    /// Material Symbols `table_rows` (U+F101).
    TableRows,
    /// Material Symbols `table_rows_narrow` (U+F73F).
    TableRowsNarrow,
    /// Material Symbols `table_sign` (U+EF2C).
    TableSign,
    /// Material Symbols `table_view` (U+F1BE).
    TableView,
    /// Material Symbols `tablet` (U+E32F).
    Tablet,
    /// Material Symbols `tablet_android` (U+E330).
    TabletAndroid,
    /// Material Symbols `tablet_camera` (U+F44D).
    TabletCamera,
    /// Material Symbols `tablet_mac` (U+E331).
    TabletMac,
    /// Material Symbols `tabs` (U+E9EE).
    Tabs,
    /// Material Symbols `tactic` (U+F564).
    Tactic,
    /// Material Symbols `tag` (U+E9EF).
    Tag,
    /// Material Symbols `tag_faces` (U+EA22).
    TagFaces,
    /// Material Symbols `takeout_dining` (U+EA74).
    TakeoutDining,
    /// Material Symbols `takeout_dining_2` (U+EF34).
    TakeoutDining2,
    /// Material Symbols `tamper_detection_off` (U+E82E).
    TamperDetectionOff,
    /// Material Symbols `tamper_detection_on` (U+F8C8).
    TamperDetectionOn,
    /// Material Symbols `tap_and_play` (U+F2CC).
    TapAndPlay,
    /// Material Symbols `tapas` (U+F1E9).
    Tapas,
    /// Material Symbols `target` (U+E719).
    Target,
    /// Material Symbols `target_check` (U+FFEB3).
    TargetCheck,
    /// Material Symbols `task` (U+F075).
    Task,
    /// Material Symbols `task_alt` (U+E2E6).
    TaskAlt,
    /// Material Symbols `tatami_seat` (U+EF33).
    TatamiSeat,
    /// Material Symbols `taunt` (U+F69F).
    Taunt,
    /// Material Symbols `taxi_alert` (U+EF74).
    TaxiAlert,
    /// Material Symbols `team_dashboard` (U+E013).
    TeamDashboard,
    /// Material Symbols `temp_preferences_custom` (U+F8C9).
    TempPreferencesCustom,
    /// Material Symbols `temp_preferences_eco` (U+F8CA).
    TempPreferencesEco,
    /// Material Symbols `temple_buddhist` (U+EAB3).
    TempleBuddhist,
    /// Material Symbols `temple_hindu` (U+EAAF).
    TempleHindu,
    /// Material Symbols `tenancy` (U+F0E3).
    Tenancy,
    /// Material Symbols `terminal` (U+EB8E).
    Terminal,
    /// Material Symbols `terminal_2` (U+FFF8E).
    Terminal2,
    /// Material Symbols `terminal_add` (U+FFED3).
    TerminalAdd,
    /// Material Symbols `terrain` (U+E564).
    Terrain,
    /// Material Symbols `text_ad` (U+E728).
    TextAd,
    /// Material Symbols `text_ad_off` (U+FFF92).
    TextAdOff,
    /// Material Symbols `text_compare` (U+F3C5).
    TextCompare,
    /// Material Symbols `text_decrease` (U+EADD).
    TextDecrease,
    /// Material Symbols `text_fields` (U+E262).
    TextFields,
    /// Material Symbols `text_fields_alt` (U+E9F1).
    TextFieldsAlt,
    /// Material Symbols `text_format` (U+E165).
    TextFormat,
    /// Material Symbols `text_increase` (U+EAE2).
    TextIncrease,
    /// Material Symbols `text_rotate_up` (U+E93A).
    TextRotateUp,
    /// Material Symbols `text_rotate_vertical` (U+E93B).
    TextRotateVertical,
    /// Material Symbols `text_rotation_angledown` (U+E93C).
    TextRotationAngledown,
    /// Material Symbols `text_rotation_angleup` (U+E93D).
    TextRotationAngleup,
    /// Material Symbols `text_rotation_down` (U+E93E).
    TextRotationDown,
    /// Material Symbols `text_rotation_none` (U+E93F).
    TextRotationNone,
    /// Material Symbols `text_select_end` (U+F73E).
    TextSelectEnd,
    /// Material Symbols `text_select_jump_to_beginning` (U+F73D).
    TextSelectJumpToBeginning,
    /// Material Symbols `text_select_jump_to_end` (U+F73C).
    TextSelectJumpToEnd,
    /// Material Symbols `text_select_move_back_character` (U+F73B).
    TextSelectMoveBackCharacter,
    /// Material Symbols `text_select_move_back_word` (U+F73A).
    TextSelectMoveBackWord,
    /// Material Symbols `text_select_move_down` (U+F739).
    TextSelectMoveDown,
    /// Material Symbols `text_select_move_forward_character` (U+F738).
    TextSelectMoveForwardCharacter,
    /// Material Symbols `text_select_move_forward_word` (U+F737).
    TextSelectMoveForwardWord,
    /// Material Symbols `text_select_move_up` (U+F736).
    TextSelectMoveUp,
    /// Material Symbols `text_select_start` (U+F735).
    TextSelectStart,
    /// Material Symbols `text_snippet` (U+F1C6).
    TextSnippet,
    /// Material Symbols `text_to_speech` (U+F1BC).
    TextToSpeech,
    /// Material Symbols `text_up` (U+F49E).
    TextUp,
    /// Material Symbols `textsms` (U+E625).
    Textsms,
    /// Material Symbols `texture` (U+E421).
    Texture,
    /// Material Symbols `texture_add` (U+F57C).
    TextureAdd,
    /// Material Symbols `texture_minus` (U+F57B).
    TextureMinus,
    /// Material Symbols `theater_comedy` (U+EA66).
    TheaterComedy,
    /// Material Symbols `theaters` (U+E8DA).
    Theaters,
    /// Material Symbols `thermometer` (U+E846).
    Thermometer,
    /// Material Symbols `thermometer_add` (U+F582).
    ThermometerAdd,
    /// Material Symbols `thermometer_alert` (U+FFFFB).
    ThermometerAlert,
    /// Material Symbols `thermometer_gain` (U+F6D8).
    ThermometerGain,
    /// Material Symbols `thermometer_loss` (U+F6D7).
    ThermometerLoss,
    /// Material Symbols `thermometer_minus` (U+F581).
    ThermometerMinus,
    /// Material Symbols `thermostat` (U+F076).
    Thermostat,
    /// Material Symbols `thermostat_arrow_down` (U+F37A).
    ThermostatArrowDown,
    /// Material Symbols `thermostat_arrow_up` (U+F379).
    ThermostatArrowUp,
    /// Material Symbols `thermostat_auto` (U+F077).
    ThermostatAuto,
    /// Material Symbols `thermostat_carbon` (U+F178).
    ThermostatCarbon,
    /// Material Symbols `things_to_do` (U+EB2A).
    ThingsToDo,
    /// Material Symbols `thread_unread` (U+F4F9).
    ThreadUnread,
    /// Material Symbols `threat_intelligence` (U+EAED).
    ThreatIntelligence,
    /// Material Symbols `thumb_down` (U+F578).
    ThumbDown,
    /// Material Symbols `thumb_down_alt` (U+F578).
    ThumbDownAlt,
    /// Material Symbols `thumb_down_filled` (U+F578).
    ThumbDownFilled,
    /// Material Symbols `thumb_down_off` (U+F578).
    ThumbDownOff,
    /// Material Symbols `thumb_down_off_alt` (U+F578).
    ThumbDownOffAlt,
    /// Material Symbols `thumb_up` (U+F577).
    ThumbUp,
    /// Material Symbols `thumb_up_alt` (U+F577).
    ThumbUpAlt,
    /// Material Symbols `thumb_up_filled` (U+F577).
    ThumbUpFilled,
    /// Material Symbols `thumb_up_off` (U+F577).
    ThumbUpOff,
    /// Material Symbols `thumb_up_off_alt` (U+F577).
    ThumbUpOffAlt,
    /// Material Symbols `thumbnail_bar` (U+F734).
    ThumbnailBar,
    /// Material Symbols `thumbs_up_double` (U+EEFC).
    ThumbsUpDouble,
    /// Material Symbols `thumbs_up_down` (U+E8DD).
    ThumbsUpDown,
    /// Material Symbols `thunderstorm` (U+EBDB).
    Thunderstorm,
    /// Material Symbols `tibia` (U+F89B).
    Tibia,
    /// Material Symbols `tibia_alt` (U+F89C).
    TibiaAlt,
    /// Material Symbols `tile_large` (U+F3C3).
    TileLarge,
    /// Material Symbols `tile_medium` (U+F3C2).
    TileMedium,
    /// Material Symbols `tile_small` (U+F3C1).
    TileSmall,
    /// Material Symbols `tilt_arrow_down` (U+FFF26).
    TiltArrowDown,
    /// Material Symbols `tilt_arrow_up` (U+FFF25).
    TiltArrowUp,
    /// Material Symbols `time_auto` (U+F0E4).
    TimeAuto,
    /// Material Symbols `time_to_leave` (U+EFF7).
    TimeToLeave,
    /// Material Symbols `timelapse` (U+E422).
    Timelapse,
    /// Material Symbols `timeline` (U+E922).
    Timeline,
    /// Material Symbols `timer` (U+E425).
    Timer,
    /// Material Symbols `timer_1` (U+F2AF).
    Timer1,
    /// Material Symbols `timer_10` (U+E423).
    Timer10,
    /// Material Symbols `timer_10_alt_1` (U+EFBF).
    Timer10Alt1,
    /// Material Symbols `timer_10_select` (U+F07A).
    Timer10Select,
    /// Material Symbols `timer_2` (U+F2AE).
    Timer2,
    /// Material Symbols `timer_3` (U+E424).
    Timer3,
    /// Material Symbols `timer_3_alt_1` (U+EFC0).
    Timer3Alt1,
    /// Material Symbols `timer_3_select` (U+F07B).
    Timer3Select,
    /// Material Symbols `timer_5` (U+F4B1).
    Timer5,
    /// Material Symbols `timer_5_shutter` (U+F4B2).
    Timer5Shutter,
    /// Material Symbols `timer_arrow_down` (U+F378).
    TimerArrowDown,
    /// Material Symbols `timer_arrow_up` (U+F377).
    TimerArrowUp,
    /// Material Symbols `timer_off` (U+E426).
    TimerOff,
    /// Material Symbols `timer_pause` (U+F4BB).
    TimerPause,
    /// Material Symbols `timer_play` (U+F4BA).
    TimerPlay,
    /// Material Symbols `tips_and_updates` (U+E79A).
    TipsAndUpdates,
    /// Material Symbols `tire_repair` (U+EBC8).
    TireRepair,
    /// Material Symbols `title` (U+E264).
    Title,
    /// Material Symbols `titlecase` (U+F489).
    Titlecase,
    /// Material Symbols `toast` (U+EFC1).
    Toast,
    /// Material Symbols `toc` (U+E8DE).
    Toc,
    /// Material Symbols `today` (U+E8DF).
    Today,
    /// Material Symbols `toggle_off` (U+E9F5).
    ToggleOff,
    /// Material Symbols `toggle_on` (U+E9F6).
    ToggleOn,
    /// Material Symbols `token` (U+EA25).
    Token,
    /// Material Symbols `toll` (U+E8E0).
    Toll,
    /// Material Symbols `tonality` (U+E427).
    Tonality,
    /// Material Symbols `tonality_2` (U+F2B4).
    Tonality2,
    /// Material Symbols `toolbar` (U+E9F7).
    Toolbar,
    /// Material Symbols `tools_flat_head` (U+F8CB).
    ToolsFlatHead,
    /// Material Symbols `tools_installation_kit` (U+E2AB).
    ToolsInstallationKit,
    /// Material Symbols `tools_ladder` (U+E2CB).
    ToolsLadder,
    /// Material Symbols `tools_level` (U+E77B).
    ToolsLevel,
    /// Material Symbols `tools_phillips` (U+F8CC).
    ToolsPhillips,
    /// Material Symbols `tools_pliers_wire_stripper` (U+E2AA).
    ToolsPliersWireStripper,
    /// Material Symbols `tools_power_drill` (U+E1E9).
    ToolsPowerDrill,
    /// Material Symbols `tools_wrench` (U+F8CD).
    ToolsWrench,
    /// Material Symbols `tooltip` (U+E9F8).
    Tooltip,
    /// Material Symbols `tooltip_2` (U+F3ED).
    Tooltip2,
    /// Material Symbols `top_panel_close` (U+F733).
    TopPanelClose,
    /// Material Symbols `top_panel_open` (U+F732).
    TopPanelOpen,
    /// Material Symbols `topic` (U+F1C8).
    Topic,
    /// Material Symbols `tornado` (U+E199).
    Tornado,
    /// Material Symbols `total_dissolved_solids` (U+F877).
    TotalDissolvedSolids,
    /// Material Symbols `touch_app` (U+E913).
    TouchApp,
    /// Material Symbols `touch_double` (U+F38B).
    TouchDouble,
    /// Material Symbols `touch_double_2` (U+FFF35).
    TouchDouble2,
    /// Material Symbols `touch_long` (U+F38A).
    TouchLong,
    /// Material Symbols `touch_triple` (U+F389).
    TouchTriple,
    /// Material Symbols `touchpad_mouse` (U+F687).
    TouchpadMouse,
    /// Material Symbols `touchpad_mouse_off` (U+F4E6).
    TouchpadMouseOff,
    /// Material Symbols `tour` (U+EF75).
    Tour,
    /// Material Symbols `toys` (U+E332).
    Toys,
    /// Material Symbols `toys_and_games` (U+EFC2).
    ToysAndGames,
    /// Material Symbols `toys_fan` (U+F887).
    ToysFan,
    /// Material Symbols `track_changes` (U+E8E1).
    TrackChanges,
    /// Material Symbols `trackpad_input` (U+F4C7).
    TrackpadInput,
    /// Material Symbols `trackpad_input_2` (U+F409).
    TrackpadInput2,
    /// Material Symbols `trackpad_input_3` (U+F408).
    TrackpadInput3,
    /// Material Symbols `traffic` (U+E565).
    Traffic,
    /// Material Symbols `traffic_jam` (U+F46F).
    TrafficJam,
    /// Material Symbols `trail_length` (U+EB5E).
    TrailLength,
    /// Material Symbols `trail_length_medium` (U+EB63).
    TrailLengthMedium,
    /// Material Symbols `trail_length_short` (U+EB6D).
    TrailLengthShort,
    /// Material Symbols `train` (U+E570).
    Train,
    /// Material Symbols `tram` (U+E571).
    Tram,
    /// Material Symbols `transcribe` (U+F8EC).
    Transcribe,
    /// Material Symbols `transfer_within_a_station` (U+E572).
    TransferWithinAStation,
    /// Material Symbols `transform` (U+E428).
    Transform,
    /// Material Symbols `transgender` (U+E58D).
    Transgender,
    /// Material Symbols `transit_enterexit` (U+E579).
    TransitEnterexit,
    /// Material Symbols `transit_ticket` (U+F3F1).
    TransitTicket,
    /// Material Symbols `transition_chop` (U+F50E).
    TransitionChop,
    /// Material Symbols `transition_dissolve` (U+F50D).
    TransitionDissolve,
    /// Material Symbols `transition_fade` (U+F50C).
    TransitionFade,
    /// Material Symbols `transition_push` (U+F50B).
    TransitionPush,
    /// Material Symbols `transition_slide` (U+F50A).
    TransitionSlide,
    /// Material Symbols `translate` (U+E8E2).
    Translate,
    /// Material Symbols `translate_indic` (U+F263).
    TranslateIndic,
    /// Material Symbols `transportation` (U+E21D).
    Transportation,
    /// Material Symbols `travel` (U+EF93).
    Travel,
    /// Material Symbols `travel_explore` (U+E2DB).
    TravelExplore,
    /// Material Symbols `travel_luggage_and_bags` (U+EFC3).
    TravelLuggageAndBags,
    /// Material Symbols `trending_down` (U+E8E3).
    TrendingDown,
    /// Material Symbols `trending_flat` (U+E8E4).
    TrendingFlat,
    /// Material Symbols `trending_up` (U+E8E5).
    TrendingUp,
    /// Material Symbols `triangle_circle` (U+EEC6).
    TriangleCircle,
    /// Material Symbols `trip` (U+E6FB).
    Trip,
    /// Material Symbols `trip_origin` (U+E57B).
    TripOrigin,
    /// Material Symbols `trolley` (U+F86B).
    Trolley,
    /// Material Symbols `trolley_cable_car` (U+F46E).
    TrolleyCableCar,
    /// Material Symbols `trophy` (U+EA23).
    Trophy,
    /// Material Symbols `troubleshoot` (U+E1D2).
    Troubleshoot,
    /// Material Symbols `try` (U+F07C).
    Try,
    /// Material Symbols `tsunami` (U+EBD8).
    Tsunami,
    /// Material Symbols `tsv` (U+E6D6).
    Tsv,
    /// Material Symbols `tty` (U+F1AA).
    Tty,
    /// Material Symbols `tune` (U+E429).
    Tune,
    /// Material Symbols `tungsten` (U+F07D).
    Tungsten,
    /// Material Symbols `turn_left` (U+EBA6).
    TurnLeft,
    /// Material Symbols `turn_right` (U+EBAB).
    TurnRight,
    /// Material Symbols `turn_sharp_left` (U+EBA7).
    TurnSharpLeft,
    /// Material Symbols `turn_sharp_right` (U+EBAA).
    TurnSharpRight,
    /// Material Symbols `turn_slight_left` (U+EBA4).
    TurnSlightLeft,
    /// Material Symbols `turn_slight_right` (U+EB9A).
    TurnSlightRight,
    /// Material Symbols `turned_in` (U+E8E7).
    TurnedIn,
    /// Material Symbols `turned_in_not` (U+E8E7).
    TurnedInNot,
    /// Material Symbols `tv` (U+E63B).
    Tv,
    /// Material Symbols `tv_displays` (U+F3EC).
    TvDisplays,
    /// Material Symbols `tv_gen` (U+E830).
    TvGen,
    /// Material Symbols `tv_guide` (U+E1DC).
    TvGuide,
    /// Material Symbols `tv_next` (U+F3EB).
    TvNext,
    /// Material Symbols `tv_off` (U+E647).
    TvOff,
    /// Material Symbols `tv_options_edit_channels` (U+E1DD).
    TvOptionsEditChannels,
    /// Material Symbols `tv_options_input_settings` (U+E1DE).
    TvOptionsInputSettings,
    /// Material Symbols `tv_remote` (U+F5D9).
    TvRemote,
    /// Material Symbols `tv_signin` (U+E71B).
    TvSignin,
    /// Material Symbols `tv_with_assistant` (U+E785).
    TvWithAssistant,
    /// Material Symbols `two_pager` (U+F51F).
    TwoPager,
    /// Material Symbols `two_pager_store` (U+F3C4).
    TwoPagerStore,
    /// Material Symbols `two_wheeler` (U+E9F9).
    TwoWheeler,
    /// Material Symbols `type_specimen` (U+F8F0).
    TypeSpecimen,
    /// Material Symbols `u_turn_left` (U+EBA1).
    UTurnLeft,
    /// Material Symbols `u_turn_right` (U+EBA2).
    UTurnRight,
    /// Material Symbols `udon` (U+EF32).
    Udon,
    /// Material Symbols `ulna_radius` (U+F89D).
    UlnaRadius,
    /// Material Symbols `ulna_radius_alt` (U+F89E).
    UlnaRadiusAlt,
    /// Material Symbols `umbrella` (U+F1AD).
    Umbrella,
    /// Material Symbols `unarchive` (U+E169).
    Unarchive,
    /// Material Symbols `undereye` (U+EEB1).
    Undereye,
    /// Material Symbols `unfold_less` (U+E5D6).
    UnfoldLess,
    /// Material Symbols `unfold_less_double` (U+F8CF).
    UnfoldLessDouble,
    /// Material Symbols `unfold_more` (U+E5D7).
    UnfoldMore,
    /// Material Symbols `unfold_more_double` (U+F8D0).
    UnfoldMoreDouble,
    /// Material Symbols `ungroup` (U+F731).
    Ungroup,
    /// Material Symbols `universal_currency` (U+E9FA).
    UniversalCurrency,
    /// Material Symbols `universal_currency_alt` (U+E734).
    UniversalCurrencyAlt,
    /// Material Symbols `universal_local` (U+E9FB).
    UniversalLocal,
    /// Material Symbols `unknown_2` (U+F49F).
    Unknown2,
    /// Material Symbols `unknown_5` (U+E6A5).
    Unknown5,
    /// Material Symbols `unknown_7` (U+F49E).
    Unknown7,
    /// Material Symbols `unknown_document` (U+F804).
    UnknownDocument,
    /// Material Symbols `unknown_med` (U+EABD).
    UnknownMed,
    /// Material Symbols `unlicense` (U+EB05).
    Unlicense,
    /// Material Symbols `unpaved_road` (U+F46D).
    UnpavedRoad,
    /// Material Symbols `unpin` (U+E6F9).
    Unpin,
    /// Material Symbols `unpublished` (U+F236).
    Unpublished,
    /// Material Symbols `unsubscribe` (U+E0EB).
    Unsubscribe,
    /// Material Symbols `upcoming` (U+F07E).
    Upcoming,
    /// Material Symbols `update` (U+E923).
    Update,
    /// Material Symbols `update_disabled` (U+E075).
    UpdateDisabled,
    /// Material Symbols `upgrade` (U+F0FB).
    Upgrade,
    /// Material Symbols `upi_pay` (U+F3CF).
    UpiPay,
    /// Material Symbols `upload_2` (U+F521).
    Upload2,
    /// Material Symbols `upload_file` (U+E9FC).
    UploadFile,
    /// Material Symbols `uppercase` (U+F488).
    Uppercase,
    /// Material Symbols `urology` (U+E137).
    Urology,
    /// Material Symbols `usb` (U+E1E0).
    Usb,
    /// Material Symbols `usb_off` (U+E4FA).
    UsbOff,
    /// Material Symbols `user_attributes` (U+E708).
    UserAttributes,
    /// Material Symbols `vaccines` (U+E138).
    Vaccines,
    /// Material Symbols `vacuum` (U+EFC5).
    Vacuum,
    /// Material Symbols `vacuum_2` (U+FFF6D).
    Vacuum2,
    /// Material Symbols `vacuum_2_on` (U+FFF6E).
    Vacuum2On,
    /// Material Symbols `valve` (U+E224).
    Valve,
    /// Material Symbols `vape_free` (U+EBC6).
    VapeFree,
    /// Material Symbols `vaping_rooms` (U+EBCF).
    VapingRooms,
    /// Material Symbols `variable_add` (U+F51E).
    VariableAdd,
    /// Material Symbols `variable_insert` (U+F51D).
    VariableInsert,
    /// Material Symbols `variable_remove` (U+F51C).
    VariableRemove,
    /// Material Symbols `variables` (U+F851).
    Variables,
    /// Material Symbols `ventilator` (U+E139).
    Ventilator,
    /// Material Symbols `verified` (U+EF76).
    Verified,
    /// Material Symbols `verified_off` (U+F30E).
    VerifiedOff,
    /// Material Symbols `verified_user` (U+F013).
    VerifiedUser,
    /// Material Symbols `vertical_align_bottom` (U+E258).
    VerticalAlignBottom,
    /// Material Symbols `vertical_align_center` (U+E259).
    VerticalAlignCenter,
    /// Material Symbols `vertical_align_top` (U+E25A).
    VerticalAlignTop,
    /// Material Symbols `vertical_distribute` (U+E076).
    VerticalDistribute,
    /// Material Symbols `vertical_shades` (U+EC0E).
    VerticalShades,
    /// Material Symbols `vertical_shades_closed` (U+EC0D).
    VerticalShadesClosed,
    /// Material Symbols `vertical_split` (U+E949).
    VerticalSplit,
    /// Material Symbols `vibration` (U+F2CB).
    Vibration,
    /// Material Symbols `video_call` (U+E070).
    VideoCall,
    /// Material Symbols `video_camera_back` (U+F07F).
    VideoCameraBack,
    /// Material Symbols `video_camera_back_add` (U+F40C).
    VideoCameraBackAdd,
    /// Material Symbols `video_camera_front` (U+F080).
    VideoCameraFront,
    /// Material Symbols `video_camera_front_off` (U+F83B).
    VideoCameraFrontOff,
    /// Material Symbols `video_chat` (U+F8A0).
    VideoChat,
    /// Material Symbols `video_file` (U+EB87).
    VideoFile,
    /// Material Symbols `video_frame_copy` (U+FFF0D).
    VideoFrameCopy,
    /// Material Symbols `video_frame_save` (U+FFF0C).
    VideoFrameSave,
    /// Material Symbols `video_label` (U+E071).
    VideoLabel,
    /// Material Symbols `video_library` (U+E04A).
    VideoLibrary,
    /// Material Symbols `video_search` (U+EFC6).
    VideoSearch,
    /// Material Symbols `video_settings` (U+EA75).
    VideoSettings,
    /// Material Symbols `video_stable` (U+F081).
    VideoStable,
    /// Material Symbols `video_template` (U+FFFD3).
    VideoTemplate,
    /// Material Symbols `videocam` (U+E04B).
    Videocam,
    /// Material Symbols `videocam_alert` (U+F390).
    VideocamAlert,
    /// Material Symbols `videocam_off` (U+E04C).
    VideocamOff,
    /// Material Symbols `videogame_asset` (U+E338).
    VideogameAsset,
    /// Material Symbols `videogame_asset_off` (U+E500).
    VideogameAssetOff,
    /// Material Symbols `view_agenda` (U+E8E9).
    ViewAgenda,
    /// Material Symbols `view_apps` (U+F376).
    ViewApps,
    /// Material Symbols `view_array` (U+E8EA).
    ViewArray,
    /// Material Symbols `view_carousel` (U+E8EB).
    ViewCarousel,
    /// Material Symbols `view_column` (U+E8EC).
    ViewColumn,
    /// Material Symbols `view_column_2` (U+F847).
    ViewColumn2,
    /// Material Symbols `view_comfy` (U+E42A).
    ViewComfy,
    /// Material Symbols `view_comfy_alt` (U+EB73).
    ViewComfyAlt,
    /// Material Symbols `view_compact` (U+E42B).
    ViewCompact,
    /// Material Symbols `view_compact_alt` (U+EB74).
    ViewCompactAlt,
    /// Material Symbols `view_cozy` (U+EB75).
    ViewCozy,
    /// Material Symbols `view_day` (U+E8ED).
    ViewDay,
    /// Material Symbols `view_headline` (U+E8EE).
    ViewHeadline,
    /// Material Symbols `view_in_ar` (U+EFC9).
    ViewInAr,
    /// Material Symbols `view_in_ar_new` (U+EFC9).
    ViewInArNew,
    /// Material Symbols `view_in_ar_off` (U+F61B).
    ViewInArOff,
    /// Material Symbols `view_kanban` (U+EB7F).
    ViewKanban,
    /// Material Symbols `view_list` (U+E8EF).
    ViewList,
    /// Material Symbols `view_module` (U+E8F0).
    ViewModule,
    /// Material Symbols `view_object_track` (U+F432).
    ViewObjectTrack,
    /// Material Symbols `view_quilt` (U+E8F1).
    ViewQuilt,
    /// Material Symbols `view_real_size` (U+F4C2).
    ViewRealSize,
    /// Material Symbols `view_sidebar` (U+F114).
    ViewSidebar,
    /// Material Symbols `view_stream` (U+E8F2).
    ViewStream,
    /// Material Symbols `view_timeline` (U+EB85).
    ViewTimeline,
    /// Material Symbols `view_week` (U+E8F3).
    ViewWeek,
    /// Material Symbols `vignette` (U+E435).
    Vignette,
    /// Material Symbols `vignette_2` (U+F2B3).
    Vignette2,
    /// Material Symbols `villa` (U+E586).
    Villa,
    /// Material Symbols `visibility` (U+E8F4).
    Visibility,
    /// Material Symbols `visibility_lock` (U+F653).
    VisibilityLock,
    /// Material Symbols `visibility_off` (U+E8F5).
    VisibilityOff,
    /// Material Symbols `vital_signs` (U+E650).
    VitalSigns,
    /// Material Symbols `vitals` (U+E13B).
    Vitals,
    /// Material Symbols `vo2_max` (U+F4AA).
    Vo2Max,
    /// Material Symbols `voice_chat` (U+E62E).
    VoiceChat,
    /// Material Symbols `voice_chat_off` (U+EEBE).
    VoiceChatOff,
    /// Material Symbols `voice_over_off` (U+E94A).
    VoiceOverOff,
    /// Material Symbols `voice_selection` (U+F58A).
    VoiceSelection,
    /// Material Symbols `voice_selection_off` (U+F42C).
    VoiceSelectionOff,
    /// Material Symbols `voicemail` (U+E0D9).
    Voicemail,
    /// Material Symbols `voicemail_2` (U+F352).
    Voicemail2,
    /// Material Symbols `volcano` (U+EBDA).
    Volcano,
    /// Material Symbols `volume_down` (U+E04D).
    VolumeDown,
    /// Material Symbols `volume_down_alt` (U+E79C).
    VolumeDownAlt,
    /// Material Symbols `volume_mute` (U+E04E).
    VolumeMute,
    /// Material Symbols `volunteer_activism` (U+EA70).
    VolunteerActivism,
    /// Material Symbols `voting_chip` (U+F852).
    VotingChip,
    /// Material Symbols `vpn_key` (U+E0DA).
    VpnKey,
    /// Material Symbols `vpn_key_alert` (U+F6CC).
    VpnKeyAlert,
    /// Material Symbols `vpn_key_off` (U+EB7A).
    VpnKeyOff,
    /// Material Symbols `vpn_lock` (U+E62F).
    VpnLock,
    /// Material Symbols `vpn_lock_2` (U+F350).
    VpnLock2,
    /// Material Symbols `vr180_create2d` (U+EFCA).
    Vr180Create2d,
    /// Material Symbols `vr180_create2d_off` (U+F571).
    Vr180Create2dOff,
    /// Material Symbols `vrpano` (U+F082).
    Vrpano,
    /// Material Symbols `walk_bike` (U+FFF01).
    WalkBike,
    /// Material Symbols `wall_art` (U+EFCB).
    WallArt,
    /// Material Symbols `wall_lamp` (U+E2B4).
    WallLamp,
    /// Material Symbols `wallet` (U+F8FF).
    Wallet,
    /// Material Symbols `wallpaper` (U+E1BC).
    Wallpaper,
    /// Material Symbols `wallpaper_slideshow` (U+F672).
    WallpaperSlideshow,
    /// Material Symbols `wand_shine` (U+F31F).
    WandShine,
    /// Material Symbols `wand_stars` (U+F31E).
    WandStars,
    /// Material Symbols `ward` (U+E13C).
    Ward,
    /// Material Symbols `warehouse` (U+EBB8).
    Warehouse,
    /// Material Symbols `warning_amber` (U+F083).
    WarningAmber,
    /// Material Symbols `warning_off` (U+F7AD).
    WarningOff,
    /// Material Symbols `wash` (U+F1B1).
    Wash,
    /// Material Symbols `washoku` (U+F280).
    Washoku,
    /// Material Symbols `watch` (U+E334).
    Watch,
    /// Material Symbols `watch_alert` (U+FFFD1).
    WatchAlert,
    /// Material Symbols `watch_arrow` (U+F2CA).
    WatchArrow,
    /// Material Symbols `watch_arrow_down` (U+FFFCF).
    WatchArrowDown,
    /// Material Symbols `watch_button` (U+FFF20).
    WatchButton,
    /// Material Symbols `watch_button_press` (U+F6AA).
    WatchButtonPress,
    /// Material Symbols `watch_check` (U+F468).
    WatchCheck,
    /// Material Symbols `watch_later` (U+EFD6).
    WatchLater,
    /// Material Symbols `watch_lock` (U+EEE9).
    WatchLock,
    /// Material Symbols `watch_off` (U+EAE3).
    WatchOff,
    /// Material Symbols `watch_screentime` (U+F6AE).
    WatchScreentime,
    /// Material Symbols `watch_vibration` (U+F467).
    WatchVibration,
    /// Material Symbols `watch_wake` (U+F6A9).
    WatchWake,
    /// Material Symbols `water` (U+F084).
    Water,
    /// Material Symbols `water_bottle` (U+F69D).
    WaterBottle,
    /// Material Symbols `water_bottle_large` (U+F69E).
    WaterBottleLarge,
    /// Material Symbols `water_damage` (U+F203).
    WaterDamage,
    /// Material Symbols `water_do` (U+F870).
    WaterDo,
    /// Material Symbols `water_drop` (U+E798).
    WaterDrop,
    /// Material Symbols `water_drops` (U+FFFA5).
    WaterDrops,
    /// Material Symbols `water_ec` (U+F875).
    WaterEc,
    /// Material Symbols `water_full` (U+F6D6).
    WaterFull,
    /// Material Symbols `water_heater` (U+E284).
    WaterHeater,
    /// Material Symbols `water_lock` (U+F6AD).
    WaterLock,
    /// Material Symbols `water_loss` (U+F6D5).
    WaterLoss,
    /// Material Symbols `water_lux` (U+F874).
    WaterLux,
    /// Material Symbols `water_medium` (U+F6D4).
    WaterMedium,
    /// Material Symbols `water_orp` (U+F878).
    WaterOrp,
    /// Material Symbols `water_ph` (U+F87A).
    WaterPh,
    /// Material Symbols `water_pump` (U+F5D8).
    WaterPump,
    /// Material Symbols `water_voc` (U+F87B).
    WaterVoc,
    /// Material Symbols `waterfall_chart` (U+EA00).
    WaterfallChart,
    /// Material Symbols `waves` (U+E176).
    Waves,
    /// Material Symbols `waving_hand` (U+E766).
    WavingHand,
    /// Material Symbols `wb_auto` (U+E42C).
    WbAuto,
    /// Material Symbols `wb_cloudy` (U+F15C).
    WbCloudy,
    /// Material Symbols `wb_incandescent` (U+E42E).
    WbIncandescent,
    /// Material Symbols `wb_iridescent` (U+F07D).
    WbIridescent,
    /// Material Symbols `wb_shade` (U+EA01).
    WbShade,
    /// Material Symbols `wb_sunny` (U+E430).
    WbSunny,
    /// Material Symbols `wb_twilight` (U+E1C6).
    WbTwilight,
    /// Material Symbols `wb_twilight_2` (U+FFF1F).
    WbTwilight2,
    /// Material Symbols `wc` (U+E63D).
    Wc,
    /// Material Symbols `weather_hail` (U+F67F).
    WeatherHail,
    /// Material Symbols `weather_mix` (U+F60B).
    WeatherMix,
    /// Material Symbols `weather_snowy` (U+E2CD).
    WeatherSnowy,
    /// Material Symbols `web` (U+E66A).
    Web,
    /// Material Symbols `web_asset` (U+E069).
    WebAsset,
    /// Material Symbols `web_asset_off` (U+EF47).
    WebAssetOff,
    /// Material Symbols `web_stories` (U+E595).
    WebStories,
    /// Material Symbols `web_traffic` (U+EA03).
    WebTraffic,
    /// Material Symbols `webhook` (U+EB92).
    Webhook,
    /// Material Symbols `weekend` (U+E16B).
    Weekend,
    /// Material Symbols `weight` (U+E13D).
    Weight,
    /// Material Symbols `west` (U+F1E6).
    West,
    /// Material Symbols `whatshot` (U+E80E).
    Whatshot,
    /// Material Symbols `wheat` (U+FFFA4).
    Wheat,
    /// Material Symbols `wheelchair_pickup` (U+F1AB).
    WheelchairPickup,
    /// Material Symbols `where_to_vote` (U+E177).
    WhereToVote,
    /// Material Symbols `widget_medium` (U+F3BA).
    WidgetMedium,
    /// Material Symbols `widget_menu` (U+EEB7).
    WidgetMenu,
    /// Material Symbols `widget_small` (U+F3B9).
    WidgetSmall,
    /// Material Symbols `widget_width` (U+F3B8).
    WidgetWidth,
    /// Material Symbols `widgets` (U+E1BD).
    Widgets,
    /// Material Symbols `width` (U+F730).
    Width,
    /// Material Symbols `width_full` (U+F8F5).
    WidthFull,
    /// Material Symbols `width_normal` (U+F8F6).
    WidthNormal,
    /// Material Symbols `width_wide` (U+F8F7).
    WidthWide,
    /// Material Symbols `wifi` (U+E63E).
    Wifi,
    /// Material Symbols `wifi_1_bar` (U+E4CA).
    Wifi1Bar,
    /// Material Symbols `wifi_2_bar` (U+E4D9).
    Wifi2Bar,
    /// Material Symbols `wifi_add` (U+F7A8).
    WifiAdd,
    /// Material Symbols `wifi_calling` (U+EF77).
    WifiCalling,
    /// Material Symbols `wifi_calling_1` (U+F0E7).
    WifiCalling1,
    /// Material Symbols `wifi_calling_2` (U+F0F6).
    WifiCalling2,
    /// Material Symbols `wifi_calling_3` (U+F0E7).
    WifiCalling3,
    /// Material Symbols `wifi_calling_bar_1` (U+F44C).
    WifiCallingBar1,
    /// Material Symbols `wifi_calling_bar_2` (U+F44B).
    WifiCallingBar2,
    /// Material Symbols `wifi_calling_bar_3` (U+F44A).
    WifiCallingBar3,
    /// Material Symbols `wifi_channel` (U+EB6A).
    WifiChannel,
    /// Material Symbols `wifi_device` (U+FFF34).
    WifiDevice,
    /// Material Symbols `wifi_find` (U+EB31).
    WifiFind,
    /// Material Symbols `wifi_home` (U+F671).
    WifiHome,
    /// Material Symbols `wifi_lock` (U+E1E1).
    WifiLock,
    /// Material Symbols `wifi_notification` (U+F670).
    WifiNotification,
    /// Material Symbols `wifi_off` (U+E648).
    WifiOff,
    /// Material Symbols `wifi_password` (U+EB6B).
    WifiPassword,
    /// Material Symbols `wifi_protected_setup` (U+F0FC).
    WifiProtectedSetup,
    /// Material Symbols `wifi_proxy` (U+F7A7).
    WifiProxy,
    /// Material Symbols `wifi_tethering` (U+E1E2).
    WifiTethering,
    /// Material Symbols `wifi_tethering_error` (U+EAD9).
    WifiTetheringError,
    /// Material Symbols `wifi_tethering_off` (U+F087).
    WifiTetheringOff,
    /// Material Symbols `wind_power` (U+EC0C).
    WindPower,
    /// Material Symbols `window` (U+F088).
    Window,
    /// Material Symbols `window_closed` (U+E77E).
    WindowClosed,
    /// Material Symbols `window_open` (U+E78C).
    WindowOpen,
    /// Material Symbols `window_sensor` (U+E2BB).
    WindowSensor,
    /// Material Symbols `windshield_defrost_auto` (U+F248).
    WindshieldDefrostAuto,
    /// Material Symbols `windshield_defrost_front` (U+F32A).
    WindshieldDefrostFront,
    /// Material Symbols `windshield_defrost_rear` (U+F329).
    WindshieldDefrostRear,
    /// Material Symbols `windshield_heat_front` (U+F328).
    WindshieldHeatFront,
    /// Material Symbols `wine_bar` (U+F1E8).
    WineBar,
    /// Material Symbols `woman` (U+E13E).
    Woman,
    /// Material Symbols `woman_2` (U+F8E7).
    Woman2,
    /// Material Symbols `work` (U+E943).
    Work,
    /// Material Symbols `work_alert` (U+F5F7).
    WorkAlert,
    /// Material Symbols `work_history` (U+EC09).
    WorkHistory,
    /// Material Symbols `work_off` (U+E942).
    WorkOff,
    /// Material Symbols `work_outline` (U+E943).
    WorkOutline,
    /// Material Symbols `work_update` (U+F5F8).
    WorkUpdate,
    /// Material Symbols `workflow` (U+EA04).
    Workflow,
    /// Material Symbols `workspace_premium` (U+E7AF).
    WorkspacePremium,
    /// Material Symbols `workspaces` (U+EA0F).
    Workspaces,
    /// Material Symbols `workspaces_outline` (U+EA0F).
    WorkspacesOutline,
    /// Material Symbols `wounds_injuries` (U+E13F).
    WoundsInjuries,
    /// Material Symbols `wrap_text` (U+E25B).
    WrapText,
    /// Material Symbols `wrist` (U+F69C).
    Wrist,
    /// Material Symbols `wrong_location` (U+EF78).
    WrongLocation,
    /// Material Symbols `wysiwyg` (U+F1C3).
    Wysiwyg,
    /// Material Symbols `x_circle` (U+E349).
    XCircle,
    /// Material Symbols `y_circle` (U+EEC5).
    YCircle,
    /// Material Symbols `yakitori` (U+EF31).
    Yakitori,
    /// Material Symbols `yard` (U+F089).
    Yard,
    /// Material Symbols `yoshoku` (U+F27F).
    Yoshoku,
    /// Material Symbols `your_trips` (U+EB2B).
    YourTrips,
    /// Material Symbols `youtube_activity` (U+F85A).
    YoutubeActivity,
    /// Material Symbols `youtube_searched_for` (U+E8FA).
    YoutubeSearchedFor,
    /// Material Symbols `zone_person_alert` (U+E781).
    ZonePersonAlert,
    /// Material Symbols `zone_person_idle` (U+E77A).
    ZonePersonIdle,
    /// Material Symbols `zone_person_urgent` (U+E788).
    ZonePersonUrgent,
    /// Material Symbols `zoom_in` (U+E8FF).
    ZoomIn,
    /// Material Symbols `zoom_in_map` (U+EB2D).
    ZoomInMap,
    /// Material Symbols `zoom_out` (U+E900).
    ZoomOut,
    /// Material Symbols `zoom_out_map` (U+E56B).
    ZoomOutMap,
}

// Generated code is formatted by the generator, not by `rustfmt`: at 4,323 tokens some
// arms exceed the line width, and reproducing rustfmt's wrapping rule in Python would be a
// second formatter to keep in sync. `#[rustfmt::skip]` keeps `cargo fmt --check` green
// over a file nobody edits by hand.
#[rustfmt::skip]
impl IconName {
    /// Returns the string representation of this icon name.
    ///
    /// The tokens are lower-case and underscore-separated (`"arrow_left"`), and are the
    /// exact spellings accepted by [`IconName::from_name`] and by the `icon` property. They
    /// are also the names used by [`Icon::set_icon`].
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Cross => "cross",
            Self::ArrowLeft => "arrow_left",
            Self::ArrowRight => "arrow_right",
            Self::ArrowUp => "arrow_up",
            Self::ArrowDown => "arrow_down",
            Self::Star => "star",
            Self::Heart => "heart",
            Self::Settings => "settings",
            Self::Home => "home",
            Self::Search => "search",
            Self::Menu => "menu",
            Self::Close => "close",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::User => "user",
            Self::Mail => "mail",
            Self::Bell => "bell",
            Self::Edit => "edit",
            Self::Trash => "trash",
            Self::Share => "share",
            Self::Refresh => "refresh",
            Self::More => "more",
            Self::Filter => "filter",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Download => "download",
            Self::Upload => "upload",
            Self::ChevronLeft => "chevron_left",
            Self::ChevronRight => "chevron_right",
            Self::ChevronUp => "chevron_up",
            Self::ChevronDown => "chevron_down",
            Self::FirstPage => "first_page",
            Self::LastPage => "last_page",
            Self::Folder => "folder",
            Self::FolderOpen => "folder_open",
            Self::File => "file",
            Self::Save => "save",
            Self::Copy => "copy",
            Self::Print => "print",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::Attachment => "attachment",
            Self::Link => "link",
            Self::Success => "success",
            Self::Help => "help",
            Self::Block => "block",
            Self::Schedule => "schedule",
            Self::Hourglass => "hourglass",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Stop => "stop",
            Self::SkipNext => "skip_next",
            Self::VolumeUp => "volume_up",
            Self::VolumeOff => "volume_off",
            Self::Sort => "sort",
            Self::BarChart => "bar_chart",
            Self::Calendar => "calendar",
            Self::Table => "table",
            Self::Chat => "chat",
            Self::Call => "call",
            Self::Send => "send",
            Self::NotificationsOff => "notifications_off",
            Self::Fullscreen => "fullscreen",
            Self::DebugBug => "debug_bug",
            Self::DesignComponent => "design_component",
            Self::DesignEvent => "design_event",
            Self::TestFlask => "test_flask",
            Self::ToolsTerminal => "tools_terminal",
            Self::ToolsTranslate => "tools_translate",
            Self::Num10k => "10k",
            Self::Num10mp => "10mp",
            Self::Num11mp => "11mp",
            Self::Num123 => "123",
            Self::Num12mp => "12mp",
            Self::Num13mp => "13mp",
            Self::Num14mp => "14mp",
            Self::Num15mp => "15mp",
            Self::Num16mp => "16mp",
            Self::Num17mp => "17mp",
            Self::Num18UpRating => "18_up_rating",
            Self::Num18mp => "18mp",
            Self::Num19mp => "19mp",
            Self::Num1k => "1k",
            Self::Num1kPlus => "1k_plus",
            Self::Num1xMobiledata => "1x_mobiledata",
            Self::Num1xMobiledataBadge => "1x_mobiledata_badge",
            Self::Num20mp => "20mp",
            Self::Num21mp => "21mp",
            Self::Num22mp => "22mp",
            Self::Num23mp => "23mp",
            Self::Num24fpsSelect => "24fps_select",
            Self::Num24mp => "24mp",
            Self::Num2d => "2d",
            Self::Num2d2 => "2d_2",
            Self::Num2k => "2k",
            Self::Num2kPlus => "2k_plus",
            Self::Num2mp => "2mp",
            Self::Num30fps => "30fps",
            Self::Num30fpsSelect => "30fps_select",
            Self::Num360 => "360",
            Self::Num3d => "3d",
            Self::Num3d2 => "3d_2",
            Self::Num3dRotation => "3d_rotation",
            Self::Num3gMobiledata => "3g_mobiledata",
            Self::Num3gMobiledataBadge => "3g_mobiledata_badge",
            Self::Num3k => "3k",
            Self::Num3kPlus => "3k_plus",
            Self::Num3mp => "3mp",
            Self::Num3p => "3p",
            Self::Num4gMobiledata => "4g_mobiledata",
            Self::Num4gMobiledataBadge => "4g_mobiledata_badge",
            Self::Num4gPlusMobiledata => "4g_plus_mobiledata",
            Self::Num4k => "4k",
            Self::Num4kPlus => "4k_plus",
            Self::Num4mp => "4mp",
            Self::Num50mp => "50mp",
            Self::Num5g => "5g",
            Self::Num5gMobiledataBadge => "5g_mobiledata_badge",
            Self::Num5k => "5k",
            Self::Num5kPlus => "5k_plus",
            Self::Num5mp => "5mp",
            Self::Num60fps => "60fps",
            Self::Num60fpsSelect => "60fps_select",
            Self::Num6FtApart => "6_ft_apart",
            Self::Num6k => "6k",
            Self::Num6kPlus => "6k_plus",
            Self::Num6mp => "6mp",
            Self::Num7k => "7k",
            Self::Num7kPlus => "7k_plus",
            Self::Num7mp => "7mp",
            Self::Num8k => "8k",
            Self::Num8kPlus => "8k_plus",
            Self::Num8mp => "8mp",
            Self::Num9k => "9k",
            Self::Num9kPlus => "9k_plus",
            Self::Num9mp => "9mp",
            Self::Abc => "abc",
            Self::AcUnit => "ac_unit",
            Self::AccessAlarm => "access_alarm",
            Self::AccessAlarms => "access_alarms",
            Self::AccessTime => "access_time",
            Self::AccessTimeFilled => "access_time_filled",
            Self::Accessibility => "accessibility",
            Self::AccessibilityNew => "accessibility_new",
            Self::Accessible => "accessible",
            Self::AccessibleForward => "accessible_forward",
            Self::AccessibleMenu => "accessible_menu",
            Self::AccountBalance => "account_balance",
            Self::AccountBalanceWallet => "account_balance_wallet",
            Self::AccountBox => "account_box",
            Self::AccountChild => "account_child",
            Self::AccountChildInvert => "account_child_invert",
            Self::AccountCircle => "account_circle",
            Self::AccountCircleFilled => "account_circle_filled",
            Self::AccountCircleOff => "account_circle_off",
            Self::AccountTree => "account_tree",
            Self::ActionKey => "action_key",
            Self::ActivityZone => "activity_zone",
            Self::Acupuncture => "acupuncture",
            Self::Acute => "acute",
            Self::Ad => "ad",
            Self::AdGroup => "ad_group",
            Self::AdGroupOff => "ad_group_off",
            Self::AdOff => "ad_off",
            Self::AdUnits => "ad_units",
            Self::AdaptiveAudioMic => "adaptive_audio_mic",
            Self::AdaptiveAudioMicOff => "adaptive_audio_mic_off",
            Self::Adb => "adb",
            Self::Add => "add",
            Self::Add2 => "add_2",
            Self::AddAPhoto => "add_a_photo",
            Self::AddAd => "add_ad",
            Self::AddAlarm => "add_alarm",
            Self::AddAlert => "add_alert",
            Self::AddBox => "add_box",
            Self::AddBusiness => "add_business",
            Self::AddCall => "add_call",
            Self::AddCard => "add_card",
            Self::AddChart => "add_chart",
            Self::AddCircle => "add_circle",
            Self::AddCircleOutline => "add_circle_outline",
            Self::AddColumnLeft => "add_column_left",
            Self::AddColumnRight => "add_column_right",
            Self::AddComment => "add_comment",
            Self::AddDiamond => "add_diamond",
            Self::AddHome => "add_home",
            Self::AddHomeWork => "add_home_work",
            Self::AddIcCall => "add_ic_call",
            Self::AddLink => "add_link",
            Self::AddLocation => "add_location",
            Self::AddLocationAlt => "add_location_alt",
            Self::AddModerator => "add_moderator",
            Self::AddNotes => "add_notes",
            Self::AddPhotoAlternate => "add_photo_alternate",
            Self::AddReaction => "add_reaction",
            Self::AddRoad => "add_road",
            Self::AddRowAbove => "add_row_above",
            Self::AddRowBelow => "add_row_below",
            Self::AddShoppingCart => "add_shopping_cart",
            Self::AddTask => "add_task",
            Self::AddToDrive => "add_to_drive",
            Self::AddToHomeScreen => "add_to_home_screen",
            Self::AddToPhotos => "add_to_photos",
            Self::AddToQueue => "add_to_queue",
            Self::AddTriangle => "add_triangle",
            Self::Addchart => "addchart",
            Self::AdfScanner => "adf_scanner",
            Self::Adjust => "adjust",
            Self::AdminMeds => "admin_meds",
            Self::AdminPanelSettings => "admin_panel_settings",
            Self::AdsClick => "ads_click",
            Self::Agender => "agender",
            Self::Agriculture => "agriculture",
            Self::Air => "air",
            Self::AirFreshener => "air_freshener",
            Self::AirPurifier => "air_purifier",
            Self::AirPurifierGen => "air_purifier_gen",
            Self::AirlineSeatFlat => "airline_seat_flat",
            Self::AirlineSeatFlatAngled => "airline_seat_flat_angled",
            Self::AirlineSeatIndividualSuite => "airline_seat_individual_suite",
            Self::AirlineSeatLegroomExtra => "airline_seat_legroom_extra",
            Self::AirlineSeatLegroomNormal => "airline_seat_legroom_normal",
            Self::AirlineSeatLegroomReduced => "airline_seat_legroom_reduced",
            Self::AirlineSeatReclineExtra => "airline_seat_recline_extra",
            Self::AirlineSeatReclineNormal => "airline_seat_recline_normal",
            Self::AirlineStops => "airline_stops",
            Self::Airlines => "airlines",
            Self::AirplaneTicket => "airplane_ticket",
            Self::AirplanemodeActive => "airplanemode_active",
            Self::AirplanemodeInactive => "airplanemode_inactive",
            Self::Airplay => "airplay",
            Self::AirportShuttle => "airport_shuttle",
            Self::Airware => "airware",
            Self::Airwave => "airwave",
            Self::Alarm => "alarm",
            Self::AlarmAdd => "alarm_add",
            Self::AlarmOff => "alarm_off",
            Self::AlarmOn => "alarm_on",
            Self::AlarmPause => "alarm_pause",
            Self::AlarmSmartWake => "alarm_smart_wake",
            Self::Album => "album",
            Self::AlignCenter => "align_center",
            Self::AlignEnd => "align_end",
            Self::AlignFlexCenter => "align_flex_center",
            Self::AlignFlexEnd => "align_flex_end",
            Self::AlignFlexStart => "align_flex_start",
            Self::AlignHorizontalCenter => "align_horizontal_center",
            Self::AlignHorizontalLeft => "align_horizontal_left",
            Self::AlignHorizontalRight => "align_horizontal_right",
            Self::AlignItemsStretch => "align_items_stretch",
            Self::AlignJustifyCenter => "align_justify_center",
            Self::AlignJustifyFlexEnd => "align_justify_flex_end",
            Self::AlignJustifyFlexStart => "align_justify_flex_start",
            Self::AlignJustifySpaceAround => "align_justify_space_around",
            Self::AlignJustifySpaceBetween => "align_justify_space_between",
            Self::AlignJustifySpaceEven => "align_justify_space_even",
            Self::AlignJustifyStretch => "align_justify_stretch",
            Self::AlignSelfStretch => "align_self_stretch",
            Self::AlignSpaceAround => "align_space_around",
            Self::AlignSpaceBetween => "align_space_between",
            Self::AlignSpaceEven => "align_space_even",
            Self::AlignStart => "align_start",
            Self::AlignStretch => "align_stretch",
            Self::AlignVerticalBottom => "align_vertical_bottom",
            Self::AlignVerticalCenter => "align_vertical_center",
            Self::AlignVerticalTop => "align_vertical_top",
            Self::AllInbox => "all_inbox",
            Self::AllInclusive => "all_inclusive",
            Self::AllMatch => "all_match",
            Self::AllOut => "all_out",
            Self::Allergies => "allergies",
            Self::Allergy => "allergy",
            Self::AltRoute => "alt_route",
            Self::AlternateEmail => "alternate_email",
            Self::Altitude => "altitude",
            Self::AmbientScreen => "ambient_screen",
            Self::Ambulance => "ambulance",
            Self::Amend => "amend",
            Self::AmpStories => "amp_stories",
            Self::Analytics => "analytics",
            Self::Anchor => "anchor",
            Self::Android => "android",
            Self::AndroidCell4Bar => "android_cell_4_bar",
            Self::AndroidCell4BarAlert => "android_cell_4_bar_alert",
            Self::AndroidCell4BarOff => "android_cell_4_bar_off",
            Self::AndroidCell4BarPlus => "android_cell_4_bar_plus",
            Self::AndroidCell5Bar => "android_cell_5_bar",
            Self::AndroidCell5BarAlert => "android_cell_5_bar_alert",
            Self::AndroidCell5BarOff => "android_cell_5_bar_off",
            Self::AndroidCell5BarPlus => "android_cell_5_bar_plus",
            Self::AndroidCellDual4Bar => "android_cell_dual_4_bar",
            Self::AndroidCellDual4BarAlert => "android_cell_dual_4_bar_alert",
            Self::AndroidCellDual4BarPlus => "android_cell_dual_4_bar_plus",
            Self::AndroidCellDual5Bar => "android_cell_dual_5_bar",
            Self::AndroidCellDual5BarAlert => "android_cell_dual_5_bar_alert",
            Self::AndroidCellDual5BarPlus => "android_cell_dual_5_bar_plus",
            Self::AndroidWifi3Bar => "android_wifi_3_bar",
            Self::AndroidWifi3BarAlert => "android_wifi_3_bar_alert",
            Self::AndroidWifi3BarLock => "android_wifi_3_bar_lock",
            Self::AndroidWifi3BarOff => "android_wifi_3_bar_off",
            Self::AndroidWifi3BarPlus => "android_wifi_3_bar_plus",
            Self::AndroidWifi3BarQuestion => "android_wifi_3_bar_question",
            Self::AndroidWifi4Bar => "android_wifi_4_bar",
            Self::AndroidWifi4BarAlert => "android_wifi_4_bar_alert",
            Self::AndroidWifi4BarLock => "android_wifi_4_bar_lock",
            Self::AndroidWifi4BarOff => "android_wifi_4_bar_off",
            Self::AndroidWifi4BarPlus => "android_wifi_4_bar_plus",
            Self::AndroidWifi4BarQuestion => "android_wifi_4_bar_question",
            Self::AnimatedImages => "animated_images",
            Self::Animation => "animation",
            Self::Announcement => "announcement",
            Self::Antigravity => "antigravity",
            Self::Aod => "aod",
            Self::AodTablet => "aod_tablet",
            Self::AodWatch => "aod_watch",
            Self::Apartment => "apartment",
            Self::Api => "api",
            Self::ApkDocument => "apk_document",
            Self::ApkInstall => "apk_install",
            Self::AppBadging => "app_badging",
            Self::AppBlocking => "app_blocking",
            Self::AppPromo => "app_promo",
            Self::AppRegistration => "app_registration",
            Self::AppSettingsAlt => "app_settings_alt",
            Self::AppShortcut => "app_shortcut",
            Self::Apparel => "apparel",
            Self::Approval => "approval",
            Self::ApprovalDelegation => "approval_delegation",
            Self::ApprovalDelegationOff => "approval_delegation_off",
            Self::Apps => "apps",
            Self::AppsOutage => "apps_outage",
            Self::AppsPlus => "apps_plus",
            Self::Aq => "aq",
            Self::AqIndoor => "aq_indoor",
            Self::ArOnYou => "ar_on_you",
            Self::ArStickers => "ar_stickers",
            Self::Architecture => "architecture",
            Self::Archive => "archive",
            Self::AreaChart => "area_chart",
            Self::ArmingCountdown => "arming_countdown",
            Self::ArrowAndEdge => "arrow_and_edge",
            Self::ArrowBack => "arrow_back",
            Self::ArrowBack2 => "arrow_back_2",
            Self::ArrowBackIos => "arrow_back_ios",
            Self::ArrowBackIosNew => "arrow_back_ios_new",
            Self::ArrowCircleDown => "arrow_circle_down",
            Self::ArrowCircleLeft => "arrow_circle_left",
            Self::ArrowCircleRight => "arrow_circle_right",
            Self::ArrowCircleUp => "arrow_circle_up",
            Self::ArrowCoolDown => "arrow_cool_down",
            Self::ArrowDownward => "arrow_downward",
            Self::ArrowDownwardAlt => "arrow_downward_alt",
            Self::ArrowDropDown => "arrow_drop_down",
            Self::ArrowDropDownCircle => "arrow_drop_down_circle",
            Self::ArrowDropUp => "arrow_drop_up",
            Self::ArrowForward => "arrow_forward",
            Self::ArrowForwardIos => "arrow_forward_ios",
            Self::ArrowInsert => "arrow_insert",
            Self::ArrowLeftAlt => "arrow_left_alt",
            Self::ArrowMenuClose => "arrow_menu_close",
            Self::ArrowMenuOpen => "arrow_menu_open",
            Self::ArrowOrEdge => "arrow_or_edge",
            Self::ArrowOutward => "arrow_outward",
            Self::ArrowRange => "arrow_range",
            Self::ArrowRightAlt => "arrow_right_alt",
            Self::ArrowSelectorTool => "arrow_selector_tool",
            Self::ArrowShapeUp => "arrow_shape_up",
            Self::ArrowShapeUpStack => "arrow_shape_up_stack",
            Self::ArrowShapeUpStack2 => "arrow_shape_up_stack_2",
            Self::ArrowSplit => "arrow_split",
            Self::ArrowTopLeft => "arrow_top_left",
            Self::ArrowTopRight => "arrow_top_right",
            Self::ArrowUploadProgress => "arrow_upload_progress",
            Self::ArrowUploadReady => "arrow_upload_ready",
            Self::ArrowUpward => "arrow_upward",
            Self::ArrowUpwardAlt => "arrow_upward_alt",
            Self::ArrowWarmUp => "arrow_warm_up",
            Self::ArrowsInput => "arrows_input",
            Self::ArrowsLeftRightCircle => "arrows_left_right_circle",
            Self::ArrowsMoreDown => "arrows_more_down",
            Self::ArrowsMoreUp => "arrows_more_up",
            Self::ArrowsOutput => "arrows_output",
            Self::ArrowsOutward => "arrows_outward",
            Self::ArrowsUpDownCircle => "arrows_up_down_circle",
            Self::ArtTrack => "art_track",
            Self::Article => "article",
            Self::ArticlePerson => "article_person",
            Self::ArticleShortcut => "article_shortcut",
            Self::Artist => "artist",
            Self::AspectRatio => "aspect_ratio",
            Self::Assessment => "assessment",
            Self::Assignment => "assignment",
            Self::AssignmentAdd => "assignment_add",
            Self::AssignmentGlobe => "assignment_globe",
            Self::AssignmentInd => "assignment_ind",
            Self::AssignmentLate => "assignment_late",
            Self::AssignmentReturn => "assignment_return",
            Self::AssignmentReturned => "assignment_returned",
            Self::AssignmentTurnedIn => "assignment_turned_in",
            Self::AssistWalker => "assist_walker",
            Self::Assistant => "assistant",
            Self::AssistantDevice => "assistant_device",
            Self::AssistantDirection => "assistant_direction",
            Self::AssistantNavigation => "assistant_navigation",
            Self::AssistantOnHub => "assistant_on_hub",
            Self::AssistantPhoto => "assistant_photo",
            Self::AssuredWorkload => "assured_workload",
            Self::Asterisk => "asterisk",
            Self::AstrophotographyAuto => "astrophotography_auto",
            Self::AstrophotographyOff => "astrophotography_off",
            Self::Atm => "atm",
            Self::Atr => "atr",
            Self::AttachEmail => "attach_email",
            Self::AttachFile => "attach_file",
            Self::AttachFileAdd => "attach_file_add",
            Self::AttachFileOff => "attach_file_off",
            Self::AttachMoney => "attach_money",
            Self::Attractions => "attractions",
            Self::Attribution => "attribution",
            Self::AudioCapture => "audio_capture",
            Self::AudioDescription => "audio_description",
            Self::AudioFile => "audio_file",
            Self::AudioVideoReceiver => "audio_video_receiver",
            Self::Audiotrack => "audiotrack",
            Self::AutoActivityZone => "auto_activity_zone",
            Self::AutoAwesome => "auto_awesome",
            Self::AutoAwesomeMosaic => "auto_awesome_mosaic",
            Self::AutoAwesomeMotion => "auto_awesome_motion",
            Self::AutoDelete => "auto_delete",
            Self::AutoDetectVoice => "auto_detect_voice",
            Self::AutoDrawSolid => "auto_draw_solid",
            Self::AutoFix => "auto_fix",
            Self::AutoFixHigh => "auto_fix_high",
            Self::AutoFixNormal => "auto_fix_normal",
            Self::AutoFixOff => "auto_fix_off",
            Self::AutoGraph => "auto_graph",
            Self::AutoLabel => "auto_label",
            Self::AutoMeetingRoom => "auto_meeting_room",
            Self::AutoMode => "auto_mode",
            Self::AutoReadPause => "auto_read_pause",
            Self::AutoReadPlay => "auto_read_play",
            Self::AutoSchedule => "auto_schedule",
            Self::AutoStories => "auto_stories",
            Self::AutoStoriesOff => "auto_stories_off",
            Self::AutoTimer => "auto_timer",
            Self::AutoTowing => "auto_towing",
            Self::AutoTransmission => "auto_transmission",
            Self::AutoVideocam => "auto_videocam",
            Self::AutofpsSelect => "autofps_select",
            Self::Automation => "automation",
            Self::Autopause => "autopause",
            Self::Autopay => "autopay",
            Self::Autoplay => "autoplay",
            Self::Autorenew => "autorenew",
            Self::Autostop => "autostop",
            Self::Av1 => "av1",
            Self::AvTimer => "av_timer",
            Self::Avc => "avc",
            Self::AvgPace => "avg_pace",
            Self::AvgTime => "avg_time",
            Self::AvocadoBean => "avocado_bean",
            Self::AwardMeal => "award_meal",
            Self::AwardStar => "award_star",
            Self::Azm => "azm",
            Self::BCircle => "b_circle",
            Self::BabyChangingStation => "baby_changing_station",
            Self::BackHand => "back_hand",
            Self::BackToTab => "back_to_tab",
            Self::BackgroundDotLarge => "background_dot_large",
            Self::BackgroundDotSmall => "background_dot_small",
            Self::BackgroundGridSmall => "background_grid_small",
            Self::BackgroundReplace => "background_replace",
            Self::BacklightHigh => "backlight_high",
            Self::BacklightHighOff => "backlight_high_off",
            Self::BacklightLow => "backlight_low",
            Self::Backpack => "backpack",
            Self::Backspace => "backspace",
            Self::Backup => "backup",
            Self::BackupTable => "backup_table",
            Self::Badge => "badge",
            Self::BadgeCriticalBattery => "badge_critical_battery",
            Self::Badminton => "badminton",
            Self::BakeryDining => "bakery_dining",
            Self::Balance => "balance",
            Self::Balcony => "balcony",
            Self::Ballot => "ballot",
            Self::BarChart4Bars => "bar_chart_4_bars",
            Self::BarChartOff => "bar_chart_off",
            Self::Barcode => "barcode",
            Self::BarcodeReader => "barcode_reader",
            Self::BarcodeScanner => "barcode_scanner",
            Self::Barefoot => "barefoot",
            Self::BatchPrediction => "batch_prediction",
            Self::BathBedrock => "bath_bedrock",
            Self::BathOutdoor => "bath_outdoor",
            Self::BathPrivate => "bath_private",
            Self::BathPublicLarge => "bath_public_large",
            Self::BathSoak => "bath_soak",
            Self::Bathroom => "bathroom",
            Self::Bathtub => "bathtub",
            Self::Battery0Bar => "battery_0_bar",
            Self::Battery1Bar => "battery_1_bar",
            Self::Battery20 => "battery_20",
            Self::Battery2Bar => "battery_2_bar",
            Self::Battery30 => "battery_30",
            Self::Battery3Bar => "battery_3_bar",
            Self::Battery4Bar => "battery_4_bar",
            Self::Battery50 => "battery_50",
            Self::Battery5Bar => "battery_5_bar",
            Self::Battery60 => "battery_60",
            Self::Battery6Bar => "battery_6_bar",
            Self::Battery80 => "battery_80",
            Self::Battery90 => "battery_90",
            Self::BatteryAlert => "battery_alert",
            Self::BatteryAndroid0 => "battery_android_0",
            Self::BatteryAndroid1 => "battery_android_1",
            Self::BatteryAndroid2 => "battery_android_2",
            Self::BatteryAndroid3 => "battery_android_3",
            Self::BatteryAndroid4 => "battery_android_4",
            Self::BatteryAndroid5 => "battery_android_5",
            Self::BatteryAndroid6 => "battery_android_6",
            Self::BatteryAndroidAlert => "battery_android_alert",
            Self::BatteryAndroidBolt => "battery_android_bolt",
            Self::BatteryAndroidFrame1 => "battery_android_frame_1",
            Self::BatteryAndroidFrame2 => "battery_android_frame_2",
            Self::BatteryAndroidFrame3 => "battery_android_frame_3",
            Self::BatteryAndroidFrame4 => "battery_android_frame_4",
            Self::BatteryAndroidFrame5 => "battery_android_frame_5",
            Self::BatteryAndroidFrame6 => "battery_android_frame_6",
            Self::BatteryAndroidFrameAlert => "battery_android_frame_alert",
            Self::BatteryAndroidFrameBolt => "battery_android_frame_bolt",
            Self::BatteryAndroidFrameFull => "battery_android_frame_full",
            Self::BatteryAndroidFramePlus => "battery_android_frame_plus",
            Self::BatteryAndroidFrameQuestion => "battery_android_frame_question",
            Self::BatteryAndroidFrameShare => "battery_android_frame_share",
            Self::BatteryAndroidFrameShield => "battery_android_frame_shield",
            Self::BatteryAndroidFull => "battery_android_full",
            Self::BatteryAndroidPlus => "battery_android_plus",
            Self::BatteryAndroidQuestion => "battery_android_question",
            Self::BatteryAndroidShare => "battery_android_share",
            Self::BatteryAndroidShield => "battery_android_shield",
            Self::BatteryChange => "battery_change",
            Self::BatteryCharging20 => "battery_charging_20",
            Self::BatteryCharging202 => "battery_charging_20_2",
            Self::BatteryCharging30 => "battery_charging_30",
            Self::BatteryCharging302 => "battery_charging_30_2",
            Self::BatteryCharging50 => "battery_charging_50",
            Self::BatteryCharging502 => "battery_charging_50_2",
            Self::BatteryCharging60 => "battery_charging_60",
            Self::BatteryCharging602 => "battery_charging_60_2",
            Self::BatteryCharging80 => "battery_charging_80",
            Self::BatteryCharging802 => "battery_charging_80_2",
            Self::BatteryCharging90 => "battery_charging_90",
            Self::BatteryChargingFull => "battery_charging_full",
            Self::BatteryChargingFull2 => "battery_charging_full_2",
            Self::BatteryError => "battery_error",
            Self::BatteryFull => "battery_full",
            Self::BatteryFullAlt => "battery_full_alt",
            Self::BatteryHoriz000 => "battery_horiz_000",
            Self::BatteryHoriz050 => "battery_horiz_050",
            Self::BatteryHoriz075 => "battery_horiz_075",
            Self::BatteryLow => "battery_low",
            Self::BatteryPlus => "battery_plus",
            Self::BatteryProfile => "battery_profile",
            Self::BatterySaver => "battery_saver",
            Self::BatteryShare => "battery_share",
            Self::BatteryStatusGood => "battery_status_good",
            Self::BatteryStd => "battery_std",
            Self::BatteryUnknown => "battery_unknown",
            Self::BatteryVert005 => "battery_vert_005",
            Self::BatteryVert020 => "battery_vert_020",
            Self::BatteryVert050 => "battery_vert_050",
            Self::BatteryVeryLow => "battery_very_low",
            Self::BeachAccess => "beach_access",
            Self::Bed => "bed",
            Self::BedroomBaby => "bedroom_baby",
            Self::BedroomChild => "bedroom_child",
            Self::BedroomParent => "bedroom_parent",
            Self::Bedtime => "bedtime",
            Self::BedtimeOff => "bedtime_off",
            Self::Beenhere => "beenhere",
            Self::BeerMeal => "beer_meal",
            Self::Bento => "bento",
            Self::Bia => "bia",
            Self::BidLandscape => "bid_landscape",
            Self::BidLandscapeDisabled => "bid_landscape_disabled",
            Self::BigtopUpdates => "bigtop_updates",
            Self::BikeDock => "bike_dock",
            Self::BikeLane => "bike_lane",
            Self::BikeScooter => "bike_scooter",
            Self::Biotech => "biotech",
            Self::Blanket => "blanket",
            Self::Blender => "blender",
            Self::Blind => "blind",
            Self::Blinds => "blinds",
            Self::Blinds2 => "blinds_2",
            Self::Blinds2Closed => "blinds_2_closed",
            Self::BlindsClosed => "blinds_closed",
            Self::BloodPressure => "blood_pressure",
            Self::Bloodtype => "bloodtype",
            Self::Bluetooth => "bluetooth",
            Self::BluetoothAudio => "bluetooth_audio",
            Self::BluetoothConnected => "bluetooth_connected",
            Self::BluetoothDisabled => "bluetooth_disabled",
            Self::BluetoothDrive => "bluetooth_drive",
            Self::BluetoothSearching => "bluetooth_searching",
            Self::BlurCircular => "blur_circular",
            Self::BlurLinear => "blur_linear",
            Self::BlurMedium => "blur_medium",
            Self::BlurOff => "blur_off",
            Self::BlurOn => "blur_on",
            Self::BlurShort => "blur_short",
            Self::BoatBus => "boat_bus",
            Self::BoatRailway => "boat_railway",
            Self::BodyFat => "body_fat",
            Self::BodySystem => "body_system",
            Self::Bolt => "bolt",
            Self::BoltBoost => "bolt_boost",
            Self::Bomb => "bomb",
            Self::Book => "book",
            Self::Book2 => "book_2",
            Self::Book3 => "book_3",
            Self::Book4 => "book_4",
            Self::Book5 => "book_5",
            Self::Book6 => "book_6",
            Self::BookOnline => "book_online",
            Self::BookRibbon => "book_ribbon",
            Self::Bookmark => "bookmark",
            Self::BookmarkAdd => "bookmark_add",
            Self::BookmarkAdded => "bookmark_added",
            Self::BookmarkBag => "bookmark_bag",
            Self::BookmarkBorder => "bookmark_border",
            Self::BookmarkCheck => "bookmark_check",
            Self::BookmarkFlag => "bookmark_flag",
            Self::BookmarkHeart => "bookmark_heart",
            Self::BookmarkManager => "bookmark_manager",
            Self::BookmarkRemove => "bookmark_remove",
            Self::BookmarkStacks => "bookmark_stacks",
            Self::BookmarkStar => "bookmark_star",
            Self::Bookmarks => "bookmarks",
            Self::BooksMoviesAndMusic => "books_movies_and_music",
            Self::BorderAll => "border_all",
            Self::BorderBottom => "border_bottom",
            Self::BorderClear => "border_clear",
            Self::BorderColor => "border_color",
            Self::BorderHorizontal => "border_horizontal",
            Self::BorderInner => "border_inner",
            Self::BorderLeft => "border_left",
            Self::BorderOuter => "border_outer",
            Self::BorderRight => "border_right",
            Self::BorderStyle => "border_style",
            Self::BorderTop => "border_top",
            Self::BorderVertical => "border_vertical",
            Self::Borg => "borg",
            Self::BottomAppBar => "bottom_app_bar",
            Self::BottomDrawer => "bottom_drawer",
            Self::BottomNavigation => "bottom_navigation",
            Self::BottomPanelClose => "bottom_panel_close",
            Self::BottomPanelOpen => "bottom_panel_open",
            Self::BottomRightClick => "bottom_right_click",
            Self::BottomSheets => "bottom_sheets",
            Self::Box => "box",
            Self::BoxAdd => "box_add",
            Self::BoxEdit => "box_edit",
            Self::Boy => "boy",
            Self::BrandAwareness => "brand_awareness",
            Self::BrandFamily => "brand_family",
            Self::BrandingWatermark => "branding_watermark",
            Self::BreakfastDining => "breakfast_dining",
            Self::BreakingNews => "breaking_news",
            Self::BreakingNewsAlt1 => "breaking_news_alt_1",
            Self::Breastfeeding => "breastfeeding",
            Self::Brick => "brick",
            Self::BriefcaseMeal => "briefcase_meal",
            Self::Brightness1 => "brightness_1",
            Self::Brightness2 => "brightness_2",
            Self::Brightness3 => "brightness_3",
            Self::Brightness4 => "brightness_4",
            Self::Brightness5 => "brightness_5",
            Self::Brightness6 => "brightness_6",
            Self::Brightness7 => "brightness_7",
            Self::BrightnessAlert => "brightness_alert",
            Self::BrightnessAuto => "brightness_auto",
            Self::BrightnessEmpty => "brightness_empty",
            Self::BrightnessHigh => "brightness_high",
            Self::BrightnessLow => "brightness_low",
            Self::BrightnessMedium => "brightness_medium",
            Self::BringYourOwnIp => "bring_your_own_ip",
            Self::BroadcastOnHome => "broadcast_on_home",
            Self::BroadcastOnPersonal => "broadcast_on_personal",
            Self::BrokenImage => "broken_image",
            Self::Browse => "browse",
            Self::BrowseActivity => "browse_activity",
            Self::BrowseGallery => "browse_gallery",
            Self::BrowserNotSupported => "browser_not_supported",
            Self::BrowserUpdated => "browser_updated",
            Self::BrunchDining => "brunch_dining",
            Self::Brush => "brush",
            Self::Bubble => "bubble",
            Self::BubbleChart => "bubble_chart",
            Self::Bubbles => "bubbles",
            Self::BucketCheck => "bucket_check",
            Self::BugReport => "bug_report",
            Self::Build => "build",
            Self::BuildCircle => "build_circle",
            Self::BulletChart => "bullet_chart",
            Self::Bungalow => "bungalow",
            Self::BurstMode => "burst_mode",
            Self::BusAlert => "bus_alert",
            Self::BusMapPin => "bus_map_pin",
            Self::BusRailway => "bus_railway",
            Self::Business => "business",
            Self::BusinessCenter => "business_center",
            Self::BusinessChip => "business_chip",
            Self::BusinessMessages => "business_messages",
            Self::ButtonsAlt => "buttons_alt",
            Self::Cabin => "cabin",
            Self::Cable => "cable",
            Self::CableCar => "cable_car",
            Self::Cached => "cached",
            Self::Cadence => "cadence",
            Self::Cake => "cake",
            Self::CakeAdd => "cake_add",
            Self::Calculate => "calculate",
            Self::CalendarAddOn => "calendar_add_on",
            Self::CalendarAppsScript => "calendar_apps_script",
            Self::CalendarCheck => "calendar_check",
            Self::CalendarClock => "calendar_clock",
            Self::CalendarLock => "calendar_lock",
            Self::CalendarMeal => "calendar_meal",
            Self::CalendarMeal2 => "calendar_meal_2",
            Self::CalendarMonth => "calendar_month",
            Self::CalendarToday => "calendar_today",
            Self::CalendarViewDay => "calendar_view_day",
            Self::CalendarViewMonth => "calendar_view_month",
            Self::CalendarViewWeek => "calendar_view_week",
            Self::CallEnd => "call_end",
            Self::CallEndAlt => "call_end_alt",
            Self::CallLog => "call_log",
            Self::CallMade => "call_made",
            Self::CallMerge => "call_merge",
            Self::CallMissed => "call_missed",
            Self::CallMissedOutgoing => "call_missed_outgoing",
            Self::CallQuality => "call_quality",
            Self::CallReceived => "call_received",
            Self::CallSplit => "call_split",
            Self::CallToAction => "call_to_action",
            Self::Camera => "camera",
            Self::CameraAlt => "camera_alt",
            Self::CameraEnhance => "camera_enhance",
            Self::CameraFront => "camera_front",
            Self::CameraIndoor => "camera_indoor",
            Self::CameraOutdoor => "camera_outdoor",
            Self::CameraRear => "camera_rear",
            Self::CameraRoll => "camera_roll",
            Self::CameraVideo => "camera_video",
            Self::Cameraswitch => "cameraswitch",
            Self::Campaign => "campaign",
            Self::Camping => "camping",
            Self::Cancel => "cancel",
            Self::CancelPresentation => "cancel_presentation",
            Self::CancelScheduleSend => "cancel_schedule_send",
            Self::Candle => "candle",
            Self::CandlestickChart => "candlestick_chart",
            Self::Cannabis => "cannabis",
            Self::CaptivePortal => "captive_portal",
            Self::Capture => "capture",
            Self::CarCrash => "car_crash",
            Self::CarDefrostLeft => "car_defrost_left",
            Self::CarDefrostLowLeft => "car_defrost_low_left",
            Self::CarDefrostLowRight => "car_defrost_low_right",
            Self::CarDefrostMidLeft => "car_defrost_mid_left",
            Self::CarDefrostMidLowLeft => "car_defrost_mid_low_left",
            Self::CarDefrostMidLowRight => "car_defrost_mid_low_right",
            Self::CarDefrostMidRight => "car_defrost_mid_right",
            Self::CarDefrostRight => "car_defrost_right",
            Self::CarFanLowLeft => "car_fan_low_left",
            Self::CarFanLowMidLeft => "car_fan_low_mid_left",
            Self::CarFanLowRight => "car_fan_low_right",
            Self::CarFanMidLeft => "car_fan_mid_left",
            Self::CarFanMidLowRight => "car_fan_mid_low_right",
            Self::CarFanMidRight => "car_fan_mid_right",
            Self::CarFanRecirculate => "car_fan_recirculate",
            Self::CarFanRecirculate2 => "car_fan_recirculate_2",
            Self::CarGear => "car_gear",
            Self::CarLock => "car_lock",
            Self::CarMirrorHeat => "car_mirror_heat",
            Self::CarRental => "car_rental",
            Self::CarRepair => "car_repair",
            Self::CarSeatOff => "car_seat_off",
            Self::CarTag => "car_tag",
            Self::CardGiftcard => "card_giftcard",
            Self::CardMembership => "card_membership",
            Self::CardTravel => "card_travel",
            Self::CardioLoad => "cardio_load",
            Self::Cardiology => "cardiology",
            Self::Cards => "cards",
            Self::CardsStack => "cards_stack",
            Self::CardsStar => "cards_star",
            Self::Carpenter => "carpenter",
            Self::CarryOnBag => "carry_on_bag",
            Self::CarryOnBagChecked => "carry_on_bag_checked",
            Self::CarryOnBagInactive => "carry_on_bag_inactive",
            Self::CarryOnBagQuestion => "carry_on_bag_question",
            Self::Cases => "cases",
            Self::Casino => "casino",
            Self::Cast => "cast",
            Self::CastConnected => "cast_connected",
            Self::CastForEducation => "cast_for_education",
            Self::CastPause => "cast_pause",
            Self::CastWarning => "cast_warning",
            Self::Castle => "castle",
            Self::Category => "category",
            Self::CategorySearch => "category_search",
            Self::Celebration => "celebration",
            Self::CellMerge => "cell_merge",
            Self::CellTower => "cell_tower",
            Self::CellWifi => "cell_wifi",
            Self::CenterFocusStrong => "center_focus_strong",
            Self::CenterFocusWeak => "center_focus_weak",
            Self::Chair => "chair",
            Self::ChairAlt => "chair_alt",
            Self::ChairCounter => "chair_counter",
            Self::ChairFireplace => "chair_fireplace",
            Self::ChairUmbrella => "chair_umbrella",
            Self::Chalet => "chalet",
            Self::ChangeCircle => "change_circle",
            Self::ChangeHistory => "change_history",
            Self::Charger => "charger",
            Self::ChargingStation => "charging_station",
            Self::ChartData => "chart_data",
            Self::ChatAddOn => "chat_add_on",
            Self::ChatAppsScript => "chat_apps_script",
            Self::ChatBubble => "chat_bubble",
            Self::ChatBubbleOff => "chat_bubble_off",
            Self::ChatBubbleOutline => "chat_bubble_outline",
            Self::ChatDashed => "chat_dashed",
            Self::ChatDisplay => "chat_display",
            Self::ChatError => "chat_error",
            Self::ChatInfo => "chat_info",
            Self::ChatPasteGo => "chat_paste_go",
            Self::ChatPasteGo2 => "chat_paste_go_2",
            Self::CheckAlert => "check_alert",
            Self::CheckBox => "check_box",
            Self::CheckBoxOutlineBlank => "check_box_outline_blank",
            Self::CheckCircle => "check_circle",
            Self::CheckCircleFilled => "check_circle_filled",
            Self::CheckCircleOutline => "check_circle_outline",
            Self::CheckCircleUnread => "check_circle_unread",
            Self::CheckInOut => "check_in_out",
            Self::CheckIndeterminateSmall => "check_indeterminate_small",
            Self::CheckSmall => "check_small",
            Self::Checkbook => "checkbook",
            Self::CheckedBag => "checked_bag",
            Self::CheckedBagQuestion => "checked_bag_question",
            Self::Checklist => "checklist",
            Self::ChecklistRtl => "checklist_rtl",
            Self::Checkroom => "checkroom",
            Self::Cheer => "cheer",
            Self::ChefHat => "chef_hat",
            Self::Chess => "chess",
            Self::ChessBishop => "chess_bishop",
            Self::ChessBishop2 => "chess_bishop_2",
            Self::ChessKing => "chess_king",
            Self::ChessKing2 => "chess_king_2",
            Self::ChessKnight => "chess_knight",
            Self::ChessPawn => "chess_pawn",
            Self::ChessPawn2 => "chess_pawn_2",
            Self::ChessQueen => "chess_queen",
            Self::ChessRook => "chess_rook",
            Self::ChevronBackward => "chevron_backward",
            Self::ChevronForward => "chevron_forward",
            Self::ChevronLineUp => "chevron_line_up",
            Self::ChildCare => "child_care",
            Self::ChildFriendly => "child_friendly",
            Self::ChildHat => "child_hat",
            Self::ChipExtraction => "chip_extraction",
            Self::Chips => "chips",
            Self::ChromeReaderMode => "chrome_reader_mode",
            Self::Chromecast2 => "chromecast_2",
            Self::ChromecastDevice => "chromecast_device",
            Self::Chronic => "chronic",
            Self::Church => "church",
            Self::CinematicBlur => "cinematic_blur",
            Self::Circle => "circle",
            Self::CircleCircle => "circle_circle",
            Self::CircleNotifications => "circle_notifications",
            Self::Circles => "circles",
            Self::CirclesExt => "circles_ext",
            Self::Clarify => "clarify",
            Self::Class => "class",
            Self::CleanHands => "clean_hands",
            Self::Cleaning => "cleaning",
            Self::CleaningBucket => "cleaning_bucket",
            Self::CleaningServices => "cleaning_services",
            Self::Clear => "clear",
            Self::ClearAll => "clear_all",
            Self::ClearDay => "clear_day",
            Self::ClearNight => "clear_night",
            Self::ClimateMiniSplit => "climate_mini_split",
            Self::ClinicalNotes => "clinical_notes",
            Self::ClockArrowDown => "clock_arrow_down",
            Self::ClockArrowUp => "clock_arrow_up",
            Self::ClockLoader10 => "clock_loader_10",
            Self::ClockLoader20 => "clock_loader_20",
            Self::ClockLoader40 => "clock_loader_40",
            Self::ClockLoader60 => "clock_loader_60",
            Self::ClockLoader80 => "clock_loader_80",
            Self::ClockLoader90 => "clock_loader_90",
            Self::CloseFullscreen => "close_fullscreen",
            Self::CloseSmall => "close_small",
            Self::ClosedCaption => "closed_caption",
            Self::ClosedCaptionAdd => "closed_caption_add",
            Self::ClosedCaptionDisabled => "closed_caption_disabled",
            Self::ClosedCaptionDisplay => "closed_caption_display",
            Self::ClosedCaptionOff => "closed_caption_off",
            Self::Cloud => "cloud",
            Self::CloudAlert => "cloud_alert",
            Self::CloudCircle => "cloud_circle",
            Self::CloudDone => "cloud_done",
            Self::CloudDownload => "cloud_download",
            Self::CloudLock => "cloud_lock",
            Self::CloudOff => "cloud_off",
            Self::CloudQueue => "cloud_queue",
            Self::CloudSync => "cloud_sync",
            Self::CloudUpload => "cloud_upload",
            Self::Cloudy => "cloudy",
            Self::CloudyFilled => "cloudy_filled",
            Self::CloudySnowing => "cloudy_snowing",
            Self::Co2 => "co2",
            Self::CoPresent => "co_present",
            Self::Code => "code",
            Self::CodeBlocks => "code_blocks",
            Self::CodeOff => "code_off",
            Self::CodeXml => "code_xml",
            Self::Coffee => "coffee",
            Self::CoffeeMaker => "coffee_maker",
            Self::Cognition => "cognition",
            Self::Cognition2 => "cognition_2",
            Self::CollapseAll => "collapse_all",
            Self::CollapseContent => "collapse_content",
            Self::Collections => "collections",
            Self::CollectionsBookmark => "collections_bookmark",
            Self::ColorLens => "color_lens",
            Self::Colorize => "colorize",
            Self::Colors => "colors",
            Self::CombineColumns => "combine_columns",
            Self::ComedyMask => "comedy_mask",
            Self::ComicBubble => "comic_bubble",
            Self::Comment => "comment",
            Self::CommentBank => "comment_bank",
            Self::CommentsDisabled => "comments_disabled",
            Self::Commit => "commit",
            Self::Communication => "communication",
            Self::Communities => "communities",
            Self::CommunitiesFilled => "communities_filled",
            Self::Commute => "commute",
            Self::Compare => "compare",
            Self::CompareArrows => "compare_arrows",
            Self::CompassCalibration => "compass_calibration",
            Self::ComponentExchange => "component_exchange",
            Self::Compost => "compost",
            Self::Compress => "compress",
            Self::Computer => "computer",
            Self::ComputerArrowUp => "computer_arrow_up",
            Self::ComputerCancel => "computer_cancel",
            Self::ComputerSound => "computer_sound",
            Self::Concierge => "concierge",
            Self::Conditions => "conditions",
            Self::ConfirmationNumber => "confirmation_number",
            Self::Congenital => "congenital",
            Self::ConnectWithoutContact => "connect_without_contact",
            Self::ConnectedTv => "connected_tv",
            Self::ConnectingAirports => "connecting_airports",
            Self::Construction => "construction",
            Self::ContactEmergency => "contact_emergency",
            Self::ContactMail => "contact_mail",
            Self::ContactPage => "contact_page",
            Self::ContactPhone => "contact_phone",
            Self::ContactPhoneFilled => "contact_phone_filled",
            Self::ContactSupport => "contact_support",
            Self::Contactless => "contactless",
            Self::ContactlessOff => "contactless_off",
            Self::Contacts => "contacts",
            Self::ContactsProduct => "contacts_product",
            Self::ContentCopy => "content_copy",
            Self::ContentCut => "content_cut",
            Self::ContentPaste => "content_paste",
            Self::ContentPasteGo => "content_paste_go",
            Self::ContentPasteOff => "content_paste_off",
            Self::ContentPasteSearch => "content_paste_search",
            Self::ContextualToken => "contextual_token",
            Self::ContextualTokenAdd => "contextual_token_add",
            Self::Contract => "contract",
            Self::ContractDelete => "contract_delete",
            Self::ContractEdit => "contract_edit",
            Self::Contrast => "contrast",
            Self::ContrastCircle => "contrast_circle",
            Self::ContrastRtlOff => "contrast_rtl_off",
            Self::ContrastSquare => "contrast_square",
            Self::ControlCamera => "control_camera",
            Self::ControlPoint => "control_point",
            Self::ControlPointDuplicate => "control_point_duplicate",
            Self::ControllerGen => "controller_gen",
            Self::Conversation => "conversation",
            Self::ConversionPath => "conversion_path",
            Self::ConversionPathOff => "conversion_path_off",
            Self::ConvertToText => "convert_to_text",
            Self::ConveyorBelt => "conveyor_belt",
            Self::Cookie => "cookie",
            Self::CookieOff => "cookie_off",
            Self::Cooking => "cooking",
            Self::CoolToDry => "cool_to_dry",
            Self::CopyAll => "copy_all",
            Self::Copyright => "copyright",
            Self::Coronavirus => "coronavirus",
            Self::CorporateFare => "corporate_fare",
            Self::Cottage => "cottage",
            Self::Counter0 => "counter_0",
            Self::Counter1 => "counter_1",
            Self::Counter2 => "counter_2",
            Self::Counter3 => "counter_3",
            Self::Counter4 => "counter_4",
            Self::Counter5 => "counter_5",
            Self::Counter6 => "counter_6",
            Self::Counter7 => "counter_7",
            Self::Counter8 => "counter_8",
            Self::Counter9 => "counter_9",
            Self::Countertops => "countertops",
            Self::Create => "create",
            Self::CreateNewFolder => "create_new_folder",
            Self::CreditCard => "credit_card",
            Self::CreditCardClock => "credit_card_clock",
            Self::CreditCardGear => "credit_card_gear",
            Self::CreditCardHeart => "credit_card_heart",
            Self::CreditCardOff => "credit_card_off",
            Self::CreditScore => "credit_score",
            Self::Crib => "crib",
            Self::CrisisAlert => "crisis_alert",
            Self::Crop => "crop",
            Self::Crop169 => "crop_16_9",
            Self::Crop219 => "crop_21_9",
            Self::Crop23 => "crop_2_3",
            Self::Crop32 => "crop_3_2",
            Self::Crop54 => "crop_5_4",
            Self::Crop75 => "crop_7_5",
            Self::Crop916 => "crop_9_16",
            Self::CropDin => "crop_din",
            Self::CropFree => "crop_free",
            Self::CropLandscape => "crop_landscape",
            Self::CropOriginal => "crop_original",
            Self::CropPortrait => "crop_portrait",
            Self::CropRotate => "crop_rotate",
            Self::CropSquare => "crop_square",
            Self::Crossword => "crossword",
            Self::Crowdsource => "crowdsource",
            Self::Crown => "crown",
            Self::CrueltyFree => "cruelty_free",
            Self::Css => "css",
            Self::Csv => "csv",
            Self::CurrencyBitcoin => "currency_bitcoin",
            Self::CurrencyExchange => "currency_exchange",
            Self::CurrencyFranc => "currency_franc",
            Self::CurrencyLira => "currency_lira",
            Self::CurrencyPound => "currency_pound",
            Self::CurrencyRuble => "currency_ruble",
            Self::CurrencyRupee => "currency_rupee",
            Self::CurrencyRupeeCircle => "currency_rupee_circle",
            Self::CurrencyYen => "currency_yen",
            Self::CurrencyYuan => "currency_yuan",
            Self::Curtains => "curtains",
            Self::CurtainsClosed => "curtains_closed",
            Self::CustomTypography => "custom_typography",
            Self::Cycle => "cycle",
            Self::Cyclone => "cyclone",
            Self::Dangerous => "dangerous",
            Self::DarkMode => "dark_mode",
            Self::Dashboard => "dashboard",
            Self::Dashboard2 => "dashboard_2",
            Self::Dashboard2Add => "dashboard_2_add",
            Self::Dashboard2Edit => "dashboard_2_edit",
            Self::Dashboard2Gear => "dashboard_2_gear",
            Self::DashboardCustomize => "dashboard_customize",
            Self::DataAlert => "data_alert",
            Self::DataArray => "data_array",
            Self::DataCheck => "data_check",
            Self::DataExploration => "data_exploration",
            Self::DataInfoAlert => "data_info_alert",
            Self::DataLossPrevention => "data_loss_prevention",
            Self::DataObject => "data_object",
            Self::DataSaverOff => "data_saver_off",
            Self::DataSaverOn => "data_saver_on",
            Self::DataTable => "data_table",
            Self::DataThresholding => "data_thresholding",
            Self::DataUsage => "data_usage",
            Self::Database => "database",
            Self::DatabaseOff => "database_off",
            Self::DatabaseSearch => "database_search",
            Self::DatabaseUpload => "database_upload",
            Self::Dataset => "dataset",
            Self::DatasetLinked => "dataset_linked",
            Self::DateRange => "date_range",
            Self::Deblur => "deblur",
            Self::Deceased => "deceased",
            Self::DecimalDecrease => "decimal_decrease",
            Self::DecimalIncrease => "decimal_increase",
            Self::Deck => "deck",
            Self::Dehaze => "dehaze",
            Self::Delete => "delete",
            Self::DeleteForever => "delete_forever",
            Self::DeleteHistory => "delete_history",
            Self::DeleteOutline => "delete_outline",
            Self::DeleteSweep => "delete_sweep",
            Self::DeliveryDining => "delivery_dining",
            Self::DeliveryTruckBolt => "delivery_truck_bolt",
            Self::DeliveryTruckSpeed => "delivery_truck_speed",
            Self::Demography => "demography",
            Self::DensityLarge => "density_large",
            Self::DensityMedium => "density_medium",
            Self::DensitySmall => "density_small",
            Self::Dentistry => "dentistry",
            Self::DepartureBoard => "departure_board",
            Self::DeployedCode => "deployed_code",
            Self::DeployedCodeAccount => "deployed_code_account",
            Self::DeployedCodeAlert => "deployed_code_alert",
            Self::DeployedCodeHistory => "deployed_code_history",
            Self::DeployedCodeUpdate => "deployed_code_update",
            Self::Dermatology => "dermatology",
            Self::Description => "description",
            Self::Deselect => "deselect",
            Self::DesignServices => "design_services",
            Self::Desk => "desk",
            Self::Deskphone => "deskphone",
            Self::DesktopAccessDisabled => "desktop_access_disabled",
            Self::DesktopCloud => "desktop_cloud",
            Self::DesktopCloudStack => "desktop_cloud_stack",
            Self::DesktopLandscape => "desktop_landscape",
            Self::DesktopLandscapeAdd => "desktop_landscape_add",
            Self::DesktopMac => "desktop_mac",
            Self::DesktopPortrait => "desktop_portrait",
            Self::DesktopWindows => "desktop_windows",
            Self::Destruction => "destruction",
            Self::Details => "details",
            Self::DetectionAndZone => "detection_and_zone",
            Self::DetectionAndZoneOff => "detection_and_zone_off",
            Self::Detector => "detector",
            Self::DetectorAlarm => "detector_alarm",
            Self::DetectorBattery => "detector_battery",
            Self::DetectorCo => "detector_co",
            Self::DetectorOffline => "detector_offline",
            Self::DetectorSmoke => "detector_smoke",
            Self::DetectorStatus => "detector_status",
            Self::DeveloperBoard => "developer_board",
            Self::DeveloperBoardOff => "developer_board_off",
            Self::DeveloperGuide => "developer_guide",
            Self::DeveloperMode => "developer_mode",
            Self::DeveloperModeTv => "developer_mode_tv",
            Self::DeviceBand => "device_band",
            Self::DeviceHub => "device_hub",
            Self::DeviceReset => "device_reset",
            Self::DeviceSwooshStar => "device_swoosh_star",
            Self::DeviceThermostat => "device_thermostat",
            Self::DeviceUnknown => "device_unknown",
            Self::Devices => "devices",
            Self::DevicesFold => "devices_fold",
            Self::DevicesFold2 => "devices_fold_2",
            Self::DevicesOff => "devices_off",
            Self::DevicesOther => "devices_other",
            Self::DevicesWearables => "devices_wearables",
            Self::DewPoint => "dew_point",
            Self::Diagnosis => "diagnosis",
            Self::DiagonalLine => "diagonal_line",
            Self::DialerSip => "dialer_sip",
            Self::Dialogs => "dialogs",
            Self::Dialpad => "dialpad",
            Self::Diamond => "diamond",
            Self::DiamondShine => "diamond_shine",
            Self::Dictionary => "dictionary",
            Self::Difference => "difference",
            Self::DigitalOutOfHome => "digital_out_of_home",
            Self::DigitalWellbeing => "digital_wellbeing",
            Self::DineHeart => "dine_heart",
            Self::DineIn => "dine_in",
            Self::DineLamp => "dine_lamp",
            Self::Dining => "dining",
            Self::DinnerDining => "dinner_dining",
            Self::Directions => "directions",
            Self::DirectionsAlt => "directions_alt",
            Self::DirectionsAltOff => "directions_alt_off",
            Self::DirectionsBike => "directions_bike",
            Self::DirectionsBoat => "directions_boat",
            Self::DirectionsBoatFilled => "directions_boat_filled",
            Self::DirectionsBus => "directions_bus",
            Self::DirectionsBusFilled => "directions_bus_filled",
            Self::DirectionsCar => "directions_car",
            Self::DirectionsCarFilled => "directions_car_filled",
            Self::DirectionsOff => "directions_off",
            Self::DirectionsRailway => "directions_railway",
            Self::DirectionsRailway2 => "directions_railway_2",
            Self::DirectionsRailwayFilled => "directions_railway_filled",
            Self::DirectionsRun => "directions_run",
            Self::DirectionsSubway => "directions_subway",
            Self::DirectionsSubwayFilled => "directions_subway_filled",
            Self::DirectionsTransit => "directions_transit",
            Self::DirectionsTransitFilled => "directions_transit_filled",
            Self::DirectionsWalk => "directions_walk",
            Self::DirectorySync => "directory_sync",
            Self::DirtyLens => "dirty_lens",
            Self::DisabledByDefault => "disabled_by_default",
            Self::DisabledVisible => "disabled_visible",
            Self::DiscFull => "disc_full",
            Self::DiscoverTune => "discover_tune",
            Self::Dishwasher => "dishwasher",
            Self::DishwasherGen => "dishwasher_gen",
            Self::DisplayAdd => "display_add",
            Self::DisplayExternalInput => "display_external_input",
            Self::DisplayGroup => "display_group",
            Self::DisplaySettings => "display_settings",
            Self::Distance => "distance",
            Self::Diversity1 => "diversity_1",
            Self::Diversity2 => "diversity_2",
            Self::Diversity3 => "diversity_3",
            Self::Diversity4 => "diversity_4",
            Self::Dns => "dns",
            Self::DoDisturb => "do_disturb",
            Self::DoDisturbAlt => "do_disturb_alt",
            Self::DoDisturbOff => "do_disturb_off",
            Self::DoDisturbOn => "do_disturb_on",
            Self::DoNotDisturb => "do_not_disturb",
            Self::DoNotDisturbAlt => "do_not_disturb_alt",
            Self::DoNotDisturbOff => "do_not_disturb_off",
            Self::DoNotDisturbOn => "do_not_disturb_on",
            Self::DoNotDisturbOnTotalSilence => "do_not_disturb_on_total_silence",
            Self::DoNotStep => "do_not_step",
            Self::DoNotTouch => "do_not_touch",
            Self::Dock => "dock",
            Self::DockToBottom => "dock_to_bottom",
            Self::DockToLeft => "dock_to_left",
            Self::DockToRight => "dock_to_right",
            Self::Docs => "docs",
            Self::DocsAddOn => "docs_add_on",
            Self::DocsAppsScript => "docs_apps_script",
            Self::DocumentScanner => "document_scanner",
            Self::DocumentSearch => "document_search",
            Self::DocumentShare => "document_share",
            Self::Domain => "domain",
            Self::DomainAdd => "domain_add",
            Self::DomainDisabled => "domain_disabled",
            Self::DomainDisabledCheck => "domain_disabled_check",
            Self::DomainVerification => "domain_verification",
            Self::DomainVerificationOff => "domain_verification_off",
            Self::DominoMask => "domino_mask",
            Self::Done => "done",
            Self::DoneAll => "done_all",
            Self::DoneOutline => "done_outline",
            Self::DonutLarge => "donut_large",
            Self::DonutSmall => "donut_small",
            Self::DoorBack => "door_back",
            Self::DoorFront => "door_front",
            Self::DoorOpen => "door_open",
            Self::DoorSensor => "door_sensor",
            Self::DoorSliding => "door_sliding",
            Self::Doorbell => "doorbell",
            Self::Doorbell3p => "doorbell_3p",
            Self::DoorbellChime => "doorbell_chime",
            Self::DoubleArrow => "double_arrow",
            Self::DownhillSkiing => "downhill_skiing",
            Self::Download2 => "download_2",
            Self::DownloadDone => "download_done",
            Self::DownloadForOffline => "download_for_offline",
            Self::Downloading => "downloading",
            Self::Draft => "draft",
            Self::DraftOrders => "draft_orders",
            Self::Drafts => "drafts",
            Self::DragClick => "drag_click",
            Self::DragHandle => "drag_handle",
            Self::DragIndicator => "drag_indicator",
            Self::DragPan => "drag_pan",
            Self::Draw => "draw",
            Self::DrawAbstract => "draw_abstract",
            Self::DrawCollage => "draw_collage",
            Self::DrawingRecognition => "drawing_recognition",
            Self::Dresser => "dresser",
            Self::DriveEta => "drive_eta",
            Self::DriveExport => "drive_export",
            Self::DriveFileMove => "drive_file_move",
            Self::DriveFileMoveOutline => "drive_file_move_outline",
            Self::DriveFileMoveRtl => "drive_file_move_rtl",
            Self::DriveFileRename => "drive_file_rename",
            Self::DriveFileRenameOutline => "drive_file_rename_outline",
            Self::DriveFolderUpload => "drive_folder_upload",
            Self::Drone => "drone",
            Self::Drone2 => "drone_2",
            Self::Dropdown => "dropdown",
            Self::DropdownMenu => "dropdown_menu",
            Self::DropperEye => "dropper_eye",
            Self::Dry => "dry",
            Self::DryCleaning => "dry_cleaning",
            Self::DualScreen => "dual_screen",
            Self::Duo => "duo",
            Self::Dvr => "dvr",
            Self::DynamicFeed => "dynamic_feed",
            Self::DynamicForm => "dynamic_form",
            Self::E911Avatar => "e911_avatar",
            Self::E911Emergency => "e911_emergency",
            Self::EMobiledata => "e_mobiledata",
            Self::EMobiledataBadge => "e_mobiledata_badge",
            Self::EarSound => "ear_sound",
            Self::EarbudCase => "earbud_case",
            Self::EarbudLeft => "earbud_left",
            Self::EarbudRight => "earbud_right",
            Self::Earbuds => "earbuds",
            Self::Earbuds2 => "earbuds_2",
            Self::EarbudsBattery => "earbuds_battery",
            Self::EarlyOn => "early_on",
            Self::Earthquake => "earthquake",
            Self::East => "east",
            Self::Ecg => "ecg",
            Self::EcgHeart => "ecg_heart",
            Self::Eco => "eco",
            Self::Eda => "eda",
            Self::EdgesensorHigh => "edgesensor_high",
            Self::EdgesensorLow => "edgesensor_low",
            Self::EditArrowDown => "edit_arrow_down",
            Self::EditArrowUp => "edit_arrow_up",
            Self::EditAttributes => "edit_attributes",
            Self::EditAudio => "edit_audio",
            Self::EditCalendar => "edit_calendar",
            Self::EditDocument => "edit_document",
            Self::EditLine => "edit_line",
            Self::EditLocation => "edit_location",
            Self::EditLocationAlt => "edit_location_alt",
            Self::EditNote => "edit_note",
            Self::EditNotifications => "edit_notifications",
            Self::EditOff => "edit_off",
            Self::EditRoad => "edit_road",
            Self::EditSquare => "edit_square",
            Self::EditorChoice => "editor_choice",
            Self::Egg => "egg",
            Self::EggAlt => "egg_alt",
            Self::Eject => "eject",
            Self::Elderly => "elderly",
            Self::ElderlyWoman => "elderly_woman",
            Self::ElectricBike => "electric_bike",
            Self::ElectricBolt => "electric_bolt",
            Self::ElectricCar => "electric_car",
            Self::ElectricMeter => "electric_meter",
            Self::ElectricMoped => "electric_moped",
            Self::ElectricRickshaw => "electric_rickshaw",
            Self::ElectricScooter => "electric_scooter",
            Self::ElectricalServices => "electrical_services",
            Self::Elevation => "elevation",
            Self::Elevator => "elevator",
            Self::Email => "email",
            Self::Emergency => "emergency",
            Self::EmergencyHeat => "emergency_heat",
            Self::EmergencyHeat2 => "emergency_heat_2",
            Self::EmergencyHome => "emergency_home",
            Self::EmergencyRecording => "emergency_recording",
            Self::EmergencyShare => "emergency_share",
            Self::EmergencyShareOff => "emergency_share_off",
            Self::EmojiEmotions => "emoji_emotions",
            Self::EmojiEvents => "emoji_events",
            Self::EmojiFlags => "emoji_flags",
            Self::EmojiFoodBeverage => "emoji_food_beverage",
            Self::EmojiLanguage => "emoji_language",
            Self::EmojiNature => "emoji_nature",
            Self::EmojiObjects => "emoji_objects",
            Self::EmojiPeople => "emoji_people",
            Self::EmojiSymbols => "emoji_symbols",
            Self::EmojiTransportation => "emoji_transportation",
            Self::Emoticon => "emoticon",
            Self::EmptyDashboard => "empty_dashboard",
            Self::Enable => "enable",
            Self::Encrypted => "encrypted",
            Self::EncryptedAdd => "encrypted_add",
            Self::EncryptedAddCircle => "encrypted_add_circle",
            Self::EncryptedMinusCircle => "encrypted_minus_circle",
            Self::EncryptedOff => "encrypted_off",
            Self::Endocrinology => "endocrinology",
            Self::Energy => "energy",
            Self::EnergyProgramSaving => "energy_program_saving",
            Self::EnergyProgramTimeUsed => "energy_program_time_used",
            Self::EnergySavingsLeaf => "energy_savings_leaf",
            Self::Engineering => "engineering",
            Self::EnhancedEncryption => "enhanced_encryption",
            Self::Ent => "ent",
            Self::Enterprise => "enterprise",
            Self::EnterpriseOff => "enterprise_off",
            Self::Equal => "equal",
            Self::Equalizer => "equalizer",
            Self::EraserSize1 => "eraser_size_1",
            Self::EraserSize2 => "eraser_size_2",
            Self::EraserSize3 => "eraser_size_3",
            Self::EraserSize4 => "eraser_size_4",
            Self::EraserSize5 => "eraser_size_5",
            Self::ErrorCircleRounded => "error_circle_rounded",
            Self::ErrorMed => "error_med",
            Self::ErrorOutline => "error_outline",
            Self::Escalator => "escalator",
            Self::EscalatorWarning => "escalator_warning",
            Self::Euro => "euro",
            Self::EuroSymbol => "euro_symbol",
            Self::EvCharger => "ev_charger",
            Self::EvMobiledataBadge => "ev_mobiledata_badge",
            Self::EvShadow => "ev_shadow",
            Self::EvShadowAdd => "ev_shadow_add",
            Self::EvShadowMinus => "ev_shadow_minus",
            Self::EvStation => "ev_station",
            Self::Event => "event",
            Self::EventAvailable => "event_available",
            Self::EventBusy => "event_busy",
            Self::EventList => "event_list",
            Self::EventNote => "event_note",
            Self::EventRepeat => "event_repeat",
            Self::EventSeat => "event_seat",
            Self::EventUpcoming => "event_upcoming",
            Self::Exclamation => "exclamation",
            Self::Exercise => "exercise",
            Self::ExitToApp => "exit_to_app",
            Self::Expand => "expand",
            Self::ExpandAll => "expand_all",
            Self::ExpandCircleDown => "expand_circle_down",
            Self::ExpandCircleRight => "expand_circle_right",
            Self::ExpandCircleUp => "expand_circle_up",
            Self::ExpandContent => "expand_content",
            Self::ExpandLess => "expand_less",
            Self::ExpandMore => "expand_more",
            Self::ExpansionPanels => "expansion_panels",
            Self::ExpensionPanels => "expension_panels",
            Self::Experiment => "experiment",
            Self::Explicit => "explicit",
            Self::Explore => "explore",
            Self::ExploreNearby => "explore_nearby",
            Self::ExploreOff => "explore_off",
            Self::Explosion => "explosion",
            Self::ExportNotes => "export_notes",
            Self::Exposure => "exposure",
            Self::ExposureNeg1 => "exposure_neg_1",
            Self::ExposureNeg2 => "exposure_neg_2",
            Self::ExposurePlus1 => "exposure_plus_1",
            Self::ExposurePlus2 => "exposure_plus_2",
            Self::ExposureZero => "exposure_zero",
            Self::Extension => "extension",
            Self::ExtensionOff => "extension_off",
            Self::EyeTracking => "eye_tracking",
            Self::Eyebrow => "eyebrow",
            Self::Eyeglasses => "eyeglasses",
            Self::Eyeglasses2 => "eyeglasses_2",
            Self::Eyeglasses2Sound => "eyeglasses_2_sound",
            Self::Eyeglasses3 => "eyeglasses_3",
            Self::Face => "face",
            Self::Face2 => "face_2",
            Self::Face3 => "face_3",
            Self::Face4 => "face_4",
            Self::Face5 => "face_5",
            Self::Face6 => "face_6",
            Self::FaceDown => "face_down",
            Self::FaceLeft => "face_left",
            Self::FaceNod => "face_nod",
            Self::FaceRetouchingNatural => "face_retouching_natural",
            Self::FaceRetouchingOff => "face_retouching_off",
            Self::FaceRight => "face_right",
            Self::FaceShake => "face_shake",
            Self::FaceUnlock => "face_unlock",
            Self::FaceUp => "face_up",
            Self::FactCheck => "fact_check",
            Self::Factory => "factory",
            Self::Falling => "falling",
            Self::FamiliarFaceAndZone => "familiar_face_and_zone",
            Self::FamilyGroup => "family_group",
            Self::FamilyHistory => "family_history",
            Self::FamilyHome => "family_home",
            Self::FamilyLink => "family_link",
            Self::FamilyRestroom => "family_restroom",
            Self::FamilyStar => "family_star",
            Self::FanFocus => "fan_focus",
            Self::FanIndirect => "fan_indirect",
            Self::FarsightDigital => "farsight_digital",
            Self::FastForward => "fast_forward",
            Self::FastRewind => "fast_rewind",
            Self::Fastfood => "fastfood",
            Self::Faucet => "faucet",
            Self::Favorite => "favorite",
            Self::FavoriteBorder => "favorite_border",
            Self::Fax => "fax",
            Self::FeatureSearch => "feature_search",
            Self::FeaturedPlayList => "featured_play_list",
            Self::FeaturedSeasonalAndGifts => "featured_seasonal_and_gifts",
            Self::FeaturedVideo => "featured_video",
            Self::Feed => "feed",
            Self::Feedback => "feedback",
            Self::Female => "female",
            Self::Femur => "femur",
            Self::FemurAlt => "femur_alt",
            Self::Fence => "fence",
            Self::Fertile => "fertile",
            Self::Festival => "festival",
            Self::FiberDvr => "fiber_dvr",
            Self::FiberManualRecord => "fiber_manual_record",
            Self::FiberNew => "fiber_new",
            Self::FiberPin => "fiber_pin",
            Self::FiberSmartRecord => "fiber_smart_record",
            Self::FileCopy => "file_copy",
            Self::FileCopyOff => "file_copy_off",
            Self::FileDownload => "file_download",
            Self::FileDownloadDone => "file_download_done",
            Self::FileDownloadOff => "file_download_off",
            Self::FileExport => "file_export",
            Self::FileJson => "file_json",
            Self::FileMap => "file_map",
            Self::FileMapStack => "file_map_stack",
            Self::FileOpen => "file_open",
            Self::FilePng => "file_png",
            Self::FilePresent => "file_present",
            Self::FileSave => "file_save",
            Self::FileSaveOff => "file_save_off",
            Self::FileUpload => "file_upload",
            Self::FileUploadOff => "file_upload_off",
            Self::Files => "files",
            Self::Filter1 => "filter_1",
            Self::Filter2 => "filter_2",
            Self::Filter3 => "filter_3",
            Self::Filter4 => "filter_4",
            Self::Filter5 => "filter_5",
            Self::Filter6 => "filter_6",
            Self::Filter7 => "filter_7",
            Self::Filter8 => "filter_8",
            Self::Filter9 => "filter_9",
            Self::Filter9Plus => "filter_9_plus",
            Self::FilterAlt => "filter_alt",
            Self::FilterAltOff => "filter_alt_off",
            Self::FilterArrowRight => "filter_arrow_right",
            Self::FilterBAndW => "filter_b_and_w",
            Self::FilterCancel => "filter_cancel",
            Self::FilterCenterFocus => "filter_center_focus",
            Self::FilterDrama => "filter_drama",
            Self::FilterFrames => "filter_frames",
            Self::FilterHdr => "filter_hdr",
            Self::FilterList => "filter_list",
            Self::FilterListAlt => "filter_list_alt",
            Self::FilterListOff => "filter_list_off",
            Self::FilterNone => "filter_none",
            Self::FilterPlus => "filter_plus",
            Self::FilterRetrolux => "filter_retrolux",
            Self::FilterTiltShift => "filter_tilt_shift",
            Self::FilterVintage => "filter_vintage",
            Self::Finance => "finance",
            Self::FinanceChip => "finance_chip",
            Self::FinanceMode => "finance_mode",
            Self::FindInPage => "find_in_page",
            Self::FindReplace => "find_replace",
            Self::Fingerprint => "fingerprint",
            Self::FingerprintOff => "fingerprint_off",
            Self::FireCheck => "fire_check",
            Self::FireExtinguisher => "fire_extinguisher",
            Self::FireHydrant => "fire_hydrant",
            Self::FireTruck => "fire_truck",
            Self::Fireplace => "fireplace",
            Self::FitPage => "fit_page",
            Self::FitPageHeight => "fit_page_height",
            Self::FitPageWidth => "fit_page_width",
            Self::FitScreen => "fit_screen",
            Self::FitWidth => "fit_width",
            Self::FitnessCenter => "fitness_center",
            Self::FitnessTracker => "fitness_tracker",
            Self::FitnessTrackers => "fitness_trackers",
            Self::Flag => "flag",
            Self::Flag2 => "flag_2",
            Self::FlagCheck => "flag_check",
            Self::FlagCircle => "flag_circle",
            Self::FlagFilled => "flag_filled",
            Self::Flaky => "flaky",
            Self::Flare => "flare",
            Self::FlashAuto => "flash_auto",
            Self::FlashOff => "flash_off",
            Self::FlashOn => "flash_on",
            Self::FlashlightOff => "flashlight_off",
            Self::FlashlightOn => "flashlight_on",
            Self::Flatware => "flatware",
            Self::FlexDirection => "flex_direction",
            Self::FlexNoWrap => "flex_no_wrap",
            Self::FlexWrap => "flex_wrap",
            Self::Flight => "flight",
            Self::FlightClass => "flight_class",
            Self::FlightLand => "flight_land",
            Self::FlightTakeoff => "flight_takeoff",
            Self::FlightsAndHotels => "flights_and_hotels",
            Self::Flightsmode => "flightsmode",
            Self::Flip => "flip",
            Self::FlipCameraAndroid => "flip_camera_android",
            Self::FlipCameraIos => "flip_camera_ios",
            Self::FlipToBack => "flip_to_back",
            Self::FlipToFront => "flip_to_front",
            Self::FloatLandscape2 => "float_landscape_2",
            Self::FloatPortrait2 => "float_portrait_2",
            Self::Flood => "flood",
            Self::Floor => "floor",
            Self::FloorLamp => "floor_lamp",
            Self::Flourescent => "flourescent",
            Self::Flowchart => "flowchart",
            Self::Flowsheet => "flowsheet",
            Self::Fluid => "fluid",
            Self::FluidBalance => "fluid_balance",
            Self::FluidMed => "fluid_med",
            Self::Fluorescent => "fluorescent",
            Self::Flutter => "flutter",
            Self::FlutterDash => "flutter_dash",
            Self::Flyover => "flyover",
            Self::FmdBad => "fmd_bad",
            Self::FmdGood => "fmd_good",
            Self::Foggy => "foggy",
            Self::FoldedHands => "folded_hands",
            Self::FolderCheck => "folder_check",
            Self::FolderCheck2 => "folder_check_2",
            Self::FolderCode => "folder_code",
            Self::FolderCopy => "folder_copy",
            Self::FolderData => "folder_data",
            Self::FolderDelete => "folder_delete",
            Self::FolderEye => "folder_eye",
            Self::FolderInfo => "folder_info",
            Self::FolderLimited => "folder_limited",
            Self::FolderManaged => "folder_managed",
            Self::FolderMatch => "folder_match",
            Self::FolderOff => "folder_off",
            Self::FolderShared => "folder_shared",
            Self::FolderSpecial => "folder_special",
            Self::FolderSupervised => "folder_supervised",
            Self::FolderZip => "folder_zip",
            Self::FollowTheSigns => "follow_the_signs",
            Self::FontDownload => "font_download",
            Self::FontDownloadOff => "font_download_off",
            Self::FoodBank => "food_bank",
            Self::FootBones => "foot_bones",
            Self::Footprint => "footprint",
            Self::ForYou => "for_you",
            Self::Forest => "forest",
            Self::ForkChart => "fork_chart",
            Self::ForkLeft => "fork_left",
            Self::ForkRight => "fork_right",
            Self::ForkSpoon => "fork_spoon",
            Self::Forklift => "forklift",
            Self::FormatAlignCenter => "format_align_center",
            Self::FormatAlignJustify => "format_align_justify",
            Self::FormatAlignLeft => "format_align_left",
            Self::FormatAlignRight => "format_align_right",
            Self::FormatBold => "format_bold",
            Self::FormatClear => "format_clear",
            Self::FormatColorFill => "format_color_fill",
            Self::FormatColorReset => "format_color_reset",
            Self::FormatColorText => "format_color_text",
            Self::FormatH1 => "format_h1",
            Self::FormatH2 => "format_h2",
            Self::FormatH3 => "format_h3",
            Self::FormatH4 => "format_h4",
            Self::FormatH5 => "format_h5",
            Self::FormatH6 => "format_h6",
            Self::FormatImageBack => "format_image_back",
            Self::FormatImageBreakLeft => "format_image_break_left",
            Self::FormatImageBreakRight => "format_image_break_right",
            Self::FormatImageFront => "format_image_front",
            Self::FormatImageInlineLeft => "format_image_inline_left",
            Self::FormatImageInlineRight => "format_image_inline_right",
            Self::FormatImageLeft => "format_image_left",
            Self::FormatImageRight => "format_image_right",
            Self::FormatIndentDecrease => "format_indent_decrease",
            Self::FormatIndentIncrease => "format_indent_increase",
            Self::FormatInkHighlighter => "format_ink_highlighter",
            Self::FormatItalic => "format_italic",
            Self::FormatLetterSpacing => "format_letter_spacing",
            Self::FormatLetterSpacing2 => "format_letter_spacing_2",
            Self::FormatLetterSpacingStandard => "format_letter_spacing_standard",
            Self::FormatLetterSpacingWide => "format_letter_spacing_wide",
            Self::FormatLetterSpacingWider => "format_letter_spacing_wider",
            Self::FormatLineSpacing => "format_line_spacing",
            Self::FormatListBulleted => "format_list_bulleted",
            Self::FormatListBulletedAdd => "format_list_bulleted_add",
            Self::FormatListNumbered => "format_list_numbered",
            Self::FormatListNumberedRtl => "format_list_numbered_rtl",
            Self::FormatOverline => "format_overline",
            Self::FormatPaint => "format_paint",
            Self::FormatPaintOff => "format_paint_off",
            Self::FormatParagraph => "format_paragraph",
            Self::FormatQuote => "format_quote",
            Self::FormatQuoteOff => "format_quote_off",
            Self::FormatShapes => "format_shapes",
            Self::FormatSize => "format_size",
            Self::FormatStrikethrough => "format_strikethrough",
            Self::FormatTextClip => "format_text_clip",
            Self::FormatTextOverflow => "format_text_overflow",
            Self::FormatTextWrap => "format_text_wrap",
            Self::FormatTextdirectionLToR => "format_textdirection_l_to_r",
            Self::FormatTextdirectionRToL => "format_textdirection_r_to_l",
            Self::FormatTextdirectionVertical => "format_textdirection_vertical",
            Self::FormatUnderlined => "format_underlined",
            Self::FormatUnderlinedSquiggle => "format_underlined_squiggle",
            Self::FormsAddOn => "forms_add_on",
            Self::FormsAppsScript => "forms_apps_script",
            Self::Fort => "fort",
            Self::Forum => "forum",
            Self::Forward => "forward",
            Self::Forward10 => "forward_10",
            Self::Forward30 => "forward_30",
            Self::Forward5 => "forward_5",
            Self::ForwardCircle => "forward_circle",
            Self::ForwardMedia => "forward_media",
            Self::ForwardToInbox => "forward_to_inbox",
            Self::Foundation => "foundation",
            Self::Fragrance => "fragrance",
            Self::FrameBug => "frame_bug",
            Self::FrameExclamation => "frame_exclamation",
            Self::FrameInspect => "frame_inspect",
            Self::FramePerson => "frame_person",
            Self::FramePersonMic => "frame_person_mic",
            Self::FramePersonOff => "frame_person_off",
            Self::FrameReload => "frame_reload",
            Self::FrameSource => "frame_source",
            Self::FreeBreakfast => "free_breakfast",
            Self::FreeCancellation => "free_cancellation",
            Self::FrontHand => "front_hand",
            Self::FrontLoader => "front_loader",
            Self::FullCoverage => "full_coverage",
            Self::FullHd => "full_hd",
            Self::FullStackedBarChart => "full_stacked_bar_chart",
            Self::FullscreenExit => "fullscreen_exit",
            Self::FullscreenPortrait => "fullscreen_portrait",
            Self::Function => "function",
            Self::FunctionSearch => "function_search",
            Self::Functions => "functions",
            Self::Funicular => "funicular",
            Self::GMobiledata => "g_mobiledata",
            Self::GMobiledataBadge => "g_mobiledata_badge",
            Self::GTranslate => "g_translate",
            Self::GalleryThumbnail => "gallery_thumbnail",
            Self::GameBumperLeft => "game_bumper_left",
            Self::GameBumperRight => "game_bumper_right",
            Self::GameButtonL => "game_button_l",
            Self::GameButtonL1 => "game_button_l1",
            Self::GameButtonL2 => "game_button_l2",
            Self::GameButtonR => "game_button_r",
            Self::GameButtonR1 => "game_button_r1",
            Self::GameButtonR2 => "game_button_r2",
            Self::GameButtonZl => "game_button_zl",
            Self::GameButtonZr => "game_button_zr",
            Self::GameStickL3 => "game_stick_l3",
            Self::GameStickLeft => "game_stick_left",
            Self::GameStickR3 => "game_stick_r3",
            Self::GameStickRight => "game_stick_right",
            Self::GameTriggerLeft => "game_trigger_left",
            Self::GameTriggerRight => "game_trigger_right",
            Self::Gamepad => "gamepad",
            Self::GamepadCircleDown => "gamepad_circle_down",
            Self::GamepadCircleLeft => "gamepad_circle_left",
            Self::GamepadCircleRight => "gamepad_circle_right",
            Self::GamepadCircleUp => "gamepad_circle_up",
            Self::GamepadDown => "gamepad_down",
            Self::GamepadLeft => "gamepad_left",
            Self::GamepadRight => "gamepad_right",
            Self::GamepadUp => "gamepad_up",
            Self::Games => "games",
            Self::Garage => "garage",
            Self::GarageCheck => "garage_check",
            Self::GarageDoor => "garage_door",
            Self::GarageDoorOpen => "garage_door_open",
            Self::GarageHome => "garage_home",
            Self::GarageMoney => "garage_money",
            Self::GardenCart => "garden_cart",
            Self::GasMeter => "gas_meter",
            Self::Gastroenterology => "gastroenterology",
            Self::Gate => "gate",
            Self::Gavel => "gavel",
            Self::GeneralDevice => "general_device",
            Self::GeneratingTokens => "generating_tokens",
            Self::Genetics => "genetics",
            Self::Genres => "genres",
            Self::Gesture => "gesture",
            Self::GestureSelect => "gesture_select",
            Self::GetApp => "get_app",
            Self::Gif => "gif",
            Self::Gif2 => "gif_2",
            Self::GifBox => "gif_box",
            Self::Girl => "girl",
            Self::Gite => "gite",
            Self::GlassCup => "glass_cup",
            Self::Globe => "globe",
            Self::Globe2Cancel => "globe_2_cancel",
            Self::Globe2Question => "globe_2_question",
            Self::GlobeAsia => "globe_asia",
            Self::GlobeBook => "globe_book",
            Self::GlobeClock => "globe_clock",
            Self::GlobeLocationPin => "globe_location_pin",
            Self::GlobeUk => "globe_uk",
            Self::Glucose => "glucose",
            Self::Glyphs => "glyphs",
            Self::GoToLine => "go_to_line",
            Self::GolfCourse => "golf_course",
            Self::GondolaLift => "gondola_lift",
            Self::GoogleHomeDevices => "google_home_devices",
            Self::GooglePlusReshare => "google_plus_reshare",
            Self::GoogleTvRemote => "google_tv_remote",
            Self::GoogleWifi => "google_wifi",
            Self::GppBad => "gpp_bad",
            Self::GppGood => "gpp_good",
            Self::GppMaybe => "gpp_maybe",
            Self::GpsFixed => "gps_fixed",
            Self::GpsNotFixed => "gps_not_fixed",
            Self::GpsOff => "gps_off",
            Self::Grade => "grade",
            Self::Gradient => "gradient",
            Self::Grading => "grading",
            Self::Grain => "grain",
            Self::Graph1 => "graph_1",
            Self::Graph2 => "graph_2",
            Self::Graph3 => "graph_3",
            Self::Graph4 => "graph_4",
            Self::Graph5 => "graph_5",
            Self::Graph6 => "graph_6",
            Self::Graph7 => "graph_7",
            Self::Graph8 => "graph_8",
            Self::GraphicEq => "graphic_eq",
            Self::GraphicEqOff => "graphic_eq_off",
            Self::Grass => "grass",
            Self::Grid3x3 => "grid_3x3",
            Self::Grid3x3Off => "grid_3x3_off",
            Self::Grid4x4 => "grid_4x4",
            Self::GridGoldenratio => "grid_goldenratio",
            Self::GridGuides => "grid_guides",
            Self::GridLayoutSide => "grid_layout_side",
            Self::GridOff => "grid_off",
            Self::GridOn => "grid_on",
            Self::GridView => "grid_view",
            Self::Grocery => "grocery",
            Self::Group => "group",
            Self::GroupAdd => "group_add",
            Self::GroupEye => "group_eye",
            Self::GroupOff => "group_off",
            Self::GroupRemove => "group_remove",
            Self::GroupSearch => "group_search",
            Self::GroupWork => "group_work",
            Self::GroupedBarChart => "grouped_bar_chart",
            Self::Groups => "groups",
            Self::Groups2 => "groups_2",
            Self::Groups3 => "groups_3",
            Self::Guardian => "guardian",
            Self::Gynecology => "gynecology",
            Self::HMobiledata => "h_mobiledata",
            Self::HMobiledataBadge => "h_mobiledata_badge",
            Self::HPlusMobiledata => "h_plus_mobiledata",
            Self::HPlusMobiledataBadge => "h_plus_mobiledata_badge",
            Self::Hail => "hail",
            Self::Hallway => "hallway",
            Self::HanamiDango => "hanami_dango",
            Self::HandBones => "hand_bones",
            Self::HandGesture => "hand_gesture",
            Self::HandGestureOff => "hand_gesture_off",
            Self::HandMeal => "hand_meal",
            Self::HandPackage => "hand_package",
            Self::HandheldController => "handheld_controller",
            Self::Handshake => "handshake",
            Self::HandwritingRecognition => "handwriting_recognition",
            Self::Handyman => "handyman",
            Self::HangoutVideo => "hangout_video",
            Self::HangoutVideoOff => "hangout_video_off",
            Self::HardDisk => "hard_disk",
            Self::HardDrive => "hard_drive",
            Self::HardDrive2 => "hard_drive_2",
            Self::Hardware => "hardware",
            Self::Hd => "hd",
            Self::HdrAuto => "hdr_auto",
            Self::HdrAutoSelect => "hdr_auto_select",
            Self::HdrEnhancedSelect => "hdr_enhanced_select",
            Self::HdrOff => "hdr_off",
            Self::HdrOffSelect => "hdr_off_select",
            Self::HdrOn => "hdr_on",
            Self::HdrOnSelect => "hdr_on_select",
            Self::HdrPlus => "hdr_plus",
            Self::HdrPlusOff => "hdr_plus_off",
            Self::HdrStrong => "hdr_strong",
            Self::HdrWeak => "hdr_weak",
            Self::HeadMountedDevice => "head_mounted_device",
            Self::Headphones => "headphones",
            Self::HeadphonesBattery => "headphones_battery",
            Self::Headset => "headset",
            Self::HeadsetMic => "headset_mic",
            Self::HeadsetOff => "headset_off",
            Self::Healing => "healing",
            Self::HealthAndBeauty => "health_and_beauty",
            Self::HealthAndSafety => "health_and_safety",
            Self::HealthCross => "health_cross",
            Self::HealthMetrics => "health_metrics",
            Self::HeapSnapshotLarge => "heap_snapshot_large",
            Self::HeapSnapshotMultiple => "heap_snapshot_multiple",
            Self::HeapSnapshotThumbnail => "heap_snapshot_thumbnail",
            Self::Hearing => "hearing",
            Self::HearingAid => "hearing_aid",
            Self::HearingAidDisabled => "hearing_aid_disabled",
            Self::HearingAidDisabledLeft => "hearing_aid_disabled_left",
            Self::HearingAidLeft => "hearing_aid_left",
            Self::HearingDisabled => "hearing_disabled",
            Self::HeartBroken => "heart_broken",
            Self::HeartCheck => "heart_check",
            Self::HeartMinus => "heart_minus",
            Self::HeartPlus => "heart_plus",
            Self::HeartSmile => "heart_smile",
            Self::Heat => "heat",
            Self::HeatPump => "heat_pump",
            Self::HeatPumpBalance => "heat_pump_balance",
            Self::Height => "height",
            Self::Helicopter => "helicopter",
            Self::HelpCenter => "help_center",
            Self::HelpClinic => "help_clinic",
            Self::HelpOutline => "help_outline",
            Self::Hematology => "hematology",
            Self::Hevc => "hevc",
            Self::Hexagon => "hexagon",
            Self::Hide => "hide",
            Self::HideImage => "hide_image",
            Self::HideSource => "hide_source",
            Self::HighChair => "high_chair",
            Self::HighDensity => "high_density",
            Self::HighQuality => "high_quality",
            Self::HighQualityOff => "high_quality_off",
            Self::HighRes => "high_res",
            Self::Highlight => "highlight",
            Self::HighlightAlt => "highlight_alt",
            Self::HighlightKeyboardFocus => "highlight_keyboard_focus",
            Self::HighlightMouseCursor => "highlight_mouse_cursor",
            Self::HighlightOff => "highlight_off",
            Self::HighlightTextCursor => "highlight_text_cursor",
            Self::HighlighterSize1 => "highlighter_size_1",
            Self::HighlighterSize2 => "highlighter_size_2",
            Self::HighlighterSize3 => "highlighter_size_3",
            Self::HighlighterSize4 => "highlighter_size_4",
            Self::HighlighterSize5 => "highlighter_size_5",
            Self::Hiking => "hiking",
            Self::History => "history",
            Self::History2 => "history_2",
            Self::HistoryEdu => "history_edu",
            Self::HistoryOff => "history_off",
            Self::HistoryToggleOff => "history_toggle_off",
            Self::Hive => "hive",
            Self::Hls => "hls",
            Self::HlsOff => "hls_off",
            Self::HolidayVillage => "holiday_village",
            Self::HomeAndGarden => "home_and_garden",
            Self::HomeAppLogo => "home_app_logo",
            Self::HomeFilled => "home_filled",
            Self::HomeHealth => "home_health",
            Self::HomeImprovementAndTools => "home_improvement_and_tools",
            Self::HomeIotDevice => "home_iot_device",
            Self::HomeMax => "home_max",
            Self::HomeMaxDots => "home_max_dots",
            Self::HomeMini => "home_mini",
            Self::HomePin => "home_pin",
            Self::HomeRepairService => "home_repair_service",
            Self::HomeSpeaker => "home_speaker",
            Self::HomeStorage => "home_storage",
            Self::HomeStorageGear => "home_storage_gear",
            Self::HomeWork => "home_work",
            Self::HorizontalAlignCenter => "horizontal_align_center",
            Self::HorizontalAlignLeft => "horizontal_align_left",
            Self::HorizontalAlignRight => "horizontal_align_right",
            Self::HorizontalDistribute => "horizontal_distribute",
            Self::HorizontalRule => "horizontal_rule",
            Self::HorizontalSplit => "horizontal_split",
            Self::Host => "host",
            Self::HotTub => "hot_tub",
            Self::Hotel => "hotel",
            Self::HotelClass => "hotel_class",
            Self::HourglassArrowDown => "hourglass_arrow_down",
            Self::HourglassArrowUp => "hourglass_arrow_up",
            Self::HourglassBottom => "hourglass_bottom",
            Self::HourglassCheck => "hourglass_check",
            Self::HourglassDisabled => "hourglass_disabled",
            Self::HourglassEmpty => "hourglass_empty",
            Self::HourglassFull => "hourglass_full",
            Self::HourglassPause => "hourglass_pause",
            Self::HourglassTop => "hourglass_top",
            Self::House => "house",
            Self::HouseSiding => "house_siding",
            Self::HouseWithShield => "house_with_shield",
            Self::Houseboat => "houseboat",
            Self::HouseholdSupplies => "household_supplies",
            Self::Hov => "hov",
            Self::HowToReg => "how_to_reg",
            Self::HowToVote => "how_to_vote",
            Self::HrResting => "hr_resting",
            Self::Html => "html",
            Self::Http => "http",
            Self::Https => "https",
            Self::Hub => "hub",
            Self::Humerus => "humerus",
            Self::HumerusAlt => "humerus_alt",
            Self::HumidityHigh => "humidity_high",
            Self::HumidityIndoor => "humidity_indoor",
            Self::HumidityLow => "humidity_low",
            Self::HumidityMid => "humidity_mid",
            Self::HumidityPercentage => "humidity_percentage",
            Self::Hvac => "hvac",
            Self::HvacMaxDefrost => "hvac_max_defrost",
            Self::IceSkating => "ice_skating",
            Self::Icecream => "icecream",
            Self::IdCard => "id_card",
            Self::IdCard2 => "id_card_2",
            Self::IdentityAwareProxy => "identity_aware_proxy",
            Self::IdentityPlatform => "identity_platform",
            Self::Ifl => "ifl",
            Self::Iframe => "iframe",
            Self::IframeOff => "iframe_off",
            Self::Image => "image",
            Self::ImageArrowUp => "image_arrow_up",
            Self::ImageAspectRatio => "image_aspect_ratio",
            Self::ImageInset => "image_inset",
            Self::ImageNotSupported => "image_not_supported",
            Self::ImageSearch => "image_search",
            Self::ImagesearchRoller => "imagesearch_roller",
            Self::Imagesmode => "imagesmode",
            Self::Immunology => "immunology",
            Self::ImportContacts => "import_contacts",
            Self::ImportExport => "import_export",
            Self::ImportSpark => "import_spark",
            Self::ImportantDevices => "important_devices",
            Self::InHomeMode => "in_home_mode",
            Self::InactiveOrder => "inactive_order",
            Self::Inbox => "inbox",
            Self::InboxCustomize => "inbox_customize",
            Self::InboxText => "inbox_text",
            Self::InboxTextAsterisk => "inbox_text_asterisk",
            Self::InboxTextPerson => "inbox_text_person",
            Self::InboxTextShare => "inbox_text_share",
            Self::IncompleteCircle => "incomplete_circle",
            Self::IndeterminateCheckBox => "indeterminate_check_box",
            Self::IndeterminateQuestionBox => "indeterminate_question_box",
            Self::InfoI => "info_i",
            Self::Infrared => "infrared",
            Self::InkEraser => "ink_eraser",
            Self::InkEraserOff => "ink_eraser_off",
            Self::InkHighlighter => "ink_highlighter",
            Self::InkHighlighterMove => "ink_highlighter_move",
            Self::InkHighlighterOff => "ink_highlighter_off",
            Self::InkMarker => "ink_marker",
            Self::InkPen => "ink_pen",
            Self::InkSelection => "ink_selection",
            Self::Inpatient => "inpatient",
            Self::Input => "input",
            Self::InputCircle => "input_circle",
            Self::InsertChart => "insert_chart",
            Self::InsertChartFilled => "insert_chart_filled",
            Self::InsertChartOutlined => "insert_chart_outlined",
            Self::InsertComment => "insert_comment",
            Self::InsertDriveFile => "insert_drive_file",
            Self::InsertEmoticon => "insert_emoticon",
            Self::InsertInvitation => "insert_invitation",
            Self::InsertLink => "insert_link",
            Self::InsertPageBreak => "insert_page_break",
            Self::InsertPhoto => "insert_photo",
            Self::InsertText => "insert_text",
            Self::Insights => "insights",
            Self::InstallDesktop => "install_desktop",
            Self::InstallMobile => "install_mobile",
            Self::InstantMix => "instant_mix",
            Self::IntegrationInstructions => "integration_instructions",
            Self::InteractiveSpace => "interactive_space",
            Self::Interests => "interests",
            Self::InterpreterMode => "interpreter_mode",
            Self::Inventory => "inventory",
            Self::Inventory2 => "inventory_2",
            Self::InvertColors => "invert_colors",
            Self::InvertColorsOff => "invert_colors_off",
            Self::Ios => "ios",
            Self::IosShare => "ios_share",
            Self::Iron => "iron",
            Self::Iso => "iso",
            Self::JamboardKiosk => "jamboard_kiosk",
            Self::JapaneseCurry => "japanese_curry",
            Self::JapaneseFlag => "japanese_flag",
            Self::Javascript => "javascript",
            Self::Jewelry => "jewelry",
            Self::Join => "join",
            Self::JoinFull => "join_full",
            Self::JoinInner => "join_inner",
            Self::JoinLeft => "join_left",
            Self::JoinRight => "join_right",
            Self::Joystick => "joystick",
            Self::JumpToElement => "jump_to_element",
            Self::KanjiAlcohol => "kanji_alcohol",
            Self::Kayaking => "kayaking",
            Self::KebabDining => "kebab_dining",
            Self::Keep => "keep",
            Self::KeepOff => "keep_off",
            Self::KeepPin => "keep_pin",
            Self::KeepPublic => "keep_public",
            Self::Kettle => "kettle",
            Self::Key => "key",
            Self::KeyOff => "key_off",
            Self::KeyVertical => "key_vertical",
            Self::KeyVisualizer => "key_visualizer",
            Self::Keyboard => "keyboard",
            Self::KeyboardAlt => "keyboard_alt",
            Self::KeyboardArrowDown => "keyboard_arrow_down",
            Self::KeyboardArrowLeft => "keyboard_arrow_left",
            Self::KeyboardArrowRight => "keyboard_arrow_right",
            Self::KeyboardArrowUp => "keyboard_arrow_up",
            Self::KeyboardBackspace => "keyboard_backspace",
            Self::KeyboardCapslock => "keyboard_capslock",
            Self::KeyboardCapslockBadge => "keyboard_capslock_badge",
            Self::KeyboardCommandKey => "keyboard_command_key",
            Self::KeyboardControlKey => "keyboard_control_key",
            Self::KeyboardDoubleArrowDown => "keyboard_double_arrow_down",
            Self::KeyboardDoubleArrowLeft => "keyboard_double_arrow_left",
            Self::KeyboardDoubleArrowRight => "keyboard_double_arrow_right",
            Self::KeyboardDoubleArrowUp => "keyboard_double_arrow_up",
            Self::KeyboardExternalInput => "keyboard_external_input",
            Self::KeyboardFull => "keyboard_full",
            Self::KeyboardHide => "keyboard_hide",
            Self::KeyboardKeys => "keyboard_keys",
            Self::KeyboardLock => "keyboard_lock",
            Self::KeyboardLockOff => "keyboard_lock_off",
            Self::KeyboardOff => "keyboard_off",
            Self::KeyboardOnscreen => "keyboard_onscreen",
            Self::KeyboardOptionKey => "keyboard_option_key",
            Self::KeyboardPreviousLanguage => "keyboard_previous_language",
            Self::KeyboardReturn => "keyboard_return",
            Self::KeyboardTab => "keyboard_tab",
            Self::KeyboardTabRtl => "keyboard_tab_rtl",
            Self::KeyboardVoice => "keyboard_voice",
            Self::KidStar => "kid_star",
            Self::KingBed => "king_bed",
            Self::Kitchen => "kitchen",
            Self::Kitesurfing => "kitesurfing",
            Self::LabPanel => "lab_panel",
            Self::LabProfile => "lab_profile",
            Self::LabResearch => "lab_research",
            Self::Label => "label",
            Self::LabelImportant => "label_important",
            Self::LabelImportantOutline => "label_important_outline",
            Self::LabelOff => "label_off",
            Self::LabelOutline => "label_outline",
            Self::Labs => "labs",
            Self::Lan => "lan",
            Self::Landscape => "landscape",
            Self::Landscape2 => "landscape_2",
            Self::Landscape2Edit => "landscape_2_edit",
            Self::Landscape2Off => "landscape_2_off",
            Self::Landslide => "landslide",
            Self::Language => "language",
            Self::LanguageChineseArray => "language_chinese_array",
            Self::LanguageChineseCangjie => "language_chinese_cangjie",
            Self::LanguageChineseDayi => "language_chinese_dayi",
            Self::LanguageChinesePinyin => "language_chinese_pinyin",
            Self::LanguageChineseQuick => "language_chinese_quick",
            Self::LanguageChineseWubi => "language_chinese_wubi",
            Self::LanguageFrench => "language_french",
            Self::LanguageGbEnglish => "language_gb_english",
            Self::LanguageInternational => "language_international",
            Self::LanguageJapaneseKana => "language_japanese_kana",
            Self::LanguageKoreanLatin => "language_korean_latin",
            Self::LanguagePinyin => "language_pinyin",
            Self::LanguageSpanish => "language_spanish",
            Self::LanguageUs => "language_us",
            Self::LanguageUsColemak => "language_us_colemak",
            Self::LanguageUsDvorak => "language_us_dvorak",
            Self::Laps => "laps",
            Self::Laptop => "laptop",
            Self::LaptopCar => "laptop_car",
            Self::LaptopChromebook => "laptop_chromebook",
            Self::LaptopMac => "laptop_mac",
            Self::LaptopWindows => "laptop_windows",
            Self::LassoSelect => "lasso_select",
            Self::Launch => "launch",
            Self::Laundry => "laundry",
            Self::Layers => "layers",
            Self::LayersClear => "layers_clear",
            Self::Lda => "lda",
            Self::Leaderboard => "leaderboard",
            Self::LeakAdd => "leak_add",
            Self::LeakRemove => "leak_remove",
            Self::LeftClick => "left_click",
            Self::LeftPanelClose => "left_panel_close",
            Self::LeftPanelOpen => "left_panel_open",
            Self::LegendToggle => "legend_toggle",
            Self::Lens => "lens",
            Self::LensBlur => "lens_blur",
            Self::LetterSwitch => "letter_switch",
            Self::LibraryAdd => "library_add",
            Self::LibraryAddCheck => "library_add_check",
            Self::LibraryBooks => "library_books",
            Self::LibraryMusic => "library_music",
            Self::License => "license",
            Self::LiftToTalk => "lift_to_talk",
            Self::Light => "light",
            Self::LightGroup => "light_group",
            Self::LightGroup2 => "light_group_2",
            Self::LightMode => "light_mode",
            Self::LightModeAuto => "light_mode_auto",
            Self::LightOff => "light_off",
            Self::Lightbulb => "lightbulb",
            Self::Lightbulb2 => "lightbulb_2",
            Self::LightbulbCircle => "lightbulb_circle",
            Self::LightbulbOutline => "lightbulb_outline",
            Self::LightningStand => "lightning_stand",
            Self::Lightstrip => "lightstrip",
            Self::LineAxis => "line_axis",
            Self::LineCurve => "line_curve",
            Self::LineEnd => "line_end",
            Self::LineEndArrow => "line_end_arrow",
            Self::LineEndArrowNotch => "line_end_arrow_notch",
            Self::LineEndCircle => "line_end_circle",
            Self::LineEndDiamond => "line_end_diamond",
            Self::LineEndSquare => "line_end_square",
            Self::LineStart => "line_start",
            Self::LineStartArrow => "line_start_arrow",
            Self::LineStartArrowNotch => "line_start_arrow_notch",
            Self::LineStartCircle => "line_start_circle",
            Self::LineStartDiamond => "line_start_diamond",
            Self::LineStartSquare => "line_start_square",
            Self::LineStyle => "line_style",
            Self::LineWeight => "line_weight",
            Self::LinearScale => "linear_scale",
            Self::Link2 => "link_2",
            Self::LinkOff => "link_off",
            Self::LinkedCamera => "linked_camera",
            Self::LinkedServices => "linked_services",
            Self::Lips => "lips",
            Self::Liquor => "liquor",
            Self::List => "list",
            Self::List2 => "list_2",
            Self::ListAlt => "list_alt",
            Self::ListAltAdd => "list_alt_add",
            Self::ListAltCheck => "list_alt_check",
            Self::ListArrow => "list_arrow",
            Self::Lists => "lists",
            Self::LiveHelp => "live_help",
            Self::LiveTv => "live_tv",
            Self::Living => "living",
            Self::LocalActivity => "local_activity",
            Self::LocalAirport => "local_airport",
            Self::LocalAtm => "local_atm",
            Self::LocalBar => "local_bar",
            Self::LocalCafe => "local_cafe",
            Self::LocalCarWash => "local_car_wash",
            Self::LocalConvenienceStore => "local_convenience_store",
            Self::LocalDining => "local_dining",
            Self::LocalDrink => "local_drink",
            Self::LocalFireDepartment => "local_fire_department",
            Self::LocalFlorist => "local_florist",
            Self::LocalGasStation => "local_gas_station",
            Self::LocalGroceryStore => "local_grocery_store",
            Self::LocalHospital => "local_hospital",
            Self::LocalHotel => "local_hotel",
            Self::LocalLaundryService => "local_laundry_service",
            Self::LocalLibrary => "local_library",
            Self::LocalMall => "local_mall",
            Self::LocalMovies => "local_movies",
            Self::LocalOffer => "local_offer",
            Self::LocalParking => "local_parking",
            Self::LocalPharmacy => "local_pharmacy",
            Self::LocalPhone => "local_phone",
            Self::LocalPizza => "local_pizza",
            Self::LocalPlay => "local_play",
            Self::LocalPolice => "local_police",
            Self::LocalPostOffice => "local_post_office",
            Self::LocalPrintshop => "local_printshop",
            Self::LocalSee => "local_see",
            Self::LocalShipping => "local_shipping",
            Self::LocalTaxi => "local_taxi",
            Self::LocationAutomation => "location_automation",
            Self::LocationAway => "location_away",
            Self::LocationChip => "location_chip",
            Self::LocationCity => "location_city",
            Self::LocationDisabled => "location_disabled",
            Self::LocationHome => "location_home",
            Self::LocationOff => "location_off",
            Self::LocationOn => "location_on",
            Self::LocationPin => "location_pin",
            Self::LocationSearching => "location_searching",
            Self::LocatorTag => "locator_tag",
            Self::LockClock => "lock_clock",
            Self::LockOpen => "lock_open",
            Self::LockOpenCircle => "lock_open_circle",
            Self::LockOpenRight => "lock_open_right",
            Self::LockOutline => "lock_outline",
            Self::LockPerson => "lock_person",
            Self::LockReset => "lock_reset",
            Self::Login => "login",
            Self::LogoDev => "logo_dev",
            Self::Logout => "logout",
            Self::Looks => "looks",
            Self::Looks3 => "looks_3",
            Self::Looks4 => "looks_4",
            Self::Looks5 => "looks_5",
            Self::Looks6 => "looks_6",
            Self::LooksOne => "looks_one",
            Self::LooksTwo => "looks_two",
            Self::Loop => "loop",
            Self::Loupe => "loupe",
            Self::LowDensity => "low_density",
            Self::LowPriority => "low_priority",
            Self::Lowercase => "lowercase",
            Self::Loyalty => "loyalty",
            Self::LteMobiledata => "lte_mobiledata",
            Self::LteMobiledataBadge => "lte_mobiledata_badge",
            Self::LtePlusMobiledata => "lte_plus_mobiledata",
            Self::LtePlusMobiledataBadge => "lte_plus_mobiledata_badge",
            Self::Luggage => "luggage",
            Self::LunchDining => "lunch_dining",
            Self::Lyrics => "lyrics",
            Self::MacroAuto => "macro_auto",
            Self::MacroOff => "macro_off",
            Self::MagicButton => "magic_button",
            Self::MagicExchange => "magic_exchange",
            Self::MagicTether => "magic_tether",
            Self::MagnificationLarge => "magnification_large",
            Self::MagnificationSmall => "magnification_small",
            Self::MagnifyDocked => "magnify_docked",
            Self::MagnifyFullscreen => "magnify_fullscreen",
            Self::MailAsterisk => "mail_asterisk",
            Self::MailLock => "mail_lock",
            Self::MailOff => "mail_off",
            Self::MailOutline => "mail_outline",
            Self::MailShield => "mail_shield",
            Self::Male => "male",
            Self::Man => "man",
            Self::Man2 => "man_2",
            Self::Man3 => "man_3",
            Self::Man4 => "man_4",
            Self::ManageAccounts => "manage_accounts",
            Self::ManageHistory => "manage_history",
            Self::ManageSearch => "manage_search",
            Self::Manga => "manga",
            Self::Manufacturing => "manufacturing",
            Self::Map => "map",
            Self::MapPinHeart => "map_pin_heart",
            Self::MapPinReview => "map_pin_review",
            Self::MapSearch => "map_search",
            Self::MapsHomeWork => "maps_home_work",
            Self::MapsUgc => "maps_ugc",
            Self::Margin => "margin",
            Self::MarkAsUnread => "mark_as_unread",
            Self::MarkChatRead => "mark_chat_read",
            Self::MarkChatUnread => "mark_chat_unread",
            Self::MarkEmailRead => "mark_email_read",
            Self::MarkEmailUnread => "mark_email_unread",
            Self::MarkUnreadChatAlt => "mark_unread_chat_alt",
            Self::Markdown => "markdown",
            Self::MarkdownConvert => "markdown_convert",
            Self::MarkdownCopy => "markdown_copy",
            Self::MarkdownDocument => "markdown_document",
            Self::MarkdownPaste => "markdown_paste",
            Self::MarkdownSpark => "markdown_spark",
            Self::Markunread => "markunread",
            Self::MarkunreadMailbox => "markunread_mailbox",
            Self::MaskedTransitions => "masked_transitions",
            Self::MaskedTransitionsAdd => "masked_transitions_add",
            Self::Masks => "masks",
            Self::Massage => "massage",
            Self::MatchCase => "match_case",
            Self::MatchCaseOff => "match_case_off",
            Self::MatchWord => "match_word",
            Self::Matter => "matter",
            Self::Maximize => "maximize",
            Self::MealDinner => "meal_dinner",
            Self::MealLunch => "meal_lunch",
            Self::MeasuringTape => "measuring_tape",
            Self::MediaBluetoothOff => "media_bluetooth_off",
            Self::MediaBluetoothOn => "media_bluetooth_on",
            Self::MediaLink => "media_link",
            Self::MediaOutput => "media_output",
            Self::MediaOutputOff => "media_output_off",
            Self::Mediation => "mediation",
            Self::MedicalInformation => "medical_information",
            Self::MedicalMask => "medical_mask",
            Self::MedicalServices => "medical_services",
            Self::Medication => "medication",
            Self::MedicationLiquid => "medication_liquid",
            Self::MeetingRoom => "meeting_room",
            Self::Memory => "memory",
            Self::MemoryAlt => "memory_alt",
            Self::MenstrualHealth => "menstrual_health",
            Self::MenuBook => "menu_book",
            Self::MenuBook2 => "menu_book_2",
            Self::MenuOpen => "menu_open",
            Self::Merge => "merge",
            Self::MergeType => "merge_type",
            Self::Message => "message",
            Self::Metabolism => "metabolism",
            Self::Metro => "metro",
            Self::MfgNestYaleLock => "mfg_nest_yale_lock",
            Self::Mic => "mic",
            Self::MicAlert => "mic_alert",
            Self::MicDouble => "mic_double",
            Self::MicExternalOff => "mic_external_off",
            Self::MicExternalOn => "mic_external_on",
            Self::MicGear => "mic_gear",
            Self::MicNone => "mic_none",
            Self::MicOff => "mic_off",
            Self::Microbiology => "microbiology",
            Self::Microwave => "microwave",
            Self::MicrowaveGen => "microwave_gen",
            Self::MilitaryTech => "military_tech",
            Self::Mimo => "mimo",
            Self::MimoDisconnect => "mimo_disconnect",
            Self::Mindfulness => "mindfulness",
            Self::Minimize => "minimize",
            Self::MinorCrash => "minor_crash",
            Self::Mintmark => "mintmark",
            Self::MissedVideoCall => "missed_video_call",
            Self::MissedVideoCallFilled => "missed_video_call_filled",
            Self::MissingController => "missing_controller",
            Self::Mist => "mist",
            Self::Mitre => "mitre",
            Self::MixtureMed => "mixture_med",
            Self::Mms => "mms",
            Self::Mobile => "mobile",
            Self::Mobile2 => "mobile_2",
            Self::Mobile3 => "mobile_3",
            Self::MobileAlert => "mobile_alert",
            Self::MobileArrowDown => "mobile_arrow_down",
            Self::MobileArrowRight => "mobile_arrow_right",
            Self::MobileArrowUpRight => "mobile_arrow_up_right",
            Self::MobileBlock => "mobile_block",
            Self::MobileCamera => "mobile_camera",
            Self::MobileCameraFront => "mobile_camera_front",
            Self::MobileCameraRear => "mobile_camera_rear",
            Self::MobileCancel => "mobile_cancel",
            Self::MobileCast => "mobile_cast",
            Self::MobileCharge => "mobile_charge",
            Self::MobileChat => "mobile_chat",
            Self::MobileCheck => "mobile_check",
            Self::MobileCode => "mobile_code",
            Self::MobileDock => "mobile_dock",
            Self::MobileDots => "mobile_dots",
            Self::MobileFriendly => "mobile_friendly",
            Self::MobileGear => "mobile_gear",
            Self::MobileHand => "mobile_hand",
            Self::MobileHandLeft => "mobile_hand_left",
            Self::MobileHandLeftOff => "mobile_hand_left_off",
            Self::MobileHandOff => "mobile_hand_off",
            Self::MobileInfo => "mobile_info",
            Self::MobileLandscape => "mobile_landscape",
            Self::MobileLayout => "mobile_layout",
            Self::MobileLockLandscape => "mobile_lock_landscape",
            Self::MobileLockPortrait => "mobile_lock_portrait",
            Self::MobileLoupe => "mobile_loupe",
            Self::MobileMenu => "mobile_menu",
            Self::MobileOff => "mobile_off",
            Self::MobileQuestion => "mobile_question",
            Self::MobileRotate => "mobile_rotate",
            Self::MobileRotateLock => "mobile_rotate_lock",
            Self::MobileScreenShare => "mobile_screen_share",
            Self::MobileScreensaver => "mobile_screensaver",
            Self::MobileSensorHi => "mobile_sensor_hi",
            Self::MobileSensorLo => "mobile_sensor_lo",
            Self::MobileShare => "mobile_share",
            Self::MobileShareStack => "mobile_share_stack",
            Self::MobileSound => "mobile_sound",
            Self::MobileSound2 => "mobile_sound_2",
            Self::MobileSoundOff => "mobile_sound_off",
            Self::MobileSpeaker => "mobile_speaker",
            Self::MobileTap => "mobile_tap",
            Self::MobileText => "mobile_text",
            Self::MobileText2 => "mobile_text_2",
            Self::MobileTheft => "mobile_theft",
            Self::MobileTicket => "mobile_ticket",
            Self::MobileUnlock => "mobile_unlock",
            Self::MobileVibrate => "mobile_vibrate",
            Self::MobileWrench => "mobile_wrench",
            Self::MobiledataArrows => "mobiledata_arrows",
            Self::MobiledataOff => "mobiledata_off",
            Self::Mode => "mode",
            Self::ModeComment => "mode_comment",
            Self::ModeCool => "mode_cool",
            Self::ModeCoolOff => "mode_cool_off",
            Self::ModeDual => "mode_dual",
            Self::ModeEdit => "mode_edit",
            Self::ModeEditOutline => "mode_edit_outline",
            Self::ModeFan => "mode_fan",
            Self::ModeFan2 => "mode_fan_2",
            Self::ModeFanOff => "mode_fan_off",
            Self::ModeHeat => "mode_heat",
            Self::ModeHeatCool => "mode_heat_cool",
            Self::ModeHeatOff => "mode_heat_off",
            Self::ModeNight => "mode_night",
            Self::ModeOfTravel => "mode_of_travel",
            Self::ModeOffOn => "mode_off_on",
            Self::ModeStandby => "mode_standby",
            Self::ModelTraining => "model_training",
            Self::Modeling => "modeling",
            Self::MonetizationOn => "monetization_on",
            Self::Money => "money",
            Self::MoneyBag => "money_bag",
            Self::MoneyOff => "money_off",
            Self::MoneyOffCsred => "money_off_csred",
            Self::MoneyRange => "money_range",
            Self::Monitor => "monitor",
            Self::MonitorHeart => "monitor_heart",
            Self::MonitorWeight => "monitor_weight",
            Self::MonitorWeightGain => "monitor_weight_gain",
            Self::MonitorWeightLoss => "monitor_weight_loss",
            Self::Monitoring => "monitoring",
            Self::MonochromePhotos => "monochrome_photos",
            Self::Monorail => "monorail",
            Self::Mood => "mood",
            Self::MoodBad => "mood_bad",
            Self::MoodHeart => "mood_heart",
            Self::MoonStars => "moon_stars",
            Self::Mop => "mop",
            Self::Moped => "moped",
            Self::MopedPackage => "moped_package",
            Self::MoreDown => "more_down",
            Self::MoreHoriz => "more_horiz",
            Self::MoreTime => "more_time",
            Self::MoreUp => "more_up",
            Self::MoreVert => "more_vert",
            Self::Mosque => "mosque",
            Self::MotionBlur => "motion_blur",
            Self::MotionMode => "motion_mode",
            Self::MotionPhotosAuto => "motion_photos_auto",
            Self::MotionPhotosOff => "motion_photos_off",
            Self::MotionPhotosOn => "motion_photos_on",
            Self::MotionPhotosPause => "motion_photos_pause",
            Self::MotionPhotosPaused => "motion_photos_paused",
            Self::MotionPlay => "motion_play",
            Self::MotionSensorActive => "motion_sensor_active",
            Self::MotionSensorAlert => "motion_sensor_alert",
            Self::MotionSensorIdle => "motion_sensor_idle",
            Self::MotionSensorUrgent => "motion_sensor_urgent",
            Self::Motorcycle => "motorcycle",
            Self::MountainFlag => "mountain_flag",
            Self::MountainSteam => "mountain_steam",
            Self::Mouse => "mouse",
            Self::MouseLock => "mouse_lock",
            Self::MouseLockOff => "mouse_lock_off",
            Self::Move => "move",
            Self::MoveDown => "move_down",
            Self::MoveGroup => "move_group",
            Self::MoveItem => "move_item",
            Self::MoveLocation => "move_location",
            Self::MoveSelectionDown => "move_selection_down",
            Self::MoveSelectionLeft => "move_selection_left",
            Self::MoveSelectionRight => "move_selection_right",
            Self::MoveSelectionUp => "move_selection_up",
            Self::MoveToInbox => "move_to_inbox",
            Self::MoveUp => "move_up",
            Self::MovedLocation => "moved_location",
            Self::Movie => "movie",
            Self::MovieCreation => "movie_creation",
            Self::MovieEdit => "movie_edit",
            Self::MovieEditOff => "movie_edit_off",
            Self::MovieFilter => "movie_filter",
            Self::MovieInfo => "movie_info",
            Self::MovieOff => "movie_off",
            Self::MovieSpeaker => "movie_speaker",
            Self::Moving => "moving",
            Self::MovingBeds => "moving_beds",
            Self::MovingMinistry => "moving_ministry",
            Self::Mp => "mp",
            Self::Multicooker => "multicooker",
            Self::MultilineChart => "multiline_chart",
            Self::MultimodalHandEye => "multimodal_hand_eye",
            Self::MultipleAirports => "multiple_airports",
            Self::MultipleStop => "multiple_stop",
            Self::Museum => "museum",
            Self::MusicCast => "music_cast",
            Self::MusicHistory => "music_history",
            Self::MusicNote => "music_note",
            Self::MusicNote2 => "music_note_2",
            Self::MusicNoteAdd => "music_note_add",
            Self::MusicOff => "music_off",
            Self::MusicVideo => "music_video",
            Self::MyLocation => "my_location",
            Self::Mystery => "mystery",
            Self::Nat => "nat",
            Self::Nature => "nature",
            Self::NaturePeople => "nature_people",
            Self::NavigateBefore => "navigate_before",
            Self::NavigateNext => "navigate_next",
            Self::Navigation => "navigation",
            Self::NearMe => "near_me",
            Self::NearMeDisabled => "near_me_disabled",
            Self::Nearby => "nearby",
            Self::NearbyError => "nearby_error",
            Self::NearbyOff => "nearby_off",
            Self::Nephrology => "nephrology",
            Self::NestAudio => "nest_audio",
            Self::NestCamFloodlight => "nest_cam_floodlight",
            Self::NestCamIndoor => "nest_cam_indoor",
            Self::NestCamIq => "nest_cam_iq",
            Self::NestCamIqOutdoor => "nest_cam_iq_outdoor",
            Self::NestCamMagnetMount => "nest_cam_magnet_mount",
            Self::NestCamOutdoor => "nest_cam_outdoor",
            Self::NestCamStand => "nest_cam_stand",
            Self::NestCamWallMount => "nest_cam_wall_mount",
            Self::NestCamWiredStand => "nest_cam_wired_stand",
            Self::NestClockFarsightAnalog => "nest_clock_farsight_analog",
            Self::NestClockFarsightDigital => "nest_clock_farsight_digital",
            Self::NestConnect => "nest_connect",
            Self::NestDetect => "nest_detect",
            Self::NestDisplay => "nest_display",
            Self::NestDisplayMax => "nest_display_max",
            Self::NestDoorbellVisitor => "nest_doorbell_visitor",
            Self::NestEcoLeaf => "nest_eco_leaf",
            Self::NestFarsightCool => "nest_farsight_cool",
            Self::NestFarsightDual => "nest_farsight_dual",
            Self::NestFarsightEco => "nest_farsight_eco",
            Self::NestFarsightHeat => "nest_farsight_heat",
            Self::NestFarsightSeasonal => "nest_farsight_seasonal",
            Self::NestFarsightWeather => "nest_farsight_weather",
            Self::NestFoundSavings => "nest_found_savings",
            Self::NestGaleWifi => "nest_gale_wifi",
            Self::NestHeatLinkE => "nest_heat_link_e",
            Self::NestHeatLinkGen3 => "nest_heat_link_gen_3",
            Self::NestHelloDoorbell => "nest_hello_doorbell",
            Self::NestLocatorTag => "nest_locator_tag",
            Self::NestMini => "nest_mini",
            Self::NestMultiRoom => "nest_multi_room",
            Self::NestProtect => "nest_protect",
            Self::NestRemote => "nest_remote",
            Self::NestRemoteComfortSensor => "nest_remote_comfort_sensor",
            Self::NestSecureAlarm => "nest_secure_alarm",
            Self::NestSunblock => "nest_sunblock",
            Self::NestTag => "nest_tag",
            Self::NestThermostat => "nest_thermostat",
            Self::NestThermostatEEu => "nest_thermostat_e_eu",
            Self::NestThermostatGen3 => "nest_thermostat_gen_3",
            Self::NestThermostatSensor => "nest_thermostat_sensor",
            Self::NestThermostatSensorEu => "nest_thermostat_sensor_eu",
            Self::NestThermostatZirconiumEu => "nest_thermostat_zirconium_eu",
            Self::NestTrueRadiant => "nest_true_radiant",
            Self::NestWakeOnApproach => "nest_wake_on_approach",
            Self::NestWakeOnPress => "nest_wake_on_press",
            Self::NestWifiGale => "nest_wifi_gale",
            Self::NestWifiMistral => "nest_wifi_mistral",
            Self::NestWifiPoint => "nest_wifi_point",
            Self::NestWifiPointVento => "nest_wifi_point_vento",
            Self::NestWifiPro => "nest_wifi_pro",
            Self::NestWifiPro2 => "nest_wifi_pro_2",
            Self::NestWifiRouter => "nest_wifi_router",
            Self::NetworkCell => "network_cell",
            Self::NetworkCheck => "network_check",
            Self::NetworkIntelNode => "network_intel_node",
            Self::NetworkIntelligence => "network_intelligence",
            Self::NetworkIntelligenceHistory => "network_intelligence_history",
            Self::NetworkIntelligenceUpdate => "network_intelligence_update",
            Self::NetworkLocked => "network_locked",
            Self::NetworkManage => "network_manage",
            Self::NetworkNode => "network_node",
            Self::NetworkPing => "network_ping",
            Self::NetworkWifi => "network_wifi",
            Self::NetworkWifi1Bar => "network_wifi_1_bar",
            Self::NetworkWifi1BarLocked => "network_wifi_1_bar_locked",
            Self::NetworkWifi2Bar => "network_wifi_2_bar",
            Self::NetworkWifi2BarLocked => "network_wifi_2_bar_locked",
            Self::NetworkWifi3Bar => "network_wifi_3_bar",
            Self::NetworkWifi3BarLocked => "network_wifi_3_bar_locked",
            Self::NetworkWifiLocked => "network_wifi_locked",
            Self::Neurology => "neurology",
            Self::NewLabel => "new_label",
            Self::NewReleases => "new_releases",
            Self::NewWindow => "new_window",
            Self::News => "news",
            Self::Newsmode => "newsmode",
            Self::Newspaper => "newspaper",
            Self::Newsstand => "newsstand",
            Self::NextPlan => "next_plan",
            Self::NextWeek => "next_week",
            Self::Nfc => "nfc",
            Self::NfcOff => "nfc_off",
            Self::NightShelter => "night_shelter",
            Self::NightSightAuto => "night_sight_auto",
            Self::NightSightAutoOff => "night_sight_auto_off",
            Self::NightSightMax => "night_sight_max",
            Self::Nightlife => "nightlife",
            Self::Nightlight => "nightlight",
            Self::NightlightRound => "nightlight_round",
            Self::NightsStay => "nights_stay",
            Self::NoAccounts => "no_accounts",
            Self::NoAdultContent => "no_adult_content",
            Self::NoBackpack => "no_backpack",
            Self::NoCrash => "no_crash",
            Self::NoDrinks => "no_drinks",
            Self::NoEncryption => "no_encryption",
            Self::NoEncryptionGmailerrorred => "no_encryption_gmailerrorred",
            Self::NoFlash => "no_flash",
            Self::NoFood => "no_food",
            Self::NoLuggage => "no_luggage",
            Self::NoMeals => "no_meals",
            Self::NoMeetingRoom => "no_meeting_room",
            Self::NoPhotography => "no_photography",
            Self::NoSim => "no_sim",
            Self::NoSound => "no_sound",
            Self::NoStroller => "no_stroller",
            Self::NoTransfer => "no_transfer",
            Self::NoiseAware => "noise_aware",
            Self::NoiseControlOff => "noise_control_off",
            Self::NoiseControlOn => "noise_control_on",
            Self::NordicWalking => "nordic_walking",
            Self::North => "north",
            Self::NorthEast => "north_east",
            Self::NorthWest => "north_west",
            Self::NotAccessible => "not_accessible",
            Self::NotAccessibleForward => "not_accessible_forward",
            Self::NotInterested => "not_interested",
            Self::NotListedLocation => "not_listed_location",
            Self::NotStarted => "not_started",
            Self::Note => "note",
            Self::NoteAdd => "note_add",
            Self::NoteAlt => "note_alt",
            Self::NoteStack => "note_stack",
            Self::NoteStackAdd => "note_stack_add",
            Self::Notes => "notes",
            Self::NotificationAdd => "notification_add",
            Self::NotificationAudio => "notification_audio",
            Self::NotificationAudioOff => "notification_audio_off",
            Self::NotificationImportant => "notification_important",
            Self::NotificationMultiple => "notification_multiple",
            Self::NotificationSettings => "notification_settings",
            Self::NotificationSound => "notification_sound",
            Self::Notifications => "notifications",
            Self::NotificationsActive => "notifications_active",
            Self::NotificationsNone => "notifications_none",
            Self::NotificationsPaused => "notifications_paused",
            Self::NotificationsUnread => "notifications_unread",
            Self::Numbers => "numbers",
            Self::Nutrition => "nutrition",
            Self::Ods => "ods",
            Self::Odt => "odt",
            Self::OfflineBolt => "offline_bolt",
            Self::OfflinePin => "offline_pin",
            Self::OfflinePinOff => "offline_pin_off",
            Self::OfflineShare => "offline_share",
            Self::OilBarrel => "oil_barrel",
            Self::Okonomiyaki => "okonomiyaki",
            Self::OnDeviceTraining => "on_device_training",
            Self::OnHubDevice => "on_hub_device",
            Self::Oncology => "oncology",
            Self::OndemandVideo => "ondemand_video",
            Self::OnlinePrediction => "online_prediction",
            Self::Onsen => "onsen",
            Self::Opacity => "opacity",
            Self::OpenInBrowser => "open_in_browser",
            Self::OpenInFull => "open_in_full",
            Self::OpenInNew => "open_in_new",
            Self::OpenInNewDown => "open_in_new_down",
            Self::OpenInNewOff => "open_in_new_off",
            Self::OpenInPhone => "open_in_phone",
            Self::OpenJam => "open_jam",
            Self::OpenRun => "open_run",
            Self::OpenWith => "open_with",
            Self::Ophthalmology => "ophthalmology",
            Self::OralDisease => "oral_disease",
            Self::Orbit => "orbit",
            Self::OrderApprove => "order_approve",
            Self::OrderPlay => "order_play",
            Self::Orders => "orders",
            Self::Orthopedics => "orthopedics",
            Self::OtherAdmission => "other_admission",
            Self::OtherHouses => "other_houses",
            Self::Outbound => "outbound",
            Self::Outbox => "outbox",
            Self::OutboxAlt => "outbox_alt",
            Self::OutdoorGarden => "outdoor_garden",
            Self::OutdoorGrill => "outdoor_grill",
            Self::OutgoingMail => "outgoing_mail",
            Self::Outlet => "outlet",
            Self::OutlinedFlag => "outlined_flag",
            Self::Outpatient => "outpatient",
            Self::OutpatientMed => "outpatient_med",
            Self::Output => "output",
            Self::OutputCircle => "output_circle",
            Self::Oven => "oven",
            Self::OvenGen => "oven_gen",
            Self::Overview => "overview",
            Self::OverviewKey => "overview_key",
            Self::Owl => "owl",
            Self::OxygenSaturation => "oxygen_saturation",
            Self::P2p => "p2p",
            Self::Pace => "pace",
            Self::Pacemaker => "pacemaker",
            Self::Package => "package",
            Self::Package2 => "package_2",
            Self::Padding => "padding",
            Self::Padel => "padel",
            Self::PageControl => "page_control",
            Self::PageFooter => "page_footer",
            Self::PageHeader => "page_header",
            Self::PageInfo => "page_info",
            Self::PageMenuIos => "page_menu_ios",
            Self::Pageless => "pageless",
            Self::Pages => "pages",
            Self::Pageview => "pageview",
            Self::Paid => "paid",
            Self::Palette => "palette",
            Self::Pallet => "pallet",
            Self::PanTool => "pan_tool",
            Self::PanToolAlt => "pan_tool_alt",
            Self::PanZoom => "pan_zoom",
            Self::Panorama => "panorama",
            Self::PanoramaFishEye => "panorama_fish_eye",
            Self::PanoramaHorizontal => "panorama_horizontal",
            Self::PanoramaPhotosphere => "panorama_photosphere",
            Self::PanoramaVertical => "panorama_vertical",
            Self::PanoramaWideAngle => "panorama_wide_angle",
            Self::Paragliding => "paragliding",
            Self::ParentChildDining => "parent_child_dining",
            Self::Park => "park",
            Self::ParkingMeter => "parking_meter",
            Self::ParkingSign => "parking_sign",
            Self::ParkingValet => "parking_valet",
            Self::PartlyCloudyDay => "partly_cloudy_day",
            Self::PartlyCloudyNight => "partly_cloudy_night",
            Self::PartnerExchange => "partner_exchange",
            Self::PartnerHeart => "partner_heart",
            Self::PartnerReports => "partner_reports",
            Self::PartyMode => "party_mode",
            Self::Passkey => "passkey",
            Self::Passport => "passport",
            Self::Password => "password",
            Self::Password2 => "password_2",
            Self::Password2Off => "password_2_off",
            Self::PatientList => "patient_list",
            Self::Pattern => "pattern",
            Self::PauseCircle => "pause_circle",
            Self::PauseCircleFilled => "pause_circle_filled",
            Self::PauseCircleOutline => "pause_circle_outline",
            Self::PausePresentation => "pause_presentation",
            Self::Payment => "payment",
            Self::PaymentArrowDown => "payment_arrow_down",
            Self::PaymentCard => "payment_card",
            Self::Payments => "payments",
            Self::PedalBike => "pedal_bike",
            Self::Pediatrics => "pediatrics",
            Self::PenSize1 => "pen_size_1",
            Self::PenSize2 => "pen_size_2",
            Self::PenSize3 => "pen_size_3",
            Self::PenSize4 => "pen_size_4",
            Self::PenSize5 => "pen_size_5",
            Self::Pending => "pending",
            Self::PendingActions => "pending_actions",
            Self::Pentagon => "pentagon",
            Self::People => "people",
            Self::PeopleAlt => "people_alt",
            Self::PeopleOutline => "people_outline",
            Self::PeopleSizeDecrease => "people_size_decrease",
            Self::PeopleSizeIncrease => "people_size_increase",
            Self::Percent => "percent",
            Self::PercentDiscount => "percent_discount",
            Self::PerformanceMax => "performance_max",
            Self::Pergola => "pergola",
            Self::PermCameraMic => "perm_camera_mic",
            Self::PermContactCalendar => "perm_contact_calendar",
            Self::PermDataSetting => "perm_data_setting",
            Self::PermDeviceInformation => "perm_device_information",
            Self::PermIdentity => "perm_identity",
            Self::PermMedia => "perm_media",
            Self::PermPhoneMsg => "perm_phone_msg",
            Self::PermScanWifi => "perm_scan_wifi",
            Self::Person => "person",
            Self::Person2 => "person_2",
            Self::Person3 => "person_3",
            Self::Person4 => "person_4",
            Self::PersonAdd => "person_add",
            Self::PersonAddAlt => "person_add_alt",
            Self::PersonAddDisabled => "person_add_disabled",
            Self::PersonAlert => "person_alert",
            Self::PersonApron => "person_apron",
            Self::PersonBook => "person_book",
            Self::PersonCancel => "person_cancel",
            Self::PersonCelebrate => "person_celebrate",
            Self::PersonCheck => "person_check",
            Self::PersonEdit => "person_edit",
            Self::PersonFilled => "person_filled",
            Self::PersonHeart => "person_heart",
            Self::PersonOff => "person_off",
            Self::PersonOutline => "person_outline",
            Self::PersonPin => "person_pin",
            Self::PersonPinCircle => "person_pin_circle",
            Self::PersonPlay => "person_play",
            Self::PersonRaisedHand => "person_raised_hand",
            Self::PersonRemove => "person_remove",
            Self::PersonSearch => "person_search",
            Self::PersonShield => "person_shield",
            Self::PersonText => "person_text",
            Self::PersonalBag => "personal_bag",
            Self::PersonalBagOff => "personal_bag_off",
            Self::PersonalBagQuestion => "personal_bag_question",
            Self::PersonalInjury => "personal_injury",
            Self::PersonalPlaces => "personal_places",
            Self::PersonalVideo => "personal_video",
            Self::PestControl => "pest_control",
            Self::PestControlRodent => "pest_control_rodent",
            Self::PetSupplies => "pet_supplies",
            Self::Pets => "pets",
            Self::Phishing => "phishing",
            Self::Phone => "phone",
            Self::PhoneAlt => "phone_alt",
            Self::PhoneAndroid => "phone_android",
            Self::PhoneBluetoothSpeaker => "phone_bluetooth_speaker",
            Self::PhoneCallback => "phone_callback",
            Self::PhoneCancel => "phone_cancel",
            Self::PhoneDisabled => "phone_disabled",
            Self::PhoneEnabled => "phone_enabled",
            Self::PhoneForwarded => "phone_forwarded",
            Self::PhoneInTalk => "phone_in_talk",
            Self::PhoneIphone => "phone_iphone",
            Self::PhoneLocked => "phone_locked",
            Self::PhoneMissed => "phone_missed",
            Self::PhonePaused => "phone_paused",
            Self::Phonelink => "phonelink",
            Self::PhonelinkErase => "phonelink_erase",
            Self::PhonelinkLock => "phonelink_lock",
            Self::PhonelinkOff => "phonelink_off",
            Self::PhonelinkRing => "phonelink_ring",
            Self::PhonelinkRingOff => "phonelink_ring_off",
            Self::PhonelinkSetup => "phonelink_setup",
            Self::Photo => "photo",
            Self::PhotoAlbum => "photo_album",
            Self::PhotoAutoMerge => "photo_auto_merge",
            Self::PhotoCamera => "photo_camera",
            Self::PhotoCameraBack => "photo_camera_back",
            Self::PhotoCameraFront => "photo_camera_front",
            Self::PhotoFilter => "photo_filter",
            Self::PhotoFrame => "photo_frame",
            Self::PhotoLibrary => "photo_library",
            Self::PhotoPrints => "photo_prints",
            Self::PhotoSizeSelectActual => "photo_size_select_actual",
            Self::PhotoSizeSelectLarge => "photo_size_select_large",
            Self::PhotoSizeSelectSmall => "photo_size_select_small",
            Self::Php => "php",
            Self::PhysicalTherapy => "physical_therapy",
            Self::Piano => "piano",
            Self::PianoOff => "piano_off",
            Self::Pickleball => "pickleball",
            Self::PictureAsPdf => "picture_as_pdf",
            Self::PictureInPicture => "picture_in_picture",
            Self::PictureInPictureAlt => "picture_in_picture_alt",
            Self::PictureInPictureCenter => "picture_in_picture_center",
            Self::PictureInPictureLarge => "picture_in_picture_large",
            Self::PictureInPictureMedium => "picture_in_picture_medium",
            Self::PictureInPictureMobile => "picture_in_picture_mobile",
            Self::PictureInPictureOff => "picture_in_picture_off",
            Self::PictureInPictureSmall => "picture_in_picture_small",
            Self::PieChart => "pie_chart",
            Self::PieChartFilled => "pie_chart_filled",
            Self::PieChartOutline => "pie_chart_outline",
            Self::PieChartOutlined => "pie_chart_outlined",
            Self::Pill => "pill",
            Self::PillOff => "pill_off",
            Self::Pin => "pin",
            Self::PinDrop => "pin_drop",
            Self::PinEnd => "pin_end",
            Self::PinHistory => "pin_history",
            Self::PinInvoke => "pin_invoke",
            Self::PinRoad => "pin_road",
            Self::PinRoad2 => "pin_road_2",
            Self::Pinboard => "pinboard",
            Self::PinboardUnread => "pinboard_unread",
            Self::Pinch => "pinch",
            Self::PinchZoomIn => "pinch_zoom_in",
            Self::PinchZoomOut => "pinch_zoom_out",
            Self::Pip => "pip",
            Self::PipExit => "pip_exit",
            Self::PivotTableChart => "pivot_table_chart",
            Self::Place => "place",
            Self::PlaceItem => "place_item",
            Self::Plagiarism => "plagiarism",
            Self::PlaneContrails => "plane_contrails",
            Self::Planet => "planet",
            Self::PlannerBannerAdPt => "planner_banner_ad_pt",
            Self::PlannerReview => "planner_review",
            Self::PlayArrow => "play_arrow",
            Self::PlayCircle => "play_circle",
            Self::PlayDisabled => "play_disabled",
            Self::PlayForWork => "play_for_work",
            Self::PlayLesson => "play_lesson",
            Self::PlayMusic => "play_music",
            Self::PlayPause => "play_pause",
            Self::PlayShapes => "play_shapes",
            Self::Playground => "playground",
            Self::Playground2 => "playground_2",
            Self::PlayingCards => "playing_cards",
            Self::PlaylistAdd => "playlist_add",
            Self::PlaylistAddCheck => "playlist_add_check",
            Self::PlaylistAddCheckCircle => "playlist_add_check_circle",
            Self::PlaylistAddCircle => "playlist_add_circle",
            Self::PlaylistPlay => "playlist_play",
            Self::PlaylistRemove => "playlist_remove",
            Self::PlugConnect => "plug_connect",
            Self::Plumbing => "plumbing",
            Self::PlusOne => "plus_one",
            Self::Podcasts => "podcasts",
            Self::Podiatry => "podiatry",
            Self::Podium => "podium",
            Self::PointOfSale => "point_of_sale",
            Self::PointScan => "point_scan",
            Self::PokerChip => "poker_chip",
            Self::Policy => "policy",
            Self::PolicyAlert => "policy_alert",
            Self::Poll => "poll",
            Self::Polyline => "polyline",
            Self::Polymer => "polymer",
            Self::Pool => "pool",
            Self::PortableWifiOff => "portable_wifi_off",
            Self::Portrait => "portrait",
            Self::PositionBottomLeft => "position_bottom_left",
            Self::PositionBottomRight => "position_bottom_right",
            Self::PositionTopRight => "position_top_right",
            Self::Post => "post",
            Self::PostAdd => "post_add",
            Self::PottedPlant => "potted_plant",
            Self::Power => "power",
            Self::PowerInput => "power_input",
            Self::PowerOff => "power_off",
            Self::PowerRounded => "power_rounded",
            Self::PowerSettingsCircle => "power_settings_circle",
            Self::PowerSettingsNew => "power_settings_new",
            Self::PrayerTimes => "prayer_times",
            Self::PrecisionManufacturing => "precision_manufacturing",
            Self::Pregnancy => "pregnancy",
            Self::PregnantWoman => "pregnant_woman",
            Self::Preliminary => "preliminary",
            Self::Prescriptions => "prescriptions",
            Self::PresentToAll => "present_to_all",
            Self::Preview => "preview",
            Self::PreviewOff => "preview_off",
            Self::PriceChange => "price_change",
            Self::PriceCheck => "price_check",
            Self::PrintAdd => "print_add",
            Self::PrintConnect => "print_connect",
            Self::PrintDisabled => "print_disabled",
            Self::PrintError => "print_error",
            Self::PrintLock => "print_lock",
            Self::Priority => "priority",
            Self::PriorityHigh => "priority_high",
            Self::Privacy => "privacy",
            Self::PrivacyTip => "privacy_tip",
            Self::PrivateConnectivity => "private_connectivity",
            Self::Problem => "problem",
            Self::Procedure => "procedure",
            Self::ProcessChart => "process_chart",
            Self::ProductionQuantityLimits => "production_quantity_limits",
            Self::Productivity => "productivity",
            Self::ProgressActivity => "progress_activity",
            Self::PromptSuggestion => "prompt_suggestion",
            Self::Propane => "propane",
            Self::PropaneTank => "propane_tank",
            Self::Psychiatry => "psychiatry",
            Self::Psychology => "psychology",
            Self::PsychologyAlt => "psychology_alt",
            Self::Public => "public",
            Self::PublicOff => "public_off",
            Self::Publish => "publish",
            Self::PublishedWithChanges => "published_with_changes",
            Self::Pulmonology => "pulmonology",
            Self::PulseAlert => "pulse_alert",
            Self::PunchClock => "punch_clock",
            Self::PushPin => "push_pin",
            Self::QrCode => "qr_code",
            Self::QrCode2 => "qr_code_2",
            Self::QrCode2Add => "qr_code_2_add",
            Self::QrCodeScanner => "qr_code_scanner",
            Self::QueryBuilder => "query_builder",
            Self::QueryStats => "query_stats",
            Self::QuestionAnswer => "question_answer",
            Self::QuestionExchange => "question_exchange",
            Self::QuestionMark => "question_mark",
            Self::Queue => "queue",
            Self::QueueMusic => "queue_music",
            Self::QueuePlayNext => "queue_play_next",
            Self::QuickPhrases => "quick_phrases",
            Self::QuickReference => "quick_reference",
            Self::QuickReferenceAll => "quick_reference_all",
            Self::QuickReorder => "quick_reorder",
            Self::Quickreply => "quickreply",
            Self::QuietTime => "quiet_time",
            Self::QuietTimeActive => "quiet_time_active",
            Self::Quiz => "quiz",
            Self::RMobiledata => "r_mobiledata",
            Self::Radar => "radar",
            Self::Radio => "radio",
            Self::RadioButtonChecked => "radio_button_checked",
            Self::RadioButtonPartial => "radio_button_partial",
            Self::RadioButtonUnchecked => "radio_button_unchecked",
            Self::Radiology => "radiology",
            Self::RailwayAlert => "railway_alert",
            Self::RailwayAlert2 => "railway_alert_2",
            Self::Rainy => "rainy",
            Self::RainyHeavy => "rainy_heavy",
            Self::RainyLight => "rainy_light",
            Self::RainySnow => "rainy_snow",
            Self::RamenDining => "ramen_dining",
            Self::RampLeft => "ramp_left",
            Self::RampRight => "ramp_right",
            Self::RangeHood => "range_hood",
            Self::RateReview => "rate_review",
            Self::RateReviewRtl => "rate_review_rtl",
            Self::Raven => "raven",
            Self::RawOff => "raw_off",
            Self::RawOn => "raw_on",
            Self::ReadMore => "read_more",
            Self::ReadinessScore => "readiness_score",
            Self::RealEstateAgent => "real_estate_agent",
            Self::RearCamera => "rear_camera",
            Self::Rebase => "rebase",
            Self::RebaseEdit => "rebase_edit",
            Self::Receipt => "receipt",
            Self::ReceiptLong => "receipt_long",
            Self::ReceiptLongOff => "receipt_long_off",
            Self::RecentActors => "recent_actors",
            Self::RecentPatient => "recent_patient",
            Self::Recenter => "recenter",
            Self::Recommend => "recommend",
            Self::RecordVoiceOver => "record_voice_over",
            Self::Rectangle => "rectangle",
            Self::RectangleAdd => "rectangle_add",
            Self::Recycling => "recycling",
            Self::Redeem => "redeem",
            Self::ReduceCapacity => "reduce_capacity",
            Self::RegularExpression => "regular_expression",
            Self::Relax => "relax",
            Self::ReleaseAlert => "release_alert",
            Self::RememberMe => "remember_me",
            Self::Reminder => "reminder",
            Self::RemindersAlt => "reminders_alt",
            Self::RemoteGen => "remote_gen",
            Self::Remove => "remove",
            Self::RemoveCircle => "remove_circle",
            Self::RemoveCircleOutline => "remove_circle_outline",
            Self::RemoveDone => "remove_done",
            Self::RemoveFromQueue => "remove_from_queue",
            Self::RemoveModerator => "remove_moderator",
            Self::RemoveRedEye => "remove_red_eye",
            Self::RemoveRoad => "remove_road",
            Self::RemoveSelection => "remove_selection",
            Self::RemoveShoppingCart => "remove_shopping_cart",
            Self::ReopenWindow => "reopen_window",
            Self::Reorder => "reorder",
            Self::Repartition => "repartition",
            Self::Repeat => "repeat",
            Self::RepeatOn => "repeat_on",
            Self::RepeatOne => "repeat_one",
            Self::RepeatOneOn => "repeat_one_on",
            Self::ReplaceAudio => "replace_audio",
            Self::ReplaceImage => "replace_image",
            Self::ReplaceVideo => "replace_video",
            Self::Replay => "replay",
            Self::Replay10 => "replay_10",
            Self::Replay30 => "replay_30",
            Self::Replay5 => "replay_5",
            Self::ReplayCircleFilled => "replay_circle_filled",
            Self::Reply => "reply",
            Self::ReplyAll => "reply_all",
            Self::Report => "report",
            Self::ReportGmailerrorred => "report_gmailerrorred",
            Self::ReportOff => "report_off",
            Self::ReportProblem => "report_problem",
            Self::RequestPage => "request_page",
            Self::RequestQuote => "request_quote",
            Self::ResetBrightness => "reset_brightness",
            Self::ResetColors => "reset_colors",
            Self::ResetExposure => "reset_exposure",
            Self::ResetFocus => "reset_focus",
            Self::ResetImage => "reset_image",
            Self::ResetIso => "reset_iso",
            Self::ResetSettings => "reset_settings",
            Self::ResetShadow => "reset_shadow",
            Self::ResetShutterSpeed => "reset_shutter_speed",
            Self::ResetTv => "reset_tv",
            Self::ResetWhiteBalance => "reset_white_balance",
            Self::ResetWrench => "reset_wrench",
            Self::Resize => "resize",
            Self::ResizeWindow => "resize_window",
            Self::RespiratoryRate => "respiratory_rate",
            Self::ResponsiveLayout => "responsive_layout",
            Self::RestArea => "rest_area",
            Self::RestartAlt => "restart_alt",
            Self::Restaurant => "restaurant",
            Self::RestaurantMenu => "restaurant_menu",
            Self::Restore => "restore",
            Self::RestoreFromTrash => "restore_from_trash",
            Self::RestorePage => "restore_page",
            Self::Resume => "resume",
            Self::Reviews => "reviews",
            Self::RewardedAds => "rewarded_ads",
            Self::Rheumatology => "rheumatology",
            Self::RibCage => "rib_cage",
            Self::RiceBowl => "rice_bowl",
            Self::RightClick => "right_click",
            Self::RightPanelClose => "right_panel_close",
            Self::RightPanelOpen => "right_panel_open",
            Self::RingVolume => "ring_volume",
            Self::RingVolumeFilled => "ring_volume_filled",
            Self::Ripples => "ripples",
            Self::Road => "road",
            Self::Robot => "robot",
            Self::Robot2 => "robot_2",
            Self::Rocket => "rocket",
            Self::RocketLaunch => "rocket_launch",
            Self::RollerShades => "roller_shades",
            Self::RollerShadesClosed => "roller_shades_closed",
            Self::RollerSkating => "roller_skating",
            Self::Roofing => "roofing",
            Self::Room => "room",
            Self::RoomPreferences => "room_preferences",
            Self::RoomService => "room_service",
            Self::Rotate90DegreesCcw => "rotate_90_degrees_ccw",
            Self::Rotate90DegreesCw => "rotate_90_degrees_cw",
            Self::RotateAuto => "rotate_auto",
            Self::RotateLeft => "rotate_left",
            Self::RotateRight => "rotate_right",
            Self::RoundaboutLeft => "roundabout_left",
            Self::RoundaboutRight => "roundabout_right",
            Self::RoundedCorner => "rounded_corner",
            Self::Route => "route",
            Self::Router => "router",
            Self::RouterOff => "router_off",
            Self::Routine => "routine",
            Self::Rowing => "rowing",
            Self::RssFeed => "rss_feed",
            Self::Rsvp => "rsvp",
            Self::Rtt => "rtt",
            Self::Rubric => "rubric",
            Self::Rule => "rule",
            Self::RuleFolder => "rule_folder",
            Self::RuleSettings => "rule_settings",
            Self::RunCircle => "run_circle",
            Self::RunningWithErrors => "running_with_errors",
            Self::RvHookup => "rv_hookup",
            Self::SafetyCheck => "safety_check",
            Self::SafetyCheckOff => "safety_check_off",
            Self::SafetyDivider => "safety_divider",
            Self::Sailing => "sailing",
            Self::Salinity => "salinity",
            Self::Sanitizer => "sanitizer",
            Self::Satellite => "satellite",
            Self::SatelliteAlt => "satellite_alt",
            Self::Sauna => "sauna",
            Self::SaveAlt => "save_alt",
            Self::SaveAs => "save_as",
            Self::SaveClock => "save_clock",
            Self::SavedSearch => "saved_search",
            Self::Savings => "savings",
            Self::Scale => "scale",
            Self::Scan => "scan",
            Self::ScanDelete => "scan_delete",
            Self::Scanner => "scanner",
            Self::ScatterPlot => "scatter_plot",
            Self::Scene => "scene",
            Self::ScheduleSend => "schedule_send",
            Self::Schema => "schema",
            Self::School => "school",
            Self::Science => "science",
            Self::ScienceOff => "science_off",
            Self::Scooter => "scooter",
            Self::Score => "score",
            Self::Scoreboard => "scoreboard",
            Self::ScreenLockLandscape => "screen_lock_landscape",
            Self::ScreenLockPortrait => "screen_lock_portrait",
            Self::ScreenLockRotation => "screen_lock_rotation",
            Self::ScreenRecord => "screen_record",
            Self::ScreenRotation => "screen_rotation",
            Self::ScreenRotationAlt => "screen_rotation_alt",
            Self::ScreenRotationUp => "screen_rotation_up",
            Self::ScreenSearchDesktop => "screen_search_desktop",
            Self::ScreenShare => "screen_share",
            Self::Screenshot => "screenshot",
            Self::ScreenshotFrame => "screenshot_frame",
            Self::ScreenshotFrame2 => "screenshot_frame_2",
            Self::ScreenshotKeyboard => "screenshot_keyboard",
            Self::ScreenshotMonitor => "screenshot_monitor",
            Self::ScreenshotRegion => "screenshot_region",
            Self::ScreenshotTablet => "screenshot_tablet",
            Self::Script => "script",
            Self::ScrollableHeader => "scrollable_header",
            Self::ScubaDiving => "scuba_diving",
            Self::Sd => "sd",
            Self::SdCard => "sd_card",
            Self::SdCardAlert => "sd_card_alert",
            Self::SdStorage => "sd_storage",
            Self::Sdk => "sdk",
            Self::SearchActivity => "search_activity",
            Self::SearchCheck => "search_check",
            Self::SearchCheck2 => "search_check_2",
            Self::SearchGear => "search_gear",
            Self::SearchHandsFree => "search_hands_free",
            Self::SearchInsights => "search_insights",
            Self::SearchOff => "search_off",
            Self::SeatCoolLeft => "seat_cool_left",
            Self::SeatCoolRight => "seat_cool_right",
            Self::SeatHeatLeft => "seat_heat_left",
            Self::SeatHeatRight => "seat_heat_right",
            Self::SeatRead => "seat_read",
            Self::SeatVentLeft => "seat_vent_left",
            Self::SeatVentRight => "seat_vent_right",
            Self::SeatWindow => "seat_window",
            Self::Security => "security",
            Self::SecurityKey => "security_key",
            Self::SecurityUpdate => "security_update",
            Self::SecurityUpdateGood => "security_update_good",
            Self::SecurityUpdateWarning => "security_update_warning",
            Self::Segment => "segment",
            Self::Select => "select",
            Self::SelectAll => "select_all",
            Self::SelectCheckBox => "select_check_box",
            Self::SelectToSpeak => "select_to_speak",
            Self::SelectWindow => "select_window",
            Self::SelectWindow2 => "select_window_2",
            Self::SelectWindowOff => "select_window_off",
            Self::SelfCare => "self_care",
            Self::SelfImprovement => "self_improvement",
            Self::Sell => "sell",
            Self::SellCloud => "sell_cloud",
            Self::SendAndArchive => "send_and_archive",
            Self::SendMoney => "send_money",
            Self::SendTimeExtension => "send_time_extension",
            Self::SendToMobile => "send_to_mobile",
            Self::SensorDoor => "sensor_door",
            Self::SensorOccupied => "sensor_occupied",
            Self::SensorWindow => "sensor_window",
            Self::Sensors => "sensors",
            Self::SensorsKrx => "sensors_krx",
            Self::SensorsKrxOff => "sensors_krx_off",
            Self::SensorsOff => "sensors_off",
            Self::SentimentCalm => "sentiment_calm",
            Self::SentimentContent => "sentiment_content",
            Self::SentimentDissatisfied => "sentiment_dissatisfied",
            Self::SentimentExcited => "sentiment_excited",
            Self::SentimentExtremelyDissatisfied => "sentiment_extremely_dissatisfied",
            Self::SentimentFrustrated => "sentiment_frustrated",
            Self::SentimentNeutral => "sentiment_neutral",
            Self::SentimentSad => "sentiment_sad",
            Self::SentimentSatisfied => "sentiment_satisfied",
            Self::SentimentSatisfiedAlt => "sentiment_satisfied_alt",
            Self::SentimentStressed => "sentiment_stressed",
            Self::SentimentVeryDissatisfied => "sentiment_very_dissatisfied",
            Self::SentimentVerySatisfied => "sentiment_very_satisfied",
            Self::SentimentWorried => "sentiment_worried",
            Self::Serif => "serif",
            Self::ServerPerson => "server_person",
            Self::ServiceToolbox => "service_toolbox",
            Self::SetMeal => "set_meal",
            Self::SettingsAccessibility => "settings_accessibility",
            Self::SettingsAccountBox => "settings_account_box",
            Self::SettingsAlert => "settings_alert",
            Self::SettingsApplications => "settings_applications",
            Self::SettingsBRoll => "settings_b_roll",
            Self::SettingsBackupRestore => "settings_backup_restore",
            Self::SettingsBluetooth => "settings_bluetooth",
            Self::SettingsBrightness => "settings_brightness",
            Self::SettingsCell => "settings_cell",
            Self::SettingsCinematicBlur => "settings_cinematic_blur",
            Self::SettingsEthernet => "settings_ethernet",
            Self::SettingsHeart => "settings_heart",
            Self::SettingsInputAntenna => "settings_input_antenna",
            Self::SettingsInputComponent => "settings_input_component",
            Self::SettingsInputComposite => "settings_input_composite",
            Self::SettingsInputHdmi => "settings_input_hdmi",
            Self::SettingsInputSvideo => "settings_input_svideo",
            Self::SettingsMotionMode => "settings_motion_mode",
            Self::SettingsNightSight => "settings_night_sight",
            Self::SettingsOverscan => "settings_overscan",
            Self::SettingsPanorama => "settings_panorama",
            Self::SettingsPhone => "settings_phone",
            Self::SettingsPhotoCamera => "settings_photo_camera",
            Self::SettingsPower => "settings_power",
            Self::SettingsRemote => "settings_remote",
            Self::SettingsScreen => "settings_screen",
            Self::SettingsSeating => "settings_seating",
            Self::SettingsSlowMotion => "settings_slow_motion",
            Self::SettingsSuggest => "settings_suggest",
            Self::SettingsSystemDaydream => "settings_system_daydream",
            Self::SettingsTimelapse => "settings_timelapse",
            Self::SettingsVideoCamera => "settings_video_camera",
            Self::SettingsVoice => "settings_voice",
            Self::SettopComponent => "settop_component",
            Self::SevereCold => "severe_cold",
            Self::Shades => "shades",
            Self::ShadesClosed => "shades_closed",
            Self::Shadow => "shadow",
            Self::ShadowAdd => "shadow_add",
            Self::ShadowMinus => "shadow_minus",
            Self::ShapeLine => "shape_line",
            Self::ShapeRecognition => "shape_recognition",
            Self::Shapes => "shapes",
            Self::ShareEta => "share_eta",
            Self::ShareLocation => "share_location",
            Self::ShareOff => "share_off",
            Self::ShareReviews => "share_reviews",
            Self::ShareWindows => "share_windows",
            Self::ShavedIce => "shaved_ice",
            Self::SheetsColumnSwap => "sheets_column_swap",
            Self::SheetsRtl => "sheets_rtl",
            Self::ShelfAutoHide => "shelf_auto_hide",
            Self::ShelfPosition => "shelf_position",
            Self::Shelves => "shelves",
            Self::Shield => "shield",
            Self::ShieldCard => "shield_card",
            Self::ShieldLock => "shield_lock",
            Self::ShieldLocked => "shield_locked",
            Self::ShieldMoon => "shield_moon",
            Self::ShieldPerson => "shield_person",
            Self::ShieldQuestion => "shield_question",
            Self::ShieldRadar => "shield_radar",
            Self::ShieldToggle => "shield_toggle",
            Self::ShieldWatch => "shield_watch",
            Self::ShieldWithHeart => "shield_with_heart",
            Self::ShieldWithHouse => "shield_with_house",
            Self::Shift => "shift",
            Self::ShiftLock => "shift_lock",
            Self::ShiftLockOff => "shift_lock_off",
            Self::ShoeCleats => "shoe_cleats",
            Self::Shop => "shop",
            Self::Shop2 => "shop_2",
            Self::ShopTwo => "shop_two",
            Self::ShoppingBag => "shopping_bag",
            Self::ShoppingBagSpeed => "shopping_bag_speed",
            Self::ShoppingBasket => "shopping_basket",
            Self::ShoppingCart => "shopping_cart",
            Self::ShoppingCartCheckout => "shopping_cart_checkout",
            Self::ShoppingCartOff => "shopping_cart_off",
            Self::Shoppingmode => "shoppingmode",
            Self::ShortStay => "short_stay",
            Self::ShortText => "short_text",
            Self::Shortcut => "shortcut",
            Self::ShowChart => "show_chart",
            Self::Shower => "shower",
            Self::Shuffle => "shuffle",
            Self::ShuffleOn => "shuffle_on",
            Self::ShutterSpeed => "shutter_speed",
            Self::ShutterSpeedAdd => "shutter_speed_add",
            Self::ShutterSpeedMinus => "shutter_speed_minus",
            Self::Sick => "sick",
            Self::SideNavigation => "side_navigation",
            Self::SignLanguage => "sign_language",
            Self::SignLanguage2 => "sign_language_2",
            Self::SignLanguageOff => "sign_language_off",
            Self::SignalCellular0Bar => "signal_cellular_0_bar",
            Self::SignalCellular1Bar => "signal_cellular_1_bar",
            Self::SignalCellular2Bar => "signal_cellular_2_bar",
            Self::SignalCellular3Bar => "signal_cellular_3_bar",
            Self::SignalCellular4Bar => "signal_cellular_4_bar",
            Self::SignalCellularAdd => "signal_cellular_add",
            Self::SignalCellularAlt => "signal_cellular_alt",
            Self::SignalCellularAlt1Bar => "signal_cellular_alt_1_bar",
            Self::SignalCellularAlt2Bar => "signal_cellular_alt_2_bar",
            Self::SignalCellularAltOff => "signal_cellular_alt_off",
            Self::SignalCellularConnectedNoInternet0Bar => "signal_cellular_connected_no_internet_0_bar",
            Self::SignalCellularConnectedNoInternet4Bar => "signal_cellular_connected_no_internet_4_bar",
            Self::SignalCellularNoSim => "signal_cellular_no_sim",
            Self::SignalCellularNodata => "signal_cellular_nodata",
            Self::SignalCellularNull => "signal_cellular_null",
            Self::SignalCellularOff => "signal_cellular_off",
            Self::SignalCellularPause => "signal_cellular_pause",
            Self::SignalDisconnected => "signal_disconnected",
            Self::SignalWifi0Bar => "signal_wifi_0_bar",
            Self::SignalWifi4Bar => "signal_wifi_4_bar",
            Self::SignalWifi4BarLock => "signal_wifi_4_bar_lock",
            Self::SignalWifiBad => "signal_wifi_bad",
            Self::SignalWifiConnectedNoInternet4 => "signal_wifi_connected_no_internet_4",
            Self::SignalWifiOff => "signal_wifi_off",
            Self::SignalWifiStatusbar4Bar => "signal_wifi_statusbar_4_bar",
            Self::SignalWifiStatusbarNotConnected => "signal_wifi_statusbar_not_connected",
            Self::SignalWifiStatusbarNull => "signal_wifi_statusbar_null",
            Self::Signature => "signature",
            Self::Signpost => "signpost",
            Self::SimCard => "sim_card",
            Self::SimCardAlert => "sim_card_alert",
            Self::SimCardDownload => "sim_card_download",
            Self::SimCardLock => "sim_card_lock",
            Self::Simulation => "simulation",
            Self::SingleArrow => "single_arrow",
            Self::SingleBed => "single_bed",
            Self::Sip => "sip",
            Self::Siren => "siren",
            Self::SirenCheck => "siren_check",
            Self::SirenOpen => "siren_open",
            Self::SirenQuestion => "siren_question",
            Self::Skateboarding => "skateboarding",
            Self::Skeleton => "skeleton",
            Self::Skillet => "skillet",
            Self::SkilletCooktop => "skillet_cooktop",
            Self::SkipPrevious => "skip_previous",
            Self::Skull => "skull",
            Self::SkullList => "skull_list",
            Self::SlabSerif => "slab_serif",
            Self::Sledding => "sledding",
            Self::Sleep => "sleep",
            Self::SleepScore => "sleep_score",
            Self::SlideLibrary => "slide_library",
            Self::Sliders => "sliders",
            Self::Slideshow => "slideshow",
            Self::SlowMotionVideo => "slow_motion_video",
            Self::SmartButton => "smart_button",
            Self::SmartCardReader => "smart_card_reader",
            Self::SmartCardReaderOff => "smart_card_reader_off",
            Self::SmartDisplay => "smart_display",
            Self::SmartOutlet => "smart_outlet",
            Self::SmartScreen => "smart_screen",
            Self::SmartToy => "smart_toy",
            Self::Smartphone => "smartphone",
            Self::SmartphoneCamera => "smartphone_camera",
            Self::SmbShare => "smb_share",
            Self::SmokeFree => "smoke_free",
            Self::SmokingRooms => "smoking_rooms",
            Self::Sms => "sms",
            Self::SmsFailed => "sms_failed",
            Self::Snail => "snail",
            Self::SnippetFolder => "snippet_folder",
            Self::Snooze => "snooze",
            Self::Snowboarding => "snowboarding",
            Self::Snowflake => "snowflake",
            Self::Snowing => "snowing",
            Self::SnowingHeavy => "snowing_heavy",
            Self::Snowmobile => "snowmobile",
            Self::Snowshoeing => "snowshoeing",
            Self::Soap => "soap",
            Self::Soba => "soba",
            Self::SocialDistance => "social_distance",
            Self::SocialLeaderboard => "social_leaderboard",
            Self::SolarPower => "solar_power",
            Self::SoloDining => "solo_dining",
            Self::SortByAlpha => "sort_by_alpha",
            Self::Sos => "sos",
            Self::SoundDetectionDogBarking => "sound_detection_dog_barking",
            Self::SoundDetectionGlassBreak => "sound_detection_glass_break",
            Self::SoundDetectionLoudSound => "sound_detection_loud_sound",
            Self::SoundSampler => "sound_sampler",
            Self::Soundbar => "soundbar",
            Self::SoupKitchen => "soup_kitchen",
            Self::Source => "source",
            Self::SourceEnvironment => "source_environment",
            Self::SourceNotes => "source_notes",
            Self::South => "south",
            Self::SouthAmerica => "south_america",
            Self::SouthEast => "south_east",
            Self::SouthWest => "south_west",
            Self::Spa => "spa",
            Self::SpaceBar => "space_bar",
            Self::SpaceDashboard => "space_dashboard",
            Self::SpaceDashboard2 => "space_dashboard_2",
            Self::SpatialAudio => "spatial_audio",
            Self::SpatialAudioOff => "spatial_audio_off",
            Self::SpatialGallery => "spatial_gallery",
            Self::SpatialSpeaker => "spatial_speaker",
            Self::SpatialTracking => "spatial_tracking",
            Self::Speaker => "speaker",
            Self::Speaker2 => "speaker_2",
            Self::Speaker3 => "speaker_3",
            Self::SpeakerGroup => "speaker_group",
            Self::SpeakerNotes => "speaker_notes",
            Self::SpeakerNotesOff => "speaker_notes_off",
            Self::SpeakerPhone => "speaker_phone",
            Self::SpecialCharacter => "special_character",
            Self::SpecificGravity => "specific_gravity",
            Self::SpeechToText => "speech_to_text",
            Self::SpeechToText2 => "speech_to_text_2",
            Self::Speed => "speed",
            Self::Speed025 => "speed_0_25",
            Self::Speed02x => "speed_0_2x",
            Self::Speed05 => "speed_0_5",
            Self::Speed05x => "speed_0_5x",
            Self::Speed075 => "speed_0_75",
            Self::Speed07x => "speed_0_7x",
            Self::Speed12 => "speed_1_2",
            Self::Speed125 => "speed_1_25",
            Self::Speed12x => "speed_1_2x",
            Self::Speed15 => "speed_1_5",
            Self::Speed15x => "speed_1_5x",
            Self::Speed175 => "speed_1_75",
            Self::Speed17x => "speed_1_7x",
            Self::Speed2 => "speed_2",
            Self::Speed2x => "speed_2x",
            Self::Speed3 => "speed_3",
            Self::Speed4 => "speed_4",
            Self::SpeedCamera => "speed_camera",
            Self::Spellcheck => "spellcheck",
            Self::SplitScene => "split_scene",
            Self::SplitScene2 => "split_scene_2",
            Self::SplitSceneDown => "split_scene_down",
            Self::SplitSceneLeft => "split_scene_left",
            Self::SplitSceneRight => "split_scene_right",
            Self::SplitSceneUp => "split_scene_up",
            Self::Splitscreen => "splitscreen",
            Self::SplitscreenAdd => "splitscreen_add",
            Self::SplitscreenBottom => "splitscreen_bottom",
            Self::SplitscreenLandscape => "splitscreen_landscape",
            Self::SplitscreenLandscapeAdd => "splitscreen_landscape_add",
            Self::SplitscreenLeft => "splitscreen_left",
            Self::SplitscreenPortrait => "splitscreen_portrait",
            Self::SplitscreenRight => "splitscreen_right",
            Self::SplitscreenTop => "splitscreen_top",
            Self::SplitscreenVerticalAdd => "splitscreen_vertical_add",
            Self::Spo2 => "spo2",
            Self::Spoke => "spoke",
            Self::Sports => "sports",
            Self::SportsAndOutdoors => "sports_and_outdoors",
            Self::SportsBar => "sports_bar",
            Self::SportsBaseball => "sports_baseball",
            Self::SportsBasketball => "sports_basketball",
            Self::SportsCricket => "sports_cricket",
            Self::SportsEsports => "sports_esports",
            Self::SportsFootball => "sports_football",
            Self::SportsGolf => "sports_golf",
            Self::SportsGymnastics => "sports_gymnastics",
            Self::SportsHandball => "sports_handball",
            Self::SportsHockey => "sports_hockey",
            Self::SportsKabaddi => "sports_kabaddi",
            Self::SportsMartialArts => "sports_martial_arts",
            Self::SportsMma => "sports_mma",
            Self::SportsMotorsports => "sports_motorsports",
            Self::SportsRugby => "sports_rugby",
            Self::SportsScore => "sports_score",
            Self::SportsSoccer => "sports_soccer",
            Self::SportsTennis => "sports_tennis",
            Self::SportsVolleyball => "sports_volleyball",
            Self::Sprinkler => "sprinkler",
            Self::Sprint => "sprint",
            Self::Sql => "sql",
            Self::Square => "square",
            Self::SquareCircle => "square_circle",
            Self::SquareDot => "square_dot",
            Self::SquareFoot => "square_foot",
            Self::SsidChart => "ssid_chart",
            Self::Stack => "stack",
            Self::StackGroup => "stack_group",
            Self::StackHexagon => "stack_hexagon",
            Self::StackOff => "stack_off",
            Self::StackStar => "stack_star",
            Self::StackedBarChart => "stacked_bar_chart",
            Self::StackedEmail => "stacked_email",
            Self::StackedInbox => "stacked_inbox",
            Self::StackedLineChart => "stacked_line_chart",
            Self::Stacks => "stacks",
            Self::StadiaController => "stadia_controller",
            Self::Stadium => "stadium",
            Self::Stairs => "stairs",
            Self::Stairs2 => "stairs_2",
            Self::StarBorder => "star_border",
            Self::StarBorderPurple500 => "star_border_purple500",
            Self::StarHalf => "star_half",
            Self::StarOutline => "star_outline",
            Self::StarPurple500 => "star_purple500",
            Self::StarRate => "star_rate",
            Self::StarRateHalf => "star_rate_half",
            Self::StarShine => "star_shine",
            Self::Stars => "stars",
            Self::Stars2 => "stars_2",
            Self::Start => "start",
            Self::Stat0 => "stat_0",
            Self::Stat1 => "stat_1",
            Self::Stat2 => "stat_2",
            Self::Stat3 => "stat_3",
            Self::StatMinus1 => "stat_minus_1",
            Self::StatMinus2 => "stat_minus_2",
            Self::StatMinus3 => "stat_minus_3",
            Self::StayCurrentLandscape => "stay_current_landscape",
            Self::StayCurrentPortrait => "stay_current_portrait",
            Self::StayPrimaryLandscape => "stay_primary_landscape",
            Self::StayPrimaryPortrait => "stay_primary_portrait",
            Self::SteeringWheelCool => "steering_wheel_cool",
            Self::SteeringWheelHeat => "steering_wheel_heat",
            Self::Step => "step",
            Self::StepInto => "step_into",
            Self::StepOut => "step_out",
            Self::StepOver => "step_over",
            Self::Steppers => "steppers",
            Self::Steps => "steps",
            Self::Stethoscope => "stethoscope",
            Self::StethoscopeArrow => "stethoscope_arrow",
            Self::StethoscopeCheck => "stethoscope_check",
            Self::Sticker => "sticker",
            Self::StickerAdd => "sticker_add",
            Self::StickyNote => "sticky_note",
            Self::StickyNote2 => "sticky_note_2",
            Self::StockMedia => "stock_media",
            Self::Stockpot => "stockpot",
            Self::StopCircle => "stop_circle",
            Self::StopScreenShare => "stop_screen_share",
            Self::Storage => "storage",
            Self::Store => "store",
            Self::StoreMallDirectory => "store_mall_directory",
            Self::Storefront => "storefront",
            Self::Storm => "storm",
            Self::Straight => "straight",
            Self::Straighten => "straighten",
            Self::Strategy => "strategy",
            Self::Stream => "stream",
            Self::StreamApps => "stream_apps",
            Self::Streetview => "streetview",
            Self::StressManagement => "stress_management",
            Self::StrikethroughS => "strikethrough_s",
            Self::StrokeFull => "stroke_full",
            Self::StrokePartial => "stroke_partial",
            Self::Stroller => "stroller",
            Self::Style => "style",
            Self::Styler => "styler",
            Self::Stylus => "stylus",
            Self::StylusBrush => "stylus_brush",
            Self::StylusFountainPen => "stylus_fountain_pen",
            Self::StylusHighlighter => "stylus_highlighter",
            Self::StylusLaserPointer => "stylus_laser_pointer",
            Self::StylusNote => "stylus_note",
            Self::StylusPen => "stylus_pen",
            Self::StylusPencil => "stylus_pencil",
            Self::SubdirectoryArrowLeft => "subdirectory_arrow_left",
            Self::SubdirectoryArrowRight => "subdirectory_arrow_right",
            Self::Subheader => "subheader",
            Self::Subject => "subject",
            Self::Subscript => "subscript",
            Self::Subscriptions => "subscriptions",
            Self::Subtitles => "subtitles",
            Self::SubtitlesGear => "subtitles_gear",
            Self::SubtitlesOff => "subtitles_off",
            Self::Subway => "subway",
            Self::SubwayWalk => "subway_walk",
            Self::Subwoofer => "subwoofer",
            Self::Summarize => "summarize",
            Self::Sunny => "sunny",
            Self::SunnySnowing => "sunny_snowing",
            Self::Superscript => "superscript",
            Self::SupervisedUserCircle => "supervised_user_circle",
            Self::SupervisedUserCircleOff => "supervised_user_circle_off",
            Self::SupervisorAccount => "supervisor_account",
            Self::Support => "support",
            Self::SupportAgent => "support_agent",
            Self::Surfing => "surfing",
            Self::Surgical => "surgical",
            Self::SurroundSound => "surround_sound",
            Self::SwapCalls => "swap_calls",
            Self::SwapDrivingApps => "swap_driving_apps",
            Self::SwapDrivingAppsWheel => "swap_driving_apps_wheel",
            Self::SwapHoriz => "swap_horiz",
            Self::SwapHorizontalCircle => "swap_horizontal_circle",
            Self::SwapVert => "swap_vert",
            Self::SwapVerticalCircle => "swap_vertical_circle",
            Self::Sweep => "sweep",
            Self::Swipe => "swipe",
            Self::SwipeDown => "swipe_down",
            Self::SwipeDownAlt => "swipe_down_alt",
            Self::SwipeLeft => "swipe_left",
            Self::SwipeLeft2 => "swipe_left_2",
            Self::SwipeLeftAlt => "swipe_left_alt",
            Self::SwipeRight => "swipe_right",
            Self::SwipeRight2 => "swipe_right_2",
            Self::SwipeRightAlt => "swipe_right_alt",
            Self::SwipeUp => "swipe_up",
            Self::SwipeUpAlt => "swipe_up_alt",
            Self::SwipeVertical => "swipe_vertical",
            Self::Switch => "switch",
            Self::SwitchAccess => "switch_access",
            Self::SwitchAccess2 => "switch_access_2",
            Self::SwitchAccess3 => "switch_access_3",
            Self::SwitchAccessShortcut => "switch_access_shortcut",
            Self::SwitchAccessShortcutAdd => "switch_access_shortcut_add",
            Self::SwitchAccount => "switch_account",
            Self::SwitchCamera => "switch_camera",
            Self::SwitchLeft => "switch_left",
            Self::SwitchOff => "switch_off",
            Self::SwitchRight => "switch_right",
            Self::SwitchVideo => "switch_video",
            Self::Switches => "switches",
            Self::SwordRose => "sword_rose",
            Self::Swords => "swords",
            Self::Symptoms => "symptoms",
            Self::Synagogue => "synagogue",
            Self::Sync => "sync",
            Self::SyncAlt => "sync_alt",
            Self::SyncArrowDown => "sync_arrow_down",
            Self::SyncArrowUp => "sync_arrow_up",
            Self::SyncDesktop => "sync_desktop",
            Self::SyncDisabled => "sync_disabled",
            Self::SyncLock => "sync_lock",
            Self::SyncProblem => "sync_problem",
            Self::SyncSavedLocally => "sync_saved_locally",
            Self::SyncSavedLocallyOff => "sync_saved_locally_off",
            Self::Syringe => "syringe",
            Self::SystemSecurityUpdate => "system_security_update",
            Self::SystemSecurityUpdateGood => "system_security_update_good",
            Self::SystemSecurityUpdateWarning => "system_security_update_warning",
            Self::SystemUpdate => "system_update",
            Self::SystemUpdateAlt => "system_update_alt",
            Self::Tab => "tab",
            Self::TabClose => "tab_close",
            Self::TabCloseInactive => "tab_close_inactive",
            Self::TabCloseRight => "tab_close_right",
            Self::TabDuplicate => "tab_duplicate",
            Self::TabGroup => "tab_group",
            Self::TabInactive => "tab_inactive",
            Self::TabMove => "tab_move",
            Self::TabNewRight => "tab_new_right",
            Self::TabRecent => "tab_recent",
            Self::TabSearch => "tab_search",
            Self::TabUnselected => "tab_unselected",
            Self::TableBar => "table_bar",
            Self::TableChart => "table_chart",
            Self::TableChartView => "table_chart_view",
            Self::TableConvert => "table_convert",
            Self::TableEdit => "table_edit",
            Self::TableEye => "table_eye",
            Self::TableLamp => "table_lamp",
            Self::TableLarge => "table_large",
            Self::TableRestaurant => "table_restaurant",
            Self::TableRows => "table_rows",
            Self::TableRowsNarrow => "table_rows_narrow",
            Self::TableSign => "table_sign",
            Self::TableView => "table_view",
            Self::Tablet => "tablet",
            Self::TabletAndroid => "tablet_android",
            Self::TabletCamera => "tablet_camera",
            Self::TabletMac => "tablet_mac",
            Self::Tabs => "tabs",
            Self::Tactic => "tactic",
            Self::Tag => "tag",
            Self::TagFaces => "tag_faces",
            Self::TakeoutDining => "takeout_dining",
            Self::TakeoutDining2 => "takeout_dining_2",
            Self::TamperDetectionOff => "tamper_detection_off",
            Self::TamperDetectionOn => "tamper_detection_on",
            Self::TapAndPlay => "tap_and_play",
            Self::Tapas => "tapas",
            Self::Target => "target",
            Self::TargetCheck => "target_check",
            Self::Task => "task",
            Self::TaskAlt => "task_alt",
            Self::TatamiSeat => "tatami_seat",
            Self::Taunt => "taunt",
            Self::TaxiAlert => "taxi_alert",
            Self::TeamDashboard => "team_dashboard",
            Self::TempPreferencesCustom => "temp_preferences_custom",
            Self::TempPreferencesEco => "temp_preferences_eco",
            Self::TempleBuddhist => "temple_buddhist",
            Self::TempleHindu => "temple_hindu",
            Self::Tenancy => "tenancy",
            Self::Terminal => "terminal",
            Self::Terminal2 => "terminal_2",
            Self::TerminalAdd => "terminal_add",
            Self::Terrain => "terrain",
            Self::TextAd => "text_ad",
            Self::TextAdOff => "text_ad_off",
            Self::TextCompare => "text_compare",
            Self::TextDecrease => "text_decrease",
            Self::TextFields => "text_fields",
            Self::TextFieldsAlt => "text_fields_alt",
            Self::TextFormat => "text_format",
            Self::TextIncrease => "text_increase",
            Self::TextRotateUp => "text_rotate_up",
            Self::TextRotateVertical => "text_rotate_vertical",
            Self::TextRotationAngledown => "text_rotation_angledown",
            Self::TextRotationAngleup => "text_rotation_angleup",
            Self::TextRotationDown => "text_rotation_down",
            Self::TextRotationNone => "text_rotation_none",
            Self::TextSelectEnd => "text_select_end",
            Self::TextSelectJumpToBeginning => "text_select_jump_to_beginning",
            Self::TextSelectJumpToEnd => "text_select_jump_to_end",
            Self::TextSelectMoveBackCharacter => "text_select_move_back_character",
            Self::TextSelectMoveBackWord => "text_select_move_back_word",
            Self::TextSelectMoveDown => "text_select_move_down",
            Self::TextSelectMoveForwardCharacter => "text_select_move_forward_character",
            Self::TextSelectMoveForwardWord => "text_select_move_forward_word",
            Self::TextSelectMoveUp => "text_select_move_up",
            Self::TextSelectStart => "text_select_start",
            Self::TextSnippet => "text_snippet",
            Self::TextToSpeech => "text_to_speech",
            Self::TextUp => "text_up",
            Self::Textsms => "textsms",
            Self::Texture => "texture",
            Self::TextureAdd => "texture_add",
            Self::TextureMinus => "texture_minus",
            Self::TheaterComedy => "theater_comedy",
            Self::Theaters => "theaters",
            Self::Thermometer => "thermometer",
            Self::ThermometerAdd => "thermometer_add",
            Self::ThermometerAlert => "thermometer_alert",
            Self::ThermometerGain => "thermometer_gain",
            Self::ThermometerLoss => "thermometer_loss",
            Self::ThermometerMinus => "thermometer_minus",
            Self::Thermostat => "thermostat",
            Self::ThermostatArrowDown => "thermostat_arrow_down",
            Self::ThermostatArrowUp => "thermostat_arrow_up",
            Self::ThermostatAuto => "thermostat_auto",
            Self::ThermostatCarbon => "thermostat_carbon",
            Self::ThingsToDo => "things_to_do",
            Self::ThreadUnread => "thread_unread",
            Self::ThreatIntelligence => "threat_intelligence",
            Self::ThumbDown => "thumb_down",
            Self::ThumbDownAlt => "thumb_down_alt",
            Self::ThumbDownFilled => "thumb_down_filled",
            Self::ThumbDownOff => "thumb_down_off",
            Self::ThumbDownOffAlt => "thumb_down_off_alt",
            Self::ThumbUp => "thumb_up",
            Self::ThumbUpAlt => "thumb_up_alt",
            Self::ThumbUpFilled => "thumb_up_filled",
            Self::ThumbUpOff => "thumb_up_off",
            Self::ThumbUpOffAlt => "thumb_up_off_alt",
            Self::ThumbnailBar => "thumbnail_bar",
            Self::ThumbsUpDouble => "thumbs_up_double",
            Self::ThumbsUpDown => "thumbs_up_down",
            Self::Thunderstorm => "thunderstorm",
            Self::Tibia => "tibia",
            Self::TibiaAlt => "tibia_alt",
            Self::TileLarge => "tile_large",
            Self::TileMedium => "tile_medium",
            Self::TileSmall => "tile_small",
            Self::TiltArrowDown => "tilt_arrow_down",
            Self::TiltArrowUp => "tilt_arrow_up",
            Self::TimeAuto => "time_auto",
            Self::TimeToLeave => "time_to_leave",
            Self::Timelapse => "timelapse",
            Self::Timeline => "timeline",
            Self::Timer => "timer",
            Self::Timer1 => "timer_1",
            Self::Timer10 => "timer_10",
            Self::Timer10Alt1 => "timer_10_alt_1",
            Self::Timer10Select => "timer_10_select",
            Self::Timer2 => "timer_2",
            Self::Timer3 => "timer_3",
            Self::Timer3Alt1 => "timer_3_alt_1",
            Self::Timer3Select => "timer_3_select",
            Self::Timer5 => "timer_5",
            Self::Timer5Shutter => "timer_5_shutter",
            Self::TimerArrowDown => "timer_arrow_down",
            Self::TimerArrowUp => "timer_arrow_up",
            Self::TimerOff => "timer_off",
            Self::TimerPause => "timer_pause",
            Self::TimerPlay => "timer_play",
            Self::TipsAndUpdates => "tips_and_updates",
            Self::TireRepair => "tire_repair",
            Self::Title => "title",
            Self::Titlecase => "titlecase",
            Self::Toast => "toast",
            Self::Toc => "toc",
            Self::Today => "today",
            Self::ToggleOff => "toggle_off",
            Self::ToggleOn => "toggle_on",
            Self::Token => "token",
            Self::Toll => "toll",
            Self::Tonality => "tonality",
            Self::Tonality2 => "tonality_2",
            Self::Toolbar => "toolbar",
            Self::ToolsFlatHead => "tools_flat_head",
            Self::ToolsInstallationKit => "tools_installation_kit",
            Self::ToolsLadder => "tools_ladder",
            Self::ToolsLevel => "tools_level",
            Self::ToolsPhillips => "tools_phillips",
            Self::ToolsPliersWireStripper => "tools_pliers_wire_stripper",
            Self::ToolsPowerDrill => "tools_power_drill",
            Self::ToolsWrench => "tools_wrench",
            Self::Tooltip => "tooltip",
            Self::Tooltip2 => "tooltip_2",
            Self::TopPanelClose => "top_panel_close",
            Self::TopPanelOpen => "top_panel_open",
            Self::Topic => "topic",
            Self::Tornado => "tornado",
            Self::TotalDissolvedSolids => "total_dissolved_solids",
            Self::TouchApp => "touch_app",
            Self::TouchDouble => "touch_double",
            Self::TouchDouble2 => "touch_double_2",
            Self::TouchLong => "touch_long",
            Self::TouchTriple => "touch_triple",
            Self::TouchpadMouse => "touchpad_mouse",
            Self::TouchpadMouseOff => "touchpad_mouse_off",
            Self::Tour => "tour",
            Self::Toys => "toys",
            Self::ToysAndGames => "toys_and_games",
            Self::ToysFan => "toys_fan",
            Self::TrackChanges => "track_changes",
            Self::TrackpadInput => "trackpad_input",
            Self::TrackpadInput2 => "trackpad_input_2",
            Self::TrackpadInput3 => "trackpad_input_3",
            Self::Traffic => "traffic",
            Self::TrafficJam => "traffic_jam",
            Self::TrailLength => "trail_length",
            Self::TrailLengthMedium => "trail_length_medium",
            Self::TrailLengthShort => "trail_length_short",
            Self::Train => "train",
            Self::Tram => "tram",
            Self::Transcribe => "transcribe",
            Self::TransferWithinAStation => "transfer_within_a_station",
            Self::Transform => "transform",
            Self::Transgender => "transgender",
            Self::TransitEnterexit => "transit_enterexit",
            Self::TransitTicket => "transit_ticket",
            Self::TransitionChop => "transition_chop",
            Self::TransitionDissolve => "transition_dissolve",
            Self::TransitionFade => "transition_fade",
            Self::TransitionPush => "transition_push",
            Self::TransitionSlide => "transition_slide",
            Self::Translate => "translate",
            Self::TranslateIndic => "translate_indic",
            Self::Transportation => "transportation",
            Self::Travel => "travel",
            Self::TravelExplore => "travel_explore",
            Self::TravelLuggageAndBags => "travel_luggage_and_bags",
            Self::TrendingDown => "trending_down",
            Self::TrendingFlat => "trending_flat",
            Self::TrendingUp => "trending_up",
            Self::TriangleCircle => "triangle_circle",
            Self::Trip => "trip",
            Self::TripOrigin => "trip_origin",
            Self::Trolley => "trolley",
            Self::TrolleyCableCar => "trolley_cable_car",
            Self::Trophy => "trophy",
            Self::Troubleshoot => "troubleshoot",
            Self::Try => "try",
            Self::Tsunami => "tsunami",
            Self::Tsv => "tsv",
            Self::Tty => "tty",
            Self::Tune => "tune",
            Self::Tungsten => "tungsten",
            Self::TurnLeft => "turn_left",
            Self::TurnRight => "turn_right",
            Self::TurnSharpLeft => "turn_sharp_left",
            Self::TurnSharpRight => "turn_sharp_right",
            Self::TurnSlightLeft => "turn_slight_left",
            Self::TurnSlightRight => "turn_slight_right",
            Self::TurnedIn => "turned_in",
            Self::TurnedInNot => "turned_in_not",
            Self::Tv => "tv",
            Self::TvDisplays => "tv_displays",
            Self::TvGen => "tv_gen",
            Self::TvGuide => "tv_guide",
            Self::TvNext => "tv_next",
            Self::TvOff => "tv_off",
            Self::TvOptionsEditChannels => "tv_options_edit_channels",
            Self::TvOptionsInputSettings => "tv_options_input_settings",
            Self::TvRemote => "tv_remote",
            Self::TvSignin => "tv_signin",
            Self::TvWithAssistant => "tv_with_assistant",
            Self::TwoPager => "two_pager",
            Self::TwoPagerStore => "two_pager_store",
            Self::TwoWheeler => "two_wheeler",
            Self::TypeSpecimen => "type_specimen",
            Self::UTurnLeft => "u_turn_left",
            Self::UTurnRight => "u_turn_right",
            Self::Udon => "udon",
            Self::UlnaRadius => "ulna_radius",
            Self::UlnaRadiusAlt => "ulna_radius_alt",
            Self::Umbrella => "umbrella",
            Self::Unarchive => "unarchive",
            Self::Undereye => "undereye",
            Self::UnfoldLess => "unfold_less",
            Self::UnfoldLessDouble => "unfold_less_double",
            Self::UnfoldMore => "unfold_more",
            Self::UnfoldMoreDouble => "unfold_more_double",
            Self::Ungroup => "ungroup",
            Self::UniversalCurrency => "universal_currency",
            Self::UniversalCurrencyAlt => "universal_currency_alt",
            Self::UniversalLocal => "universal_local",
            Self::Unknown2 => "unknown_2",
            Self::Unknown5 => "unknown_5",
            Self::Unknown7 => "unknown_7",
            Self::UnknownDocument => "unknown_document",
            Self::UnknownMed => "unknown_med",
            Self::Unlicense => "unlicense",
            Self::UnpavedRoad => "unpaved_road",
            Self::Unpin => "unpin",
            Self::Unpublished => "unpublished",
            Self::Unsubscribe => "unsubscribe",
            Self::Upcoming => "upcoming",
            Self::Update => "update",
            Self::UpdateDisabled => "update_disabled",
            Self::Upgrade => "upgrade",
            Self::UpiPay => "upi_pay",
            Self::Upload2 => "upload_2",
            Self::UploadFile => "upload_file",
            Self::Uppercase => "uppercase",
            Self::Urology => "urology",
            Self::Usb => "usb",
            Self::UsbOff => "usb_off",
            Self::UserAttributes => "user_attributes",
            Self::Vaccines => "vaccines",
            Self::Vacuum => "vacuum",
            Self::Vacuum2 => "vacuum_2",
            Self::Vacuum2On => "vacuum_2_on",
            Self::Valve => "valve",
            Self::VapeFree => "vape_free",
            Self::VapingRooms => "vaping_rooms",
            Self::VariableAdd => "variable_add",
            Self::VariableInsert => "variable_insert",
            Self::VariableRemove => "variable_remove",
            Self::Variables => "variables",
            Self::Ventilator => "ventilator",
            Self::Verified => "verified",
            Self::VerifiedOff => "verified_off",
            Self::VerifiedUser => "verified_user",
            Self::VerticalAlignBottom => "vertical_align_bottom",
            Self::VerticalAlignCenter => "vertical_align_center",
            Self::VerticalAlignTop => "vertical_align_top",
            Self::VerticalDistribute => "vertical_distribute",
            Self::VerticalShades => "vertical_shades",
            Self::VerticalShadesClosed => "vertical_shades_closed",
            Self::VerticalSplit => "vertical_split",
            Self::Vibration => "vibration",
            Self::VideoCall => "video_call",
            Self::VideoCameraBack => "video_camera_back",
            Self::VideoCameraBackAdd => "video_camera_back_add",
            Self::VideoCameraFront => "video_camera_front",
            Self::VideoCameraFrontOff => "video_camera_front_off",
            Self::VideoChat => "video_chat",
            Self::VideoFile => "video_file",
            Self::VideoFrameCopy => "video_frame_copy",
            Self::VideoFrameSave => "video_frame_save",
            Self::VideoLabel => "video_label",
            Self::VideoLibrary => "video_library",
            Self::VideoSearch => "video_search",
            Self::VideoSettings => "video_settings",
            Self::VideoStable => "video_stable",
            Self::VideoTemplate => "video_template",
            Self::Videocam => "videocam",
            Self::VideocamAlert => "videocam_alert",
            Self::VideocamOff => "videocam_off",
            Self::VideogameAsset => "videogame_asset",
            Self::VideogameAssetOff => "videogame_asset_off",
            Self::ViewAgenda => "view_agenda",
            Self::ViewApps => "view_apps",
            Self::ViewArray => "view_array",
            Self::ViewCarousel => "view_carousel",
            Self::ViewColumn => "view_column",
            Self::ViewColumn2 => "view_column_2",
            Self::ViewComfy => "view_comfy",
            Self::ViewComfyAlt => "view_comfy_alt",
            Self::ViewCompact => "view_compact",
            Self::ViewCompactAlt => "view_compact_alt",
            Self::ViewCozy => "view_cozy",
            Self::ViewDay => "view_day",
            Self::ViewHeadline => "view_headline",
            Self::ViewInAr => "view_in_ar",
            Self::ViewInArNew => "view_in_ar_new",
            Self::ViewInArOff => "view_in_ar_off",
            Self::ViewKanban => "view_kanban",
            Self::ViewList => "view_list",
            Self::ViewModule => "view_module",
            Self::ViewObjectTrack => "view_object_track",
            Self::ViewQuilt => "view_quilt",
            Self::ViewRealSize => "view_real_size",
            Self::ViewSidebar => "view_sidebar",
            Self::ViewStream => "view_stream",
            Self::ViewTimeline => "view_timeline",
            Self::ViewWeek => "view_week",
            Self::Vignette => "vignette",
            Self::Vignette2 => "vignette_2",
            Self::Villa => "villa",
            Self::Visibility => "visibility",
            Self::VisibilityLock => "visibility_lock",
            Self::VisibilityOff => "visibility_off",
            Self::VitalSigns => "vital_signs",
            Self::Vitals => "vitals",
            Self::Vo2Max => "vo2_max",
            Self::VoiceChat => "voice_chat",
            Self::VoiceChatOff => "voice_chat_off",
            Self::VoiceOverOff => "voice_over_off",
            Self::VoiceSelection => "voice_selection",
            Self::VoiceSelectionOff => "voice_selection_off",
            Self::Voicemail => "voicemail",
            Self::Voicemail2 => "voicemail_2",
            Self::Volcano => "volcano",
            Self::VolumeDown => "volume_down",
            Self::VolumeDownAlt => "volume_down_alt",
            Self::VolumeMute => "volume_mute",
            Self::VolunteerActivism => "volunteer_activism",
            Self::VotingChip => "voting_chip",
            Self::VpnKey => "vpn_key",
            Self::VpnKeyAlert => "vpn_key_alert",
            Self::VpnKeyOff => "vpn_key_off",
            Self::VpnLock => "vpn_lock",
            Self::VpnLock2 => "vpn_lock_2",
            Self::Vr180Create2d => "vr180_create2d",
            Self::Vr180Create2dOff => "vr180_create2d_off",
            Self::Vrpano => "vrpano",
            Self::WalkBike => "walk_bike",
            Self::WallArt => "wall_art",
            Self::WallLamp => "wall_lamp",
            Self::Wallet => "wallet",
            Self::Wallpaper => "wallpaper",
            Self::WallpaperSlideshow => "wallpaper_slideshow",
            Self::WandShine => "wand_shine",
            Self::WandStars => "wand_stars",
            Self::Ward => "ward",
            Self::Warehouse => "warehouse",
            Self::WarningAmber => "warning_amber",
            Self::WarningOff => "warning_off",
            Self::Wash => "wash",
            Self::Washoku => "washoku",
            Self::Watch => "watch",
            Self::WatchAlert => "watch_alert",
            Self::WatchArrow => "watch_arrow",
            Self::WatchArrowDown => "watch_arrow_down",
            Self::WatchButton => "watch_button",
            Self::WatchButtonPress => "watch_button_press",
            Self::WatchCheck => "watch_check",
            Self::WatchLater => "watch_later",
            Self::WatchLock => "watch_lock",
            Self::WatchOff => "watch_off",
            Self::WatchScreentime => "watch_screentime",
            Self::WatchVibration => "watch_vibration",
            Self::WatchWake => "watch_wake",
            Self::Water => "water",
            Self::WaterBottle => "water_bottle",
            Self::WaterBottleLarge => "water_bottle_large",
            Self::WaterDamage => "water_damage",
            Self::WaterDo => "water_do",
            Self::WaterDrop => "water_drop",
            Self::WaterDrops => "water_drops",
            Self::WaterEc => "water_ec",
            Self::WaterFull => "water_full",
            Self::WaterHeater => "water_heater",
            Self::WaterLock => "water_lock",
            Self::WaterLoss => "water_loss",
            Self::WaterLux => "water_lux",
            Self::WaterMedium => "water_medium",
            Self::WaterOrp => "water_orp",
            Self::WaterPh => "water_ph",
            Self::WaterPump => "water_pump",
            Self::WaterVoc => "water_voc",
            Self::WaterfallChart => "waterfall_chart",
            Self::Waves => "waves",
            Self::WavingHand => "waving_hand",
            Self::WbAuto => "wb_auto",
            Self::WbCloudy => "wb_cloudy",
            Self::WbIncandescent => "wb_incandescent",
            Self::WbIridescent => "wb_iridescent",
            Self::WbShade => "wb_shade",
            Self::WbSunny => "wb_sunny",
            Self::WbTwilight => "wb_twilight",
            Self::WbTwilight2 => "wb_twilight_2",
            Self::Wc => "wc",
            Self::WeatherHail => "weather_hail",
            Self::WeatherMix => "weather_mix",
            Self::WeatherSnowy => "weather_snowy",
            Self::Web => "web",
            Self::WebAsset => "web_asset",
            Self::WebAssetOff => "web_asset_off",
            Self::WebStories => "web_stories",
            Self::WebTraffic => "web_traffic",
            Self::Webhook => "webhook",
            Self::Weekend => "weekend",
            Self::Weight => "weight",
            Self::West => "west",
            Self::Whatshot => "whatshot",
            Self::Wheat => "wheat",
            Self::WheelchairPickup => "wheelchair_pickup",
            Self::WhereToVote => "where_to_vote",
            Self::WidgetMedium => "widget_medium",
            Self::WidgetMenu => "widget_menu",
            Self::WidgetSmall => "widget_small",
            Self::WidgetWidth => "widget_width",
            Self::Widgets => "widgets",
            Self::Width => "width",
            Self::WidthFull => "width_full",
            Self::WidthNormal => "width_normal",
            Self::WidthWide => "width_wide",
            Self::Wifi => "wifi",
            Self::Wifi1Bar => "wifi_1_bar",
            Self::Wifi2Bar => "wifi_2_bar",
            Self::WifiAdd => "wifi_add",
            Self::WifiCalling => "wifi_calling",
            Self::WifiCalling1 => "wifi_calling_1",
            Self::WifiCalling2 => "wifi_calling_2",
            Self::WifiCalling3 => "wifi_calling_3",
            Self::WifiCallingBar1 => "wifi_calling_bar_1",
            Self::WifiCallingBar2 => "wifi_calling_bar_2",
            Self::WifiCallingBar3 => "wifi_calling_bar_3",
            Self::WifiChannel => "wifi_channel",
            Self::WifiDevice => "wifi_device",
            Self::WifiFind => "wifi_find",
            Self::WifiHome => "wifi_home",
            Self::WifiLock => "wifi_lock",
            Self::WifiNotification => "wifi_notification",
            Self::WifiOff => "wifi_off",
            Self::WifiPassword => "wifi_password",
            Self::WifiProtectedSetup => "wifi_protected_setup",
            Self::WifiProxy => "wifi_proxy",
            Self::WifiTethering => "wifi_tethering",
            Self::WifiTetheringError => "wifi_tethering_error",
            Self::WifiTetheringOff => "wifi_tethering_off",
            Self::WindPower => "wind_power",
            Self::Window => "window",
            Self::WindowClosed => "window_closed",
            Self::WindowOpen => "window_open",
            Self::WindowSensor => "window_sensor",
            Self::WindshieldDefrostAuto => "windshield_defrost_auto",
            Self::WindshieldDefrostFront => "windshield_defrost_front",
            Self::WindshieldDefrostRear => "windshield_defrost_rear",
            Self::WindshieldHeatFront => "windshield_heat_front",
            Self::WineBar => "wine_bar",
            Self::Woman => "woman",
            Self::Woman2 => "woman_2",
            Self::Work => "work",
            Self::WorkAlert => "work_alert",
            Self::WorkHistory => "work_history",
            Self::WorkOff => "work_off",
            Self::WorkOutline => "work_outline",
            Self::WorkUpdate => "work_update",
            Self::Workflow => "workflow",
            Self::WorkspacePremium => "workspace_premium",
            Self::Workspaces => "workspaces",
            Self::WorkspacesOutline => "workspaces_outline",
            Self::WoundsInjuries => "wounds_injuries",
            Self::WrapText => "wrap_text",
            Self::Wrist => "wrist",
            Self::WrongLocation => "wrong_location",
            Self::Wysiwyg => "wysiwyg",
            Self::XCircle => "x_circle",
            Self::YCircle => "y_circle",
            Self::Yakitori => "yakitori",
            Self::Yard => "yard",
            Self::Yoshoku => "yoshoku",
            Self::YourTrips => "your_trips",
            Self::YoutubeActivity => "youtube_activity",
            Self::YoutubeSearchedFor => "youtube_searched_for",
            Self::ZonePersonAlert => "zone_person_alert",
            Self::ZonePersonIdle => "zone_person_idle",
            Self::ZonePersonUrgent => "zone_person_urgent",
            Self::ZoomIn => "zoom_in",
            Self::ZoomInMap => "zoom_in_map",
            Self::ZoomOut => "zoom_out",
            Self::ZoomOutMap => "zoom_out_map",
        }
    }

    /// Parses a token back to its variant, or `None` when it is not one.
    ///
    /// The match is exact and case-sensitive: only the tokens produced by
    /// [`IconName::as_str`] are accepted, so `"ArrowLeft"` and `"arrow left"` both return
    /// `None`. Use [`Icon::set_icon`] when an unrecognised name should fall back to a
    /// placeholder rather than being rejected.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "check" => Some(Self::Check),
            "cross" => Some(Self::Cross),
            "arrow_left" => Some(Self::ArrowLeft),
            "arrow_right" => Some(Self::ArrowRight),
            "arrow_up" => Some(Self::ArrowUp),
            "arrow_down" => Some(Self::ArrowDown),
            "star" => Some(Self::Star),
            "heart" => Some(Self::Heart),
            "settings" => Some(Self::Settings),
            "home" => Some(Self::Home),
            "search" => Some(Self::Search),
            "menu" => Some(Self::Menu),
            "close" => Some(Self::Close),
            "plus" => Some(Self::Plus),
            "minus" => Some(Self::Minus),
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            "user" => Some(Self::User),
            "mail" => Some(Self::Mail),
            "bell" => Some(Self::Bell),
            "edit" => Some(Self::Edit),
            "trash" => Some(Self::Trash),
            "share" => Some(Self::Share),
            "refresh" => Some(Self::Refresh),
            "more" => Some(Self::More),
            "filter" => Some(Self::Filter),
            "lock" => Some(Self::Lock),
            "unlock" => Some(Self::Unlock),
            "download" => Some(Self::Download),
            "upload" => Some(Self::Upload),
            "chevron_left" => Some(Self::ChevronLeft),
            "chevron_right" => Some(Self::ChevronRight),
            "chevron_up" => Some(Self::ChevronUp),
            "chevron_down" => Some(Self::ChevronDown),
            "first_page" => Some(Self::FirstPage),
            "last_page" => Some(Self::LastPage),
            "folder" => Some(Self::Folder),
            "folder_open" => Some(Self::FolderOpen),
            "file" => Some(Self::File),
            "save" => Some(Self::Save),
            "copy" => Some(Self::Copy),
            "print" => Some(Self::Print),
            "undo" => Some(Self::Undo),
            "redo" => Some(Self::Redo),
            "cut" => Some(Self::Cut),
            "paste" => Some(Self::Paste),
            "attachment" => Some(Self::Attachment),
            "link" => Some(Self::Link),
            "success" => Some(Self::Success),
            "help" => Some(Self::Help),
            "block" => Some(Self::Block),
            "schedule" => Some(Self::Schedule),
            "hourglass" => Some(Self::Hourglass),
            "play" => Some(Self::Play),
            "pause" => Some(Self::Pause),
            "stop" => Some(Self::Stop),
            "skip_next" => Some(Self::SkipNext),
            "volume_up" => Some(Self::VolumeUp),
            "volume_off" => Some(Self::VolumeOff),
            "sort" => Some(Self::Sort),
            "bar_chart" => Some(Self::BarChart),
            "calendar" => Some(Self::Calendar),
            "table" => Some(Self::Table),
            "chat" => Some(Self::Chat),
            "call" => Some(Self::Call),
            "send" => Some(Self::Send),
            "notifications_off" => Some(Self::NotificationsOff),
            "fullscreen" => Some(Self::Fullscreen),
            "debug_bug" => Some(Self::DebugBug),
            "design_component" => Some(Self::DesignComponent),
            "design_event" => Some(Self::DesignEvent),
            "test_flask" => Some(Self::TestFlask),
            "tools_terminal" => Some(Self::ToolsTerminal),
            "tools_translate" => Some(Self::ToolsTranslate),
            "10k" => Some(Self::Num10k),
            "10mp" => Some(Self::Num10mp),
            "11mp" => Some(Self::Num11mp),
            "123" => Some(Self::Num123),
            "12mp" => Some(Self::Num12mp),
            "13mp" => Some(Self::Num13mp),
            "14mp" => Some(Self::Num14mp),
            "15mp" => Some(Self::Num15mp),
            "16mp" => Some(Self::Num16mp),
            "17mp" => Some(Self::Num17mp),
            "18_up_rating" => Some(Self::Num18UpRating),
            "18mp" => Some(Self::Num18mp),
            "19mp" => Some(Self::Num19mp),
            "1k" => Some(Self::Num1k),
            "1k_plus" => Some(Self::Num1kPlus),
            "1x_mobiledata" => Some(Self::Num1xMobiledata),
            "1x_mobiledata_badge" => Some(Self::Num1xMobiledataBadge),
            "20mp" => Some(Self::Num20mp),
            "21mp" => Some(Self::Num21mp),
            "22mp" => Some(Self::Num22mp),
            "23mp" => Some(Self::Num23mp),
            "24fps_select" => Some(Self::Num24fpsSelect),
            "24mp" => Some(Self::Num24mp),
            "2d" => Some(Self::Num2d),
            "2d_2" => Some(Self::Num2d2),
            "2k" => Some(Self::Num2k),
            "2k_plus" => Some(Self::Num2kPlus),
            "2mp" => Some(Self::Num2mp),
            "30fps" => Some(Self::Num30fps),
            "30fps_select" => Some(Self::Num30fpsSelect),
            "360" => Some(Self::Num360),
            "3d" => Some(Self::Num3d),
            "3d_2" => Some(Self::Num3d2),
            "3d_rotation" => Some(Self::Num3dRotation),
            "3g_mobiledata" => Some(Self::Num3gMobiledata),
            "3g_mobiledata_badge" => Some(Self::Num3gMobiledataBadge),
            "3k" => Some(Self::Num3k),
            "3k_plus" => Some(Self::Num3kPlus),
            "3mp" => Some(Self::Num3mp),
            "3p" => Some(Self::Num3p),
            "4g_mobiledata" => Some(Self::Num4gMobiledata),
            "4g_mobiledata_badge" => Some(Self::Num4gMobiledataBadge),
            "4g_plus_mobiledata" => Some(Self::Num4gPlusMobiledata),
            "4k" => Some(Self::Num4k),
            "4k_plus" => Some(Self::Num4kPlus),
            "4mp" => Some(Self::Num4mp),
            "50mp" => Some(Self::Num50mp),
            "5g" => Some(Self::Num5g),
            "5g_mobiledata_badge" => Some(Self::Num5gMobiledataBadge),
            "5k" => Some(Self::Num5k),
            "5k_plus" => Some(Self::Num5kPlus),
            "5mp" => Some(Self::Num5mp),
            "60fps" => Some(Self::Num60fps),
            "60fps_select" => Some(Self::Num60fpsSelect),
            "6_ft_apart" => Some(Self::Num6FtApart),
            "6k" => Some(Self::Num6k),
            "6k_plus" => Some(Self::Num6kPlus),
            "6mp" => Some(Self::Num6mp),
            "7k" => Some(Self::Num7k),
            "7k_plus" => Some(Self::Num7kPlus),
            "7mp" => Some(Self::Num7mp),
            "8k" => Some(Self::Num8k),
            "8k_plus" => Some(Self::Num8kPlus),
            "8mp" => Some(Self::Num8mp),
            "9k" => Some(Self::Num9k),
            "9k_plus" => Some(Self::Num9kPlus),
            "9mp" => Some(Self::Num9mp),
            "abc" => Some(Self::Abc),
            "ac_unit" => Some(Self::AcUnit),
            "access_alarm" => Some(Self::AccessAlarm),
            "access_alarms" => Some(Self::AccessAlarms),
            "access_time" => Some(Self::AccessTime),
            "access_time_filled" => Some(Self::AccessTimeFilled),
            "accessibility" => Some(Self::Accessibility),
            "accessibility_new" => Some(Self::AccessibilityNew),
            "accessible" => Some(Self::Accessible),
            "accessible_forward" => Some(Self::AccessibleForward),
            "accessible_menu" => Some(Self::AccessibleMenu),
            "account_balance" => Some(Self::AccountBalance),
            "account_balance_wallet" => Some(Self::AccountBalanceWallet),
            "account_box" => Some(Self::AccountBox),
            "account_child" => Some(Self::AccountChild),
            "account_child_invert" => Some(Self::AccountChildInvert),
            "account_circle" => Some(Self::AccountCircle),
            "account_circle_filled" => Some(Self::AccountCircleFilled),
            "account_circle_off" => Some(Self::AccountCircleOff),
            "account_tree" => Some(Self::AccountTree),
            "action_key" => Some(Self::ActionKey),
            "activity_zone" => Some(Self::ActivityZone),
            "acupuncture" => Some(Self::Acupuncture),
            "acute" => Some(Self::Acute),
            "ad" => Some(Self::Ad),
            "ad_group" => Some(Self::AdGroup),
            "ad_group_off" => Some(Self::AdGroupOff),
            "ad_off" => Some(Self::AdOff),
            "ad_units" => Some(Self::AdUnits),
            "adaptive_audio_mic" => Some(Self::AdaptiveAudioMic),
            "adaptive_audio_mic_off" => Some(Self::AdaptiveAudioMicOff),
            "adb" => Some(Self::Adb),
            "add" => Some(Self::Add),
            "add_2" => Some(Self::Add2),
            "add_a_photo" => Some(Self::AddAPhoto),
            "add_ad" => Some(Self::AddAd),
            "add_alarm" => Some(Self::AddAlarm),
            "add_alert" => Some(Self::AddAlert),
            "add_box" => Some(Self::AddBox),
            "add_business" => Some(Self::AddBusiness),
            "add_call" => Some(Self::AddCall),
            "add_card" => Some(Self::AddCard),
            "add_chart" => Some(Self::AddChart),
            "add_circle" => Some(Self::AddCircle),
            "add_circle_outline" => Some(Self::AddCircleOutline),
            "add_column_left" => Some(Self::AddColumnLeft),
            "add_column_right" => Some(Self::AddColumnRight),
            "add_comment" => Some(Self::AddComment),
            "add_diamond" => Some(Self::AddDiamond),
            "add_home" => Some(Self::AddHome),
            "add_home_work" => Some(Self::AddHomeWork),
            "add_ic_call" => Some(Self::AddIcCall),
            "add_link" => Some(Self::AddLink),
            "add_location" => Some(Self::AddLocation),
            "add_location_alt" => Some(Self::AddLocationAlt),
            "add_moderator" => Some(Self::AddModerator),
            "add_notes" => Some(Self::AddNotes),
            "add_photo_alternate" => Some(Self::AddPhotoAlternate),
            "add_reaction" => Some(Self::AddReaction),
            "add_road" => Some(Self::AddRoad),
            "add_row_above" => Some(Self::AddRowAbove),
            "add_row_below" => Some(Self::AddRowBelow),
            "add_shopping_cart" => Some(Self::AddShoppingCart),
            "add_task" => Some(Self::AddTask),
            "add_to_drive" => Some(Self::AddToDrive),
            "add_to_home_screen" => Some(Self::AddToHomeScreen),
            "add_to_photos" => Some(Self::AddToPhotos),
            "add_to_queue" => Some(Self::AddToQueue),
            "add_triangle" => Some(Self::AddTriangle),
            "addchart" => Some(Self::Addchart),
            "adf_scanner" => Some(Self::AdfScanner),
            "adjust" => Some(Self::Adjust),
            "admin_meds" => Some(Self::AdminMeds),
            "admin_panel_settings" => Some(Self::AdminPanelSettings),
            "ads_click" => Some(Self::AdsClick),
            "agender" => Some(Self::Agender),
            "agriculture" => Some(Self::Agriculture),
            "air" => Some(Self::Air),
            "air_freshener" => Some(Self::AirFreshener),
            "air_purifier" => Some(Self::AirPurifier),
            "air_purifier_gen" => Some(Self::AirPurifierGen),
            "airline_seat_flat" => Some(Self::AirlineSeatFlat),
            "airline_seat_flat_angled" => Some(Self::AirlineSeatFlatAngled),
            "airline_seat_individual_suite" => Some(Self::AirlineSeatIndividualSuite),
            "airline_seat_legroom_extra" => Some(Self::AirlineSeatLegroomExtra),
            "airline_seat_legroom_normal" => Some(Self::AirlineSeatLegroomNormal),
            "airline_seat_legroom_reduced" => Some(Self::AirlineSeatLegroomReduced),
            "airline_seat_recline_extra" => Some(Self::AirlineSeatReclineExtra),
            "airline_seat_recline_normal" => Some(Self::AirlineSeatReclineNormal),
            "airline_stops" => Some(Self::AirlineStops),
            "airlines" => Some(Self::Airlines),
            "airplane_ticket" => Some(Self::AirplaneTicket),
            "airplanemode_active" => Some(Self::AirplanemodeActive),
            "airplanemode_inactive" => Some(Self::AirplanemodeInactive),
            "airplay" => Some(Self::Airplay),
            "airport_shuttle" => Some(Self::AirportShuttle),
            "airware" => Some(Self::Airware),
            "airwave" => Some(Self::Airwave),
            "alarm" => Some(Self::Alarm),
            "alarm_add" => Some(Self::AlarmAdd),
            "alarm_off" => Some(Self::AlarmOff),
            "alarm_on" => Some(Self::AlarmOn),
            "alarm_pause" => Some(Self::AlarmPause),
            "alarm_smart_wake" => Some(Self::AlarmSmartWake),
            "album" => Some(Self::Album),
            "align_center" => Some(Self::AlignCenter),
            "align_end" => Some(Self::AlignEnd),
            "align_flex_center" => Some(Self::AlignFlexCenter),
            "align_flex_end" => Some(Self::AlignFlexEnd),
            "align_flex_start" => Some(Self::AlignFlexStart),
            "align_horizontal_center" => Some(Self::AlignHorizontalCenter),
            "align_horizontal_left" => Some(Self::AlignHorizontalLeft),
            "align_horizontal_right" => Some(Self::AlignHorizontalRight),
            "align_items_stretch" => Some(Self::AlignItemsStretch),
            "align_justify_center" => Some(Self::AlignJustifyCenter),
            "align_justify_flex_end" => Some(Self::AlignJustifyFlexEnd),
            "align_justify_flex_start" => Some(Self::AlignJustifyFlexStart),
            "align_justify_space_around" => Some(Self::AlignJustifySpaceAround),
            "align_justify_space_between" => Some(Self::AlignJustifySpaceBetween),
            "align_justify_space_even" => Some(Self::AlignJustifySpaceEven),
            "align_justify_stretch" => Some(Self::AlignJustifyStretch),
            "align_self_stretch" => Some(Self::AlignSelfStretch),
            "align_space_around" => Some(Self::AlignSpaceAround),
            "align_space_between" => Some(Self::AlignSpaceBetween),
            "align_space_even" => Some(Self::AlignSpaceEven),
            "align_start" => Some(Self::AlignStart),
            "align_stretch" => Some(Self::AlignStretch),
            "align_vertical_bottom" => Some(Self::AlignVerticalBottom),
            "align_vertical_center" => Some(Self::AlignVerticalCenter),
            "align_vertical_top" => Some(Self::AlignVerticalTop),
            "all_inbox" => Some(Self::AllInbox),
            "all_inclusive" => Some(Self::AllInclusive),
            "all_match" => Some(Self::AllMatch),
            "all_out" => Some(Self::AllOut),
            "allergies" => Some(Self::Allergies),
            "allergy" => Some(Self::Allergy),
            "alt_route" => Some(Self::AltRoute),
            "alternate_email" => Some(Self::AlternateEmail),
            "altitude" => Some(Self::Altitude),
            "ambient_screen" => Some(Self::AmbientScreen),
            "ambulance" => Some(Self::Ambulance),
            "amend" => Some(Self::Amend),
            "amp_stories" => Some(Self::AmpStories),
            "analytics" => Some(Self::Analytics),
            "anchor" => Some(Self::Anchor),
            "android" => Some(Self::Android),
            "android_cell_4_bar" => Some(Self::AndroidCell4Bar),
            "android_cell_4_bar_alert" => Some(Self::AndroidCell4BarAlert),
            "android_cell_4_bar_off" => Some(Self::AndroidCell4BarOff),
            "android_cell_4_bar_plus" => Some(Self::AndroidCell4BarPlus),
            "android_cell_5_bar" => Some(Self::AndroidCell5Bar),
            "android_cell_5_bar_alert" => Some(Self::AndroidCell5BarAlert),
            "android_cell_5_bar_off" => Some(Self::AndroidCell5BarOff),
            "android_cell_5_bar_plus" => Some(Self::AndroidCell5BarPlus),
            "android_cell_dual_4_bar" => Some(Self::AndroidCellDual4Bar),
            "android_cell_dual_4_bar_alert" => Some(Self::AndroidCellDual4BarAlert),
            "android_cell_dual_4_bar_plus" => Some(Self::AndroidCellDual4BarPlus),
            "android_cell_dual_5_bar" => Some(Self::AndroidCellDual5Bar),
            "android_cell_dual_5_bar_alert" => Some(Self::AndroidCellDual5BarAlert),
            "android_cell_dual_5_bar_plus" => Some(Self::AndroidCellDual5BarPlus),
            "android_wifi_3_bar" => Some(Self::AndroidWifi3Bar),
            "android_wifi_3_bar_alert" => Some(Self::AndroidWifi3BarAlert),
            "android_wifi_3_bar_lock" => Some(Self::AndroidWifi3BarLock),
            "android_wifi_3_bar_off" => Some(Self::AndroidWifi3BarOff),
            "android_wifi_3_bar_plus" => Some(Self::AndroidWifi3BarPlus),
            "android_wifi_3_bar_question" => Some(Self::AndroidWifi3BarQuestion),
            "android_wifi_4_bar" => Some(Self::AndroidWifi4Bar),
            "android_wifi_4_bar_alert" => Some(Self::AndroidWifi4BarAlert),
            "android_wifi_4_bar_lock" => Some(Self::AndroidWifi4BarLock),
            "android_wifi_4_bar_off" => Some(Self::AndroidWifi4BarOff),
            "android_wifi_4_bar_plus" => Some(Self::AndroidWifi4BarPlus),
            "android_wifi_4_bar_question" => Some(Self::AndroidWifi4BarQuestion),
            "animated_images" => Some(Self::AnimatedImages),
            "animation" => Some(Self::Animation),
            "announcement" => Some(Self::Announcement),
            "antigravity" => Some(Self::Antigravity),
            "aod" => Some(Self::Aod),
            "aod_tablet" => Some(Self::AodTablet),
            "aod_watch" => Some(Self::AodWatch),
            "apartment" => Some(Self::Apartment),
            "api" => Some(Self::Api),
            "apk_document" => Some(Self::ApkDocument),
            "apk_install" => Some(Self::ApkInstall),
            "app_badging" => Some(Self::AppBadging),
            "app_blocking" => Some(Self::AppBlocking),
            "app_promo" => Some(Self::AppPromo),
            "app_registration" => Some(Self::AppRegistration),
            "app_settings_alt" => Some(Self::AppSettingsAlt),
            "app_shortcut" => Some(Self::AppShortcut),
            "apparel" => Some(Self::Apparel),
            "approval" => Some(Self::Approval),
            "approval_delegation" => Some(Self::ApprovalDelegation),
            "approval_delegation_off" => Some(Self::ApprovalDelegationOff),
            "apps" => Some(Self::Apps),
            "apps_outage" => Some(Self::AppsOutage),
            "apps_plus" => Some(Self::AppsPlus),
            "aq" => Some(Self::Aq),
            "aq_indoor" => Some(Self::AqIndoor),
            "ar_on_you" => Some(Self::ArOnYou),
            "ar_stickers" => Some(Self::ArStickers),
            "architecture" => Some(Self::Architecture),
            "archive" => Some(Self::Archive),
            "area_chart" => Some(Self::AreaChart),
            "arming_countdown" => Some(Self::ArmingCountdown),
            "arrow_and_edge" => Some(Self::ArrowAndEdge),
            "arrow_back" => Some(Self::ArrowBack),
            "arrow_back_2" => Some(Self::ArrowBack2),
            "arrow_back_ios" => Some(Self::ArrowBackIos),
            "arrow_back_ios_new" => Some(Self::ArrowBackIosNew),
            "arrow_circle_down" => Some(Self::ArrowCircleDown),
            "arrow_circle_left" => Some(Self::ArrowCircleLeft),
            "arrow_circle_right" => Some(Self::ArrowCircleRight),
            "arrow_circle_up" => Some(Self::ArrowCircleUp),
            "arrow_cool_down" => Some(Self::ArrowCoolDown),
            "arrow_downward" => Some(Self::ArrowDownward),
            "arrow_downward_alt" => Some(Self::ArrowDownwardAlt),
            "arrow_drop_down" => Some(Self::ArrowDropDown),
            "arrow_drop_down_circle" => Some(Self::ArrowDropDownCircle),
            "arrow_drop_up" => Some(Self::ArrowDropUp),
            "arrow_forward" => Some(Self::ArrowForward),
            "arrow_forward_ios" => Some(Self::ArrowForwardIos),
            "arrow_insert" => Some(Self::ArrowInsert),
            "arrow_left_alt" => Some(Self::ArrowLeftAlt),
            "arrow_menu_close" => Some(Self::ArrowMenuClose),
            "arrow_menu_open" => Some(Self::ArrowMenuOpen),
            "arrow_or_edge" => Some(Self::ArrowOrEdge),
            "arrow_outward" => Some(Self::ArrowOutward),
            "arrow_range" => Some(Self::ArrowRange),
            "arrow_right_alt" => Some(Self::ArrowRightAlt),
            "arrow_selector_tool" => Some(Self::ArrowSelectorTool),
            "arrow_shape_up" => Some(Self::ArrowShapeUp),
            "arrow_shape_up_stack" => Some(Self::ArrowShapeUpStack),
            "arrow_shape_up_stack_2" => Some(Self::ArrowShapeUpStack2),
            "arrow_split" => Some(Self::ArrowSplit),
            "arrow_top_left" => Some(Self::ArrowTopLeft),
            "arrow_top_right" => Some(Self::ArrowTopRight),
            "arrow_upload_progress" => Some(Self::ArrowUploadProgress),
            "arrow_upload_ready" => Some(Self::ArrowUploadReady),
            "arrow_upward" => Some(Self::ArrowUpward),
            "arrow_upward_alt" => Some(Self::ArrowUpwardAlt),
            "arrow_warm_up" => Some(Self::ArrowWarmUp),
            "arrows_input" => Some(Self::ArrowsInput),
            "arrows_left_right_circle" => Some(Self::ArrowsLeftRightCircle),
            "arrows_more_down" => Some(Self::ArrowsMoreDown),
            "arrows_more_up" => Some(Self::ArrowsMoreUp),
            "arrows_output" => Some(Self::ArrowsOutput),
            "arrows_outward" => Some(Self::ArrowsOutward),
            "arrows_up_down_circle" => Some(Self::ArrowsUpDownCircle),
            "art_track" => Some(Self::ArtTrack),
            "article" => Some(Self::Article),
            "article_person" => Some(Self::ArticlePerson),
            "article_shortcut" => Some(Self::ArticleShortcut),
            "artist" => Some(Self::Artist),
            "aspect_ratio" => Some(Self::AspectRatio),
            "assessment" => Some(Self::Assessment),
            "assignment" => Some(Self::Assignment),
            "assignment_add" => Some(Self::AssignmentAdd),
            "assignment_globe" => Some(Self::AssignmentGlobe),
            "assignment_ind" => Some(Self::AssignmentInd),
            "assignment_late" => Some(Self::AssignmentLate),
            "assignment_return" => Some(Self::AssignmentReturn),
            "assignment_returned" => Some(Self::AssignmentReturned),
            "assignment_turned_in" => Some(Self::AssignmentTurnedIn),
            "assist_walker" => Some(Self::AssistWalker),
            "assistant" => Some(Self::Assistant),
            "assistant_device" => Some(Self::AssistantDevice),
            "assistant_direction" => Some(Self::AssistantDirection),
            "assistant_navigation" => Some(Self::AssistantNavigation),
            "assistant_on_hub" => Some(Self::AssistantOnHub),
            "assistant_photo" => Some(Self::AssistantPhoto),
            "assured_workload" => Some(Self::AssuredWorkload),
            "asterisk" => Some(Self::Asterisk),
            "astrophotography_auto" => Some(Self::AstrophotographyAuto),
            "astrophotography_off" => Some(Self::AstrophotographyOff),
            "atm" => Some(Self::Atm),
            "atr" => Some(Self::Atr),
            "attach_email" => Some(Self::AttachEmail),
            "attach_file" => Some(Self::AttachFile),
            "attach_file_add" => Some(Self::AttachFileAdd),
            "attach_file_off" => Some(Self::AttachFileOff),
            "attach_money" => Some(Self::AttachMoney),
            "attractions" => Some(Self::Attractions),
            "attribution" => Some(Self::Attribution),
            "audio_capture" => Some(Self::AudioCapture),
            "audio_description" => Some(Self::AudioDescription),
            "audio_file" => Some(Self::AudioFile),
            "audio_video_receiver" => Some(Self::AudioVideoReceiver),
            "audiotrack" => Some(Self::Audiotrack),
            "auto_activity_zone" => Some(Self::AutoActivityZone),
            "auto_awesome" => Some(Self::AutoAwesome),
            "auto_awesome_mosaic" => Some(Self::AutoAwesomeMosaic),
            "auto_awesome_motion" => Some(Self::AutoAwesomeMotion),
            "auto_delete" => Some(Self::AutoDelete),
            "auto_detect_voice" => Some(Self::AutoDetectVoice),
            "auto_draw_solid" => Some(Self::AutoDrawSolid),
            "auto_fix" => Some(Self::AutoFix),
            "auto_fix_high" => Some(Self::AutoFixHigh),
            "auto_fix_normal" => Some(Self::AutoFixNormal),
            "auto_fix_off" => Some(Self::AutoFixOff),
            "auto_graph" => Some(Self::AutoGraph),
            "auto_label" => Some(Self::AutoLabel),
            "auto_meeting_room" => Some(Self::AutoMeetingRoom),
            "auto_mode" => Some(Self::AutoMode),
            "auto_read_pause" => Some(Self::AutoReadPause),
            "auto_read_play" => Some(Self::AutoReadPlay),
            "auto_schedule" => Some(Self::AutoSchedule),
            "auto_stories" => Some(Self::AutoStories),
            "auto_stories_off" => Some(Self::AutoStoriesOff),
            "auto_timer" => Some(Self::AutoTimer),
            "auto_towing" => Some(Self::AutoTowing),
            "auto_transmission" => Some(Self::AutoTransmission),
            "auto_videocam" => Some(Self::AutoVideocam),
            "autofps_select" => Some(Self::AutofpsSelect),
            "automation" => Some(Self::Automation),
            "autopause" => Some(Self::Autopause),
            "autopay" => Some(Self::Autopay),
            "autoplay" => Some(Self::Autoplay),
            "autorenew" => Some(Self::Autorenew),
            "autostop" => Some(Self::Autostop),
            "av1" => Some(Self::Av1),
            "av_timer" => Some(Self::AvTimer),
            "avc" => Some(Self::Avc),
            "avg_pace" => Some(Self::AvgPace),
            "avg_time" => Some(Self::AvgTime),
            "avocado_bean" => Some(Self::AvocadoBean),
            "award_meal" => Some(Self::AwardMeal),
            "award_star" => Some(Self::AwardStar),
            "azm" => Some(Self::Azm),
            "b_circle" => Some(Self::BCircle),
            "baby_changing_station" => Some(Self::BabyChangingStation),
            "back_hand" => Some(Self::BackHand),
            "back_to_tab" => Some(Self::BackToTab),
            "background_dot_large" => Some(Self::BackgroundDotLarge),
            "background_dot_small" => Some(Self::BackgroundDotSmall),
            "background_grid_small" => Some(Self::BackgroundGridSmall),
            "background_replace" => Some(Self::BackgroundReplace),
            "backlight_high" => Some(Self::BacklightHigh),
            "backlight_high_off" => Some(Self::BacklightHighOff),
            "backlight_low" => Some(Self::BacklightLow),
            "backpack" => Some(Self::Backpack),
            "backspace" => Some(Self::Backspace),
            "backup" => Some(Self::Backup),
            "backup_table" => Some(Self::BackupTable),
            "badge" => Some(Self::Badge),
            "badge_critical_battery" => Some(Self::BadgeCriticalBattery),
            "badminton" => Some(Self::Badminton),
            "bakery_dining" => Some(Self::BakeryDining),
            "balance" => Some(Self::Balance),
            "balcony" => Some(Self::Balcony),
            "ballot" => Some(Self::Ballot),
            "bar_chart_4_bars" => Some(Self::BarChart4Bars),
            "bar_chart_off" => Some(Self::BarChartOff),
            "barcode" => Some(Self::Barcode),
            "barcode_reader" => Some(Self::BarcodeReader),
            "barcode_scanner" => Some(Self::BarcodeScanner),
            "barefoot" => Some(Self::Barefoot),
            "batch_prediction" => Some(Self::BatchPrediction),
            "bath_bedrock" => Some(Self::BathBedrock),
            "bath_outdoor" => Some(Self::BathOutdoor),
            "bath_private" => Some(Self::BathPrivate),
            "bath_public_large" => Some(Self::BathPublicLarge),
            "bath_soak" => Some(Self::BathSoak),
            "bathroom" => Some(Self::Bathroom),
            "bathtub" => Some(Self::Bathtub),
            "battery_0_bar" => Some(Self::Battery0Bar),
            "battery_1_bar" => Some(Self::Battery1Bar),
            "battery_20" => Some(Self::Battery20),
            "battery_2_bar" => Some(Self::Battery2Bar),
            "battery_30" => Some(Self::Battery30),
            "battery_3_bar" => Some(Self::Battery3Bar),
            "battery_4_bar" => Some(Self::Battery4Bar),
            "battery_50" => Some(Self::Battery50),
            "battery_5_bar" => Some(Self::Battery5Bar),
            "battery_60" => Some(Self::Battery60),
            "battery_6_bar" => Some(Self::Battery6Bar),
            "battery_80" => Some(Self::Battery80),
            "battery_90" => Some(Self::Battery90),
            "battery_alert" => Some(Self::BatteryAlert),
            "battery_android_0" => Some(Self::BatteryAndroid0),
            "battery_android_1" => Some(Self::BatteryAndroid1),
            "battery_android_2" => Some(Self::BatteryAndroid2),
            "battery_android_3" => Some(Self::BatteryAndroid3),
            "battery_android_4" => Some(Self::BatteryAndroid4),
            "battery_android_5" => Some(Self::BatteryAndroid5),
            "battery_android_6" => Some(Self::BatteryAndroid6),
            "battery_android_alert" => Some(Self::BatteryAndroidAlert),
            "battery_android_bolt" => Some(Self::BatteryAndroidBolt),
            "battery_android_frame_1" => Some(Self::BatteryAndroidFrame1),
            "battery_android_frame_2" => Some(Self::BatteryAndroidFrame2),
            "battery_android_frame_3" => Some(Self::BatteryAndroidFrame3),
            "battery_android_frame_4" => Some(Self::BatteryAndroidFrame4),
            "battery_android_frame_5" => Some(Self::BatteryAndroidFrame5),
            "battery_android_frame_6" => Some(Self::BatteryAndroidFrame6),
            "battery_android_frame_alert" => Some(Self::BatteryAndroidFrameAlert),
            "battery_android_frame_bolt" => Some(Self::BatteryAndroidFrameBolt),
            "battery_android_frame_full" => Some(Self::BatteryAndroidFrameFull),
            "battery_android_frame_plus" => Some(Self::BatteryAndroidFramePlus),
            "battery_android_frame_question" => Some(Self::BatteryAndroidFrameQuestion),
            "battery_android_frame_share" => Some(Self::BatteryAndroidFrameShare),
            "battery_android_frame_shield" => Some(Self::BatteryAndroidFrameShield),
            "battery_android_full" => Some(Self::BatteryAndroidFull),
            "battery_android_plus" => Some(Self::BatteryAndroidPlus),
            "battery_android_question" => Some(Self::BatteryAndroidQuestion),
            "battery_android_share" => Some(Self::BatteryAndroidShare),
            "battery_android_shield" => Some(Self::BatteryAndroidShield),
            "battery_change" => Some(Self::BatteryChange),
            "battery_charging_20" => Some(Self::BatteryCharging20),
            "battery_charging_20_2" => Some(Self::BatteryCharging202),
            "battery_charging_30" => Some(Self::BatteryCharging30),
            "battery_charging_30_2" => Some(Self::BatteryCharging302),
            "battery_charging_50" => Some(Self::BatteryCharging50),
            "battery_charging_50_2" => Some(Self::BatteryCharging502),
            "battery_charging_60" => Some(Self::BatteryCharging60),
            "battery_charging_60_2" => Some(Self::BatteryCharging602),
            "battery_charging_80" => Some(Self::BatteryCharging80),
            "battery_charging_80_2" => Some(Self::BatteryCharging802),
            "battery_charging_90" => Some(Self::BatteryCharging90),
            "battery_charging_full" => Some(Self::BatteryChargingFull),
            "battery_charging_full_2" => Some(Self::BatteryChargingFull2),
            "battery_error" => Some(Self::BatteryError),
            "battery_full" => Some(Self::BatteryFull),
            "battery_full_alt" => Some(Self::BatteryFullAlt),
            "battery_horiz_000" => Some(Self::BatteryHoriz000),
            "battery_horiz_050" => Some(Self::BatteryHoriz050),
            "battery_horiz_075" => Some(Self::BatteryHoriz075),
            "battery_low" => Some(Self::BatteryLow),
            "battery_plus" => Some(Self::BatteryPlus),
            "battery_profile" => Some(Self::BatteryProfile),
            "battery_saver" => Some(Self::BatterySaver),
            "battery_share" => Some(Self::BatteryShare),
            "battery_status_good" => Some(Self::BatteryStatusGood),
            "battery_std" => Some(Self::BatteryStd),
            "battery_unknown" => Some(Self::BatteryUnknown),
            "battery_vert_005" => Some(Self::BatteryVert005),
            "battery_vert_020" => Some(Self::BatteryVert020),
            "battery_vert_050" => Some(Self::BatteryVert050),
            "battery_very_low" => Some(Self::BatteryVeryLow),
            "beach_access" => Some(Self::BeachAccess),
            "bed" => Some(Self::Bed),
            "bedroom_baby" => Some(Self::BedroomBaby),
            "bedroom_child" => Some(Self::BedroomChild),
            "bedroom_parent" => Some(Self::BedroomParent),
            "bedtime" => Some(Self::Bedtime),
            "bedtime_off" => Some(Self::BedtimeOff),
            "beenhere" => Some(Self::Beenhere),
            "beer_meal" => Some(Self::BeerMeal),
            "bento" => Some(Self::Bento),
            "bia" => Some(Self::Bia),
            "bid_landscape" => Some(Self::BidLandscape),
            "bid_landscape_disabled" => Some(Self::BidLandscapeDisabled),
            "bigtop_updates" => Some(Self::BigtopUpdates),
            "bike_dock" => Some(Self::BikeDock),
            "bike_lane" => Some(Self::BikeLane),
            "bike_scooter" => Some(Self::BikeScooter),
            "biotech" => Some(Self::Biotech),
            "blanket" => Some(Self::Blanket),
            "blender" => Some(Self::Blender),
            "blind" => Some(Self::Blind),
            "blinds" => Some(Self::Blinds),
            "blinds_2" => Some(Self::Blinds2),
            "blinds_2_closed" => Some(Self::Blinds2Closed),
            "blinds_closed" => Some(Self::BlindsClosed),
            "blood_pressure" => Some(Self::BloodPressure),
            "bloodtype" => Some(Self::Bloodtype),
            "bluetooth" => Some(Self::Bluetooth),
            "bluetooth_audio" => Some(Self::BluetoothAudio),
            "bluetooth_connected" => Some(Self::BluetoothConnected),
            "bluetooth_disabled" => Some(Self::BluetoothDisabled),
            "bluetooth_drive" => Some(Self::BluetoothDrive),
            "bluetooth_searching" => Some(Self::BluetoothSearching),
            "blur_circular" => Some(Self::BlurCircular),
            "blur_linear" => Some(Self::BlurLinear),
            "blur_medium" => Some(Self::BlurMedium),
            "blur_off" => Some(Self::BlurOff),
            "blur_on" => Some(Self::BlurOn),
            "blur_short" => Some(Self::BlurShort),
            "boat_bus" => Some(Self::BoatBus),
            "boat_railway" => Some(Self::BoatRailway),
            "body_fat" => Some(Self::BodyFat),
            "body_system" => Some(Self::BodySystem),
            "bolt" => Some(Self::Bolt),
            "bolt_boost" => Some(Self::BoltBoost),
            "bomb" => Some(Self::Bomb),
            "book" => Some(Self::Book),
            "book_2" => Some(Self::Book2),
            "book_3" => Some(Self::Book3),
            "book_4" => Some(Self::Book4),
            "book_5" => Some(Self::Book5),
            "book_6" => Some(Self::Book6),
            "book_online" => Some(Self::BookOnline),
            "book_ribbon" => Some(Self::BookRibbon),
            "bookmark" => Some(Self::Bookmark),
            "bookmark_add" => Some(Self::BookmarkAdd),
            "bookmark_added" => Some(Self::BookmarkAdded),
            "bookmark_bag" => Some(Self::BookmarkBag),
            "bookmark_border" => Some(Self::BookmarkBorder),
            "bookmark_check" => Some(Self::BookmarkCheck),
            "bookmark_flag" => Some(Self::BookmarkFlag),
            "bookmark_heart" => Some(Self::BookmarkHeart),
            "bookmark_manager" => Some(Self::BookmarkManager),
            "bookmark_remove" => Some(Self::BookmarkRemove),
            "bookmark_stacks" => Some(Self::BookmarkStacks),
            "bookmark_star" => Some(Self::BookmarkStar),
            "bookmarks" => Some(Self::Bookmarks),
            "books_movies_and_music" => Some(Self::BooksMoviesAndMusic),
            "border_all" => Some(Self::BorderAll),
            "border_bottom" => Some(Self::BorderBottom),
            "border_clear" => Some(Self::BorderClear),
            "border_color" => Some(Self::BorderColor),
            "border_horizontal" => Some(Self::BorderHorizontal),
            "border_inner" => Some(Self::BorderInner),
            "border_left" => Some(Self::BorderLeft),
            "border_outer" => Some(Self::BorderOuter),
            "border_right" => Some(Self::BorderRight),
            "border_style" => Some(Self::BorderStyle),
            "border_top" => Some(Self::BorderTop),
            "border_vertical" => Some(Self::BorderVertical),
            "borg" => Some(Self::Borg),
            "bottom_app_bar" => Some(Self::BottomAppBar),
            "bottom_drawer" => Some(Self::BottomDrawer),
            "bottom_navigation" => Some(Self::BottomNavigation),
            "bottom_panel_close" => Some(Self::BottomPanelClose),
            "bottom_panel_open" => Some(Self::BottomPanelOpen),
            "bottom_right_click" => Some(Self::BottomRightClick),
            "bottom_sheets" => Some(Self::BottomSheets),
            "box" => Some(Self::Box),
            "box_add" => Some(Self::BoxAdd),
            "box_edit" => Some(Self::BoxEdit),
            "boy" => Some(Self::Boy),
            "brand_awareness" => Some(Self::BrandAwareness),
            "brand_family" => Some(Self::BrandFamily),
            "branding_watermark" => Some(Self::BrandingWatermark),
            "breakfast_dining" => Some(Self::BreakfastDining),
            "breaking_news" => Some(Self::BreakingNews),
            "breaking_news_alt_1" => Some(Self::BreakingNewsAlt1),
            "breastfeeding" => Some(Self::Breastfeeding),
            "brick" => Some(Self::Brick),
            "briefcase_meal" => Some(Self::BriefcaseMeal),
            "brightness_1" => Some(Self::Brightness1),
            "brightness_2" => Some(Self::Brightness2),
            "brightness_3" => Some(Self::Brightness3),
            "brightness_4" => Some(Self::Brightness4),
            "brightness_5" => Some(Self::Brightness5),
            "brightness_6" => Some(Self::Brightness6),
            "brightness_7" => Some(Self::Brightness7),
            "brightness_alert" => Some(Self::BrightnessAlert),
            "brightness_auto" => Some(Self::BrightnessAuto),
            "brightness_empty" => Some(Self::BrightnessEmpty),
            "brightness_high" => Some(Self::BrightnessHigh),
            "brightness_low" => Some(Self::BrightnessLow),
            "brightness_medium" => Some(Self::BrightnessMedium),
            "bring_your_own_ip" => Some(Self::BringYourOwnIp),
            "broadcast_on_home" => Some(Self::BroadcastOnHome),
            "broadcast_on_personal" => Some(Self::BroadcastOnPersonal),
            "broken_image" => Some(Self::BrokenImage),
            "browse" => Some(Self::Browse),
            "browse_activity" => Some(Self::BrowseActivity),
            "browse_gallery" => Some(Self::BrowseGallery),
            "browser_not_supported" => Some(Self::BrowserNotSupported),
            "browser_updated" => Some(Self::BrowserUpdated),
            "brunch_dining" => Some(Self::BrunchDining),
            "brush" => Some(Self::Brush),
            "bubble" => Some(Self::Bubble),
            "bubble_chart" => Some(Self::BubbleChart),
            "bubbles" => Some(Self::Bubbles),
            "bucket_check" => Some(Self::BucketCheck),
            "bug_report" => Some(Self::BugReport),
            "build" => Some(Self::Build),
            "build_circle" => Some(Self::BuildCircle),
            "bullet_chart" => Some(Self::BulletChart),
            "bungalow" => Some(Self::Bungalow),
            "burst_mode" => Some(Self::BurstMode),
            "bus_alert" => Some(Self::BusAlert),
            "bus_map_pin" => Some(Self::BusMapPin),
            "bus_railway" => Some(Self::BusRailway),
            "business" => Some(Self::Business),
            "business_center" => Some(Self::BusinessCenter),
            "business_chip" => Some(Self::BusinessChip),
            "business_messages" => Some(Self::BusinessMessages),
            "buttons_alt" => Some(Self::ButtonsAlt),
            "cabin" => Some(Self::Cabin),
            "cable" => Some(Self::Cable),
            "cable_car" => Some(Self::CableCar),
            "cached" => Some(Self::Cached),
            "cadence" => Some(Self::Cadence),
            "cake" => Some(Self::Cake),
            "cake_add" => Some(Self::CakeAdd),
            "calculate" => Some(Self::Calculate),
            "calendar_add_on" => Some(Self::CalendarAddOn),
            "calendar_apps_script" => Some(Self::CalendarAppsScript),
            "calendar_check" => Some(Self::CalendarCheck),
            "calendar_clock" => Some(Self::CalendarClock),
            "calendar_lock" => Some(Self::CalendarLock),
            "calendar_meal" => Some(Self::CalendarMeal),
            "calendar_meal_2" => Some(Self::CalendarMeal2),
            "calendar_month" => Some(Self::CalendarMonth),
            "calendar_today" => Some(Self::CalendarToday),
            "calendar_view_day" => Some(Self::CalendarViewDay),
            "calendar_view_month" => Some(Self::CalendarViewMonth),
            "calendar_view_week" => Some(Self::CalendarViewWeek),
            "call_end" => Some(Self::CallEnd),
            "call_end_alt" => Some(Self::CallEndAlt),
            "call_log" => Some(Self::CallLog),
            "call_made" => Some(Self::CallMade),
            "call_merge" => Some(Self::CallMerge),
            "call_missed" => Some(Self::CallMissed),
            "call_missed_outgoing" => Some(Self::CallMissedOutgoing),
            "call_quality" => Some(Self::CallQuality),
            "call_received" => Some(Self::CallReceived),
            "call_split" => Some(Self::CallSplit),
            "call_to_action" => Some(Self::CallToAction),
            "camera" => Some(Self::Camera),
            "camera_alt" => Some(Self::CameraAlt),
            "camera_enhance" => Some(Self::CameraEnhance),
            "camera_front" => Some(Self::CameraFront),
            "camera_indoor" => Some(Self::CameraIndoor),
            "camera_outdoor" => Some(Self::CameraOutdoor),
            "camera_rear" => Some(Self::CameraRear),
            "camera_roll" => Some(Self::CameraRoll),
            "camera_video" => Some(Self::CameraVideo),
            "cameraswitch" => Some(Self::Cameraswitch),
            "campaign" => Some(Self::Campaign),
            "camping" => Some(Self::Camping),
            "cancel" => Some(Self::Cancel),
            "cancel_presentation" => Some(Self::CancelPresentation),
            "cancel_schedule_send" => Some(Self::CancelScheduleSend),
            "candle" => Some(Self::Candle),
            "candlestick_chart" => Some(Self::CandlestickChart),
            "cannabis" => Some(Self::Cannabis),
            "captive_portal" => Some(Self::CaptivePortal),
            "capture" => Some(Self::Capture),
            "car_crash" => Some(Self::CarCrash),
            "car_defrost_left" => Some(Self::CarDefrostLeft),
            "car_defrost_low_left" => Some(Self::CarDefrostLowLeft),
            "car_defrost_low_right" => Some(Self::CarDefrostLowRight),
            "car_defrost_mid_left" => Some(Self::CarDefrostMidLeft),
            "car_defrost_mid_low_left" => Some(Self::CarDefrostMidLowLeft),
            "car_defrost_mid_low_right" => Some(Self::CarDefrostMidLowRight),
            "car_defrost_mid_right" => Some(Self::CarDefrostMidRight),
            "car_defrost_right" => Some(Self::CarDefrostRight),
            "car_fan_low_left" => Some(Self::CarFanLowLeft),
            "car_fan_low_mid_left" => Some(Self::CarFanLowMidLeft),
            "car_fan_low_right" => Some(Self::CarFanLowRight),
            "car_fan_mid_left" => Some(Self::CarFanMidLeft),
            "car_fan_mid_low_right" => Some(Self::CarFanMidLowRight),
            "car_fan_mid_right" => Some(Self::CarFanMidRight),
            "car_fan_recirculate" => Some(Self::CarFanRecirculate),
            "car_fan_recirculate_2" => Some(Self::CarFanRecirculate2),
            "car_gear" => Some(Self::CarGear),
            "car_lock" => Some(Self::CarLock),
            "car_mirror_heat" => Some(Self::CarMirrorHeat),
            "car_rental" => Some(Self::CarRental),
            "car_repair" => Some(Self::CarRepair),
            "car_seat_off" => Some(Self::CarSeatOff),
            "car_tag" => Some(Self::CarTag),
            "card_giftcard" => Some(Self::CardGiftcard),
            "card_membership" => Some(Self::CardMembership),
            "card_travel" => Some(Self::CardTravel),
            "cardio_load" => Some(Self::CardioLoad),
            "cardiology" => Some(Self::Cardiology),
            "cards" => Some(Self::Cards),
            "cards_stack" => Some(Self::CardsStack),
            "cards_star" => Some(Self::CardsStar),
            "carpenter" => Some(Self::Carpenter),
            "carry_on_bag" => Some(Self::CarryOnBag),
            "carry_on_bag_checked" => Some(Self::CarryOnBagChecked),
            "carry_on_bag_inactive" => Some(Self::CarryOnBagInactive),
            "carry_on_bag_question" => Some(Self::CarryOnBagQuestion),
            "cases" => Some(Self::Cases),
            "casino" => Some(Self::Casino),
            "cast" => Some(Self::Cast),
            "cast_connected" => Some(Self::CastConnected),
            "cast_for_education" => Some(Self::CastForEducation),
            "cast_pause" => Some(Self::CastPause),
            "cast_warning" => Some(Self::CastWarning),
            "castle" => Some(Self::Castle),
            "category" => Some(Self::Category),
            "category_search" => Some(Self::CategorySearch),
            "celebration" => Some(Self::Celebration),
            "cell_merge" => Some(Self::CellMerge),
            "cell_tower" => Some(Self::CellTower),
            "cell_wifi" => Some(Self::CellWifi),
            "center_focus_strong" => Some(Self::CenterFocusStrong),
            "center_focus_weak" => Some(Self::CenterFocusWeak),
            "chair" => Some(Self::Chair),
            "chair_alt" => Some(Self::ChairAlt),
            "chair_counter" => Some(Self::ChairCounter),
            "chair_fireplace" => Some(Self::ChairFireplace),
            "chair_umbrella" => Some(Self::ChairUmbrella),
            "chalet" => Some(Self::Chalet),
            "change_circle" => Some(Self::ChangeCircle),
            "change_history" => Some(Self::ChangeHistory),
            "charger" => Some(Self::Charger),
            "charging_station" => Some(Self::ChargingStation),
            "chart_data" => Some(Self::ChartData),
            "chat_add_on" => Some(Self::ChatAddOn),
            "chat_apps_script" => Some(Self::ChatAppsScript),
            "chat_bubble" => Some(Self::ChatBubble),
            "chat_bubble_off" => Some(Self::ChatBubbleOff),
            "chat_bubble_outline" => Some(Self::ChatBubbleOutline),
            "chat_dashed" => Some(Self::ChatDashed),
            "chat_display" => Some(Self::ChatDisplay),
            "chat_error" => Some(Self::ChatError),
            "chat_info" => Some(Self::ChatInfo),
            "chat_paste_go" => Some(Self::ChatPasteGo),
            "chat_paste_go_2" => Some(Self::ChatPasteGo2),
            "check_alert" => Some(Self::CheckAlert),
            "check_box" => Some(Self::CheckBox),
            "check_box_outline_blank" => Some(Self::CheckBoxOutlineBlank),
            "check_circle" => Some(Self::CheckCircle),
            "check_circle_filled" => Some(Self::CheckCircleFilled),
            "check_circle_outline" => Some(Self::CheckCircleOutline),
            "check_circle_unread" => Some(Self::CheckCircleUnread),
            "check_in_out" => Some(Self::CheckInOut),
            "check_indeterminate_small" => Some(Self::CheckIndeterminateSmall),
            "check_small" => Some(Self::CheckSmall),
            "checkbook" => Some(Self::Checkbook),
            "checked_bag" => Some(Self::CheckedBag),
            "checked_bag_question" => Some(Self::CheckedBagQuestion),
            "checklist" => Some(Self::Checklist),
            "checklist_rtl" => Some(Self::ChecklistRtl),
            "checkroom" => Some(Self::Checkroom),
            "cheer" => Some(Self::Cheer),
            "chef_hat" => Some(Self::ChefHat),
            "chess" => Some(Self::Chess),
            "chess_bishop" => Some(Self::ChessBishop),
            "chess_bishop_2" => Some(Self::ChessBishop2),
            "chess_king" => Some(Self::ChessKing),
            "chess_king_2" => Some(Self::ChessKing2),
            "chess_knight" => Some(Self::ChessKnight),
            "chess_pawn" => Some(Self::ChessPawn),
            "chess_pawn_2" => Some(Self::ChessPawn2),
            "chess_queen" => Some(Self::ChessQueen),
            "chess_rook" => Some(Self::ChessRook),
            "chevron_backward" => Some(Self::ChevronBackward),
            "chevron_forward" => Some(Self::ChevronForward),
            "chevron_line_up" => Some(Self::ChevronLineUp),
            "child_care" => Some(Self::ChildCare),
            "child_friendly" => Some(Self::ChildFriendly),
            "child_hat" => Some(Self::ChildHat),
            "chip_extraction" => Some(Self::ChipExtraction),
            "chips" => Some(Self::Chips),
            "chrome_reader_mode" => Some(Self::ChromeReaderMode),
            "chromecast_2" => Some(Self::Chromecast2),
            "chromecast_device" => Some(Self::ChromecastDevice),
            "chronic" => Some(Self::Chronic),
            "church" => Some(Self::Church),
            "cinematic_blur" => Some(Self::CinematicBlur),
            "circle" => Some(Self::Circle),
            "circle_circle" => Some(Self::CircleCircle),
            "circle_notifications" => Some(Self::CircleNotifications),
            "circles" => Some(Self::Circles),
            "circles_ext" => Some(Self::CirclesExt),
            "clarify" => Some(Self::Clarify),
            "class" => Some(Self::Class),
            "clean_hands" => Some(Self::CleanHands),
            "cleaning" => Some(Self::Cleaning),
            "cleaning_bucket" => Some(Self::CleaningBucket),
            "cleaning_services" => Some(Self::CleaningServices),
            "clear" => Some(Self::Clear),
            "clear_all" => Some(Self::ClearAll),
            "clear_day" => Some(Self::ClearDay),
            "clear_night" => Some(Self::ClearNight),
            "climate_mini_split" => Some(Self::ClimateMiniSplit),
            "clinical_notes" => Some(Self::ClinicalNotes),
            "clock_arrow_down" => Some(Self::ClockArrowDown),
            "clock_arrow_up" => Some(Self::ClockArrowUp),
            "clock_loader_10" => Some(Self::ClockLoader10),
            "clock_loader_20" => Some(Self::ClockLoader20),
            "clock_loader_40" => Some(Self::ClockLoader40),
            "clock_loader_60" => Some(Self::ClockLoader60),
            "clock_loader_80" => Some(Self::ClockLoader80),
            "clock_loader_90" => Some(Self::ClockLoader90),
            "close_fullscreen" => Some(Self::CloseFullscreen),
            "close_small" => Some(Self::CloseSmall),
            "closed_caption" => Some(Self::ClosedCaption),
            "closed_caption_add" => Some(Self::ClosedCaptionAdd),
            "closed_caption_disabled" => Some(Self::ClosedCaptionDisabled),
            "closed_caption_display" => Some(Self::ClosedCaptionDisplay),
            "closed_caption_off" => Some(Self::ClosedCaptionOff),
            "cloud" => Some(Self::Cloud),
            "cloud_alert" => Some(Self::CloudAlert),
            "cloud_circle" => Some(Self::CloudCircle),
            "cloud_done" => Some(Self::CloudDone),
            "cloud_download" => Some(Self::CloudDownload),
            "cloud_lock" => Some(Self::CloudLock),
            "cloud_off" => Some(Self::CloudOff),
            "cloud_queue" => Some(Self::CloudQueue),
            "cloud_sync" => Some(Self::CloudSync),
            "cloud_upload" => Some(Self::CloudUpload),
            "cloudy" => Some(Self::Cloudy),
            "cloudy_filled" => Some(Self::CloudyFilled),
            "cloudy_snowing" => Some(Self::CloudySnowing),
            "co2" => Some(Self::Co2),
            "co_present" => Some(Self::CoPresent),
            "code" => Some(Self::Code),
            "code_blocks" => Some(Self::CodeBlocks),
            "code_off" => Some(Self::CodeOff),
            "code_xml" => Some(Self::CodeXml),
            "coffee" => Some(Self::Coffee),
            "coffee_maker" => Some(Self::CoffeeMaker),
            "cognition" => Some(Self::Cognition),
            "cognition_2" => Some(Self::Cognition2),
            "collapse_all" => Some(Self::CollapseAll),
            "collapse_content" => Some(Self::CollapseContent),
            "collections" => Some(Self::Collections),
            "collections_bookmark" => Some(Self::CollectionsBookmark),
            "color_lens" => Some(Self::ColorLens),
            "colorize" => Some(Self::Colorize),
            "colors" => Some(Self::Colors),
            "combine_columns" => Some(Self::CombineColumns),
            "comedy_mask" => Some(Self::ComedyMask),
            "comic_bubble" => Some(Self::ComicBubble),
            "comment" => Some(Self::Comment),
            "comment_bank" => Some(Self::CommentBank),
            "comments_disabled" => Some(Self::CommentsDisabled),
            "commit" => Some(Self::Commit),
            "communication" => Some(Self::Communication),
            "communities" => Some(Self::Communities),
            "communities_filled" => Some(Self::CommunitiesFilled),
            "commute" => Some(Self::Commute),
            "compare" => Some(Self::Compare),
            "compare_arrows" => Some(Self::CompareArrows),
            "compass_calibration" => Some(Self::CompassCalibration),
            "component_exchange" => Some(Self::ComponentExchange),
            "compost" => Some(Self::Compost),
            "compress" => Some(Self::Compress),
            "computer" => Some(Self::Computer),
            "computer_arrow_up" => Some(Self::ComputerArrowUp),
            "computer_cancel" => Some(Self::ComputerCancel),
            "computer_sound" => Some(Self::ComputerSound),
            "concierge" => Some(Self::Concierge),
            "conditions" => Some(Self::Conditions),
            "confirmation_number" => Some(Self::ConfirmationNumber),
            "congenital" => Some(Self::Congenital),
            "connect_without_contact" => Some(Self::ConnectWithoutContact),
            "connected_tv" => Some(Self::ConnectedTv),
            "connecting_airports" => Some(Self::ConnectingAirports),
            "construction" => Some(Self::Construction),
            "contact_emergency" => Some(Self::ContactEmergency),
            "contact_mail" => Some(Self::ContactMail),
            "contact_page" => Some(Self::ContactPage),
            "contact_phone" => Some(Self::ContactPhone),
            "contact_phone_filled" => Some(Self::ContactPhoneFilled),
            "contact_support" => Some(Self::ContactSupport),
            "contactless" => Some(Self::Contactless),
            "contactless_off" => Some(Self::ContactlessOff),
            "contacts" => Some(Self::Contacts),
            "contacts_product" => Some(Self::ContactsProduct),
            "content_copy" => Some(Self::ContentCopy),
            "content_cut" => Some(Self::ContentCut),
            "content_paste" => Some(Self::ContentPaste),
            "content_paste_go" => Some(Self::ContentPasteGo),
            "content_paste_off" => Some(Self::ContentPasteOff),
            "content_paste_search" => Some(Self::ContentPasteSearch),
            "contextual_token" => Some(Self::ContextualToken),
            "contextual_token_add" => Some(Self::ContextualTokenAdd),
            "contract" => Some(Self::Contract),
            "contract_delete" => Some(Self::ContractDelete),
            "contract_edit" => Some(Self::ContractEdit),
            "contrast" => Some(Self::Contrast),
            "contrast_circle" => Some(Self::ContrastCircle),
            "contrast_rtl_off" => Some(Self::ContrastRtlOff),
            "contrast_square" => Some(Self::ContrastSquare),
            "control_camera" => Some(Self::ControlCamera),
            "control_point" => Some(Self::ControlPoint),
            "control_point_duplicate" => Some(Self::ControlPointDuplicate),
            "controller_gen" => Some(Self::ControllerGen),
            "conversation" => Some(Self::Conversation),
            "conversion_path" => Some(Self::ConversionPath),
            "conversion_path_off" => Some(Self::ConversionPathOff),
            "convert_to_text" => Some(Self::ConvertToText),
            "conveyor_belt" => Some(Self::ConveyorBelt),
            "cookie" => Some(Self::Cookie),
            "cookie_off" => Some(Self::CookieOff),
            "cooking" => Some(Self::Cooking),
            "cool_to_dry" => Some(Self::CoolToDry),
            "copy_all" => Some(Self::CopyAll),
            "copyright" => Some(Self::Copyright),
            "coronavirus" => Some(Self::Coronavirus),
            "corporate_fare" => Some(Self::CorporateFare),
            "cottage" => Some(Self::Cottage),
            "counter_0" => Some(Self::Counter0),
            "counter_1" => Some(Self::Counter1),
            "counter_2" => Some(Self::Counter2),
            "counter_3" => Some(Self::Counter3),
            "counter_4" => Some(Self::Counter4),
            "counter_5" => Some(Self::Counter5),
            "counter_6" => Some(Self::Counter6),
            "counter_7" => Some(Self::Counter7),
            "counter_8" => Some(Self::Counter8),
            "counter_9" => Some(Self::Counter9),
            "countertops" => Some(Self::Countertops),
            "create" => Some(Self::Create),
            "create_new_folder" => Some(Self::CreateNewFolder),
            "credit_card" => Some(Self::CreditCard),
            "credit_card_clock" => Some(Self::CreditCardClock),
            "credit_card_gear" => Some(Self::CreditCardGear),
            "credit_card_heart" => Some(Self::CreditCardHeart),
            "credit_card_off" => Some(Self::CreditCardOff),
            "credit_score" => Some(Self::CreditScore),
            "crib" => Some(Self::Crib),
            "crisis_alert" => Some(Self::CrisisAlert),
            "crop" => Some(Self::Crop),
            "crop_16_9" => Some(Self::Crop169),
            "crop_21_9" => Some(Self::Crop219),
            "crop_2_3" => Some(Self::Crop23),
            "crop_3_2" => Some(Self::Crop32),
            "crop_5_4" => Some(Self::Crop54),
            "crop_7_5" => Some(Self::Crop75),
            "crop_9_16" => Some(Self::Crop916),
            "crop_din" => Some(Self::CropDin),
            "crop_free" => Some(Self::CropFree),
            "crop_landscape" => Some(Self::CropLandscape),
            "crop_original" => Some(Self::CropOriginal),
            "crop_portrait" => Some(Self::CropPortrait),
            "crop_rotate" => Some(Self::CropRotate),
            "crop_square" => Some(Self::CropSquare),
            "crossword" => Some(Self::Crossword),
            "crowdsource" => Some(Self::Crowdsource),
            "crown" => Some(Self::Crown),
            "cruelty_free" => Some(Self::CrueltyFree),
            "css" => Some(Self::Css),
            "csv" => Some(Self::Csv),
            "currency_bitcoin" => Some(Self::CurrencyBitcoin),
            "currency_exchange" => Some(Self::CurrencyExchange),
            "currency_franc" => Some(Self::CurrencyFranc),
            "currency_lira" => Some(Self::CurrencyLira),
            "currency_pound" => Some(Self::CurrencyPound),
            "currency_ruble" => Some(Self::CurrencyRuble),
            "currency_rupee" => Some(Self::CurrencyRupee),
            "currency_rupee_circle" => Some(Self::CurrencyRupeeCircle),
            "currency_yen" => Some(Self::CurrencyYen),
            "currency_yuan" => Some(Self::CurrencyYuan),
            "curtains" => Some(Self::Curtains),
            "curtains_closed" => Some(Self::CurtainsClosed),
            "custom_typography" => Some(Self::CustomTypography),
            "cycle" => Some(Self::Cycle),
            "cyclone" => Some(Self::Cyclone),
            "dangerous" => Some(Self::Dangerous),
            "dark_mode" => Some(Self::DarkMode),
            "dashboard" => Some(Self::Dashboard),
            "dashboard_2" => Some(Self::Dashboard2),
            "dashboard_2_add" => Some(Self::Dashboard2Add),
            "dashboard_2_edit" => Some(Self::Dashboard2Edit),
            "dashboard_2_gear" => Some(Self::Dashboard2Gear),
            "dashboard_customize" => Some(Self::DashboardCustomize),
            "data_alert" => Some(Self::DataAlert),
            "data_array" => Some(Self::DataArray),
            "data_check" => Some(Self::DataCheck),
            "data_exploration" => Some(Self::DataExploration),
            "data_info_alert" => Some(Self::DataInfoAlert),
            "data_loss_prevention" => Some(Self::DataLossPrevention),
            "data_object" => Some(Self::DataObject),
            "data_saver_off" => Some(Self::DataSaverOff),
            "data_saver_on" => Some(Self::DataSaverOn),
            "data_table" => Some(Self::DataTable),
            "data_thresholding" => Some(Self::DataThresholding),
            "data_usage" => Some(Self::DataUsage),
            "database" => Some(Self::Database),
            "database_off" => Some(Self::DatabaseOff),
            "database_search" => Some(Self::DatabaseSearch),
            "database_upload" => Some(Self::DatabaseUpload),
            "dataset" => Some(Self::Dataset),
            "dataset_linked" => Some(Self::DatasetLinked),
            "date_range" => Some(Self::DateRange),
            "deblur" => Some(Self::Deblur),
            "deceased" => Some(Self::Deceased),
            "decimal_decrease" => Some(Self::DecimalDecrease),
            "decimal_increase" => Some(Self::DecimalIncrease),
            "deck" => Some(Self::Deck),
            "dehaze" => Some(Self::Dehaze),
            "delete" => Some(Self::Delete),
            "delete_forever" => Some(Self::DeleteForever),
            "delete_history" => Some(Self::DeleteHistory),
            "delete_outline" => Some(Self::DeleteOutline),
            "delete_sweep" => Some(Self::DeleteSweep),
            "delivery_dining" => Some(Self::DeliveryDining),
            "delivery_truck_bolt" => Some(Self::DeliveryTruckBolt),
            "delivery_truck_speed" => Some(Self::DeliveryTruckSpeed),
            "demography" => Some(Self::Demography),
            "density_large" => Some(Self::DensityLarge),
            "density_medium" => Some(Self::DensityMedium),
            "density_small" => Some(Self::DensitySmall),
            "dentistry" => Some(Self::Dentistry),
            "departure_board" => Some(Self::DepartureBoard),
            "deployed_code" => Some(Self::DeployedCode),
            "deployed_code_account" => Some(Self::DeployedCodeAccount),
            "deployed_code_alert" => Some(Self::DeployedCodeAlert),
            "deployed_code_history" => Some(Self::DeployedCodeHistory),
            "deployed_code_update" => Some(Self::DeployedCodeUpdate),
            "dermatology" => Some(Self::Dermatology),
            "description" => Some(Self::Description),
            "deselect" => Some(Self::Deselect),
            "design_services" => Some(Self::DesignServices),
            "desk" => Some(Self::Desk),
            "deskphone" => Some(Self::Deskphone),
            "desktop_access_disabled" => Some(Self::DesktopAccessDisabled),
            "desktop_cloud" => Some(Self::DesktopCloud),
            "desktop_cloud_stack" => Some(Self::DesktopCloudStack),
            "desktop_landscape" => Some(Self::DesktopLandscape),
            "desktop_landscape_add" => Some(Self::DesktopLandscapeAdd),
            "desktop_mac" => Some(Self::DesktopMac),
            "desktop_portrait" => Some(Self::DesktopPortrait),
            "desktop_windows" => Some(Self::DesktopWindows),
            "destruction" => Some(Self::Destruction),
            "details" => Some(Self::Details),
            "detection_and_zone" => Some(Self::DetectionAndZone),
            "detection_and_zone_off" => Some(Self::DetectionAndZoneOff),
            "detector" => Some(Self::Detector),
            "detector_alarm" => Some(Self::DetectorAlarm),
            "detector_battery" => Some(Self::DetectorBattery),
            "detector_co" => Some(Self::DetectorCo),
            "detector_offline" => Some(Self::DetectorOffline),
            "detector_smoke" => Some(Self::DetectorSmoke),
            "detector_status" => Some(Self::DetectorStatus),
            "developer_board" => Some(Self::DeveloperBoard),
            "developer_board_off" => Some(Self::DeveloperBoardOff),
            "developer_guide" => Some(Self::DeveloperGuide),
            "developer_mode" => Some(Self::DeveloperMode),
            "developer_mode_tv" => Some(Self::DeveloperModeTv),
            "device_band" => Some(Self::DeviceBand),
            "device_hub" => Some(Self::DeviceHub),
            "device_reset" => Some(Self::DeviceReset),
            "device_swoosh_star" => Some(Self::DeviceSwooshStar),
            "device_thermostat" => Some(Self::DeviceThermostat),
            "device_unknown" => Some(Self::DeviceUnknown),
            "devices" => Some(Self::Devices),
            "devices_fold" => Some(Self::DevicesFold),
            "devices_fold_2" => Some(Self::DevicesFold2),
            "devices_off" => Some(Self::DevicesOff),
            "devices_other" => Some(Self::DevicesOther),
            "devices_wearables" => Some(Self::DevicesWearables),
            "dew_point" => Some(Self::DewPoint),
            "diagnosis" => Some(Self::Diagnosis),
            "diagonal_line" => Some(Self::DiagonalLine),
            "dialer_sip" => Some(Self::DialerSip),
            "dialogs" => Some(Self::Dialogs),
            "dialpad" => Some(Self::Dialpad),
            "diamond" => Some(Self::Diamond),
            "diamond_shine" => Some(Self::DiamondShine),
            "dictionary" => Some(Self::Dictionary),
            "difference" => Some(Self::Difference),
            "digital_out_of_home" => Some(Self::DigitalOutOfHome),
            "digital_wellbeing" => Some(Self::DigitalWellbeing),
            "dine_heart" => Some(Self::DineHeart),
            "dine_in" => Some(Self::DineIn),
            "dine_lamp" => Some(Self::DineLamp),
            "dining" => Some(Self::Dining),
            "dinner_dining" => Some(Self::DinnerDining),
            "directions" => Some(Self::Directions),
            "directions_alt" => Some(Self::DirectionsAlt),
            "directions_alt_off" => Some(Self::DirectionsAltOff),
            "directions_bike" => Some(Self::DirectionsBike),
            "directions_boat" => Some(Self::DirectionsBoat),
            "directions_boat_filled" => Some(Self::DirectionsBoatFilled),
            "directions_bus" => Some(Self::DirectionsBus),
            "directions_bus_filled" => Some(Self::DirectionsBusFilled),
            "directions_car" => Some(Self::DirectionsCar),
            "directions_car_filled" => Some(Self::DirectionsCarFilled),
            "directions_off" => Some(Self::DirectionsOff),
            "directions_railway" => Some(Self::DirectionsRailway),
            "directions_railway_2" => Some(Self::DirectionsRailway2),
            "directions_railway_filled" => Some(Self::DirectionsRailwayFilled),
            "directions_run" => Some(Self::DirectionsRun),
            "directions_subway" => Some(Self::DirectionsSubway),
            "directions_subway_filled" => Some(Self::DirectionsSubwayFilled),
            "directions_transit" => Some(Self::DirectionsTransit),
            "directions_transit_filled" => Some(Self::DirectionsTransitFilled),
            "directions_walk" => Some(Self::DirectionsWalk),
            "directory_sync" => Some(Self::DirectorySync),
            "dirty_lens" => Some(Self::DirtyLens),
            "disabled_by_default" => Some(Self::DisabledByDefault),
            "disabled_visible" => Some(Self::DisabledVisible),
            "disc_full" => Some(Self::DiscFull),
            "discover_tune" => Some(Self::DiscoverTune),
            "dishwasher" => Some(Self::Dishwasher),
            "dishwasher_gen" => Some(Self::DishwasherGen),
            "display_add" => Some(Self::DisplayAdd),
            "display_external_input" => Some(Self::DisplayExternalInput),
            "display_group" => Some(Self::DisplayGroup),
            "display_settings" => Some(Self::DisplaySettings),
            "distance" => Some(Self::Distance),
            "diversity_1" => Some(Self::Diversity1),
            "diversity_2" => Some(Self::Diversity2),
            "diversity_3" => Some(Self::Diversity3),
            "diversity_4" => Some(Self::Diversity4),
            "dns" => Some(Self::Dns),
            "do_disturb" => Some(Self::DoDisturb),
            "do_disturb_alt" => Some(Self::DoDisturbAlt),
            "do_disturb_off" => Some(Self::DoDisturbOff),
            "do_disturb_on" => Some(Self::DoDisturbOn),
            "do_not_disturb" => Some(Self::DoNotDisturb),
            "do_not_disturb_alt" => Some(Self::DoNotDisturbAlt),
            "do_not_disturb_off" => Some(Self::DoNotDisturbOff),
            "do_not_disturb_on" => Some(Self::DoNotDisturbOn),
            "do_not_disturb_on_total_silence" => Some(Self::DoNotDisturbOnTotalSilence),
            "do_not_step" => Some(Self::DoNotStep),
            "do_not_touch" => Some(Self::DoNotTouch),
            "dock" => Some(Self::Dock),
            "dock_to_bottom" => Some(Self::DockToBottom),
            "dock_to_left" => Some(Self::DockToLeft),
            "dock_to_right" => Some(Self::DockToRight),
            "docs" => Some(Self::Docs),
            "docs_add_on" => Some(Self::DocsAddOn),
            "docs_apps_script" => Some(Self::DocsAppsScript),
            "document_scanner" => Some(Self::DocumentScanner),
            "document_search" => Some(Self::DocumentSearch),
            "document_share" => Some(Self::DocumentShare),
            "domain" => Some(Self::Domain),
            "domain_add" => Some(Self::DomainAdd),
            "domain_disabled" => Some(Self::DomainDisabled),
            "domain_disabled_check" => Some(Self::DomainDisabledCheck),
            "domain_verification" => Some(Self::DomainVerification),
            "domain_verification_off" => Some(Self::DomainVerificationOff),
            "domino_mask" => Some(Self::DominoMask),
            "done" => Some(Self::Done),
            "done_all" => Some(Self::DoneAll),
            "done_outline" => Some(Self::DoneOutline),
            "donut_large" => Some(Self::DonutLarge),
            "donut_small" => Some(Self::DonutSmall),
            "door_back" => Some(Self::DoorBack),
            "door_front" => Some(Self::DoorFront),
            "door_open" => Some(Self::DoorOpen),
            "door_sensor" => Some(Self::DoorSensor),
            "door_sliding" => Some(Self::DoorSliding),
            "doorbell" => Some(Self::Doorbell),
            "doorbell_3p" => Some(Self::Doorbell3p),
            "doorbell_chime" => Some(Self::DoorbellChime),
            "double_arrow" => Some(Self::DoubleArrow),
            "downhill_skiing" => Some(Self::DownhillSkiing),
            "download_2" => Some(Self::Download2),
            "download_done" => Some(Self::DownloadDone),
            "download_for_offline" => Some(Self::DownloadForOffline),
            "downloading" => Some(Self::Downloading),
            "draft" => Some(Self::Draft),
            "draft_orders" => Some(Self::DraftOrders),
            "drafts" => Some(Self::Drafts),
            "drag_click" => Some(Self::DragClick),
            "drag_handle" => Some(Self::DragHandle),
            "drag_indicator" => Some(Self::DragIndicator),
            "drag_pan" => Some(Self::DragPan),
            "draw" => Some(Self::Draw),
            "draw_abstract" => Some(Self::DrawAbstract),
            "draw_collage" => Some(Self::DrawCollage),
            "drawing_recognition" => Some(Self::DrawingRecognition),
            "dresser" => Some(Self::Dresser),
            "drive_eta" => Some(Self::DriveEta),
            "drive_export" => Some(Self::DriveExport),
            "drive_file_move" => Some(Self::DriveFileMove),
            "drive_file_move_outline" => Some(Self::DriveFileMoveOutline),
            "drive_file_move_rtl" => Some(Self::DriveFileMoveRtl),
            "drive_file_rename" => Some(Self::DriveFileRename),
            "drive_file_rename_outline" => Some(Self::DriveFileRenameOutline),
            "drive_folder_upload" => Some(Self::DriveFolderUpload),
            "drone" => Some(Self::Drone),
            "drone_2" => Some(Self::Drone2),
            "dropdown" => Some(Self::Dropdown),
            "dropdown_menu" => Some(Self::DropdownMenu),
            "dropper_eye" => Some(Self::DropperEye),
            "dry" => Some(Self::Dry),
            "dry_cleaning" => Some(Self::DryCleaning),
            "dual_screen" => Some(Self::DualScreen),
            "duo" => Some(Self::Duo),
            "dvr" => Some(Self::Dvr),
            "dynamic_feed" => Some(Self::DynamicFeed),
            "dynamic_form" => Some(Self::DynamicForm),
            "e911_avatar" => Some(Self::E911Avatar),
            "e911_emergency" => Some(Self::E911Emergency),
            "e_mobiledata" => Some(Self::EMobiledata),
            "e_mobiledata_badge" => Some(Self::EMobiledataBadge),
            "ear_sound" => Some(Self::EarSound),
            "earbud_case" => Some(Self::EarbudCase),
            "earbud_left" => Some(Self::EarbudLeft),
            "earbud_right" => Some(Self::EarbudRight),
            "earbuds" => Some(Self::Earbuds),
            "earbuds_2" => Some(Self::Earbuds2),
            "earbuds_battery" => Some(Self::EarbudsBattery),
            "early_on" => Some(Self::EarlyOn),
            "earthquake" => Some(Self::Earthquake),
            "east" => Some(Self::East),
            "ecg" => Some(Self::Ecg),
            "ecg_heart" => Some(Self::EcgHeart),
            "eco" => Some(Self::Eco),
            "eda" => Some(Self::Eda),
            "edgesensor_high" => Some(Self::EdgesensorHigh),
            "edgesensor_low" => Some(Self::EdgesensorLow),
            "edit_arrow_down" => Some(Self::EditArrowDown),
            "edit_arrow_up" => Some(Self::EditArrowUp),
            "edit_attributes" => Some(Self::EditAttributes),
            "edit_audio" => Some(Self::EditAudio),
            "edit_calendar" => Some(Self::EditCalendar),
            "edit_document" => Some(Self::EditDocument),
            "edit_line" => Some(Self::EditLine),
            "edit_location" => Some(Self::EditLocation),
            "edit_location_alt" => Some(Self::EditLocationAlt),
            "edit_note" => Some(Self::EditNote),
            "edit_notifications" => Some(Self::EditNotifications),
            "edit_off" => Some(Self::EditOff),
            "edit_road" => Some(Self::EditRoad),
            "edit_square" => Some(Self::EditSquare),
            "editor_choice" => Some(Self::EditorChoice),
            "egg" => Some(Self::Egg),
            "egg_alt" => Some(Self::EggAlt),
            "eject" => Some(Self::Eject),
            "elderly" => Some(Self::Elderly),
            "elderly_woman" => Some(Self::ElderlyWoman),
            "electric_bike" => Some(Self::ElectricBike),
            "electric_bolt" => Some(Self::ElectricBolt),
            "electric_car" => Some(Self::ElectricCar),
            "electric_meter" => Some(Self::ElectricMeter),
            "electric_moped" => Some(Self::ElectricMoped),
            "electric_rickshaw" => Some(Self::ElectricRickshaw),
            "electric_scooter" => Some(Self::ElectricScooter),
            "electrical_services" => Some(Self::ElectricalServices),
            "elevation" => Some(Self::Elevation),
            "elevator" => Some(Self::Elevator),
            "email" => Some(Self::Email),
            "emergency" => Some(Self::Emergency),
            "emergency_heat" => Some(Self::EmergencyHeat),
            "emergency_heat_2" => Some(Self::EmergencyHeat2),
            "emergency_home" => Some(Self::EmergencyHome),
            "emergency_recording" => Some(Self::EmergencyRecording),
            "emergency_share" => Some(Self::EmergencyShare),
            "emergency_share_off" => Some(Self::EmergencyShareOff),
            "emoji_emotions" => Some(Self::EmojiEmotions),
            "emoji_events" => Some(Self::EmojiEvents),
            "emoji_flags" => Some(Self::EmojiFlags),
            "emoji_food_beverage" => Some(Self::EmojiFoodBeverage),
            "emoji_language" => Some(Self::EmojiLanguage),
            "emoji_nature" => Some(Self::EmojiNature),
            "emoji_objects" => Some(Self::EmojiObjects),
            "emoji_people" => Some(Self::EmojiPeople),
            "emoji_symbols" => Some(Self::EmojiSymbols),
            "emoji_transportation" => Some(Self::EmojiTransportation),
            "emoticon" => Some(Self::Emoticon),
            "empty_dashboard" => Some(Self::EmptyDashboard),
            "enable" => Some(Self::Enable),
            "encrypted" => Some(Self::Encrypted),
            "encrypted_add" => Some(Self::EncryptedAdd),
            "encrypted_add_circle" => Some(Self::EncryptedAddCircle),
            "encrypted_minus_circle" => Some(Self::EncryptedMinusCircle),
            "encrypted_off" => Some(Self::EncryptedOff),
            "endocrinology" => Some(Self::Endocrinology),
            "energy" => Some(Self::Energy),
            "energy_program_saving" => Some(Self::EnergyProgramSaving),
            "energy_program_time_used" => Some(Self::EnergyProgramTimeUsed),
            "energy_savings_leaf" => Some(Self::EnergySavingsLeaf),
            "engineering" => Some(Self::Engineering),
            "enhanced_encryption" => Some(Self::EnhancedEncryption),
            "ent" => Some(Self::Ent),
            "enterprise" => Some(Self::Enterprise),
            "enterprise_off" => Some(Self::EnterpriseOff),
            "equal" => Some(Self::Equal),
            "equalizer" => Some(Self::Equalizer),
            "eraser_size_1" => Some(Self::EraserSize1),
            "eraser_size_2" => Some(Self::EraserSize2),
            "eraser_size_3" => Some(Self::EraserSize3),
            "eraser_size_4" => Some(Self::EraserSize4),
            "eraser_size_5" => Some(Self::EraserSize5),
            "error_circle_rounded" => Some(Self::ErrorCircleRounded),
            "error_med" => Some(Self::ErrorMed),
            "error_outline" => Some(Self::ErrorOutline),
            "escalator" => Some(Self::Escalator),
            "escalator_warning" => Some(Self::EscalatorWarning),
            "euro" => Some(Self::Euro),
            "euro_symbol" => Some(Self::EuroSymbol),
            "ev_charger" => Some(Self::EvCharger),
            "ev_mobiledata_badge" => Some(Self::EvMobiledataBadge),
            "ev_shadow" => Some(Self::EvShadow),
            "ev_shadow_add" => Some(Self::EvShadowAdd),
            "ev_shadow_minus" => Some(Self::EvShadowMinus),
            "ev_station" => Some(Self::EvStation),
            "event" => Some(Self::Event),
            "event_available" => Some(Self::EventAvailable),
            "event_busy" => Some(Self::EventBusy),
            "event_list" => Some(Self::EventList),
            "event_note" => Some(Self::EventNote),
            "event_repeat" => Some(Self::EventRepeat),
            "event_seat" => Some(Self::EventSeat),
            "event_upcoming" => Some(Self::EventUpcoming),
            "exclamation" => Some(Self::Exclamation),
            "exercise" => Some(Self::Exercise),
            "exit_to_app" => Some(Self::ExitToApp),
            "expand" => Some(Self::Expand),
            "expand_all" => Some(Self::ExpandAll),
            "expand_circle_down" => Some(Self::ExpandCircleDown),
            "expand_circle_right" => Some(Self::ExpandCircleRight),
            "expand_circle_up" => Some(Self::ExpandCircleUp),
            "expand_content" => Some(Self::ExpandContent),
            "expand_less" => Some(Self::ExpandLess),
            "expand_more" => Some(Self::ExpandMore),
            "expansion_panels" => Some(Self::ExpansionPanels),
            "expension_panels" => Some(Self::ExpensionPanels),
            "experiment" => Some(Self::Experiment),
            "explicit" => Some(Self::Explicit),
            "explore" => Some(Self::Explore),
            "explore_nearby" => Some(Self::ExploreNearby),
            "explore_off" => Some(Self::ExploreOff),
            "explosion" => Some(Self::Explosion),
            "export_notes" => Some(Self::ExportNotes),
            "exposure" => Some(Self::Exposure),
            "exposure_neg_1" => Some(Self::ExposureNeg1),
            "exposure_neg_2" => Some(Self::ExposureNeg2),
            "exposure_plus_1" => Some(Self::ExposurePlus1),
            "exposure_plus_2" => Some(Self::ExposurePlus2),
            "exposure_zero" => Some(Self::ExposureZero),
            "extension" => Some(Self::Extension),
            "extension_off" => Some(Self::ExtensionOff),
            "eye_tracking" => Some(Self::EyeTracking),
            "eyebrow" => Some(Self::Eyebrow),
            "eyeglasses" => Some(Self::Eyeglasses),
            "eyeglasses_2" => Some(Self::Eyeglasses2),
            "eyeglasses_2_sound" => Some(Self::Eyeglasses2Sound),
            "eyeglasses_3" => Some(Self::Eyeglasses3),
            "face" => Some(Self::Face),
            "face_2" => Some(Self::Face2),
            "face_3" => Some(Self::Face3),
            "face_4" => Some(Self::Face4),
            "face_5" => Some(Self::Face5),
            "face_6" => Some(Self::Face6),
            "face_down" => Some(Self::FaceDown),
            "face_left" => Some(Self::FaceLeft),
            "face_nod" => Some(Self::FaceNod),
            "face_retouching_natural" => Some(Self::FaceRetouchingNatural),
            "face_retouching_off" => Some(Self::FaceRetouchingOff),
            "face_right" => Some(Self::FaceRight),
            "face_shake" => Some(Self::FaceShake),
            "face_unlock" => Some(Self::FaceUnlock),
            "face_up" => Some(Self::FaceUp),
            "fact_check" => Some(Self::FactCheck),
            "factory" => Some(Self::Factory),
            "falling" => Some(Self::Falling),
            "familiar_face_and_zone" => Some(Self::FamiliarFaceAndZone),
            "family_group" => Some(Self::FamilyGroup),
            "family_history" => Some(Self::FamilyHistory),
            "family_home" => Some(Self::FamilyHome),
            "family_link" => Some(Self::FamilyLink),
            "family_restroom" => Some(Self::FamilyRestroom),
            "family_star" => Some(Self::FamilyStar),
            "fan_focus" => Some(Self::FanFocus),
            "fan_indirect" => Some(Self::FanIndirect),
            "farsight_digital" => Some(Self::FarsightDigital),
            "fast_forward" => Some(Self::FastForward),
            "fast_rewind" => Some(Self::FastRewind),
            "fastfood" => Some(Self::Fastfood),
            "faucet" => Some(Self::Faucet),
            "favorite" => Some(Self::Favorite),
            "favorite_border" => Some(Self::FavoriteBorder),
            "fax" => Some(Self::Fax),
            "feature_search" => Some(Self::FeatureSearch),
            "featured_play_list" => Some(Self::FeaturedPlayList),
            "featured_seasonal_and_gifts" => Some(Self::FeaturedSeasonalAndGifts),
            "featured_video" => Some(Self::FeaturedVideo),
            "feed" => Some(Self::Feed),
            "feedback" => Some(Self::Feedback),
            "female" => Some(Self::Female),
            "femur" => Some(Self::Femur),
            "femur_alt" => Some(Self::FemurAlt),
            "fence" => Some(Self::Fence),
            "fertile" => Some(Self::Fertile),
            "festival" => Some(Self::Festival),
            "fiber_dvr" => Some(Self::FiberDvr),
            "fiber_manual_record" => Some(Self::FiberManualRecord),
            "fiber_new" => Some(Self::FiberNew),
            "fiber_pin" => Some(Self::FiberPin),
            "fiber_smart_record" => Some(Self::FiberSmartRecord),
            "file_copy" => Some(Self::FileCopy),
            "file_copy_off" => Some(Self::FileCopyOff),
            "file_download" => Some(Self::FileDownload),
            "file_download_done" => Some(Self::FileDownloadDone),
            "file_download_off" => Some(Self::FileDownloadOff),
            "file_export" => Some(Self::FileExport),
            "file_json" => Some(Self::FileJson),
            "file_map" => Some(Self::FileMap),
            "file_map_stack" => Some(Self::FileMapStack),
            "file_open" => Some(Self::FileOpen),
            "file_png" => Some(Self::FilePng),
            "file_present" => Some(Self::FilePresent),
            "file_save" => Some(Self::FileSave),
            "file_save_off" => Some(Self::FileSaveOff),
            "file_upload" => Some(Self::FileUpload),
            "file_upload_off" => Some(Self::FileUploadOff),
            "files" => Some(Self::Files),
            "filter_1" => Some(Self::Filter1),
            "filter_2" => Some(Self::Filter2),
            "filter_3" => Some(Self::Filter3),
            "filter_4" => Some(Self::Filter4),
            "filter_5" => Some(Self::Filter5),
            "filter_6" => Some(Self::Filter6),
            "filter_7" => Some(Self::Filter7),
            "filter_8" => Some(Self::Filter8),
            "filter_9" => Some(Self::Filter9),
            "filter_9_plus" => Some(Self::Filter9Plus),
            "filter_alt" => Some(Self::FilterAlt),
            "filter_alt_off" => Some(Self::FilterAltOff),
            "filter_arrow_right" => Some(Self::FilterArrowRight),
            "filter_b_and_w" => Some(Self::FilterBAndW),
            "filter_cancel" => Some(Self::FilterCancel),
            "filter_center_focus" => Some(Self::FilterCenterFocus),
            "filter_drama" => Some(Self::FilterDrama),
            "filter_frames" => Some(Self::FilterFrames),
            "filter_hdr" => Some(Self::FilterHdr),
            "filter_list" => Some(Self::FilterList),
            "filter_list_alt" => Some(Self::FilterListAlt),
            "filter_list_off" => Some(Self::FilterListOff),
            "filter_none" => Some(Self::FilterNone),
            "filter_plus" => Some(Self::FilterPlus),
            "filter_retrolux" => Some(Self::FilterRetrolux),
            "filter_tilt_shift" => Some(Self::FilterTiltShift),
            "filter_vintage" => Some(Self::FilterVintage),
            "finance" => Some(Self::Finance),
            "finance_chip" => Some(Self::FinanceChip),
            "finance_mode" => Some(Self::FinanceMode),
            "find_in_page" => Some(Self::FindInPage),
            "find_replace" => Some(Self::FindReplace),
            "fingerprint" => Some(Self::Fingerprint),
            "fingerprint_off" => Some(Self::FingerprintOff),
            "fire_check" => Some(Self::FireCheck),
            "fire_extinguisher" => Some(Self::FireExtinguisher),
            "fire_hydrant" => Some(Self::FireHydrant),
            "fire_truck" => Some(Self::FireTruck),
            "fireplace" => Some(Self::Fireplace),
            "fit_page" => Some(Self::FitPage),
            "fit_page_height" => Some(Self::FitPageHeight),
            "fit_page_width" => Some(Self::FitPageWidth),
            "fit_screen" => Some(Self::FitScreen),
            "fit_width" => Some(Self::FitWidth),
            "fitness_center" => Some(Self::FitnessCenter),
            "fitness_tracker" => Some(Self::FitnessTracker),
            "fitness_trackers" => Some(Self::FitnessTrackers),
            "flag" => Some(Self::Flag),
            "flag_2" => Some(Self::Flag2),
            "flag_check" => Some(Self::FlagCheck),
            "flag_circle" => Some(Self::FlagCircle),
            "flag_filled" => Some(Self::FlagFilled),
            "flaky" => Some(Self::Flaky),
            "flare" => Some(Self::Flare),
            "flash_auto" => Some(Self::FlashAuto),
            "flash_off" => Some(Self::FlashOff),
            "flash_on" => Some(Self::FlashOn),
            "flashlight_off" => Some(Self::FlashlightOff),
            "flashlight_on" => Some(Self::FlashlightOn),
            "flatware" => Some(Self::Flatware),
            "flex_direction" => Some(Self::FlexDirection),
            "flex_no_wrap" => Some(Self::FlexNoWrap),
            "flex_wrap" => Some(Self::FlexWrap),
            "flight" => Some(Self::Flight),
            "flight_class" => Some(Self::FlightClass),
            "flight_land" => Some(Self::FlightLand),
            "flight_takeoff" => Some(Self::FlightTakeoff),
            "flights_and_hotels" => Some(Self::FlightsAndHotels),
            "flightsmode" => Some(Self::Flightsmode),
            "flip" => Some(Self::Flip),
            "flip_camera_android" => Some(Self::FlipCameraAndroid),
            "flip_camera_ios" => Some(Self::FlipCameraIos),
            "flip_to_back" => Some(Self::FlipToBack),
            "flip_to_front" => Some(Self::FlipToFront),
            "float_landscape_2" => Some(Self::FloatLandscape2),
            "float_portrait_2" => Some(Self::FloatPortrait2),
            "flood" => Some(Self::Flood),
            "floor" => Some(Self::Floor),
            "floor_lamp" => Some(Self::FloorLamp),
            "flourescent" => Some(Self::Flourescent),
            "flowchart" => Some(Self::Flowchart),
            "flowsheet" => Some(Self::Flowsheet),
            "fluid" => Some(Self::Fluid),
            "fluid_balance" => Some(Self::FluidBalance),
            "fluid_med" => Some(Self::FluidMed),
            "fluorescent" => Some(Self::Fluorescent),
            "flutter" => Some(Self::Flutter),
            "flutter_dash" => Some(Self::FlutterDash),
            "flyover" => Some(Self::Flyover),
            "fmd_bad" => Some(Self::FmdBad),
            "fmd_good" => Some(Self::FmdGood),
            "foggy" => Some(Self::Foggy),
            "folded_hands" => Some(Self::FoldedHands),
            "folder_check" => Some(Self::FolderCheck),
            "folder_check_2" => Some(Self::FolderCheck2),
            "folder_code" => Some(Self::FolderCode),
            "folder_copy" => Some(Self::FolderCopy),
            "folder_data" => Some(Self::FolderData),
            "folder_delete" => Some(Self::FolderDelete),
            "folder_eye" => Some(Self::FolderEye),
            "folder_info" => Some(Self::FolderInfo),
            "folder_limited" => Some(Self::FolderLimited),
            "folder_managed" => Some(Self::FolderManaged),
            "folder_match" => Some(Self::FolderMatch),
            "folder_off" => Some(Self::FolderOff),
            "folder_shared" => Some(Self::FolderShared),
            "folder_special" => Some(Self::FolderSpecial),
            "folder_supervised" => Some(Self::FolderSupervised),
            "folder_zip" => Some(Self::FolderZip),
            "follow_the_signs" => Some(Self::FollowTheSigns),
            "font_download" => Some(Self::FontDownload),
            "font_download_off" => Some(Self::FontDownloadOff),
            "food_bank" => Some(Self::FoodBank),
            "foot_bones" => Some(Self::FootBones),
            "footprint" => Some(Self::Footprint),
            "for_you" => Some(Self::ForYou),
            "forest" => Some(Self::Forest),
            "fork_chart" => Some(Self::ForkChart),
            "fork_left" => Some(Self::ForkLeft),
            "fork_right" => Some(Self::ForkRight),
            "fork_spoon" => Some(Self::ForkSpoon),
            "forklift" => Some(Self::Forklift),
            "format_align_center" => Some(Self::FormatAlignCenter),
            "format_align_justify" => Some(Self::FormatAlignJustify),
            "format_align_left" => Some(Self::FormatAlignLeft),
            "format_align_right" => Some(Self::FormatAlignRight),
            "format_bold" => Some(Self::FormatBold),
            "format_clear" => Some(Self::FormatClear),
            "format_color_fill" => Some(Self::FormatColorFill),
            "format_color_reset" => Some(Self::FormatColorReset),
            "format_color_text" => Some(Self::FormatColorText),
            "format_h1" => Some(Self::FormatH1),
            "format_h2" => Some(Self::FormatH2),
            "format_h3" => Some(Self::FormatH3),
            "format_h4" => Some(Self::FormatH4),
            "format_h5" => Some(Self::FormatH5),
            "format_h6" => Some(Self::FormatH6),
            "format_image_back" => Some(Self::FormatImageBack),
            "format_image_break_left" => Some(Self::FormatImageBreakLeft),
            "format_image_break_right" => Some(Self::FormatImageBreakRight),
            "format_image_front" => Some(Self::FormatImageFront),
            "format_image_inline_left" => Some(Self::FormatImageInlineLeft),
            "format_image_inline_right" => Some(Self::FormatImageInlineRight),
            "format_image_left" => Some(Self::FormatImageLeft),
            "format_image_right" => Some(Self::FormatImageRight),
            "format_indent_decrease" => Some(Self::FormatIndentDecrease),
            "format_indent_increase" => Some(Self::FormatIndentIncrease),
            "format_ink_highlighter" => Some(Self::FormatInkHighlighter),
            "format_italic" => Some(Self::FormatItalic),
            "format_letter_spacing" => Some(Self::FormatLetterSpacing),
            "format_letter_spacing_2" => Some(Self::FormatLetterSpacing2),
            "format_letter_spacing_standard" => Some(Self::FormatLetterSpacingStandard),
            "format_letter_spacing_wide" => Some(Self::FormatLetterSpacingWide),
            "format_letter_spacing_wider" => Some(Self::FormatLetterSpacingWider),
            "format_line_spacing" => Some(Self::FormatLineSpacing),
            "format_list_bulleted" => Some(Self::FormatListBulleted),
            "format_list_bulleted_add" => Some(Self::FormatListBulletedAdd),
            "format_list_numbered" => Some(Self::FormatListNumbered),
            "format_list_numbered_rtl" => Some(Self::FormatListNumberedRtl),
            "format_overline" => Some(Self::FormatOverline),
            "format_paint" => Some(Self::FormatPaint),
            "format_paint_off" => Some(Self::FormatPaintOff),
            "format_paragraph" => Some(Self::FormatParagraph),
            "format_quote" => Some(Self::FormatQuote),
            "format_quote_off" => Some(Self::FormatQuoteOff),
            "format_shapes" => Some(Self::FormatShapes),
            "format_size" => Some(Self::FormatSize),
            "format_strikethrough" => Some(Self::FormatStrikethrough),
            "format_text_clip" => Some(Self::FormatTextClip),
            "format_text_overflow" => Some(Self::FormatTextOverflow),
            "format_text_wrap" => Some(Self::FormatTextWrap),
            "format_textdirection_l_to_r" => Some(Self::FormatTextdirectionLToR),
            "format_textdirection_r_to_l" => Some(Self::FormatTextdirectionRToL),
            "format_textdirection_vertical" => Some(Self::FormatTextdirectionVertical),
            "format_underlined" => Some(Self::FormatUnderlined),
            "format_underlined_squiggle" => Some(Self::FormatUnderlinedSquiggle),
            "forms_add_on" => Some(Self::FormsAddOn),
            "forms_apps_script" => Some(Self::FormsAppsScript),
            "fort" => Some(Self::Fort),
            "forum" => Some(Self::Forum),
            "forward" => Some(Self::Forward),
            "forward_10" => Some(Self::Forward10),
            "forward_30" => Some(Self::Forward30),
            "forward_5" => Some(Self::Forward5),
            "forward_circle" => Some(Self::ForwardCircle),
            "forward_media" => Some(Self::ForwardMedia),
            "forward_to_inbox" => Some(Self::ForwardToInbox),
            "foundation" => Some(Self::Foundation),
            "fragrance" => Some(Self::Fragrance),
            "frame_bug" => Some(Self::FrameBug),
            "frame_exclamation" => Some(Self::FrameExclamation),
            "frame_inspect" => Some(Self::FrameInspect),
            "frame_person" => Some(Self::FramePerson),
            "frame_person_mic" => Some(Self::FramePersonMic),
            "frame_person_off" => Some(Self::FramePersonOff),
            "frame_reload" => Some(Self::FrameReload),
            "frame_source" => Some(Self::FrameSource),
            "free_breakfast" => Some(Self::FreeBreakfast),
            "free_cancellation" => Some(Self::FreeCancellation),
            "front_hand" => Some(Self::FrontHand),
            "front_loader" => Some(Self::FrontLoader),
            "full_coverage" => Some(Self::FullCoverage),
            "full_hd" => Some(Self::FullHd),
            "full_stacked_bar_chart" => Some(Self::FullStackedBarChart),
            "fullscreen_exit" => Some(Self::FullscreenExit),
            "fullscreen_portrait" => Some(Self::FullscreenPortrait),
            "function" => Some(Self::Function),
            "function_search" => Some(Self::FunctionSearch),
            "functions" => Some(Self::Functions),
            "funicular" => Some(Self::Funicular),
            "g_mobiledata" => Some(Self::GMobiledata),
            "g_mobiledata_badge" => Some(Self::GMobiledataBadge),
            "g_translate" => Some(Self::GTranslate),
            "gallery_thumbnail" => Some(Self::GalleryThumbnail),
            "game_bumper_left" => Some(Self::GameBumperLeft),
            "game_bumper_right" => Some(Self::GameBumperRight),
            "game_button_l" => Some(Self::GameButtonL),
            "game_button_l1" => Some(Self::GameButtonL1),
            "game_button_l2" => Some(Self::GameButtonL2),
            "game_button_r" => Some(Self::GameButtonR),
            "game_button_r1" => Some(Self::GameButtonR1),
            "game_button_r2" => Some(Self::GameButtonR2),
            "game_button_zl" => Some(Self::GameButtonZl),
            "game_button_zr" => Some(Self::GameButtonZr),
            "game_stick_l3" => Some(Self::GameStickL3),
            "game_stick_left" => Some(Self::GameStickLeft),
            "game_stick_r3" => Some(Self::GameStickR3),
            "game_stick_right" => Some(Self::GameStickRight),
            "game_trigger_left" => Some(Self::GameTriggerLeft),
            "game_trigger_right" => Some(Self::GameTriggerRight),
            "gamepad" => Some(Self::Gamepad),
            "gamepad_circle_down" => Some(Self::GamepadCircleDown),
            "gamepad_circle_left" => Some(Self::GamepadCircleLeft),
            "gamepad_circle_right" => Some(Self::GamepadCircleRight),
            "gamepad_circle_up" => Some(Self::GamepadCircleUp),
            "gamepad_down" => Some(Self::GamepadDown),
            "gamepad_left" => Some(Self::GamepadLeft),
            "gamepad_right" => Some(Self::GamepadRight),
            "gamepad_up" => Some(Self::GamepadUp),
            "games" => Some(Self::Games),
            "garage" => Some(Self::Garage),
            "garage_check" => Some(Self::GarageCheck),
            "garage_door" => Some(Self::GarageDoor),
            "garage_door_open" => Some(Self::GarageDoorOpen),
            "garage_home" => Some(Self::GarageHome),
            "garage_money" => Some(Self::GarageMoney),
            "garden_cart" => Some(Self::GardenCart),
            "gas_meter" => Some(Self::GasMeter),
            "gastroenterology" => Some(Self::Gastroenterology),
            "gate" => Some(Self::Gate),
            "gavel" => Some(Self::Gavel),
            "general_device" => Some(Self::GeneralDevice),
            "generating_tokens" => Some(Self::GeneratingTokens),
            "genetics" => Some(Self::Genetics),
            "genres" => Some(Self::Genres),
            "gesture" => Some(Self::Gesture),
            "gesture_select" => Some(Self::GestureSelect),
            "get_app" => Some(Self::GetApp),
            "gif" => Some(Self::Gif),
            "gif_2" => Some(Self::Gif2),
            "gif_box" => Some(Self::GifBox),
            "girl" => Some(Self::Girl),
            "gite" => Some(Self::Gite),
            "glass_cup" => Some(Self::GlassCup),
            "globe" => Some(Self::Globe),
            "globe_2_cancel" => Some(Self::Globe2Cancel),
            "globe_2_question" => Some(Self::Globe2Question),
            "globe_asia" => Some(Self::GlobeAsia),
            "globe_book" => Some(Self::GlobeBook),
            "globe_clock" => Some(Self::GlobeClock),
            "globe_location_pin" => Some(Self::GlobeLocationPin),
            "globe_uk" => Some(Self::GlobeUk),
            "glucose" => Some(Self::Glucose),
            "glyphs" => Some(Self::Glyphs),
            "go_to_line" => Some(Self::GoToLine),
            "golf_course" => Some(Self::GolfCourse),
            "gondola_lift" => Some(Self::GondolaLift),
            "google_home_devices" => Some(Self::GoogleHomeDevices),
            "google_plus_reshare" => Some(Self::GooglePlusReshare),
            "google_tv_remote" => Some(Self::GoogleTvRemote),
            "google_wifi" => Some(Self::GoogleWifi),
            "gpp_bad" => Some(Self::GppBad),
            "gpp_good" => Some(Self::GppGood),
            "gpp_maybe" => Some(Self::GppMaybe),
            "gps_fixed" => Some(Self::GpsFixed),
            "gps_not_fixed" => Some(Self::GpsNotFixed),
            "gps_off" => Some(Self::GpsOff),
            "grade" => Some(Self::Grade),
            "gradient" => Some(Self::Gradient),
            "grading" => Some(Self::Grading),
            "grain" => Some(Self::Grain),
            "graph_1" => Some(Self::Graph1),
            "graph_2" => Some(Self::Graph2),
            "graph_3" => Some(Self::Graph3),
            "graph_4" => Some(Self::Graph4),
            "graph_5" => Some(Self::Graph5),
            "graph_6" => Some(Self::Graph6),
            "graph_7" => Some(Self::Graph7),
            "graph_8" => Some(Self::Graph8),
            "graphic_eq" => Some(Self::GraphicEq),
            "graphic_eq_off" => Some(Self::GraphicEqOff),
            "grass" => Some(Self::Grass),
            "grid_3x3" => Some(Self::Grid3x3),
            "grid_3x3_off" => Some(Self::Grid3x3Off),
            "grid_4x4" => Some(Self::Grid4x4),
            "grid_goldenratio" => Some(Self::GridGoldenratio),
            "grid_guides" => Some(Self::GridGuides),
            "grid_layout_side" => Some(Self::GridLayoutSide),
            "grid_off" => Some(Self::GridOff),
            "grid_on" => Some(Self::GridOn),
            "grid_view" => Some(Self::GridView),
            "grocery" => Some(Self::Grocery),
            "group" => Some(Self::Group),
            "group_add" => Some(Self::GroupAdd),
            "group_eye" => Some(Self::GroupEye),
            "group_off" => Some(Self::GroupOff),
            "group_remove" => Some(Self::GroupRemove),
            "group_search" => Some(Self::GroupSearch),
            "group_work" => Some(Self::GroupWork),
            "grouped_bar_chart" => Some(Self::GroupedBarChart),
            "groups" => Some(Self::Groups),
            "groups_2" => Some(Self::Groups2),
            "groups_3" => Some(Self::Groups3),
            "guardian" => Some(Self::Guardian),
            "gynecology" => Some(Self::Gynecology),
            "h_mobiledata" => Some(Self::HMobiledata),
            "h_mobiledata_badge" => Some(Self::HMobiledataBadge),
            "h_plus_mobiledata" => Some(Self::HPlusMobiledata),
            "h_plus_mobiledata_badge" => Some(Self::HPlusMobiledataBadge),
            "hail" => Some(Self::Hail),
            "hallway" => Some(Self::Hallway),
            "hanami_dango" => Some(Self::HanamiDango),
            "hand_bones" => Some(Self::HandBones),
            "hand_gesture" => Some(Self::HandGesture),
            "hand_gesture_off" => Some(Self::HandGestureOff),
            "hand_meal" => Some(Self::HandMeal),
            "hand_package" => Some(Self::HandPackage),
            "handheld_controller" => Some(Self::HandheldController),
            "handshake" => Some(Self::Handshake),
            "handwriting_recognition" => Some(Self::HandwritingRecognition),
            "handyman" => Some(Self::Handyman),
            "hangout_video" => Some(Self::HangoutVideo),
            "hangout_video_off" => Some(Self::HangoutVideoOff),
            "hard_disk" => Some(Self::HardDisk),
            "hard_drive" => Some(Self::HardDrive),
            "hard_drive_2" => Some(Self::HardDrive2),
            "hardware" => Some(Self::Hardware),
            "hd" => Some(Self::Hd),
            "hdr_auto" => Some(Self::HdrAuto),
            "hdr_auto_select" => Some(Self::HdrAutoSelect),
            "hdr_enhanced_select" => Some(Self::HdrEnhancedSelect),
            "hdr_off" => Some(Self::HdrOff),
            "hdr_off_select" => Some(Self::HdrOffSelect),
            "hdr_on" => Some(Self::HdrOn),
            "hdr_on_select" => Some(Self::HdrOnSelect),
            "hdr_plus" => Some(Self::HdrPlus),
            "hdr_plus_off" => Some(Self::HdrPlusOff),
            "hdr_strong" => Some(Self::HdrStrong),
            "hdr_weak" => Some(Self::HdrWeak),
            "head_mounted_device" => Some(Self::HeadMountedDevice),
            "headphones" => Some(Self::Headphones),
            "headphones_battery" => Some(Self::HeadphonesBattery),
            "headset" => Some(Self::Headset),
            "headset_mic" => Some(Self::HeadsetMic),
            "headset_off" => Some(Self::HeadsetOff),
            "healing" => Some(Self::Healing),
            "health_and_beauty" => Some(Self::HealthAndBeauty),
            "health_and_safety" => Some(Self::HealthAndSafety),
            "health_cross" => Some(Self::HealthCross),
            "health_metrics" => Some(Self::HealthMetrics),
            "heap_snapshot_large" => Some(Self::HeapSnapshotLarge),
            "heap_snapshot_multiple" => Some(Self::HeapSnapshotMultiple),
            "heap_snapshot_thumbnail" => Some(Self::HeapSnapshotThumbnail),
            "hearing" => Some(Self::Hearing),
            "hearing_aid" => Some(Self::HearingAid),
            "hearing_aid_disabled" => Some(Self::HearingAidDisabled),
            "hearing_aid_disabled_left" => Some(Self::HearingAidDisabledLeft),
            "hearing_aid_left" => Some(Self::HearingAidLeft),
            "hearing_disabled" => Some(Self::HearingDisabled),
            "heart_broken" => Some(Self::HeartBroken),
            "heart_check" => Some(Self::HeartCheck),
            "heart_minus" => Some(Self::HeartMinus),
            "heart_plus" => Some(Self::HeartPlus),
            "heart_smile" => Some(Self::HeartSmile),
            "heat" => Some(Self::Heat),
            "heat_pump" => Some(Self::HeatPump),
            "heat_pump_balance" => Some(Self::HeatPumpBalance),
            "height" => Some(Self::Height),
            "helicopter" => Some(Self::Helicopter),
            "help_center" => Some(Self::HelpCenter),
            "help_clinic" => Some(Self::HelpClinic),
            "help_outline" => Some(Self::HelpOutline),
            "hematology" => Some(Self::Hematology),
            "hevc" => Some(Self::Hevc),
            "hexagon" => Some(Self::Hexagon),
            "hide" => Some(Self::Hide),
            "hide_image" => Some(Self::HideImage),
            "hide_source" => Some(Self::HideSource),
            "high_chair" => Some(Self::HighChair),
            "high_density" => Some(Self::HighDensity),
            "high_quality" => Some(Self::HighQuality),
            "high_quality_off" => Some(Self::HighQualityOff),
            "high_res" => Some(Self::HighRes),
            "highlight" => Some(Self::Highlight),
            "highlight_alt" => Some(Self::HighlightAlt),
            "highlight_keyboard_focus" => Some(Self::HighlightKeyboardFocus),
            "highlight_mouse_cursor" => Some(Self::HighlightMouseCursor),
            "highlight_off" => Some(Self::HighlightOff),
            "highlight_text_cursor" => Some(Self::HighlightTextCursor),
            "highlighter_size_1" => Some(Self::HighlighterSize1),
            "highlighter_size_2" => Some(Self::HighlighterSize2),
            "highlighter_size_3" => Some(Self::HighlighterSize3),
            "highlighter_size_4" => Some(Self::HighlighterSize4),
            "highlighter_size_5" => Some(Self::HighlighterSize5),
            "hiking" => Some(Self::Hiking),
            "history" => Some(Self::History),
            "history_2" => Some(Self::History2),
            "history_edu" => Some(Self::HistoryEdu),
            "history_off" => Some(Self::HistoryOff),
            "history_toggle_off" => Some(Self::HistoryToggleOff),
            "hive" => Some(Self::Hive),
            "hls" => Some(Self::Hls),
            "hls_off" => Some(Self::HlsOff),
            "holiday_village" => Some(Self::HolidayVillage),
            "home_and_garden" => Some(Self::HomeAndGarden),
            "home_app_logo" => Some(Self::HomeAppLogo),
            "home_filled" => Some(Self::HomeFilled),
            "home_health" => Some(Self::HomeHealth),
            "home_improvement_and_tools" => Some(Self::HomeImprovementAndTools),
            "home_iot_device" => Some(Self::HomeIotDevice),
            "home_max" => Some(Self::HomeMax),
            "home_max_dots" => Some(Self::HomeMaxDots),
            "home_mini" => Some(Self::HomeMini),
            "home_pin" => Some(Self::HomePin),
            "home_repair_service" => Some(Self::HomeRepairService),
            "home_speaker" => Some(Self::HomeSpeaker),
            "home_storage" => Some(Self::HomeStorage),
            "home_storage_gear" => Some(Self::HomeStorageGear),
            "home_work" => Some(Self::HomeWork),
            "horizontal_align_center" => Some(Self::HorizontalAlignCenter),
            "horizontal_align_left" => Some(Self::HorizontalAlignLeft),
            "horizontal_align_right" => Some(Self::HorizontalAlignRight),
            "horizontal_distribute" => Some(Self::HorizontalDistribute),
            "horizontal_rule" => Some(Self::HorizontalRule),
            "horizontal_split" => Some(Self::HorizontalSplit),
            "host" => Some(Self::Host),
            "hot_tub" => Some(Self::HotTub),
            "hotel" => Some(Self::Hotel),
            "hotel_class" => Some(Self::HotelClass),
            "hourglass_arrow_down" => Some(Self::HourglassArrowDown),
            "hourglass_arrow_up" => Some(Self::HourglassArrowUp),
            "hourglass_bottom" => Some(Self::HourglassBottom),
            "hourglass_check" => Some(Self::HourglassCheck),
            "hourglass_disabled" => Some(Self::HourglassDisabled),
            "hourglass_empty" => Some(Self::HourglassEmpty),
            "hourglass_full" => Some(Self::HourglassFull),
            "hourglass_pause" => Some(Self::HourglassPause),
            "hourglass_top" => Some(Self::HourglassTop),
            "house" => Some(Self::House),
            "house_siding" => Some(Self::HouseSiding),
            "house_with_shield" => Some(Self::HouseWithShield),
            "houseboat" => Some(Self::Houseboat),
            "household_supplies" => Some(Self::HouseholdSupplies),
            "hov" => Some(Self::Hov),
            "how_to_reg" => Some(Self::HowToReg),
            "how_to_vote" => Some(Self::HowToVote),
            "hr_resting" => Some(Self::HrResting),
            "html" => Some(Self::Html),
            "http" => Some(Self::Http),
            "https" => Some(Self::Https),
            "hub" => Some(Self::Hub),
            "humerus" => Some(Self::Humerus),
            "humerus_alt" => Some(Self::HumerusAlt),
            "humidity_high" => Some(Self::HumidityHigh),
            "humidity_indoor" => Some(Self::HumidityIndoor),
            "humidity_low" => Some(Self::HumidityLow),
            "humidity_mid" => Some(Self::HumidityMid),
            "humidity_percentage" => Some(Self::HumidityPercentage),
            "hvac" => Some(Self::Hvac),
            "hvac_max_defrost" => Some(Self::HvacMaxDefrost),
            "ice_skating" => Some(Self::IceSkating),
            "icecream" => Some(Self::Icecream),
            "id_card" => Some(Self::IdCard),
            "id_card_2" => Some(Self::IdCard2),
            "identity_aware_proxy" => Some(Self::IdentityAwareProxy),
            "identity_platform" => Some(Self::IdentityPlatform),
            "ifl" => Some(Self::Ifl),
            "iframe" => Some(Self::Iframe),
            "iframe_off" => Some(Self::IframeOff),
            "image" => Some(Self::Image),
            "image_arrow_up" => Some(Self::ImageArrowUp),
            "image_aspect_ratio" => Some(Self::ImageAspectRatio),
            "image_inset" => Some(Self::ImageInset),
            "image_not_supported" => Some(Self::ImageNotSupported),
            "image_search" => Some(Self::ImageSearch),
            "imagesearch_roller" => Some(Self::ImagesearchRoller),
            "imagesmode" => Some(Self::Imagesmode),
            "immunology" => Some(Self::Immunology),
            "import_contacts" => Some(Self::ImportContacts),
            "import_export" => Some(Self::ImportExport),
            "import_spark" => Some(Self::ImportSpark),
            "important_devices" => Some(Self::ImportantDevices),
            "in_home_mode" => Some(Self::InHomeMode),
            "inactive_order" => Some(Self::InactiveOrder),
            "inbox" => Some(Self::Inbox),
            "inbox_customize" => Some(Self::InboxCustomize),
            "inbox_text" => Some(Self::InboxText),
            "inbox_text_asterisk" => Some(Self::InboxTextAsterisk),
            "inbox_text_person" => Some(Self::InboxTextPerson),
            "inbox_text_share" => Some(Self::InboxTextShare),
            "incomplete_circle" => Some(Self::IncompleteCircle),
            "indeterminate_check_box" => Some(Self::IndeterminateCheckBox),
            "indeterminate_question_box" => Some(Self::IndeterminateQuestionBox),
            "info_i" => Some(Self::InfoI),
            "infrared" => Some(Self::Infrared),
            "ink_eraser" => Some(Self::InkEraser),
            "ink_eraser_off" => Some(Self::InkEraserOff),
            "ink_highlighter" => Some(Self::InkHighlighter),
            "ink_highlighter_move" => Some(Self::InkHighlighterMove),
            "ink_highlighter_off" => Some(Self::InkHighlighterOff),
            "ink_marker" => Some(Self::InkMarker),
            "ink_pen" => Some(Self::InkPen),
            "ink_selection" => Some(Self::InkSelection),
            "inpatient" => Some(Self::Inpatient),
            "input" => Some(Self::Input),
            "input_circle" => Some(Self::InputCircle),
            "insert_chart" => Some(Self::InsertChart),
            "insert_chart_filled" => Some(Self::InsertChartFilled),
            "insert_chart_outlined" => Some(Self::InsertChartOutlined),
            "insert_comment" => Some(Self::InsertComment),
            "insert_drive_file" => Some(Self::InsertDriveFile),
            "insert_emoticon" => Some(Self::InsertEmoticon),
            "insert_invitation" => Some(Self::InsertInvitation),
            "insert_link" => Some(Self::InsertLink),
            "insert_page_break" => Some(Self::InsertPageBreak),
            "insert_photo" => Some(Self::InsertPhoto),
            "insert_text" => Some(Self::InsertText),
            "insights" => Some(Self::Insights),
            "install_desktop" => Some(Self::InstallDesktop),
            "install_mobile" => Some(Self::InstallMobile),
            "instant_mix" => Some(Self::InstantMix),
            "integration_instructions" => Some(Self::IntegrationInstructions),
            "interactive_space" => Some(Self::InteractiveSpace),
            "interests" => Some(Self::Interests),
            "interpreter_mode" => Some(Self::InterpreterMode),
            "inventory" => Some(Self::Inventory),
            "inventory_2" => Some(Self::Inventory2),
            "invert_colors" => Some(Self::InvertColors),
            "invert_colors_off" => Some(Self::InvertColorsOff),
            "ios" => Some(Self::Ios),
            "ios_share" => Some(Self::IosShare),
            "iron" => Some(Self::Iron),
            "iso" => Some(Self::Iso),
            "jamboard_kiosk" => Some(Self::JamboardKiosk),
            "japanese_curry" => Some(Self::JapaneseCurry),
            "japanese_flag" => Some(Self::JapaneseFlag),
            "javascript" => Some(Self::Javascript),
            "jewelry" => Some(Self::Jewelry),
            "join" => Some(Self::Join),
            "join_full" => Some(Self::JoinFull),
            "join_inner" => Some(Self::JoinInner),
            "join_left" => Some(Self::JoinLeft),
            "join_right" => Some(Self::JoinRight),
            "joystick" => Some(Self::Joystick),
            "jump_to_element" => Some(Self::JumpToElement),
            "kanji_alcohol" => Some(Self::KanjiAlcohol),
            "kayaking" => Some(Self::Kayaking),
            "kebab_dining" => Some(Self::KebabDining),
            "keep" => Some(Self::Keep),
            "keep_off" => Some(Self::KeepOff),
            "keep_pin" => Some(Self::KeepPin),
            "keep_public" => Some(Self::KeepPublic),
            "kettle" => Some(Self::Kettle),
            "key" => Some(Self::Key),
            "key_off" => Some(Self::KeyOff),
            "key_vertical" => Some(Self::KeyVertical),
            "key_visualizer" => Some(Self::KeyVisualizer),
            "keyboard" => Some(Self::Keyboard),
            "keyboard_alt" => Some(Self::KeyboardAlt),
            "keyboard_arrow_down" => Some(Self::KeyboardArrowDown),
            "keyboard_arrow_left" => Some(Self::KeyboardArrowLeft),
            "keyboard_arrow_right" => Some(Self::KeyboardArrowRight),
            "keyboard_arrow_up" => Some(Self::KeyboardArrowUp),
            "keyboard_backspace" => Some(Self::KeyboardBackspace),
            "keyboard_capslock" => Some(Self::KeyboardCapslock),
            "keyboard_capslock_badge" => Some(Self::KeyboardCapslockBadge),
            "keyboard_command_key" => Some(Self::KeyboardCommandKey),
            "keyboard_control_key" => Some(Self::KeyboardControlKey),
            "keyboard_double_arrow_down" => Some(Self::KeyboardDoubleArrowDown),
            "keyboard_double_arrow_left" => Some(Self::KeyboardDoubleArrowLeft),
            "keyboard_double_arrow_right" => Some(Self::KeyboardDoubleArrowRight),
            "keyboard_double_arrow_up" => Some(Self::KeyboardDoubleArrowUp),
            "keyboard_external_input" => Some(Self::KeyboardExternalInput),
            "keyboard_full" => Some(Self::KeyboardFull),
            "keyboard_hide" => Some(Self::KeyboardHide),
            "keyboard_keys" => Some(Self::KeyboardKeys),
            "keyboard_lock" => Some(Self::KeyboardLock),
            "keyboard_lock_off" => Some(Self::KeyboardLockOff),
            "keyboard_off" => Some(Self::KeyboardOff),
            "keyboard_onscreen" => Some(Self::KeyboardOnscreen),
            "keyboard_option_key" => Some(Self::KeyboardOptionKey),
            "keyboard_previous_language" => Some(Self::KeyboardPreviousLanguage),
            "keyboard_return" => Some(Self::KeyboardReturn),
            "keyboard_tab" => Some(Self::KeyboardTab),
            "keyboard_tab_rtl" => Some(Self::KeyboardTabRtl),
            "keyboard_voice" => Some(Self::KeyboardVoice),
            "kid_star" => Some(Self::KidStar),
            "king_bed" => Some(Self::KingBed),
            "kitchen" => Some(Self::Kitchen),
            "kitesurfing" => Some(Self::Kitesurfing),
            "lab_panel" => Some(Self::LabPanel),
            "lab_profile" => Some(Self::LabProfile),
            "lab_research" => Some(Self::LabResearch),
            "label" => Some(Self::Label),
            "label_important" => Some(Self::LabelImportant),
            "label_important_outline" => Some(Self::LabelImportantOutline),
            "label_off" => Some(Self::LabelOff),
            "label_outline" => Some(Self::LabelOutline),
            "labs" => Some(Self::Labs),
            "lan" => Some(Self::Lan),
            "landscape" => Some(Self::Landscape),
            "landscape_2" => Some(Self::Landscape2),
            "landscape_2_edit" => Some(Self::Landscape2Edit),
            "landscape_2_off" => Some(Self::Landscape2Off),
            "landslide" => Some(Self::Landslide),
            "language" => Some(Self::Language),
            "language_chinese_array" => Some(Self::LanguageChineseArray),
            "language_chinese_cangjie" => Some(Self::LanguageChineseCangjie),
            "language_chinese_dayi" => Some(Self::LanguageChineseDayi),
            "language_chinese_pinyin" => Some(Self::LanguageChinesePinyin),
            "language_chinese_quick" => Some(Self::LanguageChineseQuick),
            "language_chinese_wubi" => Some(Self::LanguageChineseWubi),
            "language_french" => Some(Self::LanguageFrench),
            "language_gb_english" => Some(Self::LanguageGbEnglish),
            "language_international" => Some(Self::LanguageInternational),
            "language_japanese_kana" => Some(Self::LanguageJapaneseKana),
            "language_korean_latin" => Some(Self::LanguageKoreanLatin),
            "language_pinyin" => Some(Self::LanguagePinyin),
            "language_spanish" => Some(Self::LanguageSpanish),
            "language_us" => Some(Self::LanguageUs),
            "language_us_colemak" => Some(Self::LanguageUsColemak),
            "language_us_dvorak" => Some(Self::LanguageUsDvorak),
            "laps" => Some(Self::Laps),
            "laptop" => Some(Self::Laptop),
            "laptop_car" => Some(Self::LaptopCar),
            "laptop_chromebook" => Some(Self::LaptopChromebook),
            "laptop_mac" => Some(Self::LaptopMac),
            "laptop_windows" => Some(Self::LaptopWindows),
            "lasso_select" => Some(Self::LassoSelect),
            "launch" => Some(Self::Launch),
            "laundry" => Some(Self::Laundry),
            "layers" => Some(Self::Layers),
            "layers_clear" => Some(Self::LayersClear),
            "lda" => Some(Self::Lda),
            "leaderboard" => Some(Self::Leaderboard),
            "leak_add" => Some(Self::LeakAdd),
            "leak_remove" => Some(Self::LeakRemove),
            "left_click" => Some(Self::LeftClick),
            "left_panel_close" => Some(Self::LeftPanelClose),
            "left_panel_open" => Some(Self::LeftPanelOpen),
            "legend_toggle" => Some(Self::LegendToggle),
            "lens" => Some(Self::Lens),
            "lens_blur" => Some(Self::LensBlur),
            "letter_switch" => Some(Self::LetterSwitch),
            "library_add" => Some(Self::LibraryAdd),
            "library_add_check" => Some(Self::LibraryAddCheck),
            "library_books" => Some(Self::LibraryBooks),
            "library_music" => Some(Self::LibraryMusic),
            "license" => Some(Self::License),
            "lift_to_talk" => Some(Self::LiftToTalk),
            "light" => Some(Self::Light),
            "light_group" => Some(Self::LightGroup),
            "light_group_2" => Some(Self::LightGroup2),
            "light_mode" => Some(Self::LightMode),
            "light_mode_auto" => Some(Self::LightModeAuto),
            "light_off" => Some(Self::LightOff),
            "lightbulb" => Some(Self::Lightbulb),
            "lightbulb_2" => Some(Self::Lightbulb2),
            "lightbulb_circle" => Some(Self::LightbulbCircle),
            "lightbulb_outline" => Some(Self::LightbulbOutline),
            "lightning_stand" => Some(Self::LightningStand),
            "lightstrip" => Some(Self::Lightstrip),
            "line_axis" => Some(Self::LineAxis),
            "line_curve" => Some(Self::LineCurve),
            "line_end" => Some(Self::LineEnd),
            "line_end_arrow" => Some(Self::LineEndArrow),
            "line_end_arrow_notch" => Some(Self::LineEndArrowNotch),
            "line_end_circle" => Some(Self::LineEndCircle),
            "line_end_diamond" => Some(Self::LineEndDiamond),
            "line_end_square" => Some(Self::LineEndSquare),
            "line_start" => Some(Self::LineStart),
            "line_start_arrow" => Some(Self::LineStartArrow),
            "line_start_arrow_notch" => Some(Self::LineStartArrowNotch),
            "line_start_circle" => Some(Self::LineStartCircle),
            "line_start_diamond" => Some(Self::LineStartDiamond),
            "line_start_square" => Some(Self::LineStartSquare),
            "line_style" => Some(Self::LineStyle),
            "line_weight" => Some(Self::LineWeight),
            "linear_scale" => Some(Self::LinearScale),
            "link_2" => Some(Self::Link2),
            "link_off" => Some(Self::LinkOff),
            "linked_camera" => Some(Self::LinkedCamera),
            "linked_services" => Some(Self::LinkedServices),
            "lips" => Some(Self::Lips),
            "liquor" => Some(Self::Liquor),
            "list" => Some(Self::List),
            "list_2" => Some(Self::List2),
            "list_alt" => Some(Self::ListAlt),
            "list_alt_add" => Some(Self::ListAltAdd),
            "list_alt_check" => Some(Self::ListAltCheck),
            "list_arrow" => Some(Self::ListArrow),
            "lists" => Some(Self::Lists),
            "live_help" => Some(Self::LiveHelp),
            "live_tv" => Some(Self::LiveTv),
            "living" => Some(Self::Living),
            "local_activity" => Some(Self::LocalActivity),
            "local_airport" => Some(Self::LocalAirport),
            "local_atm" => Some(Self::LocalAtm),
            "local_bar" => Some(Self::LocalBar),
            "local_cafe" => Some(Self::LocalCafe),
            "local_car_wash" => Some(Self::LocalCarWash),
            "local_convenience_store" => Some(Self::LocalConvenienceStore),
            "local_dining" => Some(Self::LocalDining),
            "local_drink" => Some(Self::LocalDrink),
            "local_fire_department" => Some(Self::LocalFireDepartment),
            "local_florist" => Some(Self::LocalFlorist),
            "local_gas_station" => Some(Self::LocalGasStation),
            "local_grocery_store" => Some(Self::LocalGroceryStore),
            "local_hospital" => Some(Self::LocalHospital),
            "local_hotel" => Some(Self::LocalHotel),
            "local_laundry_service" => Some(Self::LocalLaundryService),
            "local_library" => Some(Self::LocalLibrary),
            "local_mall" => Some(Self::LocalMall),
            "local_movies" => Some(Self::LocalMovies),
            "local_offer" => Some(Self::LocalOffer),
            "local_parking" => Some(Self::LocalParking),
            "local_pharmacy" => Some(Self::LocalPharmacy),
            "local_phone" => Some(Self::LocalPhone),
            "local_pizza" => Some(Self::LocalPizza),
            "local_play" => Some(Self::LocalPlay),
            "local_police" => Some(Self::LocalPolice),
            "local_post_office" => Some(Self::LocalPostOffice),
            "local_printshop" => Some(Self::LocalPrintshop),
            "local_see" => Some(Self::LocalSee),
            "local_shipping" => Some(Self::LocalShipping),
            "local_taxi" => Some(Self::LocalTaxi),
            "location_automation" => Some(Self::LocationAutomation),
            "location_away" => Some(Self::LocationAway),
            "location_chip" => Some(Self::LocationChip),
            "location_city" => Some(Self::LocationCity),
            "location_disabled" => Some(Self::LocationDisabled),
            "location_home" => Some(Self::LocationHome),
            "location_off" => Some(Self::LocationOff),
            "location_on" => Some(Self::LocationOn),
            "location_pin" => Some(Self::LocationPin),
            "location_searching" => Some(Self::LocationSearching),
            "locator_tag" => Some(Self::LocatorTag),
            "lock_clock" => Some(Self::LockClock),
            "lock_open" => Some(Self::LockOpen),
            "lock_open_circle" => Some(Self::LockOpenCircle),
            "lock_open_right" => Some(Self::LockOpenRight),
            "lock_outline" => Some(Self::LockOutline),
            "lock_person" => Some(Self::LockPerson),
            "lock_reset" => Some(Self::LockReset),
            "login" => Some(Self::Login),
            "logo_dev" => Some(Self::LogoDev),
            "logout" => Some(Self::Logout),
            "looks" => Some(Self::Looks),
            "looks_3" => Some(Self::Looks3),
            "looks_4" => Some(Self::Looks4),
            "looks_5" => Some(Self::Looks5),
            "looks_6" => Some(Self::Looks6),
            "looks_one" => Some(Self::LooksOne),
            "looks_two" => Some(Self::LooksTwo),
            "loop" => Some(Self::Loop),
            "loupe" => Some(Self::Loupe),
            "low_density" => Some(Self::LowDensity),
            "low_priority" => Some(Self::LowPriority),
            "lowercase" => Some(Self::Lowercase),
            "loyalty" => Some(Self::Loyalty),
            "lte_mobiledata" => Some(Self::LteMobiledata),
            "lte_mobiledata_badge" => Some(Self::LteMobiledataBadge),
            "lte_plus_mobiledata" => Some(Self::LtePlusMobiledata),
            "lte_plus_mobiledata_badge" => Some(Self::LtePlusMobiledataBadge),
            "luggage" => Some(Self::Luggage),
            "lunch_dining" => Some(Self::LunchDining),
            "lyrics" => Some(Self::Lyrics),
            "macro_auto" => Some(Self::MacroAuto),
            "macro_off" => Some(Self::MacroOff),
            "magic_button" => Some(Self::MagicButton),
            "magic_exchange" => Some(Self::MagicExchange),
            "magic_tether" => Some(Self::MagicTether),
            "magnification_large" => Some(Self::MagnificationLarge),
            "magnification_small" => Some(Self::MagnificationSmall),
            "magnify_docked" => Some(Self::MagnifyDocked),
            "magnify_fullscreen" => Some(Self::MagnifyFullscreen),
            "mail_asterisk" => Some(Self::MailAsterisk),
            "mail_lock" => Some(Self::MailLock),
            "mail_off" => Some(Self::MailOff),
            "mail_outline" => Some(Self::MailOutline),
            "mail_shield" => Some(Self::MailShield),
            "male" => Some(Self::Male),
            "man" => Some(Self::Man),
            "man_2" => Some(Self::Man2),
            "man_3" => Some(Self::Man3),
            "man_4" => Some(Self::Man4),
            "manage_accounts" => Some(Self::ManageAccounts),
            "manage_history" => Some(Self::ManageHistory),
            "manage_search" => Some(Self::ManageSearch),
            "manga" => Some(Self::Manga),
            "manufacturing" => Some(Self::Manufacturing),
            "map" => Some(Self::Map),
            "map_pin_heart" => Some(Self::MapPinHeart),
            "map_pin_review" => Some(Self::MapPinReview),
            "map_search" => Some(Self::MapSearch),
            "maps_home_work" => Some(Self::MapsHomeWork),
            "maps_ugc" => Some(Self::MapsUgc),
            "margin" => Some(Self::Margin),
            "mark_as_unread" => Some(Self::MarkAsUnread),
            "mark_chat_read" => Some(Self::MarkChatRead),
            "mark_chat_unread" => Some(Self::MarkChatUnread),
            "mark_email_read" => Some(Self::MarkEmailRead),
            "mark_email_unread" => Some(Self::MarkEmailUnread),
            "mark_unread_chat_alt" => Some(Self::MarkUnreadChatAlt),
            "markdown" => Some(Self::Markdown),
            "markdown_convert" => Some(Self::MarkdownConvert),
            "markdown_copy" => Some(Self::MarkdownCopy),
            "markdown_document" => Some(Self::MarkdownDocument),
            "markdown_paste" => Some(Self::MarkdownPaste),
            "markdown_spark" => Some(Self::MarkdownSpark),
            "markunread" => Some(Self::Markunread),
            "markunread_mailbox" => Some(Self::MarkunreadMailbox),
            "masked_transitions" => Some(Self::MaskedTransitions),
            "masked_transitions_add" => Some(Self::MaskedTransitionsAdd),
            "masks" => Some(Self::Masks),
            "massage" => Some(Self::Massage),
            "match_case" => Some(Self::MatchCase),
            "match_case_off" => Some(Self::MatchCaseOff),
            "match_word" => Some(Self::MatchWord),
            "matter" => Some(Self::Matter),
            "maximize" => Some(Self::Maximize),
            "meal_dinner" => Some(Self::MealDinner),
            "meal_lunch" => Some(Self::MealLunch),
            "measuring_tape" => Some(Self::MeasuringTape),
            "media_bluetooth_off" => Some(Self::MediaBluetoothOff),
            "media_bluetooth_on" => Some(Self::MediaBluetoothOn),
            "media_link" => Some(Self::MediaLink),
            "media_output" => Some(Self::MediaOutput),
            "media_output_off" => Some(Self::MediaOutputOff),
            "mediation" => Some(Self::Mediation),
            "medical_information" => Some(Self::MedicalInformation),
            "medical_mask" => Some(Self::MedicalMask),
            "medical_services" => Some(Self::MedicalServices),
            "medication" => Some(Self::Medication),
            "medication_liquid" => Some(Self::MedicationLiquid),
            "meeting_room" => Some(Self::MeetingRoom),
            "memory" => Some(Self::Memory),
            "memory_alt" => Some(Self::MemoryAlt),
            "menstrual_health" => Some(Self::MenstrualHealth),
            "menu_book" => Some(Self::MenuBook),
            "menu_book_2" => Some(Self::MenuBook2),
            "menu_open" => Some(Self::MenuOpen),
            "merge" => Some(Self::Merge),
            "merge_type" => Some(Self::MergeType),
            "message" => Some(Self::Message),
            "metabolism" => Some(Self::Metabolism),
            "metro" => Some(Self::Metro),
            "mfg_nest_yale_lock" => Some(Self::MfgNestYaleLock),
            "mic" => Some(Self::Mic),
            "mic_alert" => Some(Self::MicAlert),
            "mic_double" => Some(Self::MicDouble),
            "mic_external_off" => Some(Self::MicExternalOff),
            "mic_external_on" => Some(Self::MicExternalOn),
            "mic_gear" => Some(Self::MicGear),
            "mic_none" => Some(Self::MicNone),
            "mic_off" => Some(Self::MicOff),
            "microbiology" => Some(Self::Microbiology),
            "microwave" => Some(Self::Microwave),
            "microwave_gen" => Some(Self::MicrowaveGen),
            "military_tech" => Some(Self::MilitaryTech),
            "mimo" => Some(Self::Mimo),
            "mimo_disconnect" => Some(Self::MimoDisconnect),
            "mindfulness" => Some(Self::Mindfulness),
            "minimize" => Some(Self::Minimize),
            "minor_crash" => Some(Self::MinorCrash),
            "mintmark" => Some(Self::Mintmark),
            "missed_video_call" => Some(Self::MissedVideoCall),
            "missed_video_call_filled" => Some(Self::MissedVideoCallFilled),
            "missing_controller" => Some(Self::MissingController),
            "mist" => Some(Self::Mist),
            "mitre" => Some(Self::Mitre),
            "mixture_med" => Some(Self::MixtureMed),
            "mms" => Some(Self::Mms),
            "mobile" => Some(Self::Mobile),
            "mobile_2" => Some(Self::Mobile2),
            "mobile_3" => Some(Self::Mobile3),
            "mobile_alert" => Some(Self::MobileAlert),
            "mobile_arrow_down" => Some(Self::MobileArrowDown),
            "mobile_arrow_right" => Some(Self::MobileArrowRight),
            "mobile_arrow_up_right" => Some(Self::MobileArrowUpRight),
            "mobile_block" => Some(Self::MobileBlock),
            "mobile_camera" => Some(Self::MobileCamera),
            "mobile_camera_front" => Some(Self::MobileCameraFront),
            "mobile_camera_rear" => Some(Self::MobileCameraRear),
            "mobile_cancel" => Some(Self::MobileCancel),
            "mobile_cast" => Some(Self::MobileCast),
            "mobile_charge" => Some(Self::MobileCharge),
            "mobile_chat" => Some(Self::MobileChat),
            "mobile_check" => Some(Self::MobileCheck),
            "mobile_code" => Some(Self::MobileCode),
            "mobile_dock" => Some(Self::MobileDock),
            "mobile_dots" => Some(Self::MobileDots),
            "mobile_friendly" => Some(Self::MobileFriendly),
            "mobile_gear" => Some(Self::MobileGear),
            "mobile_hand" => Some(Self::MobileHand),
            "mobile_hand_left" => Some(Self::MobileHandLeft),
            "mobile_hand_left_off" => Some(Self::MobileHandLeftOff),
            "mobile_hand_off" => Some(Self::MobileHandOff),
            "mobile_info" => Some(Self::MobileInfo),
            "mobile_landscape" => Some(Self::MobileLandscape),
            "mobile_layout" => Some(Self::MobileLayout),
            "mobile_lock_landscape" => Some(Self::MobileLockLandscape),
            "mobile_lock_portrait" => Some(Self::MobileLockPortrait),
            "mobile_loupe" => Some(Self::MobileLoupe),
            "mobile_menu" => Some(Self::MobileMenu),
            "mobile_off" => Some(Self::MobileOff),
            "mobile_question" => Some(Self::MobileQuestion),
            "mobile_rotate" => Some(Self::MobileRotate),
            "mobile_rotate_lock" => Some(Self::MobileRotateLock),
            "mobile_screen_share" => Some(Self::MobileScreenShare),
            "mobile_screensaver" => Some(Self::MobileScreensaver),
            "mobile_sensor_hi" => Some(Self::MobileSensorHi),
            "mobile_sensor_lo" => Some(Self::MobileSensorLo),
            "mobile_share" => Some(Self::MobileShare),
            "mobile_share_stack" => Some(Self::MobileShareStack),
            "mobile_sound" => Some(Self::MobileSound),
            "mobile_sound_2" => Some(Self::MobileSound2),
            "mobile_sound_off" => Some(Self::MobileSoundOff),
            "mobile_speaker" => Some(Self::MobileSpeaker),
            "mobile_tap" => Some(Self::MobileTap),
            "mobile_text" => Some(Self::MobileText),
            "mobile_text_2" => Some(Self::MobileText2),
            "mobile_theft" => Some(Self::MobileTheft),
            "mobile_ticket" => Some(Self::MobileTicket),
            "mobile_unlock" => Some(Self::MobileUnlock),
            "mobile_vibrate" => Some(Self::MobileVibrate),
            "mobile_wrench" => Some(Self::MobileWrench),
            "mobiledata_arrows" => Some(Self::MobiledataArrows),
            "mobiledata_off" => Some(Self::MobiledataOff),
            "mode" => Some(Self::Mode),
            "mode_comment" => Some(Self::ModeComment),
            "mode_cool" => Some(Self::ModeCool),
            "mode_cool_off" => Some(Self::ModeCoolOff),
            "mode_dual" => Some(Self::ModeDual),
            "mode_edit" => Some(Self::ModeEdit),
            "mode_edit_outline" => Some(Self::ModeEditOutline),
            "mode_fan" => Some(Self::ModeFan),
            "mode_fan_2" => Some(Self::ModeFan2),
            "mode_fan_off" => Some(Self::ModeFanOff),
            "mode_heat" => Some(Self::ModeHeat),
            "mode_heat_cool" => Some(Self::ModeHeatCool),
            "mode_heat_off" => Some(Self::ModeHeatOff),
            "mode_night" => Some(Self::ModeNight),
            "mode_of_travel" => Some(Self::ModeOfTravel),
            "mode_off_on" => Some(Self::ModeOffOn),
            "mode_standby" => Some(Self::ModeStandby),
            "model_training" => Some(Self::ModelTraining),
            "modeling" => Some(Self::Modeling),
            "monetization_on" => Some(Self::MonetizationOn),
            "money" => Some(Self::Money),
            "money_bag" => Some(Self::MoneyBag),
            "money_off" => Some(Self::MoneyOff),
            "money_off_csred" => Some(Self::MoneyOffCsred),
            "money_range" => Some(Self::MoneyRange),
            "monitor" => Some(Self::Monitor),
            "monitor_heart" => Some(Self::MonitorHeart),
            "monitor_weight" => Some(Self::MonitorWeight),
            "monitor_weight_gain" => Some(Self::MonitorWeightGain),
            "monitor_weight_loss" => Some(Self::MonitorWeightLoss),
            "monitoring" => Some(Self::Monitoring),
            "monochrome_photos" => Some(Self::MonochromePhotos),
            "monorail" => Some(Self::Monorail),
            "mood" => Some(Self::Mood),
            "mood_bad" => Some(Self::MoodBad),
            "mood_heart" => Some(Self::MoodHeart),
            "moon_stars" => Some(Self::MoonStars),
            "mop" => Some(Self::Mop),
            "moped" => Some(Self::Moped),
            "moped_package" => Some(Self::MopedPackage),
            "more_down" => Some(Self::MoreDown),
            "more_horiz" => Some(Self::MoreHoriz),
            "more_time" => Some(Self::MoreTime),
            "more_up" => Some(Self::MoreUp),
            "more_vert" => Some(Self::MoreVert),
            "mosque" => Some(Self::Mosque),
            "motion_blur" => Some(Self::MotionBlur),
            "motion_mode" => Some(Self::MotionMode),
            "motion_photos_auto" => Some(Self::MotionPhotosAuto),
            "motion_photos_off" => Some(Self::MotionPhotosOff),
            "motion_photos_on" => Some(Self::MotionPhotosOn),
            "motion_photos_pause" => Some(Self::MotionPhotosPause),
            "motion_photos_paused" => Some(Self::MotionPhotosPaused),
            "motion_play" => Some(Self::MotionPlay),
            "motion_sensor_active" => Some(Self::MotionSensorActive),
            "motion_sensor_alert" => Some(Self::MotionSensorAlert),
            "motion_sensor_idle" => Some(Self::MotionSensorIdle),
            "motion_sensor_urgent" => Some(Self::MotionSensorUrgent),
            "motorcycle" => Some(Self::Motorcycle),
            "mountain_flag" => Some(Self::MountainFlag),
            "mountain_steam" => Some(Self::MountainSteam),
            "mouse" => Some(Self::Mouse),
            "mouse_lock" => Some(Self::MouseLock),
            "mouse_lock_off" => Some(Self::MouseLockOff),
            "move" => Some(Self::Move),
            "move_down" => Some(Self::MoveDown),
            "move_group" => Some(Self::MoveGroup),
            "move_item" => Some(Self::MoveItem),
            "move_location" => Some(Self::MoveLocation),
            "move_selection_down" => Some(Self::MoveSelectionDown),
            "move_selection_left" => Some(Self::MoveSelectionLeft),
            "move_selection_right" => Some(Self::MoveSelectionRight),
            "move_selection_up" => Some(Self::MoveSelectionUp),
            "move_to_inbox" => Some(Self::MoveToInbox),
            "move_up" => Some(Self::MoveUp),
            "moved_location" => Some(Self::MovedLocation),
            "movie" => Some(Self::Movie),
            "movie_creation" => Some(Self::MovieCreation),
            "movie_edit" => Some(Self::MovieEdit),
            "movie_edit_off" => Some(Self::MovieEditOff),
            "movie_filter" => Some(Self::MovieFilter),
            "movie_info" => Some(Self::MovieInfo),
            "movie_off" => Some(Self::MovieOff),
            "movie_speaker" => Some(Self::MovieSpeaker),
            "moving" => Some(Self::Moving),
            "moving_beds" => Some(Self::MovingBeds),
            "moving_ministry" => Some(Self::MovingMinistry),
            "mp" => Some(Self::Mp),
            "multicooker" => Some(Self::Multicooker),
            "multiline_chart" => Some(Self::MultilineChart),
            "multimodal_hand_eye" => Some(Self::MultimodalHandEye),
            "multiple_airports" => Some(Self::MultipleAirports),
            "multiple_stop" => Some(Self::MultipleStop),
            "museum" => Some(Self::Museum),
            "music_cast" => Some(Self::MusicCast),
            "music_history" => Some(Self::MusicHistory),
            "music_note" => Some(Self::MusicNote),
            "music_note_2" => Some(Self::MusicNote2),
            "music_note_add" => Some(Self::MusicNoteAdd),
            "music_off" => Some(Self::MusicOff),
            "music_video" => Some(Self::MusicVideo),
            "my_location" => Some(Self::MyLocation),
            "mystery" => Some(Self::Mystery),
            "nat" => Some(Self::Nat),
            "nature" => Some(Self::Nature),
            "nature_people" => Some(Self::NaturePeople),
            "navigate_before" => Some(Self::NavigateBefore),
            "navigate_next" => Some(Self::NavigateNext),
            "navigation" => Some(Self::Navigation),
            "near_me" => Some(Self::NearMe),
            "near_me_disabled" => Some(Self::NearMeDisabled),
            "nearby" => Some(Self::Nearby),
            "nearby_error" => Some(Self::NearbyError),
            "nearby_off" => Some(Self::NearbyOff),
            "nephrology" => Some(Self::Nephrology),
            "nest_audio" => Some(Self::NestAudio),
            "nest_cam_floodlight" => Some(Self::NestCamFloodlight),
            "nest_cam_indoor" => Some(Self::NestCamIndoor),
            "nest_cam_iq" => Some(Self::NestCamIq),
            "nest_cam_iq_outdoor" => Some(Self::NestCamIqOutdoor),
            "nest_cam_magnet_mount" => Some(Self::NestCamMagnetMount),
            "nest_cam_outdoor" => Some(Self::NestCamOutdoor),
            "nest_cam_stand" => Some(Self::NestCamStand),
            "nest_cam_wall_mount" => Some(Self::NestCamWallMount),
            "nest_cam_wired_stand" => Some(Self::NestCamWiredStand),
            "nest_clock_farsight_analog" => Some(Self::NestClockFarsightAnalog),
            "nest_clock_farsight_digital" => Some(Self::NestClockFarsightDigital),
            "nest_connect" => Some(Self::NestConnect),
            "nest_detect" => Some(Self::NestDetect),
            "nest_display" => Some(Self::NestDisplay),
            "nest_display_max" => Some(Self::NestDisplayMax),
            "nest_doorbell_visitor" => Some(Self::NestDoorbellVisitor),
            "nest_eco_leaf" => Some(Self::NestEcoLeaf),
            "nest_farsight_cool" => Some(Self::NestFarsightCool),
            "nest_farsight_dual" => Some(Self::NestFarsightDual),
            "nest_farsight_eco" => Some(Self::NestFarsightEco),
            "nest_farsight_heat" => Some(Self::NestFarsightHeat),
            "nest_farsight_seasonal" => Some(Self::NestFarsightSeasonal),
            "nest_farsight_weather" => Some(Self::NestFarsightWeather),
            "nest_found_savings" => Some(Self::NestFoundSavings),
            "nest_gale_wifi" => Some(Self::NestGaleWifi),
            "nest_heat_link_e" => Some(Self::NestHeatLinkE),
            "nest_heat_link_gen_3" => Some(Self::NestHeatLinkGen3),
            "nest_hello_doorbell" => Some(Self::NestHelloDoorbell),
            "nest_locator_tag" => Some(Self::NestLocatorTag),
            "nest_mini" => Some(Self::NestMini),
            "nest_multi_room" => Some(Self::NestMultiRoom),
            "nest_protect" => Some(Self::NestProtect),
            "nest_remote" => Some(Self::NestRemote),
            "nest_remote_comfort_sensor" => Some(Self::NestRemoteComfortSensor),
            "nest_secure_alarm" => Some(Self::NestSecureAlarm),
            "nest_sunblock" => Some(Self::NestSunblock),
            "nest_tag" => Some(Self::NestTag),
            "nest_thermostat" => Some(Self::NestThermostat),
            "nest_thermostat_e_eu" => Some(Self::NestThermostatEEu),
            "nest_thermostat_gen_3" => Some(Self::NestThermostatGen3),
            "nest_thermostat_sensor" => Some(Self::NestThermostatSensor),
            "nest_thermostat_sensor_eu" => Some(Self::NestThermostatSensorEu),
            "nest_thermostat_zirconium_eu" => Some(Self::NestThermostatZirconiumEu),
            "nest_true_radiant" => Some(Self::NestTrueRadiant),
            "nest_wake_on_approach" => Some(Self::NestWakeOnApproach),
            "nest_wake_on_press" => Some(Self::NestWakeOnPress),
            "nest_wifi_gale" => Some(Self::NestWifiGale),
            "nest_wifi_mistral" => Some(Self::NestWifiMistral),
            "nest_wifi_point" => Some(Self::NestWifiPoint),
            "nest_wifi_point_vento" => Some(Self::NestWifiPointVento),
            "nest_wifi_pro" => Some(Self::NestWifiPro),
            "nest_wifi_pro_2" => Some(Self::NestWifiPro2),
            "nest_wifi_router" => Some(Self::NestWifiRouter),
            "network_cell" => Some(Self::NetworkCell),
            "network_check" => Some(Self::NetworkCheck),
            "network_intel_node" => Some(Self::NetworkIntelNode),
            "network_intelligence" => Some(Self::NetworkIntelligence),
            "network_intelligence_history" => Some(Self::NetworkIntelligenceHistory),
            "network_intelligence_update" => Some(Self::NetworkIntelligenceUpdate),
            "network_locked" => Some(Self::NetworkLocked),
            "network_manage" => Some(Self::NetworkManage),
            "network_node" => Some(Self::NetworkNode),
            "network_ping" => Some(Self::NetworkPing),
            "network_wifi" => Some(Self::NetworkWifi),
            "network_wifi_1_bar" => Some(Self::NetworkWifi1Bar),
            "network_wifi_1_bar_locked" => Some(Self::NetworkWifi1BarLocked),
            "network_wifi_2_bar" => Some(Self::NetworkWifi2Bar),
            "network_wifi_2_bar_locked" => Some(Self::NetworkWifi2BarLocked),
            "network_wifi_3_bar" => Some(Self::NetworkWifi3Bar),
            "network_wifi_3_bar_locked" => Some(Self::NetworkWifi3BarLocked),
            "network_wifi_locked" => Some(Self::NetworkWifiLocked),
            "neurology" => Some(Self::Neurology),
            "new_label" => Some(Self::NewLabel),
            "new_releases" => Some(Self::NewReleases),
            "new_window" => Some(Self::NewWindow),
            "news" => Some(Self::News),
            "newsmode" => Some(Self::Newsmode),
            "newspaper" => Some(Self::Newspaper),
            "newsstand" => Some(Self::Newsstand),
            "next_plan" => Some(Self::NextPlan),
            "next_week" => Some(Self::NextWeek),
            "nfc" => Some(Self::Nfc),
            "nfc_off" => Some(Self::NfcOff),
            "night_shelter" => Some(Self::NightShelter),
            "night_sight_auto" => Some(Self::NightSightAuto),
            "night_sight_auto_off" => Some(Self::NightSightAutoOff),
            "night_sight_max" => Some(Self::NightSightMax),
            "nightlife" => Some(Self::Nightlife),
            "nightlight" => Some(Self::Nightlight),
            "nightlight_round" => Some(Self::NightlightRound),
            "nights_stay" => Some(Self::NightsStay),
            "no_accounts" => Some(Self::NoAccounts),
            "no_adult_content" => Some(Self::NoAdultContent),
            "no_backpack" => Some(Self::NoBackpack),
            "no_crash" => Some(Self::NoCrash),
            "no_drinks" => Some(Self::NoDrinks),
            "no_encryption" => Some(Self::NoEncryption),
            "no_encryption_gmailerrorred" => Some(Self::NoEncryptionGmailerrorred),
            "no_flash" => Some(Self::NoFlash),
            "no_food" => Some(Self::NoFood),
            "no_luggage" => Some(Self::NoLuggage),
            "no_meals" => Some(Self::NoMeals),
            "no_meeting_room" => Some(Self::NoMeetingRoom),
            "no_photography" => Some(Self::NoPhotography),
            "no_sim" => Some(Self::NoSim),
            "no_sound" => Some(Self::NoSound),
            "no_stroller" => Some(Self::NoStroller),
            "no_transfer" => Some(Self::NoTransfer),
            "noise_aware" => Some(Self::NoiseAware),
            "noise_control_off" => Some(Self::NoiseControlOff),
            "noise_control_on" => Some(Self::NoiseControlOn),
            "nordic_walking" => Some(Self::NordicWalking),
            "north" => Some(Self::North),
            "north_east" => Some(Self::NorthEast),
            "north_west" => Some(Self::NorthWest),
            "not_accessible" => Some(Self::NotAccessible),
            "not_accessible_forward" => Some(Self::NotAccessibleForward),
            "not_interested" => Some(Self::NotInterested),
            "not_listed_location" => Some(Self::NotListedLocation),
            "not_started" => Some(Self::NotStarted),
            "note" => Some(Self::Note),
            "note_add" => Some(Self::NoteAdd),
            "note_alt" => Some(Self::NoteAlt),
            "note_stack" => Some(Self::NoteStack),
            "note_stack_add" => Some(Self::NoteStackAdd),
            "notes" => Some(Self::Notes),
            "notification_add" => Some(Self::NotificationAdd),
            "notification_audio" => Some(Self::NotificationAudio),
            "notification_audio_off" => Some(Self::NotificationAudioOff),
            "notification_important" => Some(Self::NotificationImportant),
            "notification_multiple" => Some(Self::NotificationMultiple),
            "notification_settings" => Some(Self::NotificationSettings),
            "notification_sound" => Some(Self::NotificationSound),
            "notifications" => Some(Self::Notifications),
            "notifications_active" => Some(Self::NotificationsActive),
            "notifications_none" => Some(Self::NotificationsNone),
            "notifications_paused" => Some(Self::NotificationsPaused),
            "notifications_unread" => Some(Self::NotificationsUnread),
            "numbers" => Some(Self::Numbers),
            "nutrition" => Some(Self::Nutrition),
            "ods" => Some(Self::Ods),
            "odt" => Some(Self::Odt),
            "offline_bolt" => Some(Self::OfflineBolt),
            "offline_pin" => Some(Self::OfflinePin),
            "offline_pin_off" => Some(Self::OfflinePinOff),
            "offline_share" => Some(Self::OfflineShare),
            "oil_barrel" => Some(Self::OilBarrel),
            "okonomiyaki" => Some(Self::Okonomiyaki),
            "on_device_training" => Some(Self::OnDeviceTraining),
            "on_hub_device" => Some(Self::OnHubDevice),
            "oncology" => Some(Self::Oncology),
            "ondemand_video" => Some(Self::OndemandVideo),
            "online_prediction" => Some(Self::OnlinePrediction),
            "onsen" => Some(Self::Onsen),
            "opacity" => Some(Self::Opacity),
            "open_in_browser" => Some(Self::OpenInBrowser),
            "open_in_full" => Some(Self::OpenInFull),
            "open_in_new" => Some(Self::OpenInNew),
            "open_in_new_down" => Some(Self::OpenInNewDown),
            "open_in_new_off" => Some(Self::OpenInNewOff),
            "open_in_phone" => Some(Self::OpenInPhone),
            "open_jam" => Some(Self::OpenJam),
            "open_run" => Some(Self::OpenRun),
            "open_with" => Some(Self::OpenWith),
            "ophthalmology" => Some(Self::Ophthalmology),
            "oral_disease" => Some(Self::OralDisease),
            "orbit" => Some(Self::Orbit),
            "order_approve" => Some(Self::OrderApprove),
            "order_play" => Some(Self::OrderPlay),
            "orders" => Some(Self::Orders),
            "orthopedics" => Some(Self::Orthopedics),
            "other_admission" => Some(Self::OtherAdmission),
            "other_houses" => Some(Self::OtherHouses),
            "outbound" => Some(Self::Outbound),
            "outbox" => Some(Self::Outbox),
            "outbox_alt" => Some(Self::OutboxAlt),
            "outdoor_garden" => Some(Self::OutdoorGarden),
            "outdoor_grill" => Some(Self::OutdoorGrill),
            "outgoing_mail" => Some(Self::OutgoingMail),
            "outlet" => Some(Self::Outlet),
            "outlined_flag" => Some(Self::OutlinedFlag),
            "outpatient" => Some(Self::Outpatient),
            "outpatient_med" => Some(Self::OutpatientMed),
            "output" => Some(Self::Output),
            "output_circle" => Some(Self::OutputCircle),
            "oven" => Some(Self::Oven),
            "oven_gen" => Some(Self::OvenGen),
            "overview" => Some(Self::Overview),
            "overview_key" => Some(Self::OverviewKey),
            "owl" => Some(Self::Owl),
            "oxygen_saturation" => Some(Self::OxygenSaturation),
            "p2p" => Some(Self::P2p),
            "pace" => Some(Self::Pace),
            "pacemaker" => Some(Self::Pacemaker),
            "package" => Some(Self::Package),
            "package_2" => Some(Self::Package2),
            "padding" => Some(Self::Padding),
            "padel" => Some(Self::Padel),
            "page_control" => Some(Self::PageControl),
            "page_footer" => Some(Self::PageFooter),
            "page_header" => Some(Self::PageHeader),
            "page_info" => Some(Self::PageInfo),
            "page_menu_ios" => Some(Self::PageMenuIos),
            "pageless" => Some(Self::Pageless),
            "pages" => Some(Self::Pages),
            "pageview" => Some(Self::Pageview),
            "paid" => Some(Self::Paid),
            "palette" => Some(Self::Palette),
            "pallet" => Some(Self::Pallet),
            "pan_tool" => Some(Self::PanTool),
            "pan_tool_alt" => Some(Self::PanToolAlt),
            "pan_zoom" => Some(Self::PanZoom),
            "panorama" => Some(Self::Panorama),
            "panorama_fish_eye" => Some(Self::PanoramaFishEye),
            "panorama_horizontal" => Some(Self::PanoramaHorizontal),
            "panorama_photosphere" => Some(Self::PanoramaPhotosphere),
            "panorama_vertical" => Some(Self::PanoramaVertical),
            "panorama_wide_angle" => Some(Self::PanoramaWideAngle),
            "paragliding" => Some(Self::Paragliding),
            "parent_child_dining" => Some(Self::ParentChildDining),
            "park" => Some(Self::Park),
            "parking_meter" => Some(Self::ParkingMeter),
            "parking_sign" => Some(Self::ParkingSign),
            "parking_valet" => Some(Self::ParkingValet),
            "partly_cloudy_day" => Some(Self::PartlyCloudyDay),
            "partly_cloudy_night" => Some(Self::PartlyCloudyNight),
            "partner_exchange" => Some(Self::PartnerExchange),
            "partner_heart" => Some(Self::PartnerHeart),
            "partner_reports" => Some(Self::PartnerReports),
            "party_mode" => Some(Self::PartyMode),
            "passkey" => Some(Self::Passkey),
            "passport" => Some(Self::Passport),
            "password" => Some(Self::Password),
            "password_2" => Some(Self::Password2),
            "password_2_off" => Some(Self::Password2Off),
            "patient_list" => Some(Self::PatientList),
            "pattern" => Some(Self::Pattern),
            "pause_circle" => Some(Self::PauseCircle),
            "pause_circle_filled" => Some(Self::PauseCircleFilled),
            "pause_circle_outline" => Some(Self::PauseCircleOutline),
            "pause_presentation" => Some(Self::PausePresentation),
            "payment" => Some(Self::Payment),
            "payment_arrow_down" => Some(Self::PaymentArrowDown),
            "payment_card" => Some(Self::PaymentCard),
            "payments" => Some(Self::Payments),
            "pedal_bike" => Some(Self::PedalBike),
            "pediatrics" => Some(Self::Pediatrics),
            "pen_size_1" => Some(Self::PenSize1),
            "pen_size_2" => Some(Self::PenSize2),
            "pen_size_3" => Some(Self::PenSize3),
            "pen_size_4" => Some(Self::PenSize4),
            "pen_size_5" => Some(Self::PenSize5),
            "pending" => Some(Self::Pending),
            "pending_actions" => Some(Self::PendingActions),
            "pentagon" => Some(Self::Pentagon),
            "people" => Some(Self::People),
            "people_alt" => Some(Self::PeopleAlt),
            "people_outline" => Some(Self::PeopleOutline),
            "people_size_decrease" => Some(Self::PeopleSizeDecrease),
            "people_size_increase" => Some(Self::PeopleSizeIncrease),
            "percent" => Some(Self::Percent),
            "percent_discount" => Some(Self::PercentDiscount),
            "performance_max" => Some(Self::PerformanceMax),
            "pergola" => Some(Self::Pergola),
            "perm_camera_mic" => Some(Self::PermCameraMic),
            "perm_contact_calendar" => Some(Self::PermContactCalendar),
            "perm_data_setting" => Some(Self::PermDataSetting),
            "perm_device_information" => Some(Self::PermDeviceInformation),
            "perm_identity" => Some(Self::PermIdentity),
            "perm_media" => Some(Self::PermMedia),
            "perm_phone_msg" => Some(Self::PermPhoneMsg),
            "perm_scan_wifi" => Some(Self::PermScanWifi),
            "person" => Some(Self::Person),
            "person_2" => Some(Self::Person2),
            "person_3" => Some(Self::Person3),
            "person_4" => Some(Self::Person4),
            "person_add" => Some(Self::PersonAdd),
            "person_add_alt" => Some(Self::PersonAddAlt),
            "person_add_disabled" => Some(Self::PersonAddDisabled),
            "person_alert" => Some(Self::PersonAlert),
            "person_apron" => Some(Self::PersonApron),
            "person_book" => Some(Self::PersonBook),
            "person_cancel" => Some(Self::PersonCancel),
            "person_celebrate" => Some(Self::PersonCelebrate),
            "person_check" => Some(Self::PersonCheck),
            "person_edit" => Some(Self::PersonEdit),
            "person_filled" => Some(Self::PersonFilled),
            "person_heart" => Some(Self::PersonHeart),
            "person_off" => Some(Self::PersonOff),
            "person_outline" => Some(Self::PersonOutline),
            "person_pin" => Some(Self::PersonPin),
            "person_pin_circle" => Some(Self::PersonPinCircle),
            "person_play" => Some(Self::PersonPlay),
            "person_raised_hand" => Some(Self::PersonRaisedHand),
            "person_remove" => Some(Self::PersonRemove),
            "person_search" => Some(Self::PersonSearch),
            "person_shield" => Some(Self::PersonShield),
            "person_text" => Some(Self::PersonText),
            "personal_bag" => Some(Self::PersonalBag),
            "personal_bag_off" => Some(Self::PersonalBagOff),
            "personal_bag_question" => Some(Self::PersonalBagQuestion),
            "personal_injury" => Some(Self::PersonalInjury),
            "personal_places" => Some(Self::PersonalPlaces),
            "personal_video" => Some(Self::PersonalVideo),
            "pest_control" => Some(Self::PestControl),
            "pest_control_rodent" => Some(Self::PestControlRodent),
            "pet_supplies" => Some(Self::PetSupplies),
            "pets" => Some(Self::Pets),
            "phishing" => Some(Self::Phishing),
            "phone" => Some(Self::Phone),
            "phone_alt" => Some(Self::PhoneAlt),
            "phone_android" => Some(Self::PhoneAndroid),
            "phone_bluetooth_speaker" => Some(Self::PhoneBluetoothSpeaker),
            "phone_callback" => Some(Self::PhoneCallback),
            "phone_cancel" => Some(Self::PhoneCancel),
            "phone_disabled" => Some(Self::PhoneDisabled),
            "phone_enabled" => Some(Self::PhoneEnabled),
            "phone_forwarded" => Some(Self::PhoneForwarded),
            "phone_in_talk" => Some(Self::PhoneInTalk),
            "phone_iphone" => Some(Self::PhoneIphone),
            "phone_locked" => Some(Self::PhoneLocked),
            "phone_missed" => Some(Self::PhoneMissed),
            "phone_paused" => Some(Self::PhonePaused),
            "phonelink" => Some(Self::Phonelink),
            "phonelink_erase" => Some(Self::PhonelinkErase),
            "phonelink_lock" => Some(Self::PhonelinkLock),
            "phonelink_off" => Some(Self::PhonelinkOff),
            "phonelink_ring" => Some(Self::PhonelinkRing),
            "phonelink_ring_off" => Some(Self::PhonelinkRingOff),
            "phonelink_setup" => Some(Self::PhonelinkSetup),
            "photo" => Some(Self::Photo),
            "photo_album" => Some(Self::PhotoAlbum),
            "photo_auto_merge" => Some(Self::PhotoAutoMerge),
            "photo_camera" => Some(Self::PhotoCamera),
            "photo_camera_back" => Some(Self::PhotoCameraBack),
            "photo_camera_front" => Some(Self::PhotoCameraFront),
            "photo_filter" => Some(Self::PhotoFilter),
            "photo_frame" => Some(Self::PhotoFrame),
            "photo_library" => Some(Self::PhotoLibrary),
            "photo_prints" => Some(Self::PhotoPrints),
            "photo_size_select_actual" => Some(Self::PhotoSizeSelectActual),
            "photo_size_select_large" => Some(Self::PhotoSizeSelectLarge),
            "photo_size_select_small" => Some(Self::PhotoSizeSelectSmall),
            "php" => Some(Self::Php),
            "physical_therapy" => Some(Self::PhysicalTherapy),
            "piano" => Some(Self::Piano),
            "piano_off" => Some(Self::PianoOff),
            "pickleball" => Some(Self::Pickleball),
            "picture_as_pdf" => Some(Self::PictureAsPdf),
            "picture_in_picture" => Some(Self::PictureInPicture),
            "picture_in_picture_alt" => Some(Self::PictureInPictureAlt),
            "picture_in_picture_center" => Some(Self::PictureInPictureCenter),
            "picture_in_picture_large" => Some(Self::PictureInPictureLarge),
            "picture_in_picture_medium" => Some(Self::PictureInPictureMedium),
            "picture_in_picture_mobile" => Some(Self::PictureInPictureMobile),
            "picture_in_picture_off" => Some(Self::PictureInPictureOff),
            "picture_in_picture_small" => Some(Self::PictureInPictureSmall),
            "pie_chart" => Some(Self::PieChart),
            "pie_chart_filled" => Some(Self::PieChartFilled),
            "pie_chart_outline" => Some(Self::PieChartOutline),
            "pie_chart_outlined" => Some(Self::PieChartOutlined),
            "pill" => Some(Self::Pill),
            "pill_off" => Some(Self::PillOff),
            "pin" => Some(Self::Pin),
            "pin_drop" => Some(Self::PinDrop),
            "pin_end" => Some(Self::PinEnd),
            "pin_history" => Some(Self::PinHistory),
            "pin_invoke" => Some(Self::PinInvoke),
            "pin_road" => Some(Self::PinRoad),
            "pin_road_2" => Some(Self::PinRoad2),
            "pinboard" => Some(Self::Pinboard),
            "pinboard_unread" => Some(Self::PinboardUnread),
            "pinch" => Some(Self::Pinch),
            "pinch_zoom_in" => Some(Self::PinchZoomIn),
            "pinch_zoom_out" => Some(Self::PinchZoomOut),
            "pip" => Some(Self::Pip),
            "pip_exit" => Some(Self::PipExit),
            "pivot_table_chart" => Some(Self::PivotTableChart),
            "place" => Some(Self::Place),
            "place_item" => Some(Self::PlaceItem),
            "plagiarism" => Some(Self::Plagiarism),
            "plane_contrails" => Some(Self::PlaneContrails),
            "planet" => Some(Self::Planet),
            "planner_banner_ad_pt" => Some(Self::PlannerBannerAdPt),
            "planner_review" => Some(Self::PlannerReview),
            "play_arrow" => Some(Self::PlayArrow),
            "play_circle" => Some(Self::PlayCircle),
            "play_disabled" => Some(Self::PlayDisabled),
            "play_for_work" => Some(Self::PlayForWork),
            "play_lesson" => Some(Self::PlayLesson),
            "play_music" => Some(Self::PlayMusic),
            "play_pause" => Some(Self::PlayPause),
            "play_shapes" => Some(Self::PlayShapes),
            "playground" => Some(Self::Playground),
            "playground_2" => Some(Self::Playground2),
            "playing_cards" => Some(Self::PlayingCards),
            "playlist_add" => Some(Self::PlaylistAdd),
            "playlist_add_check" => Some(Self::PlaylistAddCheck),
            "playlist_add_check_circle" => Some(Self::PlaylistAddCheckCircle),
            "playlist_add_circle" => Some(Self::PlaylistAddCircle),
            "playlist_play" => Some(Self::PlaylistPlay),
            "playlist_remove" => Some(Self::PlaylistRemove),
            "plug_connect" => Some(Self::PlugConnect),
            "plumbing" => Some(Self::Plumbing),
            "plus_one" => Some(Self::PlusOne),
            "podcasts" => Some(Self::Podcasts),
            "podiatry" => Some(Self::Podiatry),
            "podium" => Some(Self::Podium),
            "point_of_sale" => Some(Self::PointOfSale),
            "point_scan" => Some(Self::PointScan),
            "poker_chip" => Some(Self::PokerChip),
            "policy" => Some(Self::Policy),
            "policy_alert" => Some(Self::PolicyAlert),
            "poll" => Some(Self::Poll),
            "polyline" => Some(Self::Polyline),
            "polymer" => Some(Self::Polymer),
            "pool" => Some(Self::Pool),
            "portable_wifi_off" => Some(Self::PortableWifiOff),
            "portrait" => Some(Self::Portrait),
            "position_bottom_left" => Some(Self::PositionBottomLeft),
            "position_bottom_right" => Some(Self::PositionBottomRight),
            "position_top_right" => Some(Self::PositionTopRight),
            "post" => Some(Self::Post),
            "post_add" => Some(Self::PostAdd),
            "potted_plant" => Some(Self::PottedPlant),
            "power" => Some(Self::Power),
            "power_input" => Some(Self::PowerInput),
            "power_off" => Some(Self::PowerOff),
            "power_rounded" => Some(Self::PowerRounded),
            "power_settings_circle" => Some(Self::PowerSettingsCircle),
            "power_settings_new" => Some(Self::PowerSettingsNew),
            "prayer_times" => Some(Self::PrayerTimes),
            "precision_manufacturing" => Some(Self::PrecisionManufacturing),
            "pregnancy" => Some(Self::Pregnancy),
            "pregnant_woman" => Some(Self::PregnantWoman),
            "preliminary" => Some(Self::Preliminary),
            "prescriptions" => Some(Self::Prescriptions),
            "present_to_all" => Some(Self::PresentToAll),
            "preview" => Some(Self::Preview),
            "preview_off" => Some(Self::PreviewOff),
            "price_change" => Some(Self::PriceChange),
            "price_check" => Some(Self::PriceCheck),
            "print_add" => Some(Self::PrintAdd),
            "print_connect" => Some(Self::PrintConnect),
            "print_disabled" => Some(Self::PrintDisabled),
            "print_error" => Some(Self::PrintError),
            "print_lock" => Some(Self::PrintLock),
            "priority" => Some(Self::Priority),
            "priority_high" => Some(Self::PriorityHigh),
            "privacy" => Some(Self::Privacy),
            "privacy_tip" => Some(Self::PrivacyTip),
            "private_connectivity" => Some(Self::PrivateConnectivity),
            "problem" => Some(Self::Problem),
            "procedure" => Some(Self::Procedure),
            "process_chart" => Some(Self::ProcessChart),
            "production_quantity_limits" => Some(Self::ProductionQuantityLimits),
            "productivity" => Some(Self::Productivity),
            "progress_activity" => Some(Self::ProgressActivity),
            "prompt_suggestion" => Some(Self::PromptSuggestion),
            "propane" => Some(Self::Propane),
            "propane_tank" => Some(Self::PropaneTank),
            "psychiatry" => Some(Self::Psychiatry),
            "psychology" => Some(Self::Psychology),
            "psychology_alt" => Some(Self::PsychologyAlt),
            "public" => Some(Self::Public),
            "public_off" => Some(Self::PublicOff),
            "publish" => Some(Self::Publish),
            "published_with_changes" => Some(Self::PublishedWithChanges),
            "pulmonology" => Some(Self::Pulmonology),
            "pulse_alert" => Some(Self::PulseAlert),
            "punch_clock" => Some(Self::PunchClock),
            "push_pin" => Some(Self::PushPin),
            "qr_code" => Some(Self::QrCode),
            "qr_code_2" => Some(Self::QrCode2),
            "qr_code_2_add" => Some(Self::QrCode2Add),
            "qr_code_scanner" => Some(Self::QrCodeScanner),
            "query_builder" => Some(Self::QueryBuilder),
            "query_stats" => Some(Self::QueryStats),
            "question_answer" => Some(Self::QuestionAnswer),
            "question_exchange" => Some(Self::QuestionExchange),
            "question_mark" => Some(Self::QuestionMark),
            "queue" => Some(Self::Queue),
            "queue_music" => Some(Self::QueueMusic),
            "queue_play_next" => Some(Self::QueuePlayNext),
            "quick_phrases" => Some(Self::QuickPhrases),
            "quick_reference" => Some(Self::QuickReference),
            "quick_reference_all" => Some(Self::QuickReferenceAll),
            "quick_reorder" => Some(Self::QuickReorder),
            "quickreply" => Some(Self::Quickreply),
            "quiet_time" => Some(Self::QuietTime),
            "quiet_time_active" => Some(Self::QuietTimeActive),
            "quiz" => Some(Self::Quiz),
            "r_mobiledata" => Some(Self::RMobiledata),
            "radar" => Some(Self::Radar),
            "radio" => Some(Self::Radio),
            "radio_button_checked" => Some(Self::RadioButtonChecked),
            "radio_button_partial" => Some(Self::RadioButtonPartial),
            "radio_button_unchecked" => Some(Self::RadioButtonUnchecked),
            "radiology" => Some(Self::Radiology),
            "railway_alert" => Some(Self::RailwayAlert),
            "railway_alert_2" => Some(Self::RailwayAlert2),
            "rainy" => Some(Self::Rainy),
            "rainy_heavy" => Some(Self::RainyHeavy),
            "rainy_light" => Some(Self::RainyLight),
            "rainy_snow" => Some(Self::RainySnow),
            "ramen_dining" => Some(Self::RamenDining),
            "ramp_left" => Some(Self::RampLeft),
            "ramp_right" => Some(Self::RampRight),
            "range_hood" => Some(Self::RangeHood),
            "rate_review" => Some(Self::RateReview),
            "rate_review_rtl" => Some(Self::RateReviewRtl),
            "raven" => Some(Self::Raven),
            "raw_off" => Some(Self::RawOff),
            "raw_on" => Some(Self::RawOn),
            "read_more" => Some(Self::ReadMore),
            "readiness_score" => Some(Self::ReadinessScore),
            "real_estate_agent" => Some(Self::RealEstateAgent),
            "rear_camera" => Some(Self::RearCamera),
            "rebase" => Some(Self::Rebase),
            "rebase_edit" => Some(Self::RebaseEdit),
            "receipt" => Some(Self::Receipt),
            "receipt_long" => Some(Self::ReceiptLong),
            "receipt_long_off" => Some(Self::ReceiptLongOff),
            "recent_actors" => Some(Self::RecentActors),
            "recent_patient" => Some(Self::RecentPatient),
            "recenter" => Some(Self::Recenter),
            "recommend" => Some(Self::Recommend),
            "record_voice_over" => Some(Self::RecordVoiceOver),
            "rectangle" => Some(Self::Rectangle),
            "rectangle_add" => Some(Self::RectangleAdd),
            "recycling" => Some(Self::Recycling),
            "redeem" => Some(Self::Redeem),
            "reduce_capacity" => Some(Self::ReduceCapacity),
            "regular_expression" => Some(Self::RegularExpression),
            "relax" => Some(Self::Relax),
            "release_alert" => Some(Self::ReleaseAlert),
            "remember_me" => Some(Self::RememberMe),
            "reminder" => Some(Self::Reminder),
            "reminders_alt" => Some(Self::RemindersAlt),
            "remote_gen" => Some(Self::RemoteGen),
            "remove" => Some(Self::Remove),
            "remove_circle" => Some(Self::RemoveCircle),
            "remove_circle_outline" => Some(Self::RemoveCircleOutline),
            "remove_done" => Some(Self::RemoveDone),
            "remove_from_queue" => Some(Self::RemoveFromQueue),
            "remove_moderator" => Some(Self::RemoveModerator),
            "remove_red_eye" => Some(Self::RemoveRedEye),
            "remove_road" => Some(Self::RemoveRoad),
            "remove_selection" => Some(Self::RemoveSelection),
            "remove_shopping_cart" => Some(Self::RemoveShoppingCart),
            "reopen_window" => Some(Self::ReopenWindow),
            "reorder" => Some(Self::Reorder),
            "repartition" => Some(Self::Repartition),
            "repeat" => Some(Self::Repeat),
            "repeat_on" => Some(Self::RepeatOn),
            "repeat_one" => Some(Self::RepeatOne),
            "repeat_one_on" => Some(Self::RepeatOneOn),
            "replace_audio" => Some(Self::ReplaceAudio),
            "replace_image" => Some(Self::ReplaceImage),
            "replace_video" => Some(Self::ReplaceVideo),
            "replay" => Some(Self::Replay),
            "replay_10" => Some(Self::Replay10),
            "replay_30" => Some(Self::Replay30),
            "replay_5" => Some(Self::Replay5),
            "replay_circle_filled" => Some(Self::ReplayCircleFilled),
            "reply" => Some(Self::Reply),
            "reply_all" => Some(Self::ReplyAll),
            "report" => Some(Self::Report),
            "report_gmailerrorred" => Some(Self::ReportGmailerrorred),
            "report_off" => Some(Self::ReportOff),
            "report_problem" => Some(Self::ReportProblem),
            "request_page" => Some(Self::RequestPage),
            "request_quote" => Some(Self::RequestQuote),
            "reset_brightness" => Some(Self::ResetBrightness),
            "reset_colors" => Some(Self::ResetColors),
            "reset_exposure" => Some(Self::ResetExposure),
            "reset_focus" => Some(Self::ResetFocus),
            "reset_image" => Some(Self::ResetImage),
            "reset_iso" => Some(Self::ResetIso),
            "reset_settings" => Some(Self::ResetSettings),
            "reset_shadow" => Some(Self::ResetShadow),
            "reset_shutter_speed" => Some(Self::ResetShutterSpeed),
            "reset_tv" => Some(Self::ResetTv),
            "reset_white_balance" => Some(Self::ResetWhiteBalance),
            "reset_wrench" => Some(Self::ResetWrench),
            "resize" => Some(Self::Resize),
            "resize_window" => Some(Self::ResizeWindow),
            "respiratory_rate" => Some(Self::RespiratoryRate),
            "responsive_layout" => Some(Self::ResponsiveLayout),
            "rest_area" => Some(Self::RestArea),
            "restart_alt" => Some(Self::RestartAlt),
            "restaurant" => Some(Self::Restaurant),
            "restaurant_menu" => Some(Self::RestaurantMenu),
            "restore" => Some(Self::Restore),
            "restore_from_trash" => Some(Self::RestoreFromTrash),
            "restore_page" => Some(Self::RestorePage),
            "resume" => Some(Self::Resume),
            "reviews" => Some(Self::Reviews),
            "rewarded_ads" => Some(Self::RewardedAds),
            "rheumatology" => Some(Self::Rheumatology),
            "rib_cage" => Some(Self::RibCage),
            "rice_bowl" => Some(Self::RiceBowl),
            "right_click" => Some(Self::RightClick),
            "right_panel_close" => Some(Self::RightPanelClose),
            "right_panel_open" => Some(Self::RightPanelOpen),
            "ring_volume" => Some(Self::RingVolume),
            "ring_volume_filled" => Some(Self::RingVolumeFilled),
            "ripples" => Some(Self::Ripples),
            "road" => Some(Self::Road),
            "robot" => Some(Self::Robot),
            "robot_2" => Some(Self::Robot2),
            "rocket" => Some(Self::Rocket),
            "rocket_launch" => Some(Self::RocketLaunch),
            "roller_shades" => Some(Self::RollerShades),
            "roller_shades_closed" => Some(Self::RollerShadesClosed),
            "roller_skating" => Some(Self::RollerSkating),
            "roofing" => Some(Self::Roofing),
            "room" => Some(Self::Room),
            "room_preferences" => Some(Self::RoomPreferences),
            "room_service" => Some(Self::RoomService),
            "rotate_90_degrees_ccw" => Some(Self::Rotate90DegreesCcw),
            "rotate_90_degrees_cw" => Some(Self::Rotate90DegreesCw),
            "rotate_auto" => Some(Self::RotateAuto),
            "rotate_left" => Some(Self::RotateLeft),
            "rotate_right" => Some(Self::RotateRight),
            "roundabout_left" => Some(Self::RoundaboutLeft),
            "roundabout_right" => Some(Self::RoundaboutRight),
            "rounded_corner" => Some(Self::RoundedCorner),
            "route" => Some(Self::Route),
            "router" => Some(Self::Router),
            "router_off" => Some(Self::RouterOff),
            "routine" => Some(Self::Routine),
            "rowing" => Some(Self::Rowing),
            "rss_feed" => Some(Self::RssFeed),
            "rsvp" => Some(Self::Rsvp),
            "rtt" => Some(Self::Rtt),
            "rubric" => Some(Self::Rubric),
            "rule" => Some(Self::Rule),
            "rule_folder" => Some(Self::RuleFolder),
            "rule_settings" => Some(Self::RuleSettings),
            "run_circle" => Some(Self::RunCircle),
            "running_with_errors" => Some(Self::RunningWithErrors),
            "rv_hookup" => Some(Self::RvHookup),
            "safety_check" => Some(Self::SafetyCheck),
            "safety_check_off" => Some(Self::SafetyCheckOff),
            "safety_divider" => Some(Self::SafetyDivider),
            "sailing" => Some(Self::Sailing),
            "salinity" => Some(Self::Salinity),
            "sanitizer" => Some(Self::Sanitizer),
            "satellite" => Some(Self::Satellite),
            "satellite_alt" => Some(Self::SatelliteAlt),
            "sauna" => Some(Self::Sauna),
            "save_alt" => Some(Self::SaveAlt),
            "save_as" => Some(Self::SaveAs),
            "save_clock" => Some(Self::SaveClock),
            "saved_search" => Some(Self::SavedSearch),
            "savings" => Some(Self::Savings),
            "scale" => Some(Self::Scale),
            "scan" => Some(Self::Scan),
            "scan_delete" => Some(Self::ScanDelete),
            "scanner" => Some(Self::Scanner),
            "scatter_plot" => Some(Self::ScatterPlot),
            "scene" => Some(Self::Scene),
            "schedule_send" => Some(Self::ScheduleSend),
            "schema" => Some(Self::Schema),
            "school" => Some(Self::School),
            "science" => Some(Self::Science),
            "science_off" => Some(Self::ScienceOff),
            "scooter" => Some(Self::Scooter),
            "score" => Some(Self::Score),
            "scoreboard" => Some(Self::Scoreboard),
            "screen_lock_landscape" => Some(Self::ScreenLockLandscape),
            "screen_lock_portrait" => Some(Self::ScreenLockPortrait),
            "screen_lock_rotation" => Some(Self::ScreenLockRotation),
            "screen_record" => Some(Self::ScreenRecord),
            "screen_rotation" => Some(Self::ScreenRotation),
            "screen_rotation_alt" => Some(Self::ScreenRotationAlt),
            "screen_rotation_up" => Some(Self::ScreenRotationUp),
            "screen_search_desktop" => Some(Self::ScreenSearchDesktop),
            "screen_share" => Some(Self::ScreenShare),
            "screenshot" => Some(Self::Screenshot),
            "screenshot_frame" => Some(Self::ScreenshotFrame),
            "screenshot_frame_2" => Some(Self::ScreenshotFrame2),
            "screenshot_keyboard" => Some(Self::ScreenshotKeyboard),
            "screenshot_monitor" => Some(Self::ScreenshotMonitor),
            "screenshot_region" => Some(Self::ScreenshotRegion),
            "screenshot_tablet" => Some(Self::ScreenshotTablet),
            "script" => Some(Self::Script),
            "scrollable_header" => Some(Self::ScrollableHeader),
            "scuba_diving" => Some(Self::ScubaDiving),
            "sd" => Some(Self::Sd),
            "sd_card" => Some(Self::SdCard),
            "sd_card_alert" => Some(Self::SdCardAlert),
            "sd_storage" => Some(Self::SdStorage),
            "sdk" => Some(Self::Sdk),
            "search_activity" => Some(Self::SearchActivity),
            "search_check" => Some(Self::SearchCheck),
            "search_check_2" => Some(Self::SearchCheck2),
            "search_gear" => Some(Self::SearchGear),
            "search_hands_free" => Some(Self::SearchHandsFree),
            "search_insights" => Some(Self::SearchInsights),
            "search_off" => Some(Self::SearchOff),
            "seat_cool_left" => Some(Self::SeatCoolLeft),
            "seat_cool_right" => Some(Self::SeatCoolRight),
            "seat_heat_left" => Some(Self::SeatHeatLeft),
            "seat_heat_right" => Some(Self::SeatHeatRight),
            "seat_read" => Some(Self::SeatRead),
            "seat_vent_left" => Some(Self::SeatVentLeft),
            "seat_vent_right" => Some(Self::SeatVentRight),
            "seat_window" => Some(Self::SeatWindow),
            "security" => Some(Self::Security),
            "security_key" => Some(Self::SecurityKey),
            "security_update" => Some(Self::SecurityUpdate),
            "security_update_good" => Some(Self::SecurityUpdateGood),
            "security_update_warning" => Some(Self::SecurityUpdateWarning),
            "segment" => Some(Self::Segment),
            "select" => Some(Self::Select),
            "select_all" => Some(Self::SelectAll),
            "select_check_box" => Some(Self::SelectCheckBox),
            "select_to_speak" => Some(Self::SelectToSpeak),
            "select_window" => Some(Self::SelectWindow),
            "select_window_2" => Some(Self::SelectWindow2),
            "select_window_off" => Some(Self::SelectWindowOff),
            "self_care" => Some(Self::SelfCare),
            "self_improvement" => Some(Self::SelfImprovement),
            "sell" => Some(Self::Sell),
            "sell_cloud" => Some(Self::SellCloud),
            "send_and_archive" => Some(Self::SendAndArchive),
            "send_money" => Some(Self::SendMoney),
            "send_time_extension" => Some(Self::SendTimeExtension),
            "send_to_mobile" => Some(Self::SendToMobile),
            "sensor_door" => Some(Self::SensorDoor),
            "sensor_occupied" => Some(Self::SensorOccupied),
            "sensor_window" => Some(Self::SensorWindow),
            "sensors" => Some(Self::Sensors),
            "sensors_krx" => Some(Self::SensorsKrx),
            "sensors_krx_off" => Some(Self::SensorsKrxOff),
            "sensors_off" => Some(Self::SensorsOff),
            "sentiment_calm" => Some(Self::SentimentCalm),
            "sentiment_content" => Some(Self::SentimentContent),
            "sentiment_dissatisfied" => Some(Self::SentimentDissatisfied),
            "sentiment_excited" => Some(Self::SentimentExcited),
            "sentiment_extremely_dissatisfied" => Some(Self::SentimentExtremelyDissatisfied),
            "sentiment_frustrated" => Some(Self::SentimentFrustrated),
            "sentiment_neutral" => Some(Self::SentimentNeutral),
            "sentiment_sad" => Some(Self::SentimentSad),
            "sentiment_satisfied" => Some(Self::SentimentSatisfied),
            "sentiment_satisfied_alt" => Some(Self::SentimentSatisfiedAlt),
            "sentiment_stressed" => Some(Self::SentimentStressed),
            "sentiment_very_dissatisfied" => Some(Self::SentimentVeryDissatisfied),
            "sentiment_very_satisfied" => Some(Self::SentimentVerySatisfied),
            "sentiment_worried" => Some(Self::SentimentWorried),
            "serif" => Some(Self::Serif),
            "server_person" => Some(Self::ServerPerson),
            "service_toolbox" => Some(Self::ServiceToolbox),
            "set_meal" => Some(Self::SetMeal),
            "settings_accessibility" => Some(Self::SettingsAccessibility),
            "settings_account_box" => Some(Self::SettingsAccountBox),
            "settings_alert" => Some(Self::SettingsAlert),
            "settings_applications" => Some(Self::SettingsApplications),
            "settings_b_roll" => Some(Self::SettingsBRoll),
            "settings_backup_restore" => Some(Self::SettingsBackupRestore),
            "settings_bluetooth" => Some(Self::SettingsBluetooth),
            "settings_brightness" => Some(Self::SettingsBrightness),
            "settings_cell" => Some(Self::SettingsCell),
            "settings_cinematic_blur" => Some(Self::SettingsCinematicBlur),
            "settings_ethernet" => Some(Self::SettingsEthernet),
            "settings_heart" => Some(Self::SettingsHeart),
            "settings_input_antenna" => Some(Self::SettingsInputAntenna),
            "settings_input_component" => Some(Self::SettingsInputComponent),
            "settings_input_composite" => Some(Self::SettingsInputComposite),
            "settings_input_hdmi" => Some(Self::SettingsInputHdmi),
            "settings_input_svideo" => Some(Self::SettingsInputSvideo),
            "settings_motion_mode" => Some(Self::SettingsMotionMode),
            "settings_night_sight" => Some(Self::SettingsNightSight),
            "settings_overscan" => Some(Self::SettingsOverscan),
            "settings_panorama" => Some(Self::SettingsPanorama),
            "settings_phone" => Some(Self::SettingsPhone),
            "settings_photo_camera" => Some(Self::SettingsPhotoCamera),
            "settings_power" => Some(Self::SettingsPower),
            "settings_remote" => Some(Self::SettingsRemote),
            "settings_screen" => Some(Self::SettingsScreen),
            "settings_seating" => Some(Self::SettingsSeating),
            "settings_slow_motion" => Some(Self::SettingsSlowMotion),
            "settings_suggest" => Some(Self::SettingsSuggest),
            "settings_system_daydream" => Some(Self::SettingsSystemDaydream),
            "settings_timelapse" => Some(Self::SettingsTimelapse),
            "settings_video_camera" => Some(Self::SettingsVideoCamera),
            "settings_voice" => Some(Self::SettingsVoice),
            "settop_component" => Some(Self::SettopComponent),
            "severe_cold" => Some(Self::SevereCold),
            "shades" => Some(Self::Shades),
            "shades_closed" => Some(Self::ShadesClosed),
            "shadow" => Some(Self::Shadow),
            "shadow_add" => Some(Self::ShadowAdd),
            "shadow_minus" => Some(Self::ShadowMinus),
            "shape_line" => Some(Self::ShapeLine),
            "shape_recognition" => Some(Self::ShapeRecognition),
            "shapes" => Some(Self::Shapes),
            "share_eta" => Some(Self::ShareEta),
            "share_location" => Some(Self::ShareLocation),
            "share_off" => Some(Self::ShareOff),
            "share_reviews" => Some(Self::ShareReviews),
            "share_windows" => Some(Self::ShareWindows),
            "shaved_ice" => Some(Self::ShavedIce),
            "sheets_column_swap" => Some(Self::SheetsColumnSwap),
            "sheets_rtl" => Some(Self::SheetsRtl),
            "shelf_auto_hide" => Some(Self::ShelfAutoHide),
            "shelf_position" => Some(Self::ShelfPosition),
            "shelves" => Some(Self::Shelves),
            "shield" => Some(Self::Shield),
            "shield_card" => Some(Self::ShieldCard),
            "shield_lock" => Some(Self::ShieldLock),
            "shield_locked" => Some(Self::ShieldLocked),
            "shield_moon" => Some(Self::ShieldMoon),
            "shield_person" => Some(Self::ShieldPerson),
            "shield_question" => Some(Self::ShieldQuestion),
            "shield_radar" => Some(Self::ShieldRadar),
            "shield_toggle" => Some(Self::ShieldToggle),
            "shield_watch" => Some(Self::ShieldWatch),
            "shield_with_heart" => Some(Self::ShieldWithHeart),
            "shield_with_house" => Some(Self::ShieldWithHouse),
            "shift" => Some(Self::Shift),
            "shift_lock" => Some(Self::ShiftLock),
            "shift_lock_off" => Some(Self::ShiftLockOff),
            "shoe_cleats" => Some(Self::ShoeCleats),
            "shop" => Some(Self::Shop),
            "shop_2" => Some(Self::Shop2),
            "shop_two" => Some(Self::ShopTwo),
            "shopping_bag" => Some(Self::ShoppingBag),
            "shopping_bag_speed" => Some(Self::ShoppingBagSpeed),
            "shopping_basket" => Some(Self::ShoppingBasket),
            "shopping_cart" => Some(Self::ShoppingCart),
            "shopping_cart_checkout" => Some(Self::ShoppingCartCheckout),
            "shopping_cart_off" => Some(Self::ShoppingCartOff),
            "shoppingmode" => Some(Self::Shoppingmode),
            "short_stay" => Some(Self::ShortStay),
            "short_text" => Some(Self::ShortText),
            "shortcut" => Some(Self::Shortcut),
            "show_chart" => Some(Self::ShowChart),
            "shower" => Some(Self::Shower),
            "shuffle" => Some(Self::Shuffle),
            "shuffle_on" => Some(Self::ShuffleOn),
            "shutter_speed" => Some(Self::ShutterSpeed),
            "shutter_speed_add" => Some(Self::ShutterSpeedAdd),
            "shutter_speed_minus" => Some(Self::ShutterSpeedMinus),
            "sick" => Some(Self::Sick),
            "side_navigation" => Some(Self::SideNavigation),
            "sign_language" => Some(Self::SignLanguage),
            "sign_language_2" => Some(Self::SignLanguage2),
            "sign_language_off" => Some(Self::SignLanguageOff),
            "signal_cellular_0_bar" => Some(Self::SignalCellular0Bar),
            "signal_cellular_1_bar" => Some(Self::SignalCellular1Bar),
            "signal_cellular_2_bar" => Some(Self::SignalCellular2Bar),
            "signal_cellular_3_bar" => Some(Self::SignalCellular3Bar),
            "signal_cellular_4_bar" => Some(Self::SignalCellular4Bar),
            "signal_cellular_add" => Some(Self::SignalCellularAdd),
            "signal_cellular_alt" => Some(Self::SignalCellularAlt),
            "signal_cellular_alt_1_bar" => Some(Self::SignalCellularAlt1Bar),
            "signal_cellular_alt_2_bar" => Some(Self::SignalCellularAlt2Bar),
            "signal_cellular_alt_off" => Some(Self::SignalCellularAltOff),
            "signal_cellular_connected_no_internet_0_bar" => Some(Self::SignalCellularConnectedNoInternet0Bar),
            "signal_cellular_connected_no_internet_4_bar" => Some(Self::SignalCellularConnectedNoInternet4Bar),
            "signal_cellular_no_sim" => Some(Self::SignalCellularNoSim),
            "signal_cellular_nodata" => Some(Self::SignalCellularNodata),
            "signal_cellular_null" => Some(Self::SignalCellularNull),
            "signal_cellular_off" => Some(Self::SignalCellularOff),
            "signal_cellular_pause" => Some(Self::SignalCellularPause),
            "signal_disconnected" => Some(Self::SignalDisconnected),
            "signal_wifi_0_bar" => Some(Self::SignalWifi0Bar),
            "signal_wifi_4_bar" => Some(Self::SignalWifi4Bar),
            "signal_wifi_4_bar_lock" => Some(Self::SignalWifi4BarLock),
            "signal_wifi_bad" => Some(Self::SignalWifiBad),
            "signal_wifi_connected_no_internet_4" => Some(Self::SignalWifiConnectedNoInternet4),
            "signal_wifi_off" => Some(Self::SignalWifiOff),
            "signal_wifi_statusbar_4_bar" => Some(Self::SignalWifiStatusbar4Bar),
            "signal_wifi_statusbar_not_connected" => Some(Self::SignalWifiStatusbarNotConnected),
            "signal_wifi_statusbar_null" => Some(Self::SignalWifiStatusbarNull),
            "signature" => Some(Self::Signature),
            "signpost" => Some(Self::Signpost),
            "sim_card" => Some(Self::SimCard),
            "sim_card_alert" => Some(Self::SimCardAlert),
            "sim_card_download" => Some(Self::SimCardDownload),
            "sim_card_lock" => Some(Self::SimCardLock),
            "simulation" => Some(Self::Simulation),
            "single_arrow" => Some(Self::SingleArrow),
            "single_bed" => Some(Self::SingleBed),
            "sip" => Some(Self::Sip),
            "siren" => Some(Self::Siren),
            "siren_check" => Some(Self::SirenCheck),
            "siren_open" => Some(Self::SirenOpen),
            "siren_question" => Some(Self::SirenQuestion),
            "skateboarding" => Some(Self::Skateboarding),
            "skeleton" => Some(Self::Skeleton),
            "skillet" => Some(Self::Skillet),
            "skillet_cooktop" => Some(Self::SkilletCooktop),
            "skip_previous" => Some(Self::SkipPrevious),
            "skull" => Some(Self::Skull),
            "skull_list" => Some(Self::SkullList),
            "slab_serif" => Some(Self::SlabSerif),
            "sledding" => Some(Self::Sledding),
            "sleep" => Some(Self::Sleep),
            "sleep_score" => Some(Self::SleepScore),
            "slide_library" => Some(Self::SlideLibrary),
            "sliders" => Some(Self::Sliders),
            "slideshow" => Some(Self::Slideshow),
            "slow_motion_video" => Some(Self::SlowMotionVideo),
            "smart_button" => Some(Self::SmartButton),
            "smart_card_reader" => Some(Self::SmartCardReader),
            "smart_card_reader_off" => Some(Self::SmartCardReaderOff),
            "smart_display" => Some(Self::SmartDisplay),
            "smart_outlet" => Some(Self::SmartOutlet),
            "smart_screen" => Some(Self::SmartScreen),
            "smart_toy" => Some(Self::SmartToy),
            "smartphone" => Some(Self::Smartphone),
            "smartphone_camera" => Some(Self::SmartphoneCamera),
            "smb_share" => Some(Self::SmbShare),
            "smoke_free" => Some(Self::SmokeFree),
            "smoking_rooms" => Some(Self::SmokingRooms),
            "sms" => Some(Self::Sms),
            "sms_failed" => Some(Self::SmsFailed),
            "snail" => Some(Self::Snail),
            "snippet_folder" => Some(Self::SnippetFolder),
            "snooze" => Some(Self::Snooze),
            "snowboarding" => Some(Self::Snowboarding),
            "snowflake" => Some(Self::Snowflake),
            "snowing" => Some(Self::Snowing),
            "snowing_heavy" => Some(Self::SnowingHeavy),
            "snowmobile" => Some(Self::Snowmobile),
            "snowshoeing" => Some(Self::Snowshoeing),
            "soap" => Some(Self::Soap),
            "soba" => Some(Self::Soba),
            "social_distance" => Some(Self::SocialDistance),
            "social_leaderboard" => Some(Self::SocialLeaderboard),
            "solar_power" => Some(Self::SolarPower),
            "solo_dining" => Some(Self::SoloDining),
            "sort_by_alpha" => Some(Self::SortByAlpha),
            "sos" => Some(Self::Sos),
            "sound_detection_dog_barking" => Some(Self::SoundDetectionDogBarking),
            "sound_detection_glass_break" => Some(Self::SoundDetectionGlassBreak),
            "sound_detection_loud_sound" => Some(Self::SoundDetectionLoudSound),
            "sound_sampler" => Some(Self::SoundSampler),
            "soundbar" => Some(Self::Soundbar),
            "soup_kitchen" => Some(Self::SoupKitchen),
            "source" => Some(Self::Source),
            "source_environment" => Some(Self::SourceEnvironment),
            "source_notes" => Some(Self::SourceNotes),
            "south" => Some(Self::South),
            "south_america" => Some(Self::SouthAmerica),
            "south_east" => Some(Self::SouthEast),
            "south_west" => Some(Self::SouthWest),
            "spa" => Some(Self::Spa),
            "space_bar" => Some(Self::SpaceBar),
            "space_dashboard" => Some(Self::SpaceDashboard),
            "space_dashboard_2" => Some(Self::SpaceDashboard2),
            "spatial_audio" => Some(Self::SpatialAudio),
            "spatial_audio_off" => Some(Self::SpatialAudioOff),
            "spatial_gallery" => Some(Self::SpatialGallery),
            "spatial_speaker" => Some(Self::SpatialSpeaker),
            "spatial_tracking" => Some(Self::SpatialTracking),
            "speaker" => Some(Self::Speaker),
            "speaker_2" => Some(Self::Speaker2),
            "speaker_3" => Some(Self::Speaker3),
            "speaker_group" => Some(Self::SpeakerGroup),
            "speaker_notes" => Some(Self::SpeakerNotes),
            "speaker_notes_off" => Some(Self::SpeakerNotesOff),
            "speaker_phone" => Some(Self::SpeakerPhone),
            "special_character" => Some(Self::SpecialCharacter),
            "specific_gravity" => Some(Self::SpecificGravity),
            "speech_to_text" => Some(Self::SpeechToText),
            "speech_to_text_2" => Some(Self::SpeechToText2),
            "speed" => Some(Self::Speed),
            "speed_0_25" => Some(Self::Speed025),
            "speed_0_2x" => Some(Self::Speed02x),
            "speed_0_5" => Some(Self::Speed05),
            "speed_0_5x" => Some(Self::Speed05x),
            "speed_0_75" => Some(Self::Speed075),
            "speed_0_7x" => Some(Self::Speed07x),
            "speed_1_2" => Some(Self::Speed12),
            "speed_1_25" => Some(Self::Speed125),
            "speed_1_2x" => Some(Self::Speed12x),
            "speed_1_5" => Some(Self::Speed15),
            "speed_1_5x" => Some(Self::Speed15x),
            "speed_1_75" => Some(Self::Speed175),
            "speed_1_7x" => Some(Self::Speed17x),
            "speed_2" => Some(Self::Speed2),
            "speed_2x" => Some(Self::Speed2x),
            "speed_3" => Some(Self::Speed3),
            "speed_4" => Some(Self::Speed4),
            "speed_camera" => Some(Self::SpeedCamera),
            "spellcheck" => Some(Self::Spellcheck),
            "split_scene" => Some(Self::SplitScene),
            "split_scene_2" => Some(Self::SplitScene2),
            "split_scene_down" => Some(Self::SplitSceneDown),
            "split_scene_left" => Some(Self::SplitSceneLeft),
            "split_scene_right" => Some(Self::SplitSceneRight),
            "split_scene_up" => Some(Self::SplitSceneUp),
            "splitscreen" => Some(Self::Splitscreen),
            "splitscreen_add" => Some(Self::SplitscreenAdd),
            "splitscreen_bottom" => Some(Self::SplitscreenBottom),
            "splitscreen_landscape" => Some(Self::SplitscreenLandscape),
            "splitscreen_landscape_add" => Some(Self::SplitscreenLandscapeAdd),
            "splitscreen_left" => Some(Self::SplitscreenLeft),
            "splitscreen_portrait" => Some(Self::SplitscreenPortrait),
            "splitscreen_right" => Some(Self::SplitscreenRight),
            "splitscreen_top" => Some(Self::SplitscreenTop),
            "splitscreen_vertical_add" => Some(Self::SplitscreenVerticalAdd),
            "spo2" => Some(Self::Spo2),
            "spoke" => Some(Self::Spoke),
            "sports" => Some(Self::Sports),
            "sports_and_outdoors" => Some(Self::SportsAndOutdoors),
            "sports_bar" => Some(Self::SportsBar),
            "sports_baseball" => Some(Self::SportsBaseball),
            "sports_basketball" => Some(Self::SportsBasketball),
            "sports_cricket" => Some(Self::SportsCricket),
            "sports_esports" => Some(Self::SportsEsports),
            "sports_football" => Some(Self::SportsFootball),
            "sports_golf" => Some(Self::SportsGolf),
            "sports_gymnastics" => Some(Self::SportsGymnastics),
            "sports_handball" => Some(Self::SportsHandball),
            "sports_hockey" => Some(Self::SportsHockey),
            "sports_kabaddi" => Some(Self::SportsKabaddi),
            "sports_martial_arts" => Some(Self::SportsMartialArts),
            "sports_mma" => Some(Self::SportsMma),
            "sports_motorsports" => Some(Self::SportsMotorsports),
            "sports_rugby" => Some(Self::SportsRugby),
            "sports_score" => Some(Self::SportsScore),
            "sports_soccer" => Some(Self::SportsSoccer),
            "sports_tennis" => Some(Self::SportsTennis),
            "sports_volleyball" => Some(Self::SportsVolleyball),
            "sprinkler" => Some(Self::Sprinkler),
            "sprint" => Some(Self::Sprint),
            "sql" => Some(Self::Sql),
            "square" => Some(Self::Square),
            "square_circle" => Some(Self::SquareCircle),
            "square_dot" => Some(Self::SquareDot),
            "square_foot" => Some(Self::SquareFoot),
            "ssid_chart" => Some(Self::SsidChart),
            "stack" => Some(Self::Stack),
            "stack_group" => Some(Self::StackGroup),
            "stack_hexagon" => Some(Self::StackHexagon),
            "stack_off" => Some(Self::StackOff),
            "stack_star" => Some(Self::StackStar),
            "stacked_bar_chart" => Some(Self::StackedBarChart),
            "stacked_email" => Some(Self::StackedEmail),
            "stacked_inbox" => Some(Self::StackedInbox),
            "stacked_line_chart" => Some(Self::StackedLineChart),
            "stacks" => Some(Self::Stacks),
            "stadia_controller" => Some(Self::StadiaController),
            "stadium" => Some(Self::Stadium),
            "stairs" => Some(Self::Stairs),
            "stairs_2" => Some(Self::Stairs2),
            "star_border" => Some(Self::StarBorder),
            "star_border_purple500" => Some(Self::StarBorderPurple500),
            "star_half" => Some(Self::StarHalf),
            "star_outline" => Some(Self::StarOutline),
            "star_purple500" => Some(Self::StarPurple500),
            "star_rate" => Some(Self::StarRate),
            "star_rate_half" => Some(Self::StarRateHalf),
            "star_shine" => Some(Self::StarShine),
            "stars" => Some(Self::Stars),
            "stars_2" => Some(Self::Stars2),
            "start" => Some(Self::Start),
            "stat_0" => Some(Self::Stat0),
            "stat_1" => Some(Self::Stat1),
            "stat_2" => Some(Self::Stat2),
            "stat_3" => Some(Self::Stat3),
            "stat_minus_1" => Some(Self::StatMinus1),
            "stat_minus_2" => Some(Self::StatMinus2),
            "stat_minus_3" => Some(Self::StatMinus3),
            "stay_current_landscape" => Some(Self::StayCurrentLandscape),
            "stay_current_portrait" => Some(Self::StayCurrentPortrait),
            "stay_primary_landscape" => Some(Self::StayPrimaryLandscape),
            "stay_primary_portrait" => Some(Self::StayPrimaryPortrait),
            "steering_wheel_cool" => Some(Self::SteeringWheelCool),
            "steering_wheel_heat" => Some(Self::SteeringWheelHeat),
            "step" => Some(Self::Step),
            "step_into" => Some(Self::StepInto),
            "step_out" => Some(Self::StepOut),
            "step_over" => Some(Self::StepOver),
            "steppers" => Some(Self::Steppers),
            "steps" => Some(Self::Steps),
            "stethoscope" => Some(Self::Stethoscope),
            "stethoscope_arrow" => Some(Self::StethoscopeArrow),
            "stethoscope_check" => Some(Self::StethoscopeCheck),
            "sticker" => Some(Self::Sticker),
            "sticker_add" => Some(Self::StickerAdd),
            "sticky_note" => Some(Self::StickyNote),
            "sticky_note_2" => Some(Self::StickyNote2),
            "stock_media" => Some(Self::StockMedia),
            "stockpot" => Some(Self::Stockpot),
            "stop_circle" => Some(Self::StopCircle),
            "stop_screen_share" => Some(Self::StopScreenShare),
            "storage" => Some(Self::Storage),
            "store" => Some(Self::Store),
            "store_mall_directory" => Some(Self::StoreMallDirectory),
            "storefront" => Some(Self::Storefront),
            "storm" => Some(Self::Storm),
            "straight" => Some(Self::Straight),
            "straighten" => Some(Self::Straighten),
            "strategy" => Some(Self::Strategy),
            "stream" => Some(Self::Stream),
            "stream_apps" => Some(Self::StreamApps),
            "streetview" => Some(Self::Streetview),
            "stress_management" => Some(Self::StressManagement),
            "strikethrough_s" => Some(Self::StrikethroughS),
            "stroke_full" => Some(Self::StrokeFull),
            "stroke_partial" => Some(Self::StrokePartial),
            "stroller" => Some(Self::Stroller),
            "style" => Some(Self::Style),
            "styler" => Some(Self::Styler),
            "stylus" => Some(Self::Stylus),
            "stylus_brush" => Some(Self::StylusBrush),
            "stylus_fountain_pen" => Some(Self::StylusFountainPen),
            "stylus_highlighter" => Some(Self::StylusHighlighter),
            "stylus_laser_pointer" => Some(Self::StylusLaserPointer),
            "stylus_note" => Some(Self::StylusNote),
            "stylus_pen" => Some(Self::StylusPen),
            "stylus_pencil" => Some(Self::StylusPencil),
            "subdirectory_arrow_left" => Some(Self::SubdirectoryArrowLeft),
            "subdirectory_arrow_right" => Some(Self::SubdirectoryArrowRight),
            "subheader" => Some(Self::Subheader),
            "subject" => Some(Self::Subject),
            "subscript" => Some(Self::Subscript),
            "subscriptions" => Some(Self::Subscriptions),
            "subtitles" => Some(Self::Subtitles),
            "subtitles_gear" => Some(Self::SubtitlesGear),
            "subtitles_off" => Some(Self::SubtitlesOff),
            "subway" => Some(Self::Subway),
            "subway_walk" => Some(Self::SubwayWalk),
            "subwoofer" => Some(Self::Subwoofer),
            "summarize" => Some(Self::Summarize),
            "sunny" => Some(Self::Sunny),
            "sunny_snowing" => Some(Self::SunnySnowing),
            "superscript" => Some(Self::Superscript),
            "supervised_user_circle" => Some(Self::SupervisedUserCircle),
            "supervised_user_circle_off" => Some(Self::SupervisedUserCircleOff),
            "supervisor_account" => Some(Self::SupervisorAccount),
            "support" => Some(Self::Support),
            "support_agent" => Some(Self::SupportAgent),
            "surfing" => Some(Self::Surfing),
            "surgical" => Some(Self::Surgical),
            "surround_sound" => Some(Self::SurroundSound),
            "swap_calls" => Some(Self::SwapCalls),
            "swap_driving_apps" => Some(Self::SwapDrivingApps),
            "swap_driving_apps_wheel" => Some(Self::SwapDrivingAppsWheel),
            "swap_horiz" => Some(Self::SwapHoriz),
            "swap_horizontal_circle" => Some(Self::SwapHorizontalCircle),
            "swap_vert" => Some(Self::SwapVert),
            "swap_vertical_circle" => Some(Self::SwapVerticalCircle),
            "sweep" => Some(Self::Sweep),
            "swipe" => Some(Self::Swipe),
            "swipe_down" => Some(Self::SwipeDown),
            "swipe_down_alt" => Some(Self::SwipeDownAlt),
            "swipe_left" => Some(Self::SwipeLeft),
            "swipe_left_2" => Some(Self::SwipeLeft2),
            "swipe_left_alt" => Some(Self::SwipeLeftAlt),
            "swipe_right" => Some(Self::SwipeRight),
            "swipe_right_2" => Some(Self::SwipeRight2),
            "swipe_right_alt" => Some(Self::SwipeRightAlt),
            "swipe_up" => Some(Self::SwipeUp),
            "swipe_up_alt" => Some(Self::SwipeUpAlt),
            "swipe_vertical" => Some(Self::SwipeVertical),
            "switch" => Some(Self::Switch),
            "switch_access" => Some(Self::SwitchAccess),
            "switch_access_2" => Some(Self::SwitchAccess2),
            "switch_access_3" => Some(Self::SwitchAccess3),
            "switch_access_shortcut" => Some(Self::SwitchAccessShortcut),
            "switch_access_shortcut_add" => Some(Self::SwitchAccessShortcutAdd),
            "switch_account" => Some(Self::SwitchAccount),
            "switch_camera" => Some(Self::SwitchCamera),
            "switch_left" => Some(Self::SwitchLeft),
            "switch_off" => Some(Self::SwitchOff),
            "switch_right" => Some(Self::SwitchRight),
            "switch_video" => Some(Self::SwitchVideo),
            "switches" => Some(Self::Switches),
            "sword_rose" => Some(Self::SwordRose),
            "swords" => Some(Self::Swords),
            "symptoms" => Some(Self::Symptoms),
            "synagogue" => Some(Self::Synagogue),
            "sync" => Some(Self::Sync),
            "sync_alt" => Some(Self::SyncAlt),
            "sync_arrow_down" => Some(Self::SyncArrowDown),
            "sync_arrow_up" => Some(Self::SyncArrowUp),
            "sync_desktop" => Some(Self::SyncDesktop),
            "sync_disabled" => Some(Self::SyncDisabled),
            "sync_lock" => Some(Self::SyncLock),
            "sync_problem" => Some(Self::SyncProblem),
            "sync_saved_locally" => Some(Self::SyncSavedLocally),
            "sync_saved_locally_off" => Some(Self::SyncSavedLocallyOff),
            "syringe" => Some(Self::Syringe),
            "system_security_update" => Some(Self::SystemSecurityUpdate),
            "system_security_update_good" => Some(Self::SystemSecurityUpdateGood),
            "system_security_update_warning" => Some(Self::SystemSecurityUpdateWarning),
            "system_update" => Some(Self::SystemUpdate),
            "system_update_alt" => Some(Self::SystemUpdateAlt),
            "tab" => Some(Self::Tab),
            "tab_close" => Some(Self::TabClose),
            "tab_close_inactive" => Some(Self::TabCloseInactive),
            "tab_close_right" => Some(Self::TabCloseRight),
            "tab_duplicate" => Some(Self::TabDuplicate),
            "tab_group" => Some(Self::TabGroup),
            "tab_inactive" => Some(Self::TabInactive),
            "tab_move" => Some(Self::TabMove),
            "tab_new_right" => Some(Self::TabNewRight),
            "tab_recent" => Some(Self::TabRecent),
            "tab_search" => Some(Self::TabSearch),
            "tab_unselected" => Some(Self::TabUnselected),
            "table_bar" => Some(Self::TableBar),
            "table_chart" => Some(Self::TableChart),
            "table_chart_view" => Some(Self::TableChartView),
            "table_convert" => Some(Self::TableConvert),
            "table_edit" => Some(Self::TableEdit),
            "table_eye" => Some(Self::TableEye),
            "table_lamp" => Some(Self::TableLamp),
            "table_large" => Some(Self::TableLarge),
            "table_restaurant" => Some(Self::TableRestaurant),
            "table_rows" => Some(Self::TableRows),
            "table_rows_narrow" => Some(Self::TableRowsNarrow),
            "table_sign" => Some(Self::TableSign),
            "table_view" => Some(Self::TableView),
            "tablet" => Some(Self::Tablet),
            "tablet_android" => Some(Self::TabletAndroid),
            "tablet_camera" => Some(Self::TabletCamera),
            "tablet_mac" => Some(Self::TabletMac),
            "tabs" => Some(Self::Tabs),
            "tactic" => Some(Self::Tactic),
            "tag" => Some(Self::Tag),
            "tag_faces" => Some(Self::TagFaces),
            "takeout_dining" => Some(Self::TakeoutDining),
            "takeout_dining_2" => Some(Self::TakeoutDining2),
            "tamper_detection_off" => Some(Self::TamperDetectionOff),
            "tamper_detection_on" => Some(Self::TamperDetectionOn),
            "tap_and_play" => Some(Self::TapAndPlay),
            "tapas" => Some(Self::Tapas),
            "target" => Some(Self::Target),
            "target_check" => Some(Self::TargetCheck),
            "task" => Some(Self::Task),
            "task_alt" => Some(Self::TaskAlt),
            "tatami_seat" => Some(Self::TatamiSeat),
            "taunt" => Some(Self::Taunt),
            "taxi_alert" => Some(Self::TaxiAlert),
            "team_dashboard" => Some(Self::TeamDashboard),
            "temp_preferences_custom" => Some(Self::TempPreferencesCustom),
            "temp_preferences_eco" => Some(Self::TempPreferencesEco),
            "temple_buddhist" => Some(Self::TempleBuddhist),
            "temple_hindu" => Some(Self::TempleHindu),
            "tenancy" => Some(Self::Tenancy),
            "terminal" => Some(Self::Terminal),
            "terminal_2" => Some(Self::Terminal2),
            "terminal_add" => Some(Self::TerminalAdd),
            "terrain" => Some(Self::Terrain),
            "text_ad" => Some(Self::TextAd),
            "text_ad_off" => Some(Self::TextAdOff),
            "text_compare" => Some(Self::TextCompare),
            "text_decrease" => Some(Self::TextDecrease),
            "text_fields" => Some(Self::TextFields),
            "text_fields_alt" => Some(Self::TextFieldsAlt),
            "text_format" => Some(Self::TextFormat),
            "text_increase" => Some(Self::TextIncrease),
            "text_rotate_up" => Some(Self::TextRotateUp),
            "text_rotate_vertical" => Some(Self::TextRotateVertical),
            "text_rotation_angledown" => Some(Self::TextRotationAngledown),
            "text_rotation_angleup" => Some(Self::TextRotationAngleup),
            "text_rotation_down" => Some(Self::TextRotationDown),
            "text_rotation_none" => Some(Self::TextRotationNone),
            "text_select_end" => Some(Self::TextSelectEnd),
            "text_select_jump_to_beginning" => Some(Self::TextSelectJumpToBeginning),
            "text_select_jump_to_end" => Some(Self::TextSelectJumpToEnd),
            "text_select_move_back_character" => Some(Self::TextSelectMoveBackCharacter),
            "text_select_move_back_word" => Some(Self::TextSelectMoveBackWord),
            "text_select_move_down" => Some(Self::TextSelectMoveDown),
            "text_select_move_forward_character" => Some(Self::TextSelectMoveForwardCharacter),
            "text_select_move_forward_word" => Some(Self::TextSelectMoveForwardWord),
            "text_select_move_up" => Some(Self::TextSelectMoveUp),
            "text_select_start" => Some(Self::TextSelectStart),
            "text_snippet" => Some(Self::TextSnippet),
            "text_to_speech" => Some(Self::TextToSpeech),
            "text_up" => Some(Self::TextUp),
            "textsms" => Some(Self::Textsms),
            "texture" => Some(Self::Texture),
            "texture_add" => Some(Self::TextureAdd),
            "texture_minus" => Some(Self::TextureMinus),
            "theater_comedy" => Some(Self::TheaterComedy),
            "theaters" => Some(Self::Theaters),
            "thermometer" => Some(Self::Thermometer),
            "thermometer_add" => Some(Self::ThermometerAdd),
            "thermometer_alert" => Some(Self::ThermometerAlert),
            "thermometer_gain" => Some(Self::ThermometerGain),
            "thermometer_loss" => Some(Self::ThermometerLoss),
            "thermometer_minus" => Some(Self::ThermometerMinus),
            "thermostat" => Some(Self::Thermostat),
            "thermostat_arrow_down" => Some(Self::ThermostatArrowDown),
            "thermostat_arrow_up" => Some(Self::ThermostatArrowUp),
            "thermostat_auto" => Some(Self::ThermostatAuto),
            "thermostat_carbon" => Some(Self::ThermostatCarbon),
            "things_to_do" => Some(Self::ThingsToDo),
            "thread_unread" => Some(Self::ThreadUnread),
            "threat_intelligence" => Some(Self::ThreatIntelligence),
            "thumb_down" => Some(Self::ThumbDown),
            "thumb_down_alt" => Some(Self::ThumbDownAlt),
            "thumb_down_filled" => Some(Self::ThumbDownFilled),
            "thumb_down_off" => Some(Self::ThumbDownOff),
            "thumb_down_off_alt" => Some(Self::ThumbDownOffAlt),
            "thumb_up" => Some(Self::ThumbUp),
            "thumb_up_alt" => Some(Self::ThumbUpAlt),
            "thumb_up_filled" => Some(Self::ThumbUpFilled),
            "thumb_up_off" => Some(Self::ThumbUpOff),
            "thumb_up_off_alt" => Some(Self::ThumbUpOffAlt),
            "thumbnail_bar" => Some(Self::ThumbnailBar),
            "thumbs_up_double" => Some(Self::ThumbsUpDouble),
            "thumbs_up_down" => Some(Self::ThumbsUpDown),
            "thunderstorm" => Some(Self::Thunderstorm),
            "tibia" => Some(Self::Tibia),
            "tibia_alt" => Some(Self::TibiaAlt),
            "tile_large" => Some(Self::TileLarge),
            "tile_medium" => Some(Self::TileMedium),
            "tile_small" => Some(Self::TileSmall),
            "tilt_arrow_down" => Some(Self::TiltArrowDown),
            "tilt_arrow_up" => Some(Self::TiltArrowUp),
            "time_auto" => Some(Self::TimeAuto),
            "time_to_leave" => Some(Self::TimeToLeave),
            "timelapse" => Some(Self::Timelapse),
            "timeline" => Some(Self::Timeline),
            "timer" => Some(Self::Timer),
            "timer_1" => Some(Self::Timer1),
            "timer_10" => Some(Self::Timer10),
            "timer_10_alt_1" => Some(Self::Timer10Alt1),
            "timer_10_select" => Some(Self::Timer10Select),
            "timer_2" => Some(Self::Timer2),
            "timer_3" => Some(Self::Timer3),
            "timer_3_alt_1" => Some(Self::Timer3Alt1),
            "timer_3_select" => Some(Self::Timer3Select),
            "timer_5" => Some(Self::Timer5),
            "timer_5_shutter" => Some(Self::Timer5Shutter),
            "timer_arrow_down" => Some(Self::TimerArrowDown),
            "timer_arrow_up" => Some(Self::TimerArrowUp),
            "timer_off" => Some(Self::TimerOff),
            "timer_pause" => Some(Self::TimerPause),
            "timer_play" => Some(Self::TimerPlay),
            "tips_and_updates" => Some(Self::TipsAndUpdates),
            "tire_repair" => Some(Self::TireRepair),
            "title" => Some(Self::Title),
            "titlecase" => Some(Self::Titlecase),
            "toast" => Some(Self::Toast),
            "toc" => Some(Self::Toc),
            "today" => Some(Self::Today),
            "toggle_off" => Some(Self::ToggleOff),
            "toggle_on" => Some(Self::ToggleOn),
            "token" => Some(Self::Token),
            "toll" => Some(Self::Toll),
            "tonality" => Some(Self::Tonality),
            "tonality_2" => Some(Self::Tonality2),
            "toolbar" => Some(Self::Toolbar),
            "tools_flat_head" => Some(Self::ToolsFlatHead),
            "tools_installation_kit" => Some(Self::ToolsInstallationKit),
            "tools_ladder" => Some(Self::ToolsLadder),
            "tools_level" => Some(Self::ToolsLevel),
            "tools_phillips" => Some(Self::ToolsPhillips),
            "tools_pliers_wire_stripper" => Some(Self::ToolsPliersWireStripper),
            "tools_power_drill" => Some(Self::ToolsPowerDrill),
            "tools_wrench" => Some(Self::ToolsWrench),
            "tooltip" => Some(Self::Tooltip),
            "tooltip_2" => Some(Self::Tooltip2),
            "top_panel_close" => Some(Self::TopPanelClose),
            "top_panel_open" => Some(Self::TopPanelOpen),
            "topic" => Some(Self::Topic),
            "tornado" => Some(Self::Tornado),
            "total_dissolved_solids" => Some(Self::TotalDissolvedSolids),
            "touch_app" => Some(Self::TouchApp),
            "touch_double" => Some(Self::TouchDouble),
            "touch_double_2" => Some(Self::TouchDouble2),
            "touch_long" => Some(Self::TouchLong),
            "touch_triple" => Some(Self::TouchTriple),
            "touchpad_mouse" => Some(Self::TouchpadMouse),
            "touchpad_mouse_off" => Some(Self::TouchpadMouseOff),
            "tour" => Some(Self::Tour),
            "toys" => Some(Self::Toys),
            "toys_and_games" => Some(Self::ToysAndGames),
            "toys_fan" => Some(Self::ToysFan),
            "track_changes" => Some(Self::TrackChanges),
            "trackpad_input" => Some(Self::TrackpadInput),
            "trackpad_input_2" => Some(Self::TrackpadInput2),
            "trackpad_input_3" => Some(Self::TrackpadInput3),
            "traffic" => Some(Self::Traffic),
            "traffic_jam" => Some(Self::TrafficJam),
            "trail_length" => Some(Self::TrailLength),
            "trail_length_medium" => Some(Self::TrailLengthMedium),
            "trail_length_short" => Some(Self::TrailLengthShort),
            "train" => Some(Self::Train),
            "tram" => Some(Self::Tram),
            "transcribe" => Some(Self::Transcribe),
            "transfer_within_a_station" => Some(Self::TransferWithinAStation),
            "transform" => Some(Self::Transform),
            "transgender" => Some(Self::Transgender),
            "transit_enterexit" => Some(Self::TransitEnterexit),
            "transit_ticket" => Some(Self::TransitTicket),
            "transition_chop" => Some(Self::TransitionChop),
            "transition_dissolve" => Some(Self::TransitionDissolve),
            "transition_fade" => Some(Self::TransitionFade),
            "transition_push" => Some(Self::TransitionPush),
            "transition_slide" => Some(Self::TransitionSlide),
            "translate" => Some(Self::Translate),
            "translate_indic" => Some(Self::TranslateIndic),
            "transportation" => Some(Self::Transportation),
            "travel" => Some(Self::Travel),
            "travel_explore" => Some(Self::TravelExplore),
            "travel_luggage_and_bags" => Some(Self::TravelLuggageAndBags),
            "trending_down" => Some(Self::TrendingDown),
            "trending_flat" => Some(Self::TrendingFlat),
            "trending_up" => Some(Self::TrendingUp),
            "triangle_circle" => Some(Self::TriangleCircle),
            "trip" => Some(Self::Trip),
            "trip_origin" => Some(Self::TripOrigin),
            "trolley" => Some(Self::Trolley),
            "trolley_cable_car" => Some(Self::TrolleyCableCar),
            "trophy" => Some(Self::Trophy),
            "troubleshoot" => Some(Self::Troubleshoot),
            "try" => Some(Self::Try),
            "tsunami" => Some(Self::Tsunami),
            "tsv" => Some(Self::Tsv),
            "tty" => Some(Self::Tty),
            "tune" => Some(Self::Tune),
            "tungsten" => Some(Self::Tungsten),
            "turn_left" => Some(Self::TurnLeft),
            "turn_right" => Some(Self::TurnRight),
            "turn_sharp_left" => Some(Self::TurnSharpLeft),
            "turn_sharp_right" => Some(Self::TurnSharpRight),
            "turn_slight_left" => Some(Self::TurnSlightLeft),
            "turn_slight_right" => Some(Self::TurnSlightRight),
            "turned_in" => Some(Self::TurnedIn),
            "turned_in_not" => Some(Self::TurnedInNot),
            "tv" => Some(Self::Tv),
            "tv_displays" => Some(Self::TvDisplays),
            "tv_gen" => Some(Self::TvGen),
            "tv_guide" => Some(Self::TvGuide),
            "tv_next" => Some(Self::TvNext),
            "tv_off" => Some(Self::TvOff),
            "tv_options_edit_channels" => Some(Self::TvOptionsEditChannels),
            "tv_options_input_settings" => Some(Self::TvOptionsInputSettings),
            "tv_remote" => Some(Self::TvRemote),
            "tv_signin" => Some(Self::TvSignin),
            "tv_with_assistant" => Some(Self::TvWithAssistant),
            "two_pager" => Some(Self::TwoPager),
            "two_pager_store" => Some(Self::TwoPagerStore),
            "two_wheeler" => Some(Self::TwoWheeler),
            "type_specimen" => Some(Self::TypeSpecimen),
            "u_turn_left" => Some(Self::UTurnLeft),
            "u_turn_right" => Some(Self::UTurnRight),
            "udon" => Some(Self::Udon),
            "ulna_radius" => Some(Self::UlnaRadius),
            "ulna_radius_alt" => Some(Self::UlnaRadiusAlt),
            "umbrella" => Some(Self::Umbrella),
            "unarchive" => Some(Self::Unarchive),
            "undereye" => Some(Self::Undereye),
            "unfold_less" => Some(Self::UnfoldLess),
            "unfold_less_double" => Some(Self::UnfoldLessDouble),
            "unfold_more" => Some(Self::UnfoldMore),
            "unfold_more_double" => Some(Self::UnfoldMoreDouble),
            "ungroup" => Some(Self::Ungroup),
            "universal_currency" => Some(Self::UniversalCurrency),
            "universal_currency_alt" => Some(Self::UniversalCurrencyAlt),
            "universal_local" => Some(Self::UniversalLocal),
            "unknown_2" => Some(Self::Unknown2),
            "unknown_5" => Some(Self::Unknown5),
            "unknown_7" => Some(Self::Unknown7),
            "unknown_document" => Some(Self::UnknownDocument),
            "unknown_med" => Some(Self::UnknownMed),
            "unlicense" => Some(Self::Unlicense),
            "unpaved_road" => Some(Self::UnpavedRoad),
            "unpin" => Some(Self::Unpin),
            "unpublished" => Some(Self::Unpublished),
            "unsubscribe" => Some(Self::Unsubscribe),
            "upcoming" => Some(Self::Upcoming),
            "update" => Some(Self::Update),
            "update_disabled" => Some(Self::UpdateDisabled),
            "upgrade" => Some(Self::Upgrade),
            "upi_pay" => Some(Self::UpiPay),
            "upload_2" => Some(Self::Upload2),
            "upload_file" => Some(Self::UploadFile),
            "uppercase" => Some(Self::Uppercase),
            "urology" => Some(Self::Urology),
            "usb" => Some(Self::Usb),
            "usb_off" => Some(Self::UsbOff),
            "user_attributes" => Some(Self::UserAttributes),
            "vaccines" => Some(Self::Vaccines),
            "vacuum" => Some(Self::Vacuum),
            "vacuum_2" => Some(Self::Vacuum2),
            "vacuum_2_on" => Some(Self::Vacuum2On),
            "valve" => Some(Self::Valve),
            "vape_free" => Some(Self::VapeFree),
            "vaping_rooms" => Some(Self::VapingRooms),
            "variable_add" => Some(Self::VariableAdd),
            "variable_insert" => Some(Self::VariableInsert),
            "variable_remove" => Some(Self::VariableRemove),
            "variables" => Some(Self::Variables),
            "ventilator" => Some(Self::Ventilator),
            "verified" => Some(Self::Verified),
            "verified_off" => Some(Self::VerifiedOff),
            "verified_user" => Some(Self::VerifiedUser),
            "vertical_align_bottom" => Some(Self::VerticalAlignBottom),
            "vertical_align_center" => Some(Self::VerticalAlignCenter),
            "vertical_align_top" => Some(Self::VerticalAlignTop),
            "vertical_distribute" => Some(Self::VerticalDistribute),
            "vertical_shades" => Some(Self::VerticalShades),
            "vertical_shades_closed" => Some(Self::VerticalShadesClosed),
            "vertical_split" => Some(Self::VerticalSplit),
            "vibration" => Some(Self::Vibration),
            "video_call" => Some(Self::VideoCall),
            "video_camera_back" => Some(Self::VideoCameraBack),
            "video_camera_back_add" => Some(Self::VideoCameraBackAdd),
            "video_camera_front" => Some(Self::VideoCameraFront),
            "video_camera_front_off" => Some(Self::VideoCameraFrontOff),
            "video_chat" => Some(Self::VideoChat),
            "video_file" => Some(Self::VideoFile),
            "video_frame_copy" => Some(Self::VideoFrameCopy),
            "video_frame_save" => Some(Self::VideoFrameSave),
            "video_label" => Some(Self::VideoLabel),
            "video_library" => Some(Self::VideoLibrary),
            "video_search" => Some(Self::VideoSearch),
            "video_settings" => Some(Self::VideoSettings),
            "video_stable" => Some(Self::VideoStable),
            "video_template" => Some(Self::VideoTemplate),
            "videocam" => Some(Self::Videocam),
            "videocam_alert" => Some(Self::VideocamAlert),
            "videocam_off" => Some(Self::VideocamOff),
            "videogame_asset" => Some(Self::VideogameAsset),
            "videogame_asset_off" => Some(Self::VideogameAssetOff),
            "view_agenda" => Some(Self::ViewAgenda),
            "view_apps" => Some(Self::ViewApps),
            "view_array" => Some(Self::ViewArray),
            "view_carousel" => Some(Self::ViewCarousel),
            "view_column" => Some(Self::ViewColumn),
            "view_column_2" => Some(Self::ViewColumn2),
            "view_comfy" => Some(Self::ViewComfy),
            "view_comfy_alt" => Some(Self::ViewComfyAlt),
            "view_compact" => Some(Self::ViewCompact),
            "view_compact_alt" => Some(Self::ViewCompactAlt),
            "view_cozy" => Some(Self::ViewCozy),
            "view_day" => Some(Self::ViewDay),
            "view_headline" => Some(Self::ViewHeadline),
            "view_in_ar" => Some(Self::ViewInAr),
            "view_in_ar_new" => Some(Self::ViewInArNew),
            "view_in_ar_off" => Some(Self::ViewInArOff),
            "view_kanban" => Some(Self::ViewKanban),
            "view_list" => Some(Self::ViewList),
            "view_module" => Some(Self::ViewModule),
            "view_object_track" => Some(Self::ViewObjectTrack),
            "view_quilt" => Some(Self::ViewQuilt),
            "view_real_size" => Some(Self::ViewRealSize),
            "view_sidebar" => Some(Self::ViewSidebar),
            "view_stream" => Some(Self::ViewStream),
            "view_timeline" => Some(Self::ViewTimeline),
            "view_week" => Some(Self::ViewWeek),
            "vignette" => Some(Self::Vignette),
            "vignette_2" => Some(Self::Vignette2),
            "villa" => Some(Self::Villa),
            "visibility" => Some(Self::Visibility),
            "visibility_lock" => Some(Self::VisibilityLock),
            "visibility_off" => Some(Self::VisibilityOff),
            "vital_signs" => Some(Self::VitalSigns),
            "vitals" => Some(Self::Vitals),
            "vo2_max" => Some(Self::Vo2Max),
            "voice_chat" => Some(Self::VoiceChat),
            "voice_chat_off" => Some(Self::VoiceChatOff),
            "voice_over_off" => Some(Self::VoiceOverOff),
            "voice_selection" => Some(Self::VoiceSelection),
            "voice_selection_off" => Some(Self::VoiceSelectionOff),
            "voicemail" => Some(Self::Voicemail),
            "voicemail_2" => Some(Self::Voicemail2),
            "volcano" => Some(Self::Volcano),
            "volume_down" => Some(Self::VolumeDown),
            "volume_down_alt" => Some(Self::VolumeDownAlt),
            "volume_mute" => Some(Self::VolumeMute),
            "volunteer_activism" => Some(Self::VolunteerActivism),
            "voting_chip" => Some(Self::VotingChip),
            "vpn_key" => Some(Self::VpnKey),
            "vpn_key_alert" => Some(Self::VpnKeyAlert),
            "vpn_key_off" => Some(Self::VpnKeyOff),
            "vpn_lock" => Some(Self::VpnLock),
            "vpn_lock_2" => Some(Self::VpnLock2),
            "vr180_create2d" => Some(Self::Vr180Create2d),
            "vr180_create2d_off" => Some(Self::Vr180Create2dOff),
            "vrpano" => Some(Self::Vrpano),
            "walk_bike" => Some(Self::WalkBike),
            "wall_art" => Some(Self::WallArt),
            "wall_lamp" => Some(Self::WallLamp),
            "wallet" => Some(Self::Wallet),
            "wallpaper" => Some(Self::Wallpaper),
            "wallpaper_slideshow" => Some(Self::WallpaperSlideshow),
            "wand_shine" => Some(Self::WandShine),
            "wand_stars" => Some(Self::WandStars),
            "ward" => Some(Self::Ward),
            "warehouse" => Some(Self::Warehouse),
            "warning_amber" => Some(Self::WarningAmber),
            "warning_off" => Some(Self::WarningOff),
            "wash" => Some(Self::Wash),
            "washoku" => Some(Self::Washoku),
            "watch" => Some(Self::Watch),
            "watch_alert" => Some(Self::WatchAlert),
            "watch_arrow" => Some(Self::WatchArrow),
            "watch_arrow_down" => Some(Self::WatchArrowDown),
            "watch_button" => Some(Self::WatchButton),
            "watch_button_press" => Some(Self::WatchButtonPress),
            "watch_check" => Some(Self::WatchCheck),
            "watch_later" => Some(Self::WatchLater),
            "watch_lock" => Some(Self::WatchLock),
            "watch_off" => Some(Self::WatchOff),
            "watch_screentime" => Some(Self::WatchScreentime),
            "watch_vibration" => Some(Self::WatchVibration),
            "watch_wake" => Some(Self::WatchWake),
            "water" => Some(Self::Water),
            "water_bottle" => Some(Self::WaterBottle),
            "water_bottle_large" => Some(Self::WaterBottleLarge),
            "water_damage" => Some(Self::WaterDamage),
            "water_do" => Some(Self::WaterDo),
            "water_drop" => Some(Self::WaterDrop),
            "water_drops" => Some(Self::WaterDrops),
            "water_ec" => Some(Self::WaterEc),
            "water_full" => Some(Self::WaterFull),
            "water_heater" => Some(Self::WaterHeater),
            "water_lock" => Some(Self::WaterLock),
            "water_loss" => Some(Self::WaterLoss),
            "water_lux" => Some(Self::WaterLux),
            "water_medium" => Some(Self::WaterMedium),
            "water_orp" => Some(Self::WaterOrp),
            "water_ph" => Some(Self::WaterPh),
            "water_pump" => Some(Self::WaterPump),
            "water_voc" => Some(Self::WaterVoc),
            "waterfall_chart" => Some(Self::WaterfallChart),
            "waves" => Some(Self::Waves),
            "waving_hand" => Some(Self::WavingHand),
            "wb_auto" => Some(Self::WbAuto),
            "wb_cloudy" => Some(Self::WbCloudy),
            "wb_incandescent" => Some(Self::WbIncandescent),
            "wb_iridescent" => Some(Self::WbIridescent),
            "wb_shade" => Some(Self::WbShade),
            "wb_sunny" => Some(Self::WbSunny),
            "wb_twilight" => Some(Self::WbTwilight),
            "wb_twilight_2" => Some(Self::WbTwilight2),
            "wc" => Some(Self::Wc),
            "weather_hail" => Some(Self::WeatherHail),
            "weather_mix" => Some(Self::WeatherMix),
            "weather_snowy" => Some(Self::WeatherSnowy),
            "web" => Some(Self::Web),
            "web_asset" => Some(Self::WebAsset),
            "web_asset_off" => Some(Self::WebAssetOff),
            "web_stories" => Some(Self::WebStories),
            "web_traffic" => Some(Self::WebTraffic),
            "webhook" => Some(Self::Webhook),
            "weekend" => Some(Self::Weekend),
            "weight" => Some(Self::Weight),
            "west" => Some(Self::West),
            "whatshot" => Some(Self::Whatshot),
            "wheat" => Some(Self::Wheat),
            "wheelchair_pickup" => Some(Self::WheelchairPickup),
            "where_to_vote" => Some(Self::WhereToVote),
            "widget_medium" => Some(Self::WidgetMedium),
            "widget_menu" => Some(Self::WidgetMenu),
            "widget_small" => Some(Self::WidgetSmall),
            "widget_width" => Some(Self::WidgetWidth),
            "widgets" => Some(Self::Widgets),
            "width" => Some(Self::Width),
            "width_full" => Some(Self::WidthFull),
            "width_normal" => Some(Self::WidthNormal),
            "width_wide" => Some(Self::WidthWide),
            "wifi" => Some(Self::Wifi),
            "wifi_1_bar" => Some(Self::Wifi1Bar),
            "wifi_2_bar" => Some(Self::Wifi2Bar),
            "wifi_add" => Some(Self::WifiAdd),
            "wifi_calling" => Some(Self::WifiCalling),
            "wifi_calling_1" => Some(Self::WifiCalling1),
            "wifi_calling_2" => Some(Self::WifiCalling2),
            "wifi_calling_3" => Some(Self::WifiCalling3),
            "wifi_calling_bar_1" => Some(Self::WifiCallingBar1),
            "wifi_calling_bar_2" => Some(Self::WifiCallingBar2),
            "wifi_calling_bar_3" => Some(Self::WifiCallingBar3),
            "wifi_channel" => Some(Self::WifiChannel),
            "wifi_device" => Some(Self::WifiDevice),
            "wifi_find" => Some(Self::WifiFind),
            "wifi_home" => Some(Self::WifiHome),
            "wifi_lock" => Some(Self::WifiLock),
            "wifi_notification" => Some(Self::WifiNotification),
            "wifi_off" => Some(Self::WifiOff),
            "wifi_password" => Some(Self::WifiPassword),
            "wifi_protected_setup" => Some(Self::WifiProtectedSetup),
            "wifi_proxy" => Some(Self::WifiProxy),
            "wifi_tethering" => Some(Self::WifiTethering),
            "wifi_tethering_error" => Some(Self::WifiTetheringError),
            "wifi_tethering_off" => Some(Self::WifiTetheringOff),
            "wind_power" => Some(Self::WindPower),
            "window" => Some(Self::Window),
            "window_closed" => Some(Self::WindowClosed),
            "window_open" => Some(Self::WindowOpen),
            "window_sensor" => Some(Self::WindowSensor),
            "windshield_defrost_auto" => Some(Self::WindshieldDefrostAuto),
            "windshield_defrost_front" => Some(Self::WindshieldDefrostFront),
            "windshield_defrost_rear" => Some(Self::WindshieldDefrostRear),
            "windshield_heat_front" => Some(Self::WindshieldHeatFront),
            "wine_bar" => Some(Self::WineBar),
            "woman" => Some(Self::Woman),
            "woman_2" => Some(Self::Woman2),
            "work" => Some(Self::Work),
            "work_alert" => Some(Self::WorkAlert),
            "work_history" => Some(Self::WorkHistory),
            "work_off" => Some(Self::WorkOff),
            "work_outline" => Some(Self::WorkOutline),
            "work_update" => Some(Self::WorkUpdate),
            "workflow" => Some(Self::Workflow),
            "workspace_premium" => Some(Self::WorkspacePremium),
            "workspaces" => Some(Self::Workspaces),
            "workspaces_outline" => Some(Self::WorkspacesOutline),
            "wounds_injuries" => Some(Self::WoundsInjuries),
            "wrap_text" => Some(Self::WrapText),
            "wrist" => Some(Self::Wrist),
            "wrong_location" => Some(Self::WrongLocation),
            "wysiwyg" => Some(Self::Wysiwyg),
            "x_circle" => Some(Self::XCircle),
            "y_circle" => Some(Self::YCircle),
            "yakitori" => Some(Self::Yakitori),
            "yard" => Some(Self::Yard),
            "yoshoku" => Some(Self::Yoshoku),
            "your_trips" => Some(Self::YourTrips),
            "youtube_activity" => Some(Self::YoutubeActivity),
            "youtube_searched_for" => Some(Self::YoutubeSearchedFor),
            "zone_person_alert" => Some(Self::ZonePersonAlert),
            "zone_person_idle" => Some(Self::ZonePersonIdle),
            "zone_person_urgent" => Some(Self::ZonePersonUrgent),
            "zoom_in" => Some(Self::ZoomIn),
            "zoom_in_map" => Some(Self::ZoomInMap),
            "zoom_out" => Some(Self::ZoomOut),
            "zoom_out_map" => Some(Self::ZoomOutMap),
            _ => None,
        }
    }

    /// The canonical token of every variant, in declaration order.
    ///
    /// The table's own view of [`IconName::ALL`], so a test can compare the two lists rather
    /// than compare each against a third copy.
    pub fn all_tokens() -> [&'static str; 4323] {
        let mut tokens = [""; 4323];
        let mut index = 0;
        let mut variant_index = 0;
        while variant_index < Self::ALL.len() {
            tokens[index] = Self::ALL[variant_index].as_str();
            index += 1;
            variant_index += 1;
        }
        tokens
    }

    /// Every variant, in declaration order.
    ///
    /// Indexed directly by the enum's discriminant — `data()` and `data_opt()` rely on that
    /// order matching the generated tables, which the integrity tests assert by name.
    pub const ALL: [IconName; 4323] = [
        Self::Check,
        Self::Cross,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::Star,
        Self::Heart,
        Self::Settings,
        Self::Home,
        Self::Search,
        Self::Menu,
        Self::Close,
        Self::Plus,
        Self::Minus,
        Self::Info,
        Self::Warning,
        Self::Error,
        Self::User,
        Self::Mail,
        Self::Bell,
        Self::Edit,
        Self::Trash,
        Self::Share,
        Self::Refresh,
        Self::More,
        Self::Filter,
        Self::Lock,
        Self::Unlock,
        Self::Download,
        Self::Upload,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::ChevronUp,
        Self::ChevronDown,
        Self::FirstPage,
        Self::LastPage,
        Self::Folder,
        Self::FolderOpen,
        Self::File,
        Self::Save,
        Self::Copy,
        Self::Print,
        Self::Undo,
        Self::Redo,
        Self::Cut,
        Self::Paste,
        Self::Attachment,
        Self::Link,
        Self::Success,
        Self::Help,
        Self::Block,
        Self::Schedule,
        Self::Hourglass,
        Self::Play,
        Self::Pause,
        Self::Stop,
        Self::SkipNext,
        Self::VolumeUp,
        Self::VolumeOff,
        Self::Sort,
        Self::BarChart,
        Self::Calendar,
        Self::Table,
        Self::Chat,
        Self::Call,
        Self::Send,
        Self::NotificationsOff,
        Self::Fullscreen,
        Self::DebugBug,
        Self::DesignComponent,
        Self::DesignEvent,
        Self::TestFlask,
        Self::ToolsTerminal,
        Self::ToolsTranslate,
        Self::Num10k,
        Self::Num10mp,
        Self::Num11mp,
        Self::Num123,
        Self::Num12mp,
        Self::Num13mp,
        Self::Num14mp,
        Self::Num15mp,
        Self::Num16mp,
        Self::Num17mp,
        Self::Num18UpRating,
        Self::Num18mp,
        Self::Num19mp,
        Self::Num1k,
        Self::Num1kPlus,
        Self::Num1xMobiledata,
        Self::Num1xMobiledataBadge,
        Self::Num20mp,
        Self::Num21mp,
        Self::Num22mp,
        Self::Num23mp,
        Self::Num24fpsSelect,
        Self::Num24mp,
        Self::Num2d,
        Self::Num2d2,
        Self::Num2k,
        Self::Num2kPlus,
        Self::Num2mp,
        Self::Num30fps,
        Self::Num30fpsSelect,
        Self::Num360,
        Self::Num3d,
        Self::Num3d2,
        Self::Num3dRotation,
        Self::Num3gMobiledata,
        Self::Num3gMobiledataBadge,
        Self::Num3k,
        Self::Num3kPlus,
        Self::Num3mp,
        Self::Num3p,
        Self::Num4gMobiledata,
        Self::Num4gMobiledataBadge,
        Self::Num4gPlusMobiledata,
        Self::Num4k,
        Self::Num4kPlus,
        Self::Num4mp,
        Self::Num50mp,
        Self::Num5g,
        Self::Num5gMobiledataBadge,
        Self::Num5k,
        Self::Num5kPlus,
        Self::Num5mp,
        Self::Num60fps,
        Self::Num60fpsSelect,
        Self::Num6FtApart,
        Self::Num6k,
        Self::Num6kPlus,
        Self::Num6mp,
        Self::Num7k,
        Self::Num7kPlus,
        Self::Num7mp,
        Self::Num8k,
        Self::Num8kPlus,
        Self::Num8mp,
        Self::Num9k,
        Self::Num9kPlus,
        Self::Num9mp,
        Self::Abc,
        Self::AcUnit,
        Self::AccessAlarm,
        Self::AccessAlarms,
        Self::AccessTime,
        Self::AccessTimeFilled,
        Self::Accessibility,
        Self::AccessibilityNew,
        Self::Accessible,
        Self::AccessibleForward,
        Self::AccessibleMenu,
        Self::AccountBalance,
        Self::AccountBalanceWallet,
        Self::AccountBox,
        Self::AccountChild,
        Self::AccountChildInvert,
        Self::AccountCircle,
        Self::AccountCircleFilled,
        Self::AccountCircleOff,
        Self::AccountTree,
        Self::ActionKey,
        Self::ActivityZone,
        Self::Acupuncture,
        Self::Acute,
        Self::Ad,
        Self::AdGroup,
        Self::AdGroupOff,
        Self::AdOff,
        Self::AdUnits,
        Self::AdaptiveAudioMic,
        Self::AdaptiveAudioMicOff,
        Self::Adb,
        Self::Add,
        Self::Add2,
        Self::AddAPhoto,
        Self::AddAd,
        Self::AddAlarm,
        Self::AddAlert,
        Self::AddBox,
        Self::AddBusiness,
        Self::AddCall,
        Self::AddCard,
        Self::AddChart,
        Self::AddCircle,
        Self::AddCircleOutline,
        Self::AddColumnLeft,
        Self::AddColumnRight,
        Self::AddComment,
        Self::AddDiamond,
        Self::AddHome,
        Self::AddHomeWork,
        Self::AddIcCall,
        Self::AddLink,
        Self::AddLocation,
        Self::AddLocationAlt,
        Self::AddModerator,
        Self::AddNotes,
        Self::AddPhotoAlternate,
        Self::AddReaction,
        Self::AddRoad,
        Self::AddRowAbove,
        Self::AddRowBelow,
        Self::AddShoppingCart,
        Self::AddTask,
        Self::AddToDrive,
        Self::AddToHomeScreen,
        Self::AddToPhotos,
        Self::AddToQueue,
        Self::AddTriangle,
        Self::Addchart,
        Self::AdfScanner,
        Self::Adjust,
        Self::AdminMeds,
        Self::AdminPanelSettings,
        Self::AdsClick,
        Self::Agender,
        Self::Agriculture,
        Self::Air,
        Self::AirFreshener,
        Self::AirPurifier,
        Self::AirPurifierGen,
        Self::AirlineSeatFlat,
        Self::AirlineSeatFlatAngled,
        Self::AirlineSeatIndividualSuite,
        Self::AirlineSeatLegroomExtra,
        Self::AirlineSeatLegroomNormal,
        Self::AirlineSeatLegroomReduced,
        Self::AirlineSeatReclineExtra,
        Self::AirlineSeatReclineNormal,
        Self::AirlineStops,
        Self::Airlines,
        Self::AirplaneTicket,
        Self::AirplanemodeActive,
        Self::AirplanemodeInactive,
        Self::Airplay,
        Self::AirportShuttle,
        Self::Airware,
        Self::Airwave,
        Self::Alarm,
        Self::AlarmAdd,
        Self::AlarmOff,
        Self::AlarmOn,
        Self::AlarmPause,
        Self::AlarmSmartWake,
        Self::Album,
        Self::AlignCenter,
        Self::AlignEnd,
        Self::AlignFlexCenter,
        Self::AlignFlexEnd,
        Self::AlignFlexStart,
        Self::AlignHorizontalCenter,
        Self::AlignHorizontalLeft,
        Self::AlignHorizontalRight,
        Self::AlignItemsStretch,
        Self::AlignJustifyCenter,
        Self::AlignJustifyFlexEnd,
        Self::AlignJustifyFlexStart,
        Self::AlignJustifySpaceAround,
        Self::AlignJustifySpaceBetween,
        Self::AlignJustifySpaceEven,
        Self::AlignJustifyStretch,
        Self::AlignSelfStretch,
        Self::AlignSpaceAround,
        Self::AlignSpaceBetween,
        Self::AlignSpaceEven,
        Self::AlignStart,
        Self::AlignStretch,
        Self::AlignVerticalBottom,
        Self::AlignVerticalCenter,
        Self::AlignVerticalTop,
        Self::AllInbox,
        Self::AllInclusive,
        Self::AllMatch,
        Self::AllOut,
        Self::Allergies,
        Self::Allergy,
        Self::AltRoute,
        Self::AlternateEmail,
        Self::Altitude,
        Self::AmbientScreen,
        Self::Ambulance,
        Self::Amend,
        Self::AmpStories,
        Self::Analytics,
        Self::Anchor,
        Self::Android,
        Self::AndroidCell4Bar,
        Self::AndroidCell4BarAlert,
        Self::AndroidCell4BarOff,
        Self::AndroidCell4BarPlus,
        Self::AndroidCell5Bar,
        Self::AndroidCell5BarAlert,
        Self::AndroidCell5BarOff,
        Self::AndroidCell5BarPlus,
        Self::AndroidCellDual4Bar,
        Self::AndroidCellDual4BarAlert,
        Self::AndroidCellDual4BarPlus,
        Self::AndroidCellDual5Bar,
        Self::AndroidCellDual5BarAlert,
        Self::AndroidCellDual5BarPlus,
        Self::AndroidWifi3Bar,
        Self::AndroidWifi3BarAlert,
        Self::AndroidWifi3BarLock,
        Self::AndroidWifi3BarOff,
        Self::AndroidWifi3BarPlus,
        Self::AndroidWifi3BarQuestion,
        Self::AndroidWifi4Bar,
        Self::AndroidWifi4BarAlert,
        Self::AndroidWifi4BarLock,
        Self::AndroidWifi4BarOff,
        Self::AndroidWifi4BarPlus,
        Self::AndroidWifi4BarQuestion,
        Self::AnimatedImages,
        Self::Animation,
        Self::Announcement,
        Self::Antigravity,
        Self::Aod,
        Self::AodTablet,
        Self::AodWatch,
        Self::Apartment,
        Self::Api,
        Self::ApkDocument,
        Self::ApkInstall,
        Self::AppBadging,
        Self::AppBlocking,
        Self::AppPromo,
        Self::AppRegistration,
        Self::AppSettingsAlt,
        Self::AppShortcut,
        Self::Apparel,
        Self::Approval,
        Self::ApprovalDelegation,
        Self::ApprovalDelegationOff,
        Self::Apps,
        Self::AppsOutage,
        Self::AppsPlus,
        Self::Aq,
        Self::AqIndoor,
        Self::ArOnYou,
        Self::ArStickers,
        Self::Architecture,
        Self::Archive,
        Self::AreaChart,
        Self::ArmingCountdown,
        Self::ArrowAndEdge,
        Self::ArrowBack,
        Self::ArrowBack2,
        Self::ArrowBackIos,
        Self::ArrowBackIosNew,
        Self::ArrowCircleDown,
        Self::ArrowCircleLeft,
        Self::ArrowCircleRight,
        Self::ArrowCircleUp,
        Self::ArrowCoolDown,
        Self::ArrowDownward,
        Self::ArrowDownwardAlt,
        Self::ArrowDropDown,
        Self::ArrowDropDownCircle,
        Self::ArrowDropUp,
        Self::ArrowForward,
        Self::ArrowForwardIos,
        Self::ArrowInsert,
        Self::ArrowLeftAlt,
        Self::ArrowMenuClose,
        Self::ArrowMenuOpen,
        Self::ArrowOrEdge,
        Self::ArrowOutward,
        Self::ArrowRange,
        Self::ArrowRightAlt,
        Self::ArrowSelectorTool,
        Self::ArrowShapeUp,
        Self::ArrowShapeUpStack,
        Self::ArrowShapeUpStack2,
        Self::ArrowSplit,
        Self::ArrowTopLeft,
        Self::ArrowTopRight,
        Self::ArrowUploadProgress,
        Self::ArrowUploadReady,
        Self::ArrowUpward,
        Self::ArrowUpwardAlt,
        Self::ArrowWarmUp,
        Self::ArrowsInput,
        Self::ArrowsLeftRightCircle,
        Self::ArrowsMoreDown,
        Self::ArrowsMoreUp,
        Self::ArrowsOutput,
        Self::ArrowsOutward,
        Self::ArrowsUpDownCircle,
        Self::ArtTrack,
        Self::Article,
        Self::ArticlePerson,
        Self::ArticleShortcut,
        Self::Artist,
        Self::AspectRatio,
        Self::Assessment,
        Self::Assignment,
        Self::AssignmentAdd,
        Self::AssignmentGlobe,
        Self::AssignmentInd,
        Self::AssignmentLate,
        Self::AssignmentReturn,
        Self::AssignmentReturned,
        Self::AssignmentTurnedIn,
        Self::AssistWalker,
        Self::Assistant,
        Self::AssistantDevice,
        Self::AssistantDirection,
        Self::AssistantNavigation,
        Self::AssistantOnHub,
        Self::AssistantPhoto,
        Self::AssuredWorkload,
        Self::Asterisk,
        Self::AstrophotographyAuto,
        Self::AstrophotographyOff,
        Self::Atm,
        Self::Atr,
        Self::AttachEmail,
        Self::AttachFile,
        Self::AttachFileAdd,
        Self::AttachFileOff,
        Self::AttachMoney,
        Self::Attractions,
        Self::Attribution,
        Self::AudioCapture,
        Self::AudioDescription,
        Self::AudioFile,
        Self::AudioVideoReceiver,
        Self::Audiotrack,
        Self::AutoActivityZone,
        Self::AutoAwesome,
        Self::AutoAwesomeMosaic,
        Self::AutoAwesomeMotion,
        Self::AutoDelete,
        Self::AutoDetectVoice,
        Self::AutoDrawSolid,
        Self::AutoFix,
        Self::AutoFixHigh,
        Self::AutoFixNormal,
        Self::AutoFixOff,
        Self::AutoGraph,
        Self::AutoLabel,
        Self::AutoMeetingRoom,
        Self::AutoMode,
        Self::AutoReadPause,
        Self::AutoReadPlay,
        Self::AutoSchedule,
        Self::AutoStories,
        Self::AutoStoriesOff,
        Self::AutoTimer,
        Self::AutoTowing,
        Self::AutoTransmission,
        Self::AutoVideocam,
        Self::AutofpsSelect,
        Self::Automation,
        Self::Autopause,
        Self::Autopay,
        Self::Autoplay,
        Self::Autorenew,
        Self::Autostop,
        Self::Av1,
        Self::AvTimer,
        Self::Avc,
        Self::AvgPace,
        Self::AvgTime,
        Self::AvocadoBean,
        Self::AwardMeal,
        Self::AwardStar,
        Self::Azm,
        Self::BCircle,
        Self::BabyChangingStation,
        Self::BackHand,
        Self::BackToTab,
        Self::BackgroundDotLarge,
        Self::BackgroundDotSmall,
        Self::BackgroundGridSmall,
        Self::BackgroundReplace,
        Self::BacklightHigh,
        Self::BacklightHighOff,
        Self::BacklightLow,
        Self::Backpack,
        Self::Backspace,
        Self::Backup,
        Self::BackupTable,
        Self::Badge,
        Self::BadgeCriticalBattery,
        Self::Badminton,
        Self::BakeryDining,
        Self::Balance,
        Self::Balcony,
        Self::Ballot,
        Self::BarChart4Bars,
        Self::BarChartOff,
        Self::Barcode,
        Self::BarcodeReader,
        Self::BarcodeScanner,
        Self::Barefoot,
        Self::BatchPrediction,
        Self::BathBedrock,
        Self::BathOutdoor,
        Self::BathPrivate,
        Self::BathPublicLarge,
        Self::BathSoak,
        Self::Bathroom,
        Self::Bathtub,
        Self::Battery0Bar,
        Self::Battery1Bar,
        Self::Battery20,
        Self::Battery2Bar,
        Self::Battery30,
        Self::Battery3Bar,
        Self::Battery4Bar,
        Self::Battery50,
        Self::Battery5Bar,
        Self::Battery60,
        Self::Battery6Bar,
        Self::Battery80,
        Self::Battery90,
        Self::BatteryAlert,
        Self::BatteryAndroid0,
        Self::BatteryAndroid1,
        Self::BatteryAndroid2,
        Self::BatteryAndroid3,
        Self::BatteryAndroid4,
        Self::BatteryAndroid5,
        Self::BatteryAndroid6,
        Self::BatteryAndroidAlert,
        Self::BatteryAndroidBolt,
        Self::BatteryAndroidFrame1,
        Self::BatteryAndroidFrame2,
        Self::BatteryAndroidFrame3,
        Self::BatteryAndroidFrame4,
        Self::BatteryAndroidFrame5,
        Self::BatteryAndroidFrame6,
        Self::BatteryAndroidFrameAlert,
        Self::BatteryAndroidFrameBolt,
        Self::BatteryAndroidFrameFull,
        Self::BatteryAndroidFramePlus,
        Self::BatteryAndroidFrameQuestion,
        Self::BatteryAndroidFrameShare,
        Self::BatteryAndroidFrameShield,
        Self::BatteryAndroidFull,
        Self::BatteryAndroidPlus,
        Self::BatteryAndroidQuestion,
        Self::BatteryAndroidShare,
        Self::BatteryAndroidShield,
        Self::BatteryChange,
        Self::BatteryCharging20,
        Self::BatteryCharging202,
        Self::BatteryCharging30,
        Self::BatteryCharging302,
        Self::BatteryCharging50,
        Self::BatteryCharging502,
        Self::BatteryCharging60,
        Self::BatteryCharging602,
        Self::BatteryCharging80,
        Self::BatteryCharging802,
        Self::BatteryCharging90,
        Self::BatteryChargingFull,
        Self::BatteryChargingFull2,
        Self::BatteryError,
        Self::BatteryFull,
        Self::BatteryFullAlt,
        Self::BatteryHoriz000,
        Self::BatteryHoriz050,
        Self::BatteryHoriz075,
        Self::BatteryLow,
        Self::BatteryPlus,
        Self::BatteryProfile,
        Self::BatterySaver,
        Self::BatteryShare,
        Self::BatteryStatusGood,
        Self::BatteryStd,
        Self::BatteryUnknown,
        Self::BatteryVert005,
        Self::BatteryVert020,
        Self::BatteryVert050,
        Self::BatteryVeryLow,
        Self::BeachAccess,
        Self::Bed,
        Self::BedroomBaby,
        Self::BedroomChild,
        Self::BedroomParent,
        Self::Bedtime,
        Self::BedtimeOff,
        Self::Beenhere,
        Self::BeerMeal,
        Self::Bento,
        Self::Bia,
        Self::BidLandscape,
        Self::BidLandscapeDisabled,
        Self::BigtopUpdates,
        Self::BikeDock,
        Self::BikeLane,
        Self::BikeScooter,
        Self::Biotech,
        Self::Blanket,
        Self::Blender,
        Self::Blind,
        Self::Blinds,
        Self::Blinds2,
        Self::Blinds2Closed,
        Self::BlindsClosed,
        Self::BloodPressure,
        Self::Bloodtype,
        Self::Bluetooth,
        Self::BluetoothAudio,
        Self::BluetoothConnected,
        Self::BluetoothDisabled,
        Self::BluetoothDrive,
        Self::BluetoothSearching,
        Self::BlurCircular,
        Self::BlurLinear,
        Self::BlurMedium,
        Self::BlurOff,
        Self::BlurOn,
        Self::BlurShort,
        Self::BoatBus,
        Self::BoatRailway,
        Self::BodyFat,
        Self::BodySystem,
        Self::Bolt,
        Self::BoltBoost,
        Self::Bomb,
        Self::Book,
        Self::Book2,
        Self::Book3,
        Self::Book4,
        Self::Book5,
        Self::Book6,
        Self::BookOnline,
        Self::BookRibbon,
        Self::Bookmark,
        Self::BookmarkAdd,
        Self::BookmarkAdded,
        Self::BookmarkBag,
        Self::BookmarkBorder,
        Self::BookmarkCheck,
        Self::BookmarkFlag,
        Self::BookmarkHeart,
        Self::BookmarkManager,
        Self::BookmarkRemove,
        Self::BookmarkStacks,
        Self::BookmarkStar,
        Self::Bookmarks,
        Self::BooksMoviesAndMusic,
        Self::BorderAll,
        Self::BorderBottom,
        Self::BorderClear,
        Self::BorderColor,
        Self::BorderHorizontal,
        Self::BorderInner,
        Self::BorderLeft,
        Self::BorderOuter,
        Self::BorderRight,
        Self::BorderStyle,
        Self::BorderTop,
        Self::BorderVertical,
        Self::Borg,
        Self::BottomAppBar,
        Self::BottomDrawer,
        Self::BottomNavigation,
        Self::BottomPanelClose,
        Self::BottomPanelOpen,
        Self::BottomRightClick,
        Self::BottomSheets,
        Self::Box,
        Self::BoxAdd,
        Self::BoxEdit,
        Self::Boy,
        Self::BrandAwareness,
        Self::BrandFamily,
        Self::BrandingWatermark,
        Self::BreakfastDining,
        Self::BreakingNews,
        Self::BreakingNewsAlt1,
        Self::Breastfeeding,
        Self::Brick,
        Self::BriefcaseMeal,
        Self::Brightness1,
        Self::Brightness2,
        Self::Brightness3,
        Self::Brightness4,
        Self::Brightness5,
        Self::Brightness6,
        Self::Brightness7,
        Self::BrightnessAlert,
        Self::BrightnessAuto,
        Self::BrightnessEmpty,
        Self::BrightnessHigh,
        Self::BrightnessLow,
        Self::BrightnessMedium,
        Self::BringYourOwnIp,
        Self::BroadcastOnHome,
        Self::BroadcastOnPersonal,
        Self::BrokenImage,
        Self::Browse,
        Self::BrowseActivity,
        Self::BrowseGallery,
        Self::BrowserNotSupported,
        Self::BrowserUpdated,
        Self::BrunchDining,
        Self::Brush,
        Self::Bubble,
        Self::BubbleChart,
        Self::Bubbles,
        Self::BucketCheck,
        Self::BugReport,
        Self::Build,
        Self::BuildCircle,
        Self::BulletChart,
        Self::Bungalow,
        Self::BurstMode,
        Self::BusAlert,
        Self::BusMapPin,
        Self::BusRailway,
        Self::Business,
        Self::BusinessCenter,
        Self::BusinessChip,
        Self::BusinessMessages,
        Self::ButtonsAlt,
        Self::Cabin,
        Self::Cable,
        Self::CableCar,
        Self::Cached,
        Self::Cadence,
        Self::Cake,
        Self::CakeAdd,
        Self::Calculate,
        Self::CalendarAddOn,
        Self::CalendarAppsScript,
        Self::CalendarCheck,
        Self::CalendarClock,
        Self::CalendarLock,
        Self::CalendarMeal,
        Self::CalendarMeal2,
        Self::CalendarMonth,
        Self::CalendarToday,
        Self::CalendarViewDay,
        Self::CalendarViewMonth,
        Self::CalendarViewWeek,
        Self::CallEnd,
        Self::CallEndAlt,
        Self::CallLog,
        Self::CallMade,
        Self::CallMerge,
        Self::CallMissed,
        Self::CallMissedOutgoing,
        Self::CallQuality,
        Self::CallReceived,
        Self::CallSplit,
        Self::CallToAction,
        Self::Camera,
        Self::CameraAlt,
        Self::CameraEnhance,
        Self::CameraFront,
        Self::CameraIndoor,
        Self::CameraOutdoor,
        Self::CameraRear,
        Self::CameraRoll,
        Self::CameraVideo,
        Self::Cameraswitch,
        Self::Campaign,
        Self::Camping,
        Self::Cancel,
        Self::CancelPresentation,
        Self::CancelScheduleSend,
        Self::Candle,
        Self::CandlestickChart,
        Self::Cannabis,
        Self::CaptivePortal,
        Self::Capture,
        Self::CarCrash,
        Self::CarDefrostLeft,
        Self::CarDefrostLowLeft,
        Self::CarDefrostLowRight,
        Self::CarDefrostMidLeft,
        Self::CarDefrostMidLowLeft,
        Self::CarDefrostMidLowRight,
        Self::CarDefrostMidRight,
        Self::CarDefrostRight,
        Self::CarFanLowLeft,
        Self::CarFanLowMidLeft,
        Self::CarFanLowRight,
        Self::CarFanMidLeft,
        Self::CarFanMidLowRight,
        Self::CarFanMidRight,
        Self::CarFanRecirculate,
        Self::CarFanRecirculate2,
        Self::CarGear,
        Self::CarLock,
        Self::CarMirrorHeat,
        Self::CarRental,
        Self::CarRepair,
        Self::CarSeatOff,
        Self::CarTag,
        Self::CardGiftcard,
        Self::CardMembership,
        Self::CardTravel,
        Self::CardioLoad,
        Self::Cardiology,
        Self::Cards,
        Self::CardsStack,
        Self::CardsStar,
        Self::Carpenter,
        Self::CarryOnBag,
        Self::CarryOnBagChecked,
        Self::CarryOnBagInactive,
        Self::CarryOnBagQuestion,
        Self::Cases,
        Self::Casino,
        Self::Cast,
        Self::CastConnected,
        Self::CastForEducation,
        Self::CastPause,
        Self::CastWarning,
        Self::Castle,
        Self::Category,
        Self::CategorySearch,
        Self::Celebration,
        Self::CellMerge,
        Self::CellTower,
        Self::CellWifi,
        Self::CenterFocusStrong,
        Self::CenterFocusWeak,
        Self::Chair,
        Self::ChairAlt,
        Self::ChairCounter,
        Self::ChairFireplace,
        Self::ChairUmbrella,
        Self::Chalet,
        Self::ChangeCircle,
        Self::ChangeHistory,
        Self::Charger,
        Self::ChargingStation,
        Self::ChartData,
        Self::ChatAddOn,
        Self::ChatAppsScript,
        Self::ChatBubble,
        Self::ChatBubbleOff,
        Self::ChatBubbleOutline,
        Self::ChatDashed,
        Self::ChatDisplay,
        Self::ChatError,
        Self::ChatInfo,
        Self::ChatPasteGo,
        Self::ChatPasteGo2,
        Self::CheckAlert,
        Self::CheckBox,
        Self::CheckBoxOutlineBlank,
        Self::CheckCircle,
        Self::CheckCircleFilled,
        Self::CheckCircleOutline,
        Self::CheckCircleUnread,
        Self::CheckInOut,
        Self::CheckIndeterminateSmall,
        Self::CheckSmall,
        Self::Checkbook,
        Self::CheckedBag,
        Self::CheckedBagQuestion,
        Self::Checklist,
        Self::ChecklistRtl,
        Self::Checkroom,
        Self::Cheer,
        Self::ChefHat,
        Self::Chess,
        Self::ChessBishop,
        Self::ChessBishop2,
        Self::ChessKing,
        Self::ChessKing2,
        Self::ChessKnight,
        Self::ChessPawn,
        Self::ChessPawn2,
        Self::ChessQueen,
        Self::ChessRook,
        Self::ChevronBackward,
        Self::ChevronForward,
        Self::ChevronLineUp,
        Self::ChildCare,
        Self::ChildFriendly,
        Self::ChildHat,
        Self::ChipExtraction,
        Self::Chips,
        Self::ChromeReaderMode,
        Self::Chromecast2,
        Self::ChromecastDevice,
        Self::Chronic,
        Self::Church,
        Self::CinematicBlur,
        Self::Circle,
        Self::CircleCircle,
        Self::CircleNotifications,
        Self::Circles,
        Self::CirclesExt,
        Self::Clarify,
        Self::Class,
        Self::CleanHands,
        Self::Cleaning,
        Self::CleaningBucket,
        Self::CleaningServices,
        Self::Clear,
        Self::ClearAll,
        Self::ClearDay,
        Self::ClearNight,
        Self::ClimateMiniSplit,
        Self::ClinicalNotes,
        Self::ClockArrowDown,
        Self::ClockArrowUp,
        Self::ClockLoader10,
        Self::ClockLoader20,
        Self::ClockLoader40,
        Self::ClockLoader60,
        Self::ClockLoader80,
        Self::ClockLoader90,
        Self::CloseFullscreen,
        Self::CloseSmall,
        Self::ClosedCaption,
        Self::ClosedCaptionAdd,
        Self::ClosedCaptionDisabled,
        Self::ClosedCaptionDisplay,
        Self::ClosedCaptionOff,
        Self::Cloud,
        Self::CloudAlert,
        Self::CloudCircle,
        Self::CloudDone,
        Self::CloudDownload,
        Self::CloudLock,
        Self::CloudOff,
        Self::CloudQueue,
        Self::CloudSync,
        Self::CloudUpload,
        Self::Cloudy,
        Self::CloudyFilled,
        Self::CloudySnowing,
        Self::Co2,
        Self::CoPresent,
        Self::Code,
        Self::CodeBlocks,
        Self::CodeOff,
        Self::CodeXml,
        Self::Coffee,
        Self::CoffeeMaker,
        Self::Cognition,
        Self::Cognition2,
        Self::CollapseAll,
        Self::CollapseContent,
        Self::Collections,
        Self::CollectionsBookmark,
        Self::ColorLens,
        Self::Colorize,
        Self::Colors,
        Self::CombineColumns,
        Self::ComedyMask,
        Self::ComicBubble,
        Self::Comment,
        Self::CommentBank,
        Self::CommentsDisabled,
        Self::Commit,
        Self::Communication,
        Self::Communities,
        Self::CommunitiesFilled,
        Self::Commute,
        Self::Compare,
        Self::CompareArrows,
        Self::CompassCalibration,
        Self::ComponentExchange,
        Self::Compost,
        Self::Compress,
        Self::Computer,
        Self::ComputerArrowUp,
        Self::ComputerCancel,
        Self::ComputerSound,
        Self::Concierge,
        Self::Conditions,
        Self::ConfirmationNumber,
        Self::Congenital,
        Self::ConnectWithoutContact,
        Self::ConnectedTv,
        Self::ConnectingAirports,
        Self::Construction,
        Self::ContactEmergency,
        Self::ContactMail,
        Self::ContactPage,
        Self::ContactPhone,
        Self::ContactPhoneFilled,
        Self::ContactSupport,
        Self::Contactless,
        Self::ContactlessOff,
        Self::Contacts,
        Self::ContactsProduct,
        Self::ContentCopy,
        Self::ContentCut,
        Self::ContentPaste,
        Self::ContentPasteGo,
        Self::ContentPasteOff,
        Self::ContentPasteSearch,
        Self::ContextualToken,
        Self::ContextualTokenAdd,
        Self::Contract,
        Self::ContractDelete,
        Self::ContractEdit,
        Self::Contrast,
        Self::ContrastCircle,
        Self::ContrastRtlOff,
        Self::ContrastSquare,
        Self::ControlCamera,
        Self::ControlPoint,
        Self::ControlPointDuplicate,
        Self::ControllerGen,
        Self::Conversation,
        Self::ConversionPath,
        Self::ConversionPathOff,
        Self::ConvertToText,
        Self::ConveyorBelt,
        Self::Cookie,
        Self::CookieOff,
        Self::Cooking,
        Self::CoolToDry,
        Self::CopyAll,
        Self::Copyright,
        Self::Coronavirus,
        Self::CorporateFare,
        Self::Cottage,
        Self::Counter0,
        Self::Counter1,
        Self::Counter2,
        Self::Counter3,
        Self::Counter4,
        Self::Counter5,
        Self::Counter6,
        Self::Counter7,
        Self::Counter8,
        Self::Counter9,
        Self::Countertops,
        Self::Create,
        Self::CreateNewFolder,
        Self::CreditCard,
        Self::CreditCardClock,
        Self::CreditCardGear,
        Self::CreditCardHeart,
        Self::CreditCardOff,
        Self::CreditScore,
        Self::Crib,
        Self::CrisisAlert,
        Self::Crop,
        Self::Crop169,
        Self::Crop219,
        Self::Crop23,
        Self::Crop32,
        Self::Crop54,
        Self::Crop75,
        Self::Crop916,
        Self::CropDin,
        Self::CropFree,
        Self::CropLandscape,
        Self::CropOriginal,
        Self::CropPortrait,
        Self::CropRotate,
        Self::CropSquare,
        Self::Crossword,
        Self::Crowdsource,
        Self::Crown,
        Self::CrueltyFree,
        Self::Css,
        Self::Csv,
        Self::CurrencyBitcoin,
        Self::CurrencyExchange,
        Self::CurrencyFranc,
        Self::CurrencyLira,
        Self::CurrencyPound,
        Self::CurrencyRuble,
        Self::CurrencyRupee,
        Self::CurrencyRupeeCircle,
        Self::CurrencyYen,
        Self::CurrencyYuan,
        Self::Curtains,
        Self::CurtainsClosed,
        Self::CustomTypography,
        Self::Cycle,
        Self::Cyclone,
        Self::Dangerous,
        Self::DarkMode,
        Self::Dashboard,
        Self::Dashboard2,
        Self::Dashboard2Add,
        Self::Dashboard2Edit,
        Self::Dashboard2Gear,
        Self::DashboardCustomize,
        Self::DataAlert,
        Self::DataArray,
        Self::DataCheck,
        Self::DataExploration,
        Self::DataInfoAlert,
        Self::DataLossPrevention,
        Self::DataObject,
        Self::DataSaverOff,
        Self::DataSaverOn,
        Self::DataTable,
        Self::DataThresholding,
        Self::DataUsage,
        Self::Database,
        Self::DatabaseOff,
        Self::DatabaseSearch,
        Self::DatabaseUpload,
        Self::Dataset,
        Self::DatasetLinked,
        Self::DateRange,
        Self::Deblur,
        Self::Deceased,
        Self::DecimalDecrease,
        Self::DecimalIncrease,
        Self::Deck,
        Self::Dehaze,
        Self::Delete,
        Self::DeleteForever,
        Self::DeleteHistory,
        Self::DeleteOutline,
        Self::DeleteSweep,
        Self::DeliveryDining,
        Self::DeliveryTruckBolt,
        Self::DeliveryTruckSpeed,
        Self::Demography,
        Self::DensityLarge,
        Self::DensityMedium,
        Self::DensitySmall,
        Self::Dentistry,
        Self::DepartureBoard,
        Self::DeployedCode,
        Self::DeployedCodeAccount,
        Self::DeployedCodeAlert,
        Self::DeployedCodeHistory,
        Self::DeployedCodeUpdate,
        Self::Dermatology,
        Self::Description,
        Self::Deselect,
        Self::DesignServices,
        Self::Desk,
        Self::Deskphone,
        Self::DesktopAccessDisabled,
        Self::DesktopCloud,
        Self::DesktopCloudStack,
        Self::DesktopLandscape,
        Self::DesktopLandscapeAdd,
        Self::DesktopMac,
        Self::DesktopPortrait,
        Self::DesktopWindows,
        Self::Destruction,
        Self::Details,
        Self::DetectionAndZone,
        Self::DetectionAndZoneOff,
        Self::Detector,
        Self::DetectorAlarm,
        Self::DetectorBattery,
        Self::DetectorCo,
        Self::DetectorOffline,
        Self::DetectorSmoke,
        Self::DetectorStatus,
        Self::DeveloperBoard,
        Self::DeveloperBoardOff,
        Self::DeveloperGuide,
        Self::DeveloperMode,
        Self::DeveloperModeTv,
        Self::DeviceBand,
        Self::DeviceHub,
        Self::DeviceReset,
        Self::DeviceSwooshStar,
        Self::DeviceThermostat,
        Self::DeviceUnknown,
        Self::Devices,
        Self::DevicesFold,
        Self::DevicesFold2,
        Self::DevicesOff,
        Self::DevicesOther,
        Self::DevicesWearables,
        Self::DewPoint,
        Self::Diagnosis,
        Self::DiagonalLine,
        Self::DialerSip,
        Self::Dialogs,
        Self::Dialpad,
        Self::Diamond,
        Self::DiamondShine,
        Self::Dictionary,
        Self::Difference,
        Self::DigitalOutOfHome,
        Self::DigitalWellbeing,
        Self::DineHeart,
        Self::DineIn,
        Self::DineLamp,
        Self::Dining,
        Self::DinnerDining,
        Self::Directions,
        Self::DirectionsAlt,
        Self::DirectionsAltOff,
        Self::DirectionsBike,
        Self::DirectionsBoat,
        Self::DirectionsBoatFilled,
        Self::DirectionsBus,
        Self::DirectionsBusFilled,
        Self::DirectionsCar,
        Self::DirectionsCarFilled,
        Self::DirectionsOff,
        Self::DirectionsRailway,
        Self::DirectionsRailway2,
        Self::DirectionsRailwayFilled,
        Self::DirectionsRun,
        Self::DirectionsSubway,
        Self::DirectionsSubwayFilled,
        Self::DirectionsTransit,
        Self::DirectionsTransitFilled,
        Self::DirectionsWalk,
        Self::DirectorySync,
        Self::DirtyLens,
        Self::DisabledByDefault,
        Self::DisabledVisible,
        Self::DiscFull,
        Self::DiscoverTune,
        Self::Dishwasher,
        Self::DishwasherGen,
        Self::DisplayAdd,
        Self::DisplayExternalInput,
        Self::DisplayGroup,
        Self::DisplaySettings,
        Self::Distance,
        Self::Diversity1,
        Self::Diversity2,
        Self::Diversity3,
        Self::Diversity4,
        Self::Dns,
        Self::DoDisturb,
        Self::DoDisturbAlt,
        Self::DoDisturbOff,
        Self::DoDisturbOn,
        Self::DoNotDisturb,
        Self::DoNotDisturbAlt,
        Self::DoNotDisturbOff,
        Self::DoNotDisturbOn,
        Self::DoNotDisturbOnTotalSilence,
        Self::DoNotStep,
        Self::DoNotTouch,
        Self::Dock,
        Self::DockToBottom,
        Self::DockToLeft,
        Self::DockToRight,
        Self::Docs,
        Self::DocsAddOn,
        Self::DocsAppsScript,
        Self::DocumentScanner,
        Self::DocumentSearch,
        Self::DocumentShare,
        Self::Domain,
        Self::DomainAdd,
        Self::DomainDisabled,
        Self::DomainDisabledCheck,
        Self::DomainVerification,
        Self::DomainVerificationOff,
        Self::DominoMask,
        Self::Done,
        Self::DoneAll,
        Self::DoneOutline,
        Self::DonutLarge,
        Self::DonutSmall,
        Self::DoorBack,
        Self::DoorFront,
        Self::DoorOpen,
        Self::DoorSensor,
        Self::DoorSliding,
        Self::Doorbell,
        Self::Doorbell3p,
        Self::DoorbellChime,
        Self::DoubleArrow,
        Self::DownhillSkiing,
        Self::Download2,
        Self::DownloadDone,
        Self::DownloadForOffline,
        Self::Downloading,
        Self::Draft,
        Self::DraftOrders,
        Self::Drafts,
        Self::DragClick,
        Self::DragHandle,
        Self::DragIndicator,
        Self::DragPan,
        Self::Draw,
        Self::DrawAbstract,
        Self::DrawCollage,
        Self::DrawingRecognition,
        Self::Dresser,
        Self::DriveEta,
        Self::DriveExport,
        Self::DriveFileMove,
        Self::DriveFileMoveOutline,
        Self::DriveFileMoveRtl,
        Self::DriveFileRename,
        Self::DriveFileRenameOutline,
        Self::DriveFolderUpload,
        Self::Drone,
        Self::Drone2,
        Self::Dropdown,
        Self::DropdownMenu,
        Self::DropperEye,
        Self::Dry,
        Self::DryCleaning,
        Self::DualScreen,
        Self::Duo,
        Self::Dvr,
        Self::DynamicFeed,
        Self::DynamicForm,
        Self::E911Avatar,
        Self::E911Emergency,
        Self::EMobiledata,
        Self::EMobiledataBadge,
        Self::EarSound,
        Self::EarbudCase,
        Self::EarbudLeft,
        Self::EarbudRight,
        Self::Earbuds,
        Self::Earbuds2,
        Self::EarbudsBattery,
        Self::EarlyOn,
        Self::Earthquake,
        Self::East,
        Self::Ecg,
        Self::EcgHeart,
        Self::Eco,
        Self::Eda,
        Self::EdgesensorHigh,
        Self::EdgesensorLow,
        Self::EditArrowDown,
        Self::EditArrowUp,
        Self::EditAttributes,
        Self::EditAudio,
        Self::EditCalendar,
        Self::EditDocument,
        Self::EditLine,
        Self::EditLocation,
        Self::EditLocationAlt,
        Self::EditNote,
        Self::EditNotifications,
        Self::EditOff,
        Self::EditRoad,
        Self::EditSquare,
        Self::EditorChoice,
        Self::Egg,
        Self::EggAlt,
        Self::Eject,
        Self::Elderly,
        Self::ElderlyWoman,
        Self::ElectricBike,
        Self::ElectricBolt,
        Self::ElectricCar,
        Self::ElectricMeter,
        Self::ElectricMoped,
        Self::ElectricRickshaw,
        Self::ElectricScooter,
        Self::ElectricalServices,
        Self::Elevation,
        Self::Elevator,
        Self::Email,
        Self::Emergency,
        Self::EmergencyHeat,
        Self::EmergencyHeat2,
        Self::EmergencyHome,
        Self::EmergencyRecording,
        Self::EmergencyShare,
        Self::EmergencyShareOff,
        Self::EmojiEmotions,
        Self::EmojiEvents,
        Self::EmojiFlags,
        Self::EmojiFoodBeverage,
        Self::EmojiLanguage,
        Self::EmojiNature,
        Self::EmojiObjects,
        Self::EmojiPeople,
        Self::EmojiSymbols,
        Self::EmojiTransportation,
        Self::Emoticon,
        Self::EmptyDashboard,
        Self::Enable,
        Self::Encrypted,
        Self::EncryptedAdd,
        Self::EncryptedAddCircle,
        Self::EncryptedMinusCircle,
        Self::EncryptedOff,
        Self::Endocrinology,
        Self::Energy,
        Self::EnergyProgramSaving,
        Self::EnergyProgramTimeUsed,
        Self::EnergySavingsLeaf,
        Self::Engineering,
        Self::EnhancedEncryption,
        Self::Ent,
        Self::Enterprise,
        Self::EnterpriseOff,
        Self::Equal,
        Self::Equalizer,
        Self::EraserSize1,
        Self::EraserSize2,
        Self::EraserSize3,
        Self::EraserSize4,
        Self::EraserSize5,
        Self::ErrorCircleRounded,
        Self::ErrorMed,
        Self::ErrorOutline,
        Self::Escalator,
        Self::EscalatorWarning,
        Self::Euro,
        Self::EuroSymbol,
        Self::EvCharger,
        Self::EvMobiledataBadge,
        Self::EvShadow,
        Self::EvShadowAdd,
        Self::EvShadowMinus,
        Self::EvStation,
        Self::Event,
        Self::EventAvailable,
        Self::EventBusy,
        Self::EventList,
        Self::EventNote,
        Self::EventRepeat,
        Self::EventSeat,
        Self::EventUpcoming,
        Self::Exclamation,
        Self::Exercise,
        Self::ExitToApp,
        Self::Expand,
        Self::ExpandAll,
        Self::ExpandCircleDown,
        Self::ExpandCircleRight,
        Self::ExpandCircleUp,
        Self::ExpandContent,
        Self::ExpandLess,
        Self::ExpandMore,
        Self::ExpansionPanels,
        Self::ExpensionPanels,
        Self::Experiment,
        Self::Explicit,
        Self::Explore,
        Self::ExploreNearby,
        Self::ExploreOff,
        Self::Explosion,
        Self::ExportNotes,
        Self::Exposure,
        Self::ExposureNeg1,
        Self::ExposureNeg2,
        Self::ExposurePlus1,
        Self::ExposurePlus2,
        Self::ExposureZero,
        Self::Extension,
        Self::ExtensionOff,
        Self::EyeTracking,
        Self::Eyebrow,
        Self::Eyeglasses,
        Self::Eyeglasses2,
        Self::Eyeglasses2Sound,
        Self::Eyeglasses3,
        Self::Face,
        Self::Face2,
        Self::Face3,
        Self::Face4,
        Self::Face5,
        Self::Face6,
        Self::FaceDown,
        Self::FaceLeft,
        Self::FaceNod,
        Self::FaceRetouchingNatural,
        Self::FaceRetouchingOff,
        Self::FaceRight,
        Self::FaceShake,
        Self::FaceUnlock,
        Self::FaceUp,
        Self::FactCheck,
        Self::Factory,
        Self::Falling,
        Self::FamiliarFaceAndZone,
        Self::FamilyGroup,
        Self::FamilyHistory,
        Self::FamilyHome,
        Self::FamilyLink,
        Self::FamilyRestroom,
        Self::FamilyStar,
        Self::FanFocus,
        Self::FanIndirect,
        Self::FarsightDigital,
        Self::FastForward,
        Self::FastRewind,
        Self::Fastfood,
        Self::Faucet,
        Self::Favorite,
        Self::FavoriteBorder,
        Self::Fax,
        Self::FeatureSearch,
        Self::FeaturedPlayList,
        Self::FeaturedSeasonalAndGifts,
        Self::FeaturedVideo,
        Self::Feed,
        Self::Feedback,
        Self::Female,
        Self::Femur,
        Self::FemurAlt,
        Self::Fence,
        Self::Fertile,
        Self::Festival,
        Self::FiberDvr,
        Self::FiberManualRecord,
        Self::FiberNew,
        Self::FiberPin,
        Self::FiberSmartRecord,
        Self::FileCopy,
        Self::FileCopyOff,
        Self::FileDownload,
        Self::FileDownloadDone,
        Self::FileDownloadOff,
        Self::FileExport,
        Self::FileJson,
        Self::FileMap,
        Self::FileMapStack,
        Self::FileOpen,
        Self::FilePng,
        Self::FilePresent,
        Self::FileSave,
        Self::FileSaveOff,
        Self::FileUpload,
        Self::FileUploadOff,
        Self::Files,
        Self::Filter1,
        Self::Filter2,
        Self::Filter3,
        Self::Filter4,
        Self::Filter5,
        Self::Filter6,
        Self::Filter7,
        Self::Filter8,
        Self::Filter9,
        Self::Filter9Plus,
        Self::FilterAlt,
        Self::FilterAltOff,
        Self::FilterArrowRight,
        Self::FilterBAndW,
        Self::FilterCancel,
        Self::FilterCenterFocus,
        Self::FilterDrama,
        Self::FilterFrames,
        Self::FilterHdr,
        Self::FilterList,
        Self::FilterListAlt,
        Self::FilterListOff,
        Self::FilterNone,
        Self::FilterPlus,
        Self::FilterRetrolux,
        Self::FilterTiltShift,
        Self::FilterVintage,
        Self::Finance,
        Self::FinanceChip,
        Self::FinanceMode,
        Self::FindInPage,
        Self::FindReplace,
        Self::Fingerprint,
        Self::FingerprintOff,
        Self::FireCheck,
        Self::FireExtinguisher,
        Self::FireHydrant,
        Self::FireTruck,
        Self::Fireplace,
        Self::FitPage,
        Self::FitPageHeight,
        Self::FitPageWidth,
        Self::FitScreen,
        Self::FitWidth,
        Self::FitnessCenter,
        Self::FitnessTracker,
        Self::FitnessTrackers,
        Self::Flag,
        Self::Flag2,
        Self::FlagCheck,
        Self::FlagCircle,
        Self::FlagFilled,
        Self::Flaky,
        Self::Flare,
        Self::FlashAuto,
        Self::FlashOff,
        Self::FlashOn,
        Self::FlashlightOff,
        Self::FlashlightOn,
        Self::Flatware,
        Self::FlexDirection,
        Self::FlexNoWrap,
        Self::FlexWrap,
        Self::Flight,
        Self::FlightClass,
        Self::FlightLand,
        Self::FlightTakeoff,
        Self::FlightsAndHotels,
        Self::Flightsmode,
        Self::Flip,
        Self::FlipCameraAndroid,
        Self::FlipCameraIos,
        Self::FlipToBack,
        Self::FlipToFront,
        Self::FloatLandscape2,
        Self::FloatPortrait2,
        Self::Flood,
        Self::Floor,
        Self::FloorLamp,
        Self::Flourescent,
        Self::Flowchart,
        Self::Flowsheet,
        Self::Fluid,
        Self::FluidBalance,
        Self::FluidMed,
        Self::Fluorescent,
        Self::Flutter,
        Self::FlutterDash,
        Self::Flyover,
        Self::FmdBad,
        Self::FmdGood,
        Self::Foggy,
        Self::FoldedHands,
        Self::FolderCheck,
        Self::FolderCheck2,
        Self::FolderCode,
        Self::FolderCopy,
        Self::FolderData,
        Self::FolderDelete,
        Self::FolderEye,
        Self::FolderInfo,
        Self::FolderLimited,
        Self::FolderManaged,
        Self::FolderMatch,
        Self::FolderOff,
        Self::FolderShared,
        Self::FolderSpecial,
        Self::FolderSupervised,
        Self::FolderZip,
        Self::FollowTheSigns,
        Self::FontDownload,
        Self::FontDownloadOff,
        Self::FoodBank,
        Self::FootBones,
        Self::Footprint,
        Self::ForYou,
        Self::Forest,
        Self::ForkChart,
        Self::ForkLeft,
        Self::ForkRight,
        Self::ForkSpoon,
        Self::Forklift,
        Self::FormatAlignCenter,
        Self::FormatAlignJustify,
        Self::FormatAlignLeft,
        Self::FormatAlignRight,
        Self::FormatBold,
        Self::FormatClear,
        Self::FormatColorFill,
        Self::FormatColorReset,
        Self::FormatColorText,
        Self::FormatH1,
        Self::FormatH2,
        Self::FormatH3,
        Self::FormatH4,
        Self::FormatH5,
        Self::FormatH6,
        Self::FormatImageBack,
        Self::FormatImageBreakLeft,
        Self::FormatImageBreakRight,
        Self::FormatImageFront,
        Self::FormatImageInlineLeft,
        Self::FormatImageInlineRight,
        Self::FormatImageLeft,
        Self::FormatImageRight,
        Self::FormatIndentDecrease,
        Self::FormatIndentIncrease,
        Self::FormatInkHighlighter,
        Self::FormatItalic,
        Self::FormatLetterSpacing,
        Self::FormatLetterSpacing2,
        Self::FormatLetterSpacingStandard,
        Self::FormatLetterSpacingWide,
        Self::FormatLetterSpacingWider,
        Self::FormatLineSpacing,
        Self::FormatListBulleted,
        Self::FormatListBulletedAdd,
        Self::FormatListNumbered,
        Self::FormatListNumberedRtl,
        Self::FormatOverline,
        Self::FormatPaint,
        Self::FormatPaintOff,
        Self::FormatParagraph,
        Self::FormatQuote,
        Self::FormatQuoteOff,
        Self::FormatShapes,
        Self::FormatSize,
        Self::FormatStrikethrough,
        Self::FormatTextClip,
        Self::FormatTextOverflow,
        Self::FormatTextWrap,
        Self::FormatTextdirectionLToR,
        Self::FormatTextdirectionRToL,
        Self::FormatTextdirectionVertical,
        Self::FormatUnderlined,
        Self::FormatUnderlinedSquiggle,
        Self::FormsAddOn,
        Self::FormsAppsScript,
        Self::Fort,
        Self::Forum,
        Self::Forward,
        Self::Forward10,
        Self::Forward30,
        Self::Forward5,
        Self::ForwardCircle,
        Self::ForwardMedia,
        Self::ForwardToInbox,
        Self::Foundation,
        Self::Fragrance,
        Self::FrameBug,
        Self::FrameExclamation,
        Self::FrameInspect,
        Self::FramePerson,
        Self::FramePersonMic,
        Self::FramePersonOff,
        Self::FrameReload,
        Self::FrameSource,
        Self::FreeBreakfast,
        Self::FreeCancellation,
        Self::FrontHand,
        Self::FrontLoader,
        Self::FullCoverage,
        Self::FullHd,
        Self::FullStackedBarChart,
        Self::FullscreenExit,
        Self::FullscreenPortrait,
        Self::Function,
        Self::FunctionSearch,
        Self::Functions,
        Self::Funicular,
        Self::GMobiledata,
        Self::GMobiledataBadge,
        Self::GTranslate,
        Self::GalleryThumbnail,
        Self::GameBumperLeft,
        Self::GameBumperRight,
        Self::GameButtonL,
        Self::GameButtonL1,
        Self::GameButtonL2,
        Self::GameButtonR,
        Self::GameButtonR1,
        Self::GameButtonR2,
        Self::GameButtonZl,
        Self::GameButtonZr,
        Self::GameStickL3,
        Self::GameStickLeft,
        Self::GameStickR3,
        Self::GameStickRight,
        Self::GameTriggerLeft,
        Self::GameTriggerRight,
        Self::Gamepad,
        Self::GamepadCircleDown,
        Self::GamepadCircleLeft,
        Self::GamepadCircleRight,
        Self::GamepadCircleUp,
        Self::GamepadDown,
        Self::GamepadLeft,
        Self::GamepadRight,
        Self::GamepadUp,
        Self::Games,
        Self::Garage,
        Self::GarageCheck,
        Self::GarageDoor,
        Self::GarageDoorOpen,
        Self::GarageHome,
        Self::GarageMoney,
        Self::GardenCart,
        Self::GasMeter,
        Self::Gastroenterology,
        Self::Gate,
        Self::Gavel,
        Self::GeneralDevice,
        Self::GeneratingTokens,
        Self::Genetics,
        Self::Genres,
        Self::Gesture,
        Self::GestureSelect,
        Self::GetApp,
        Self::Gif,
        Self::Gif2,
        Self::GifBox,
        Self::Girl,
        Self::Gite,
        Self::GlassCup,
        Self::Globe,
        Self::Globe2Cancel,
        Self::Globe2Question,
        Self::GlobeAsia,
        Self::GlobeBook,
        Self::GlobeClock,
        Self::GlobeLocationPin,
        Self::GlobeUk,
        Self::Glucose,
        Self::Glyphs,
        Self::GoToLine,
        Self::GolfCourse,
        Self::GondolaLift,
        Self::GoogleHomeDevices,
        Self::GooglePlusReshare,
        Self::GoogleTvRemote,
        Self::GoogleWifi,
        Self::GppBad,
        Self::GppGood,
        Self::GppMaybe,
        Self::GpsFixed,
        Self::GpsNotFixed,
        Self::GpsOff,
        Self::Grade,
        Self::Gradient,
        Self::Grading,
        Self::Grain,
        Self::Graph1,
        Self::Graph2,
        Self::Graph3,
        Self::Graph4,
        Self::Graph5,
        Self::Graph6,
        Self::Graph7,
        Self::Graph8,
        Self::GraphicEq,
        Self::GraphicEqOff,
        Self::Grass,
        Self::Grid3x3,
        Self::Grid3x3Off,
        Self::Grid4x4,
        Self::GridGoldenratio,
        Self::GridGuides,
        Self::GridLayoutSide,
        Self::GridOff,
        Self::GridOn,
        Self::GridView,
        Self::Grocery,
        Self::Group,
        Self::GroupAdd,
        Self::GroupEye,
        Self::GroupOff,
        Self::GroupRemove,
        Self::GroupSearch,
        Self::GroupWork,
        Self::GroupedBarChart,
        Self::Groups,
        Self::Groups2,
        Self::Groups3,
        Self::Guardian,
        Self::Gynecology,
        Self::HMobiledata,
        Self::HMobiledataBadge,
        Self::HPlusMobiledata,
        Self::HPlusMobiledataBadge,
        Self::Hail,
        Self::Hallway,
        Self::HanamiDango,
        Self::HandBones,
        Self::HandGesture,
        Self::HandGestureOff,
        Self::HandMeal,
        Self::HandPackage,
        Self::HandheldController,
        Self::Handshake,
        Self::HandwritingRecognition,
        Self::Handyman,
        Self::HangoutVideo,
        Self::HangoutVideoOff,
        Self::HardDisk,
        Self::HardDrive,
        Self::HardDrive2,
        Self::Hardware,
        Self::Hd,
        Self::HdrAuto,
        Self::HdrAutoSelect,
        Self::HdrEnhancedSelect,
        Self::HdrOff,
        Self::HdrOffSelect,
        Self::HdrOn,
        Self::HdrOnSelect,
        Self::HdrPlus,
        Self::HdrPlusOff,
        Self::HdrStrong,
        Self::HdrWeak,
        Self::HeadMountedDevice,
        Self::Headphones,
        Self::HeadphonesBattery,
        Self::Headset,
        Self::HeadsetMic,
        Self::HeadsetOff,
        Self::Healing,
        Self::HealthAndBeauty,
        Self::HealthAndSafety,
        Self::HealthCross,
        Self::HealthMetrics,
        Self::HeapSnapshotLarge,
        Self::HeapSnapshotMultiple,
        Self::HeapSnapshotThumbnail,
        Self::Hearing,
        Self::HearingAid,
        Self::HearingAidDisabled,
        Self::HearingAidDisabledLeft,
        Self::HearingAidLeft,
        Self::HearingDisabled,
        Self::HeartBroken,
        Self::HeartCheck,
        Self::HeartMinus,
        Self::HeartPlus,
        Self::HeartSmile,
        Self::Heat,
        Self::HeatPump,
        Self::HeatPumpBalance,
        Self::Height,
        Self::Helicopter,
        Self::HelpCenter,
        Self::HelpClinic,
        Self::HelpOutline,
        Self::Hematology,
        Self::Hevc,
        Self::Hexagon,
        Self::Hide,
        Self::HideImage,
        Self::HideSource,
        Self::HighChair,
        Self::HighDensity,
        Self::HighQuality,
        Self::HighQualityOff,
        Self::HighRes,
        Self::Highlight,
        Self::HighlightAlt,
        Self::HighlightKeyboardFocus,
        Self::HighlightMouseCursor,
        Self::HighlightOff,
        Self::HighlightTextCursor,
        Self::HighlighterSize1,
        Self::HighlighterSize2,
        Self::HighlighterSize3,
        Self::HighlighterSize4,
        Self::HighlighterSize5,
        Self::Hiking,
        Self::History,
        Self::History2,
        Self::HistoryEdu,
        Self::HistoryOff,
        Self::HistoryToggleOff,
        Self::Hive,
        Self::Hls,
        Self::HlsOff,
        Self::HolidayVillage,
        Self::HomeAndGarden,
        Self::HomeAppLogo,
        Self::HomeFilled,
        Self::HomeHealth,
        Self::HomeImprovementAndTools,
        Self::HomeIotDevice,
        Self::HomeMax,
        Self::HomeMaxDots,
        Self::HomeMini,
        Self::HomePin,
        Self::HomeRepairService,
        Self::HomeSpeaker,
        Self::HomeStorage,
        Self::HomeStorageGear,
        Self::HomeWork,
        Self::HorizontalAlignCenter,
        Self::HorizontalAlignLeft,
        Self::HorizontalAlignRight,
        Self::HorizontalDistribute,
        Self::HorizontalRule,
        Self::HorizontalSplit,
        Self::Host,
        Self::HotTub,
        Self::Hotel,
        Self::HotelClass,
        Self::HourglassArrowDown,
        Self::HourglassArrowUp,
        Self::HourglassBottom,
        Self::HourglassCheck,
        Self::HourglassDisabled,
        Self::HourglassEmpty,
        Self::HourglassFull,
        Self::HourglassPause,
        Self::HourglassTop,
        Self::House,
        Self::HouseSiding,
        Self::HouseWithShield,
        Self::Houseboat,
        Self::HouseholdSupplies,
        Self::Hov,
        Self::HowToReg,
        Self::HowToVote,
        Self::HrResting,
        Self::Html,
        Self::Http,
        Self::Https,
        Self::Hub,
        Self::Humerus,
        Self::HumerusAlt,
        Self::HumidityHigh,
        Self::HumidityIndoor,
        Self::HumidityLow,
        Self::HumidityMid,
        Self::HumidityPercentage,
        Self::Hvac,
        Self::HvacMaxDefrost,
        Self::IceSkating,
        Self::Icecream,
        Self::IdCard,
        Self::IdCard2,
        Self::IdentityAwareProxy,
        Self::IdentityPlatform,
        Self::Ifl,
        Self::Iframe,
        Self::IframeOff,
        Self::Image,
        Self::ImageArrowUp,
        Self::ImageAspectRatio,
        Self::ImageInset,
        Self::ImageNotSupported,
        Self::ImageSearch,
        Self::ImagesearchRoller,
        Self::Imagesmode,
        Self::Immunology,
        Self::ImportContacts,
        Self::ImportExport,
        Self::ImportSpark,
        Self::ImportantDevices,
        Self::InHomeMode,
        Self::InactiveOrder,
        Self::Inbox,
        Self::InboxCustomize,
        Self::InboxText,
        Self::InboxTextAsterisk,
        Self::InboxTextPerson,
        Self::InboxTextShare,
        Self::IncompleteCircle,
        Self::IndeterminateCheckBox,
        Self::IndeterminateQuestionBox,
        Self::InfoI,
        Self::Infrared,
        Self::InkEraser,
        Self::InkEraserOff,
        Self::InkHighlighter,
        Self::InkHighlighterMove,
        Self::InkHighlighterOff,
        Self::InkMarker,
        Self::InkPen,
        Self::InkSelection,
        Self::Inpatient,
        Self::Input,
        Self::InputCircle,
        Self::InsertChart,
        Self::InsertChartFilled,
        Self::InsertChartOutlined,
        Self::InsertComment,
        Self::InsertDriveFile,
        Self::InsertEmoticon,
        Self::InsertInvitation,
        Self::InsertLink,
        Self::InsertPageBreak,
        Self::InsertPhoto,
        Self::InsertText,
        Self::Insights,
        Self::InstallDesktop,
        Self::InstallMobile,
        Self::InstantMix,
        Self::IntegrationInstructions,
        Self::InteractiveSpace,
        Self::Interests,
        Self::InterpreterMode,
        Self::Inventory,
        Self::Inventory2,
        Self::InvertColors,
        Self::InvertColorsOff,
        Self::Ios,
        Self::IosShare,
        Self::Iron,
        Self::Iso,
        Self::JamboardKiosk,
        Self::JapaneseCurry,
        Self::JapaneseFlag,
        Self::Javascript,
        Self::Jewelry,
        Self::Join,
        Self::JoinFull,
        Self::JoinInner,
        Self::JoinLeft,
        Self::JoinRight,
        Self::Joystick,
        Self::JumpToElement,
        Self::KanjiAlcohol,
        Self::Kayaking,
        Self::KebabDining,
        Self::Keep,
        Self::KeepOff,
        Self::KeepPin,
        Self::KeepPublic,
        Self::Kettle,
        Self::Key,
        Self::KeyOff,
        Self::KeyVertical,
        Self::KeyVisualizer,
        Self::Keyboard,
        Self::KeyboardAlt,
        Self::KeyboardArrowDown,
        Self::KeyboardArrowLeft,
        Self::KeyboardArrowRight,
        Self::KeyboardArrowUp,
        Self::KeyboardBackspace,
        Self::KeyboardCapslock,
        Self::KeyboardCapslockBadge,
        Self::KeyboardCommandKey,
        Self::KeyboardControlKey,
        Self::KeyboardDoubleArrowDown,
        Self::KeyboardDoubleArrowLeft,
        Self::KeyboardDoubleArrowRight,
        Self::KeyboardDoubleArrowUp,
        Self::KeyboardExternalInput,
        Self::KeyboardFull,
        Self::KeyboardHide,
        Self::KeyboardKeys,
        Self::KeyboardLock,
        Self::KeyboardLockOff,
        Self::KeyboardOff,
        Self::KeyboardOnscreen,
        Self::KeyboardOptionKey,
        Self::KeyboardPreviousLanguage,
        Self::KeyboardReturn,
        Self::KeyboardTab,
        Self::KeyboardTabRtl,
        Self::KeyboardVoice,
        Self::KidStar,
        Self::KingBed,
        Self::Kitchen,
        Self::Kitesurfing,
        Self::LabPanel,
        Self::LabProfile,
        Self::LabResearch,
        Self::Label,
        Self::LabelImportant,
        Self::LabelImportantOutline,
        Self::LabelOff,
        Self::LabelOutline,
        Self::Labs,
        Self::Lan,
        Self::Landscape,
        Self::Landscape2,
        Self::Landscape2Edit,
        Self::Landscape2Off,
        Self::Landslide,
        Self::Language,
        Self::LanguageChineseArray,
        Self::LanguageChineseCangjie,
        Self::LanguageChineseDayi,
        Self::LanguageChinesePinyin,
        Self::LanguageChineseQuick,
        Self::LanguageChineseWubi,
        Self::LanguageFrench,
        Self::LanguageGbEnglish,
        Self::LanguageInternational,
        Self::LanguageJapaneseKana,
        Self::LanguageKoreanLatin,
        Self::LanguagePinyin,
        Self::LanguageSpanish,
        Self::LanguageUs,
        Self::LanguageUsColemak,
        Self::LanguageUsDvorak,
        Self::Laps,
        Self::Laptop,
        Self::LaptopCar,
        Self::LaptopChromebook,
        Self::LaptopMac,
        Self::LaptopWindows,
        Self::LassoSelect,
        Self::Launch,
        Self::Laundry,
        Self::Layers,
        Self::LayersClear,
        Self::Lda,
        Self::Leaderboard,
        Self::LeakAdd,
        Self::LeakRemove,
        Self::LeftClick,
        Self::LeftPanelClose,
        Self::LeftPanelOpen,
        Self::LegendToggle,
        Self::Lens,
        Self::LensBlur,
        Self::LetterSwitch,
        Self::LibraryAdd,
        Self::LibraryAddCheck,
        Self::LibraryBooks,
        Self::LibraryMusic,
        Self::License,
        Self::LiftToTalk,
        Self::Light,
        Self::LightGroup,
        Self::LightGroup2,
        Self::LightMode,
        Self::LightModeAuto,
        Self::LightOff,
        Self::Lightbulb,
        Self::Lightbulb2,
        Self::LightbulbCircle,
        Self::LightbulbOutline,
        Self::LightningStand,
        Self::Lightstrip,
        Self::LineAxis,
        Self::LineCurve,
        Self::LineEnd,
        Self::LineEndArrow,
        Self::LineEndArrowNotch,
        Self::LineEndCircle,
        Self::LineEndDiamond,
        Self::LineEndSquare,
        Self::LineStart,
        Self::LineStartArrow,
        Self::LineStartArrowNotch,
        Self::LineStartCircle,
        Self::LineStartDiamond,
        Self::LineStartSquare,
        Self::LineStyle,
        Self::LineWeight,
        Self::LinearScale,
        Self::Link2,
        Self::LinkOff,
        Self::LinkedCamera,
        Self::LinkedServices,
        Self::Lips,
        Self::Liquor,
        Self::List,
        Self::List2,
        Self::ListAlt,
        Self::ListAltAdd,
        Self::ListAltCheck,
        Self::ListArrow,
        Self::Lists,
        Self::LiveHelp,
        Self::LiveTv,
        Self::Living,
        Self::LocalActivity,
        Self::LocalAirport,
        Self::LocalAtm,
        Self::LocalBar,
        Self::LocalCafe,
        Self::LocalCarWash,
        Self::LocalConvenienceStore,
        Self::LocalDining,
        Self::LocalDrink,
        Self::LocalFireDepartment,
        Self::LocalFlorist,
        Self::LocalGasStation,
        Self::LocalGroceryStore,
        Self::LocalHospital,
        Self::LocalHotel,
        Self::LocalLaundryService,
        Self::LocalLibrary,
        Self::LocalMall,
        Self::LocalMovies,
        Self::LocalOffer,
        Self::LocalParking,
        Self::LocalPharmacy,
        Self::LocalPhone,
        Self::LocalPizza,
        Self::LocalPlay,
        Self::LocalPolice,
        Self::LocalPostOffice,
        Self::LocalPrintshop,
        Self::LocalSee,
        Self::LocalShipping,
        Self::LocalTaxi,
        Self::LocationAutomation,
        Self::LocationAway,
        Self::LocationChip,
        Self::LocationCity,
        Self::LocationDisabled,
        Self::LocationHome,
        Self::LocationOff,
        Self::LocationOn,
        Self::LocationPin,
        Self::LocationSearching,
        Self::LocatorTag,
        Self::LockClock,
        Self::LockOpen,
        Self::LockOpenCircle,
        Self::LockOpenRight,
        Self::LockOutline,
        Self::LockPerson,
        Self::LockReset,
        Self::Login,
        Self::LogoDev,
        Self::Logout,
        Self::Looks,
        Self::Looks3,
        Self::Looks4,
        Self::Looks5,
        Self::Looks6,
        Self::LooksOne,
        Self::LooksTwo,
        Self::Loop,
        Self::Loupe,
        Self::LowDensity,
        Self::LowPriority,
        Self::Lowercase,
        Self::Loyalty,
        Self::LteMobiledata,
        Self::LteMobiledataBadge,
        Self::LtePlusMobiledata,
        Self::LtePlusMobiledataBadge,
        Self::Luggage,
        Self::LunchDining,
        Self::Lyrics,
        Self::MacroAuto,
        Self::MacroOff,
        Self::MagicButton,
        Self::MagicExchange,
        Self::MagicTether,
        Self::MagnificationLarge,
        Self::MagnificationSmall,
        Self::MagnifyDocked,
        Self::MagnifyFullscreen,
        Self::MailAsterisk,
        Self::MailLock,
        Self::MailOff,
        Self::MailOutline,
        Self::MailShield,
        Self::Male,
        Self::Man,
        Self::Man2,
        Self::Man3,
        Self::Man4,
        Self::ManageAccounts,
        Self::ManageHistory,
        Self::ManageSearch,
        Self::Manga,
        Self::Manufacturing,
        Self::Map,
        Self::MapPinHeart,
        Self::MapPinReview,
        Self::MapSearch,
        Self::MapsHomeWork,
        Self::MapsUgc,
        Self::Margin,
        Self::MarkAsUnread,
        Self::MarkChatRead,
        Self::MarkChatUnread,
        Self::MarkEmailRead,
        Self::MarkEmailUnread,
        Self::MarkUnreadChatAlt,
        Self::Markdown,
        Self::MarkdownConvert,
        Self::MarkdownCopy,
        Self::MarkdownDocument,
        Self::MarkdownPaste,
        Self::MarkdownSpark,
        Self::Markunread,
        Self::MarkunreadMailbox,
        Self::MaskedTransitions,
        Self::MaskedTransitionsAdd,
        Self::Masks,
        Self::Massage,
        Self::MatchCase,
        Self::MatchCaseOff,
        Self::MatchWord,
        Self::Matter,
        Self::Maximize,
        Self::MealDinner,
        Self::MealLunch,
        Self::MeasuringTape,
        Self::MediaBluetoothOff,
        Self::MediaBluetoothOn,
        Self::MediaLink,
        Self::MediaOutput,
        Self::MediaOutputOff,
        Self::Mediation,
        Self::MedicalInformation,
        Self::MedicalMask,
        Self::MedicalServices,
        Self::Medication,
        Self::MedicationLiquid,
        Self::MeetingRoom,
        Self::Memory,
        Self::MemoryAlt,
        Self::MenstrualHealth,
        Self::MenuBook,
        Self::MenuBook2,
        Self::MenuOpen,
        Self::Merge,
        Self::MergeType,
        Self::Message,
        Self::Metabolism,
        Self::Metro,
        Self::MfgNestYaleLock,
        Self::Mic,
        Self::MicAlert,
        Self::MicDouble,
        Self::MicExternalOff,
        Self::MicExternalOn,
        Self::MicGear,
        Self::MicNone,
        Self::MicOff,
        Self::Microbiology,
        Self::Microwave,
        Self::MicrowaveGen,
        Self::MilitaryTech,
        Self::Mimo,
        Self::MimoDisconnect,
        Self::Mindfulness,
        Self::Minimize,
        Self::MinorCrash,
        Self::Mintmark,
        Self::MissedVideoCall,
        Self::MissedVideoCallFilled,
        Self::MissingController,
        Self::Mist,
        Self::Mitre,
        Self::MixtureMed,
        Self::Mms,
        Self::Mobile,
        Self::Mobile2,
        Self::Mobile3,
        Self::MobileAlert,
        Self::MobileArrowDown,
        Self::MobileArrowRight,
        Self::MobileArrowUpRight,
        Self::MobileBlock,
        Self::MobileCamera,
        Self::MobileCameraFront,
        Self::MobileCameraRear,
        Self::MobileCancel,
        Self::MobileCast,
        Self::MobileCharge,
        Self::MobileChat,
        Self::MobileCheck,
        Self::MobileCode,
        Self::MobileDock,
        Self::MobileDots,
        Self::MobileFriendly,
        Self::MobileGear,
        Self::MobileHand,
        Self::MobileHandLeft,
        Self::MobileHandLeftOff,
        Self::MobileHandOff,
        Self::MobileInfo,
        Self::MobileLandscape,
        Self::MobileLayout,
        Self::MobileLockLandscape,
        Self::MobileLockPortrait,
        Self::MobileLoupe,
        Self::MobileMenu,
        Self::MobileOff,
        Self::MobileQuestion,
        Self::MobileRotate,
        Self::MobileRotateLock,
        Self::MobileScreenShare,
        Self::MobileScreensaver,
        Self::MobileSensorHi,
        Self::MobileSensorLo,
        Self::MobileShare,
        Self::MobileShareStack,
        Self::MobileSound,
        Self::MobileSound2,
        Self::MobileSoundOff,
        Self::MobileSpeaker,
        Self::MobileTap,
        Self::MobileText,
        Self::MobileText2,
        Self::MobileTheft,
        Self::MobileTicket,
        Self::MobileUnlock,
        Self::MobileVibrate,
        Self::MobileWrench,
        Self::MobiledataArrows,
        Self::MobiledataOff,
        Self::Mode,
        Self::ModeComment,
        Self::ModeCool,
        Self::ModeCoolOff,
        Self::ModeDual,
        Self::ModeEdit,
        Self::ModeEditOutline,
        Self::ModeFan,
        Self::ModeFan2,
        Self::ModeFanOff,
        Self::ModeHeat,
        Self::ModeHeatCool,
        Self::ModeHeatOff,
        Self::ModeNight,
        Self::ModeOfTravel,
        Self::ModeOffOn,
        Self::ModeStandby,
        Self::ModelTraining,
        Self::Modeling,
        Self::MonetizationOn,
        Self::Money,
        Self::MoneyBag,
        Self::MoneyOff,
        Self::MoneyOffCsred,
        Self::MoneyRange,
        Self::Monitor,
        Self::MonitorHeart,
        Self::MonitorWeight,
        Self::MonitorWeightGain,
        Self::MonitorWeightLoss,
        Self::Monitoring,
        Self::MonochromePhotos,
        Self::Monorail,
        Self::Mood,
        Self::MoodBad,
        Self::MoodHeart,
        Self::MoonStars,
        Self::Mop,
        Self::Moped,
        Self::MopedPackage,
        Self::MoreDown,
        Self::MoreHoriz,
        Self::MoreTime,
        Self::MoreUp,
        Self::MoreVert,
        Self::Mosque,
        Self::MotionBlur,
        Self::MotionMode,
        Self::MotionPhotosAuto,
        Self::MotionPhotosOff,
        Self::MotionPhotosOn,
        Self::MotionPhotosPause,
        Self::MotionPhotosPaused,
        Self::MotionPlay,
        Self::MotionSensorActive,
        Self::MotionSensorAlert,
        Self::MotionSensorIdle,
        Self::MotionSensorUrgent,
        Self::Motorcycle,
        Self::MountainFlag,
        Self::MountainSteam,
        Self::Mouse,
        Self::MouseLock,
        Self::MouseLockOff,
        Self::Move,
        Self::MoveDown,
        Self::MoveGroup,
        Self::MoveItem,
        Self::MoveLocation,
        Self::MoveSelectionDown,
        Self::MoveSelectionLeft,
        Self::MoveSelectionRight,
        Self::MoveSelectionUp,
        Self::MoveToInbox,
        Self::MoveUp,
        Self::MovedLocation,
        Self::Movie,
        Self::MovieCreation,
        Self::MovieEdit,
        Self::MovieEditOff,
        Self::MovieFilter,
        Self::MovieInfo,
        Self::MovieOff,
        Self::MovieSpeaker,
        Self::Moving,
        Self::MovingBeds,
        Self::MovingMinistry,
        Self::Mp,
        Self::Multicooker,
        Self::MultilineChart,
        Self::MultimodalHandEye,
        Self::MultipleAirports,
        Self::MultipleStop,
        Self::Museum,
        Self::MusicCast,
        Self::MusicHistory,
        Self::MusicNote,
        Self::MusicNote2,
        Self::MusicNoteAdd,
        Self::MusicOff,
        Self::MusicVideo,
        Self::MyLocation,
        Self::Mystery,
        Self::Nat,
        Self::Nature,
        Self::NaturePeople,
        Self::NavigateBefore,
        Self::NavigateNext,
        Self::Navigation,
        Self::NearMe,
        Self::NearMeDisabled,
        Self::Nearby,
        Self::NearbyError,
        Self::NearbyOff,
        Self::Nephrology,
        Self::NestAudio,
        Self::NestCamFloodlight,
        Self::NestCamIndoor,
        Self::NestCamIq,
        Self::NestCamIqOutdoor,
        Self::NestCamMagnetMount,
        Self::NestCamOutdoor,
        Self::NestCamStand,
        Self::NestCamWallMount,
        Self::NestCamWiredStand,
        Self::NestClockFarsightAnalog,
        Self::NestClockFarsightDigital,
        Self::NestConnect,
        Self::NestDetect,
        Self::NestDisplay,
        Self::NestDisplayMax,
        Self::NestDoorbellVisitor,
        Self::NestEcoLeaf,
        Self::NestFarsightCool,
        Self::NestFarsightDual,
        Self::NestFarsightEco,
        Self::NestFarsightHeat,
        Self::NestFarsightSeasonal,
        Self::NestFarsightWeather,
        Self::NestFoundSavings,
        Self::NestGaleWifi,
        Self::NestHeatLinkE,
        Self::NestHeatLinkGen3,
        Self::NestHelloDoorbell,
        Self::NestLocatorTag,
        Self::NestMini,
        Self::NestMultiRoom,
        Self::NestProtect,
        Self::NestRemote,
        Self::NestRemoteComfortSensor,
        Self::NestSecureAlarm,
        Self::NestSunblock,
        Self::NestTag,
        Self::NestThermostat,
        Self::NestThermostatEEu,
        Self::NestThermostatGen3,
        Self::NestThermostatSensor,
        Self::NestThermostatSensorEu,
        Self::NestThermostatZirconiumEu,
        Self::NestTrueRadiant,
        Self::NestWakeOnApproach,
        Self::NestWakeOnPress,
        Self::NestWifiGale,
        Self::NestWifiMistral,
        Self::NestWifiPoint,
        Self::NestWifiPointVento,
        Self::NestWifiPro,
        Self::NestWifiPro2,
        Self::NestWifiRouter,
        Self::NetworkCell,
        Self::NetworkCheck,
        Self::NetworkIntelNode,
        Self::NetworkIntelligence,
        Self::NetworkIntelligenceHistory,
        Self::NetworkIntelligenceUpdate,
        Self::NetworkLocked,
        Self::NetworkManage,
        Self::NetworkNode,
        Self::NetworkPing,
        Self::NetworkWifi,
        Self::NetworkWifi1Bar,
        Self::NetworkWifi1BarLocked,
        Self::NetworkWifi2Bar,
        Self::NetworkWifi2BarLocked,
        Self::NetworkWifi3Bar,
        Self::NetworkWifi3BarLocked,
        Self::NetworkWifiLocked,
        Self::Neurology,
        Self::NewLabel,
        Self::NewReleases,
        Self::NewWindow,
        Self::News,
        Self::Newsmode,
        Self::Newspaper,
        Self::Newsstand,
        Self::NextPlan,
        Self::NextWeek,
        Self::Nfc,
        Self::NfcOff,
        Self::NightShelter,
        Self::NightSightAuto,
        Self::NightSightAutoOff,
        Self::NightSightMax,
        Self::Nightlife,
        Self::Nightlight,
        Self::NightlightRound,
        Self::NightsStay,
        Self::NoAccounts,
        Self::NoAdultContent,
        Self::NoBackpack,
        Self::NoCrash,
        Self::NoDrinks,
        Self::NoEncryption,
        Self::NoEncryptionGmailerrorred,
        Self::NoFlash,
        Self::NoFood,
        Self::NoLuggage,
        Self::NoMeals,
        Self::NoMeetingRoom,
        Self::NoPhotography,
        Self::NoSim,
        Self::NoSound,
        Self::NoStroller,
        Self::NoTransfer,
        Self::NoiseAware,
        Self::NoiseControlOff,
        Self::NoiseControlOn,
        Self::NordicWalking,
        Self::North,
        Self::NorthEast,
        Self::NorthWest,
        Self::NotAccessible,
        Self::NotAccessibleForward,
        Self::NotInterested,
        Self::NotListedLocation,
        Self::NotStarted,
        Self::Note,
        Self::NoteAdd,
        Self::NoteAlt,
        Self::NoteStack,
        Self::NoteStackAdd,
        Self::Notes,
        Self::NotificationAdd,
        Self::NotificationAudio,
        Self::NotificationAudioOff,
        Self::NotificationImportant,
        Self::NotificationMultiple,
        Self::NotificationSettings,
        Self::NotificationSound,
        Self::Notifications,
        Self::NotificationsActive,
        Self::NotificationsNone,
        Self::NotificationsPaused,
        Self::NotificationsUnread,
        Self::Numbers,
        Self::Nutrition,
        Self::Ods,
        Self::Odt,
        Self::OfflineBolt,
        Self::OfflinePin,
        Self::OfflinePinOff,
        Self::OfflineShare,
        Self::OilBarrel,
        Self::Okonomiyaki,
        Self::OnDeviceTraining,
        Self::OnHubDevice,
        Self::Oncology,
        Self::OndemandVideo,
        Self::OnlinePrediction,
        Self::Onsen,
        Self::Opacity,
        Self::OpenInBrowser,
        Self::OpenInFull,
        Self::OpenInNew,
        Self::OpenInNewDown,
        Self::OpenInNewOff,
        Self::OpenInPhone,
        Self::OpenJam,
        Self::OpenRun,
        Self::OpenWith,
        Self::Ophthalmology,
        Self::OralDisease,
        Self::Orbit,
        Self::OrderApprove,
        Self::OrderPlay,
        Self::Orders,
        Self::Orthopedics,
        Self::OtherAdmission,
        Self::OtherHouses,
        Self::Outbound,
        Self::Outbox,
        Self::OutboxAlt,
        Self::OutdoorGarden,
        Self::OutdoorGrill,
        Self::OutgoingMail,
        Self::Outlet,
        Self::OutlinedFlag,
        Self::Outpatient,
        Self::OutpatientMed,
        Self::Output,
        Self::OutputCircle,
        Self::Oven,
        Self::OvenGen,
        Self::Overview,
        Self::OverviewKey,
        Self::Owl,
        Self::OxygenSaturation,
        Self::P2p,
        Self::Pace,
        Self::Pacemaker,
        Self::Package,
        Self::Package2,
        Self::Padding,
        Self::Padel,
        Self::PageControl,
        Self::PageFooter,
        Self::PageHeader,
        Self::PageInfo,
        Self::PageMenuIos,
        Self::Pageless,
        Self::Pages,
        Self::Pageview,
        Self::Paid,
        Self::Palette,
        Self::Pallet,
        Self::PanTool,
        Self::PanToolAlt,
        Self::PanZoom,
        Self::Panorama,
        Self::PanoramaFishEye,
        Self::PanoramaHorizontal,
        Self::PanoramaPhotosphere,
        Self::PanoramaVertical,
        Self::PanoramaWideAngle,
        Self::Paragliding,
        Self::ParentChildDining,
        Self::Park,
        Self::ParkingMeter,
        Self::ParkingSign,
        Self::ParkingValet,
        Self::PartlyCloudyDay,
        Self::PartlyCloudyNight,
        Self::PartnerExchange,
        Self::PartnerHeart,
        Self::PartnerReports,
        Self::PartyMode,
        Self::Passkey,
        Self::Passport,
        Self::Password,
        Self::Password2,
        Self::Password2Off,
        Self::PatientList,
        Self::Pattern,
        Self::PauseCircle,
        Self::PauseCircleFilled,
        Self::PauseCircleOutline,
        Self::PausePresentation,
        Self::Payment,
        Self::PaymentArrowDown,
        Self::PaymentCard,
        Self::Payments,
        Self::PedalBike,
        Self::Pediatrics,
        Self::PenSize1,
        Self::PenSize2,
        Self::PenSize3,
        Self::PenSize4,
        Self::PenSize5,
        Self::Pending,
        Self::PendingActions,
        Self::Pentagon,
        Self::People,
        Self::PeopleAlt,
        Self::PeopleOutline,
        Self::PeopleSizeDecrease,
        Self::PeopleSizeIncrease,
        Self::Percent,
        Self::PercentDiscount,
        Self::PerformanceMax,
        Self::Pergola,
        Self::PermCameraMic,
        Self::PermContactCalendar,
        Self::PermDataSetting,
        Self::PermDeviceInformation,
        Self::PermIdentity,
        Self::PermMedia,
        Self::PermPhoneMsg,
        Self::PermScanWifi,
        Self::Person,
        Self::Person2,
        Self::Person3,
        Self::Person4,
        Self::PersonAdd,
        Self::PersonAddAlt,
        Self::PersonAddDisabled,
        Self::PersonAlert,
        Self::PersonApron,
        Self::PersonBook,
        Self::PersonCancel,
        Self::PersonCelebrate,
        Self::PersonCheck,
        Self::PersonEdit,
        Self::PersonFilled,
        Self::PersonHeart,
        Self::PersonOff,
        Self::PersonOutline,
        Self::PersonPin,
        Self::PersonPinCircle,
        Self::PersonPlay,
        Self::PersonRaisedHand,
        Self::PersonRemove,
        Self::PersonSearch,
        Self::PersonShield,
        Self::PersonText,
        Self::PersonalBag,
        Self::PersonalBagOff,
        Self::PersonalBagQuestion,
        Self::PersonalInjury,
        Self::PersonalPlaces,
        Self::PersonalVideo,
        Self::PestControl,
        Self::PestControlRodent,
        Self::PetSupplies,
        Self::Pets,
        Self::Phishing,
        Self::Phone,
        Self::PhoneAlt,
        Self::PhoneAndroid,
        Self::PhoneBluetoothSpeaker,
        Self::PhoneCallback,
        Self::PhoneCancel,
        Self::PhoneDisabled,
        Self::PhoneEnabled,
        Self::PhoneForwarded,
        Self::PhoneInTalk,
        Self::PhoneIphone,
        Self::PhoneLocked,
        Self::PhoneMissed,
        Self::PhonePaused,
        Self::Phonelink,
        Self::PhonelinkErase,
        Self::PhonelinkLock,
        Self::PhonelinkOff,
        Self::PhonelinkRing,
        Self::PhonelinkRingOff,
        Self::PhonelinkSetup,
        Self::Photo,
        Self::PhotoAlbum,
        Self::PhotoAutoMerge,
        Self::PhotoCamera,
        Self::PhotoCameraBack,
        Self::PhotoCameraFront,
        Self::PhotoFilter,
        Self::PhotoFrame,
        Self::PhotoLibrary,
        Self::PhotoPrints,
        Self::PhotoSizeSelectActual,
        Self::PhotoSizeSelectLarge,
        Self::PhotoSizeSelectSmall,
        Self::Php,
        Self::PhysicalTherapy,
        Self::Piano,
        Self::PianoOff,
        Self::Pickleball,
        Self::PictureAsPdf,
        Self::PictureInPicture,
        Self::PictureInPictureAlt,
        Self::PictureInPictureCenter,
        Self::PictureInPictureLarge,
        Self::PictureInPictureMedium,
        Self::PictureInPictureMobile,
        Self::PictureInPictureOff,
        Self::PictureInPictureSmall,
        Self::PieChart,
        Self::PieChartFilled,
        Self::PieChartOutline,
        Self::PieChartOutlined,
        Self::Pill,
        Self::PillOff,
        Self::Pin,
        Self::PinDrop,
        Self::PinEnd,
        Self::PinHistory,
        Self::PinInvoke,
        Self::PinRoad,
        Self::PinRoad2,
        Self::Pinboard,
        Self::PinboardUnread,
        Self::Pinch,
        Self::PinchZoomIn,
        Self::PinchZoomOut,
        Self::Pip,
        Self::PipExit,
        Self::PivotTableChart,
        Self::Place,
        Self::PlaceItem,
        Self::Plagiarism,
        Self::PlaneContrails,
        Self::Planet,
        Self::PlannerBannerAdPt,
        Self::PlannerReview,
        Self::PlayArrow,
        Self::PlayCircle,
        Self::PlayDisabled,
        Self::PlayForWork,
        Self::PlayLesson,
        Self::PlayMusic,
        Self::PlayPause,
        Self::PlayShapes,
        Self::Playground,
        Self::Playground2,
        Self::PlayingCards,
        Self::PlaylistAdd,
        Self::PlaylistAddCheck,
        Self::PlaylistAddCheckCircle,
        Self::PlaylistAddCircle,
        Self::PlaylistPlay,
        Self::PlaylistRemove,
        Self::PlugConnect,
        Self::Plumbing,
        Self::PlusOne,
        Self::Podcasts,
        Self::Podiatry,
        Self::Podium,
        Self::PointOfSale,
        Self::PointScan,
        Self::PokerChip,
        Self::Policy,
        Self::PolicyAlert,
        Self::Poll,
        Self::Polyline,
        Self::Polymer,
        Self::Pool,
        Self::PortableWifiOff,
        Self::Portrait,
        Self::PositionBottomLeft,
        Self::PositionBottomRight,
        Self::PositionTopRight,
        Self::Post,
        Self::PostAdd,
        Self::PottedPlant,
        Self::Power,
        Self::PowerInput,
        Self::PowerOff,
        Self::PowerRounded,
        Self::PowerSettingsCircle,
        Self::PowerSettingsNew,
        Self::PrayerTimes,
        Self::PrecisionManufacturing,
        Self::Pregnancy,
        Self::PregnantWoman,
        Self::Preliminary,
        Self::Prescriptions,
        Self::PresentToAll,
        Self::Preview,
        Self::PreviewOff,
        Self::PriceChange,
        Self::PriceCheck,
        Self::PrintAdd,
        Self::PrintConnect,
        Self::PrintDisabled,
        Self::PrintError,
        Self::PrintLock,
        Self::Priority,
        Self::PriorityHigh,
        Self::Privacy,
        Self::PrivacyTip,
        Self::PrivateConnectivity,
        Self::Problem,
        Self::Procedure,
        Self::ProcessChart,
        Self::ProductionQuantityLimits,
        Self::Productivity,
        Self::ProgressActivity,
        Self::PromptSuggestion,
        Self::Propane,
        Self::PropaneTank,
        Self::Psychiatry,
        Self::Psychology,
        Self::PsychologyAlt,
        Self::Public,
        Self::PublicOff,
        Self::Publish,
        Self::PublishedWithChanges,
        Self::Pulmonology,
        Self::PulseAlert,
        Self::PunchClock,
        Self::PushPin,
        Self::QrCode,
        Self::QrCode2,
        Self::QrCode2Add,
        Self::QrCodeScanner,
        Self::QueryBuilder,
        Self::QueryStats,
        Self::QuestionAnswer,
        Self::QuestionExchange,
        Self::QuestionMark,
        Self::Queue,
        Self::QueueMusic,
        Self::QueuePlayNext,
        Self::QuickPhrases,
        Self::QuickReference,
        Self::QuickReferenceAll,
        Self::QuickReorder,
        Self::Quickreply,
        Self::QuietTime,
        Self::QuietTimeActive,
        Self::Quiz,
        Self::RMobiledata,
        Self::Radar,
        Self::Radio,
        Self::RadioButtonChecked,
        Self::RadioButtonPartial,
        Self::RadioButtonUnchecked,
        Self::Radiology,
        Self::RailwayAlert,
        Self::RailwayAlert2,
        Self::Rainy,
        Self::RainyHeavy,
        Self::RainyLight,
        Self::RainySnow,
        Self::RamenDining,
        Self::RampLeft,
        Self::RampRight,
        Self::RangeHood,
        Self::RateReview,
        Self::RateReviewRtl,
        Self::Raven,
        Self::RawOff,
        Self::RawOn,
        Self::ReadMore,
        Self::ReadinessScore,
        Self::RealEstateAgent,
        Self::RearCamera,
        Self::Rebase,
        Self::RebaseEdit,
        Self::Receipt,
        Self::ReceiptLong,
        Self::ReceiptLongOff,
        Self::RecentActors,
        Self::RecentPatient,
        Self::Recenter,
        Self::Recommend,
        Self::RecordVoiceOver,
        Self::Rectangle,
        Self::RectangleAdd,
        Self::Recycling,
        Self::Redeem,
        Self::ReduceCapacity,
        Self::RegularExpression,
        Self::Relax,
        Self::ReleaseAlert,
        Self::RememberMe,
        Self::Reminder,
        Self::RemindersAlt,
        Self::RemoteGen,
        Self::Remove,
        Self::RemoveCircle,
        Self::RemoveCircleOutline,
        Self::RemoveDone,
        Self::RemoveFromQueue,
        Self::RemoveModerator,
        Self::RemoveRedEye,
        Self::RemoveRoad,
        Self::RemoveSelection,
        Self::RemoveShoppingCart,
        Self::ReopenWindow,
        Self::Reorder,
        Self::Repartition,
        Self::Repeat,
        Self::RepeatOn,
        Self::RepeatOne,
        Self::RepeatOneOn,
        Self::ReplaceAudio,
        Self::ReplaceImage,
        Self::ReplaceVideo,
        Self::Replay,
        Self::Replay10,
        Self::Replay30,
        Self::Replay5,
        Self::ReplayCircleFilled,
        Self::Reply,
        Self::ReplyAll,
        Self::Report,
        Self::ReportGmailerrorred,
        Self::ReportOff,
        Self::ReportProblem,
        Self::RequestPage,
        Self::RequestQuote,
        Self::ResetBrightness,
        Self::ResetColors,
        Self::ResetExposure,
        Self::ResetFocus,
        Self::ResetImage,
        Self::ResetIso,
        Self::ResetSettings,
        Self::ResetShadow,
        Self::ResetShutterSpeed,
        Self::ResetTv,
        Self::ResetWhiteBalance,
        Self::ResetWrench,
        Self::Resize,
        Self::ResizeWindow,
        Self::RespiratoryRate,
        Self::ResponsiveLayout,
        Self::RestArea,
        Self::RestartAlt,
        Self::Restaurant,
        Self::RestaurantMenu,
        Self::Restore,
        Self::RestoreFromTrash,
        Self::RestorePage,
        Self::Resume,
        Self::Reviews,
        Self::RewardedAds,
        Self::Rheumatology,
        Self::RibCage,
        Self::RiceBowl,
        Self::RightClick,
        Self::RightPanelClose,
        Self::RightPanelOpen,
        Self::RingVolume,
        Self::RingVolumeFilled,
        Self::Ripples,
        Self::Road,
        Self::Robot,
        Self::Robot2,
        Self::Rocket,
        Self::RocketLaunch,
        Self::RollerShades,
        Self::RollerShadesClosed,
        Self::RollerSkating,
        Self::Roofing,
        Self::Room,
        Self::RoomPreferences,
        Self::RoomService,
        Self::Rotate90DegreesCcw,
        Self::Rotate90DegreesCw,
        Self::RotateAuto,
        Self::RotateLeft,
        Self::RotateRight,
        Self::RoundaboutLeft,
        Self::RoundaboutRight,
        Self::RoundedCorner,
        Self::Route,
        Self::Router,
        Self::RouterOff,
        Self::Routine,
        Self::Rowing,
        Self::RssFeed,
        Self::Rsvp,
        Self::Rtt,
        Self::Rubric,
        Self::Rule,
        Self::RuleFolder,
        Self::RuleSettings,
        Self::RunCircle,
        Self::RunningWithErrors,
        Self::RvHookup,
        Self::SafetyCheck,
        Self::SafetyCheckOff,
        Self::SafetyDivider,
        Self::Sailing,
        Self::Salinity,
        Self::Sanitizer,
        Self::Satellite,
        Self::SatelliteAlt,
        Self::Sauna,
        Self::SaveAlt,
        Self::SaveAs,
        Self::SaveClock,
        Self::SavedSearch,
        Self::Savings,
        Self::Scale,
        Self::Scan,
        Self::ScanDelete,
        Self::Scanner,
        Self::ScatterPlot,
        Self::Scene,
        Self::ScheduleSend,
        Self::Schema,
        Self::School,
        Self::Science,
        Self::ScienceOff,
        Self::Scooter,
        Self::Score,
        Self::Scoreboard,
        Self::ScreenLockLandscape,
        Self::ScreenLockPortrait,
        Self::ScreenLockRotation,
        Self::ScreenRecord,
        Self::ScreenRotation,
        Self::ScreenRotationAlt,
        Self::ScreenRotationUp,
        Self::ScreenSearchDesktop,
        Self::ScreenShare,
        Self::Screenshot,
        Self::ScreenshotFrame,
        Self::ScreenshotFrame2,
        Self::ScreenshotKeyboard,
        Self::ScreenshotMonitor,
        Self::ScreenshotRegion,
        Self::ScreenshotTablet,
        Self::Script,
        Self::ScrollableHeader,
        Self::ScubaDiving,
        Self::Sd,
        Self::SdCard,
        Self::SdCardAlert,
        Self::SdStorage,
        Self::Sdk,
        Self::SearchActivity,
        Self::SearchCheck,
        Self::SearchCheck2,
        Self::SearchGear,
        Self::SearchHandsFree,
        Self::SearchInsights,
        Self::SearchOff,
        Self::SeatCoolLeft,
        Self::SeatCoolRight,
        Self::SeatHeatLeft,
        Self::SeatHeatRight,
        Self::SeatRead,
        Self::SeatVentLeft,
        Self::SeatVentRight,
        Self::SeatWindow,
        Self::Security,
        Self::SecurityKey,
        Self::SecurityUpdate,
        Self::SecurityUpdateGood,
        Self::SecurityUpdateWarning,
        Self::Segment,
        Self::Select,
        Self::SelectAll,
        Self::SelectCheckBox,
        Self::SelectToSpeak,
        Self::SelectWindow,
        Self::SelectWindow2,
        Self::SelectWindowOff,
        Self::SelfCare,
        Self::SelfImprovement,
        Self::Sell,
        Self::SellCloud,
        Self::SendAndArchive,
        Self::SendMoney,
        Self::SendTimeExtension,
        Self::SendToMobile,
        Self::SensorDoor,
        Self::SensorOccupied,
        Self::SensorWindow,
        Self::Sensors,
        Self::SensorsKrx,
        Self::SensorsKrxOff,
        Self::SensorsOff,
        Self::SentimentCalm,
        Self::SentimentContent,
        Self::SentimentDissatisfied,
        Self::SentimentExcited,
        Self::SentimentExtremelyDissatisfied,
        Self::SentimentFrustrated,
        Self::SentimentNeutral,
        Self::SentimentSad,
        Self::SentimentSatisfied,
        Self::SentimentSatisfiedAlt,
        Self::SentimentStressed,
        Self::SentimentVeryDissatisfied,
        Self::SentimentVerySatisfied,
        Self::SentimentWorried,
        Self::Serif,
        Self::ServerPerson,
        Self::ServiceToolbox,
        Self::SetMeal,
        Self::SettingsAccessibility,
        Self::SettingsAccountBox,
        Self::SettingsAlert,
        Self::SettingsApplications,
        Self::SettingsBRoll,
        Self::SettingsBackupRestore,
        Self::SettingsBluetooth,
        Self::SettingsBrightness,
        Self::SettingsCell,
        Self::SettingsCinematicBlur,
        Self::SettingsEthernet,
        Self::SettingsHeart,
        Self::SettingsInputAntenna,
        Self::SettingsInputComponent,
        Self::SettingsInputComposite,
        Self::SettingsInputHdmi,
        Self::SettingsInputSvideo,
        Self::SettingsMotionMode,
        Self::SettingsNightSight,
        Self::SettingsOverscan,
        Self::SettingsPanorama,
        Self::SettingsPhone,
        Self::SettingsPhotoCamera,
        Self::SettingsPower,
        Self::SettingsRemote,
        Self::SettingsScreen,
        Self::SettingsSeating,
        Self::SettingsSlowMotion,
        Self::SettingsSuggest,
        Self::SettingsSystemDaydream,
        Self::SettingsTimelapse,
        Self::SettingsVideoCamera,
        Self::SettingsVoice,
        Self::SettopComponent,
        Self::SevereCold,
        Self::Shades,
        Self::ShadesClosed,
        Self::Shadow,
        Self::ShadowAdd,
        Self::ShadowMinus,
        Self::ShapeLine,
        Self::ShapeRecognition,
        Self::Shapes,
        Self::ShareEta,
        Self::ShareLocation,
        Self::ShareOff,
        Self::ShareReviews,
        Self::ShareWindows,
        Self::ShavedIce,
        Self::SheetsColumnSwap,
        Self::SheetsRtl,
        Self::ShelfAutoHide,
        Self::ShelfPosition,
        Self::Shelves,
        Self::Shield,
        Self::ShieldCard,
        Self::ShieldLock,
        Self::ShieldLocked,
        Self::ShieldMoon,
        Self::ShieldPerson,
        Self::ShieldQuestion,
        Self::ShieldRadar,
        Self::ShieldToggle,
        Self::ShieldWatch,
        Self::ShieldWithHeart,
        Self::ShieldWithHouse,
        Self::Shift,
        Self::ShiftLock,
        Self::ShiftLockOff,
        Self::ShoeCleats,
        Self::Shop,
        Self::Shop2,
        Self::ShopTwo,
        Self::ShoppingBag,
        Self::ShoppingBagSpeed,
        Self::ShoppingBasket,
        Self::ShoppingCart,
        Self::ShoppingCartCheckout,
        Self::ShoppingCartOff,
        Self::Shoppingmode,
        Self::ShortStay,
        Self::ShortText,
        Self::Shortcut,
        Self::ShowChart,
        Self::Shower,
        Self::Shuffle,
        Self::ShuffleOn,
        Self::ShutterSpeed,
        Self::ShutterSpeedAdd,
        Self::ShutterSpeedMinus,
        Self::Sick,
        Self::SideNavigation,
        Self::SignLanguage,
        Self::SignLanguage2,
        Self::SignLanguageOff,
        Self::SignalCellular0Bar,
        Self::SignalCellular1Bar,
        Self::SignalCellular2Bar,
        Self::SignalCellular3Bar,
        Self::SignalCellular4Bar,
        Self::SignalCellularAdd,
        Self::SignalCellularAlt,
        Self::SignalCellularAlt1Bar,
        Self::SignalCellularAlt2Bar,
        Self::SignalCellularAltOff,
        Self::SignalCellularConnectedNoInternet0Bar,
        Self::SignalCellularConnectedNoInternet4Bar,
        Self::SignalCellularNoSim,
        Self::SignalCellularNodata,
        Self::SignalCellularNull,
        Self::SignalCellularOff,
        Self::SignalCellularPause,
        Self::SignalDisconnected,
        Self::SignalWifi0Bar,
        Self::SignalWifi4Bar,
        Self::SignalWifi4BarLock,
        Self::SignalWifiBad,
        Self::SignalWifiConnectedNoInternet4,
        Self::SignalWifiOff,
        Self::SignalWifiStatusbar4Bar,
        Self::SignalWifiStatusbarNotConnected,
        Self::SignalWifiStatusbarNull,
        Self::Signature,
        Self::Signpost,
        Self::SimCard,
        Self::SimCardAlert,
        Self::SimCardDownload,
        Self::SimCardLock,
        Self::Simulation,
        Self::SingleArrow,
        Self::SingleBed,
        Self::Sip,
        Self::Siren,
        Self::SirenCheck,
        Self::SirenOpen,
        Self::SirenQuestion,
        Self::Skateboarding,
        Self::Skeleton,
        Self::Skillet,
        Self::SkilletCooktop,
        Self::SkipPrevious,
        Self::Skull,
        Self::SkullList,
        Self::SlabSerif,
        Self::Sledding,
        Self::Sleep,
        Self::SleepScore,
        Self::SlideLibrary,
        Self::Sliders,
        Self::Slideshow,
        Self::SlowMotionVideo,
        Self::SmartButton,
        Self::SmartCardReader,
        Self::SmartCardReaderOff,
        Self::SmartDisplay,
        Self::SmartOutlet,
        Self::SmartScreen,
        Self::SmartToy,
        Self::Smartphone,
        Self::SmartphoneCamera,
        Self::SmbShare,
        Self::SmokeFree,
        Self::SmokingRooms,
        Self::Sms,
        Self::SmsFailed,
        Self::Snail,
        Self::SnippetFolder,
        Self::Snooze,
        Self::Snowboarding,
        Self::Snowflake,
        Self::Snowing,
        Self::SnowingHeavy,
        Self::Snowmobile,
        Self::Snowshoeing,
        Self::Soap,
        Self::Soba,
        Self::SocialDistance,
        Self::SocialLeaderboard,
        Self::SolarPower,
        Self::SoloDining,
        Self::SortByAlpha,
        Self::Sos,
        Self::SoundDetectionDogBarking,
        Self::SoundDetectionGlassBreak,
        Self::SoundDetectionLoudSound,
        Self::SoundSampler,
        Self::Soundbar,
        Self::SoupKitchen,
        Self::Source,
        Self::SourceEnvironment,
        Self::SourceNotes,
        Self::South,
        Self::SouthAmerica,
        Self::SouthEast,
        Self::SouthWest,
        Self::Spa,
        Self::SpaceBar,
        Self::SpaceDashboard,
        Self::SpaceDashboard2,
        Self::SpatialAudio,
        Self::SpatialAudioOff,
        Self::SpatialGallery,
        Self::SpatialSpeaker,
        Self::SpatialTracking,
        Self::Speaker,
        Self::Speaker2,
        Self::Speaker3,
        Self::SpeakerGroup,
        Self::SpeakerNotes,
        Self::SpeakerNotesOff,
        Self::SpeakerPhone,
        Self::SpecialCharacter,
        Self::SpecificGravity,
        Self::SpeechToText,
        Self::SpeechToText2,
        Self::Speed,
        Self::Speed025,
        Self::Speed02x,
        Self::Speed05,
        Self::Speed05x,
        Self::Speed075,
        Self::Speed07x,
        Self::Speed12,
        Self::Speed125,
        Self::Speed12x,
        Self::Speed15,
        Self::Speed15x,
        Self::Speed175,
        Self::Speed17x,
        Self::Speed2,
        Self::Speed2x,
        Self::Speed3,
        Self::Speed4,
        Self::SpeedCamera,
        Self::Spellcheck,
        Self::SplitScene,
        Self::SplitScene2,
        Self::SplitSceneDown,
        Self::SplitSceneLeft,
        Self::SplitSceneRight,
        Self::SplitSceneUp,
        Self::Splitscreen,
        Self::SplitscreenAdd,
        Self::SplitscreenBottom,
        Self::SplitscreenLandscape,
        Self::SplitscreenLandscapeAdd,
        Self::SplitscreenLeft,
        Self::SplitscreenPortrait,
        Self::SplitscreenRight,
        Self::SplitscreenTop,
        Self::SplitscreenVerticalAdd,
        Self::Spo2,
        Self::Spoke,
        Self::Sports,
        Self::SportsAndOutdoors,
        Self::SportsBar,
        Self::SportsBaseball,
        Self::SportsBasketball,
        Self::SportsCricket,
        Self::SportsEsports,
        Self::SportsFootball,
        Self::SportsGolf,
        Self::SportsGymnastics,
        Self::SportsHandball,
        Self::SportsHockey,
        Self::SportsKabaddi,
        Self::SportsMartialArts,
        Self::SportsMma,
        Self::SportsMotorsports,
        Self::SportsRugby,
        Self::SportsScore,
        Self::SportsSoccer,
        Self::SportsTennis,
        Self::SportsVolleyball,
        Self::Sprinkler,
        Self::Sprint,
        Self::Sql,
        Self::Square,
        Self::SquareCircle,
        Self::SquareDot,
        Self::SquareFoot,
        Self::SsidChart,
        Self::Stack,
        Self::StackGroup,
        Self::StackHexagon,
        Self::StackOff,
        Self::StackStar,
        Self::StackedBarChart,
        Self::StackedEmail,
        Self::StackedInbox,
        Self::StackedLineChart,
        Self::Stacks,
        Self::StadiaController,
        Self::Stadium,
        Self::Stairs,
        Self::Stairs2,
        Self::StarBorder,
        Self::StarBorderPurple500,
        Self::StarHalf,
        Self::StarOutline,
        Self::StarPurple500,
        Self::StarRate,
        Self::StarRateHalf,
        Self::StarShine,
        Self::Stars,
        Self::Stars2,
        Self::Start,
        Self::Stat0,
        Self::Stat1,
        Self::Stat2,
        Self::Stat3,
        Self::StatMinus1,
        Self::StatMinus2,
        Self::StatMinus3,
        Self::StayCurrentLandscape,
        Self::StayCurrentPortrait,
        Self::StayPrimaryLandscape,
        Self::StayPrimaryPortrait,
        Self::SteeringWheelCool,
        Self::SteeringWheelHeat,
        Self::Step,
        Self::StepInto,
        Self::StepOut,
        Self::StepOver,
        Self::Steppers,
        Self::Steps,
        Self::Stethoscope,
        Self::StethoscopeArrow,
        Self::StethoscopeCheck,
        Self::Sticker,
        Self::StickerAdd,
        Self::StickyNote,
        Self::StickyNote2,
        Self::StockMedia,
        Self::Stockpot,
        Self::StopCircle,
        Self::StopScreenShare,
        Self::Storage,
        Self::Store,
        Self::StoreMallDirectory,
        Self::Storefront,
        Self::Storm,
        Self::Straight,
        Self::Straighten,
        Self::Strategy,
        Self::Stream,
        Self::StreamApps,
        Self::Streetview,
        Self::StressManagement,
        Self::StrikethroughS,
        Self::StrokeFull,
        Self::StrokePartial,
        Self::Stroller,
        Self::Style,
        Self::Styler,
        Self::Stylus,
        Self::StylusBrush,
        Self::StylusFountainPen,
        Self::StylusHighlighter,
        Self::StylusLaserPointer,
        Self::StylusNote,
        Self::StylusPen,
        Self::StylusPencil,
        Self::SubdirectoryArrowLeft,
        Self::SubdirectoryArrowRight,
        Self::Subheader,
        Self::Subject,
        Self::Subscript,
        Self::Subscriptions,
        Self::Subtitles,
        Self::SubtitlesGear,
        Self::SubtitlesOff,
        Self::Subway,
        Self::SubwayWalk,
        Self::Subwoofer,
        Self::Summarize,
        Self::Sunny,
        Self::SunnySnowing,
        Self::Superscript,
        Self::SupervisedUserCircle,
        Self::SupervisedUserCircleOff,
        Self::SupervisorAccount,
        Self::Support,
        Self::SupportAgent,
        Self::Surfing,
        Self::Surgical,
        Self::SurroundSound,
        Self::SwapCalls,
        Self::SwapDrivingApps,
        Self::SwapDrivingAppsWheel,
        Self::SwapHoriz,
        Self::SwapHorizontalCircle,
        Self::SwapVert,
        Self::SwapVerticalCircle,
        Self::Sweep,
        Self::Swipe,
        Self::SwipeDown,
        Self::SwipeDownAlt,
        Self::SwipeLeft,
        Self::SwipeLeft2,
        Self::SwipeLeftAlt,
        Self::SwipeRight,
        Self::SwipeRight2,
        Self::SwipeRightAlt,
        Self::SwipeUp,
        Self::SwipeUpAlt,
        Self::SwipeVertical,
        Self::Switch,
        Self::SwitchAccess,
        Self::SwitchAccess2,
        Self::SwitchAccess3,
        Self::SwitchAccessShortcut,
        Self::SwitchAccessShortcutAdd,
        Self::SwitchAccount,
        Self::SwitchCamera,
        Self::SwitchLeft,
        Self::SwitchOff,
        Self::SwitchRight,
        Self::SwitchVideo,
        Self::Switches,
        Self::SwordRose,
        Self::Swords,
        Self::Symptoms,
        Self::Synagogue,
        Self::Sync,
        Self::SyncAlt,
        Self::SyncArrowDown,
        Self::SyncArrowUp,
        Self::SyncDesktop,
        Self::SyncDisabled,
        Self::SyncLock,
        Self::SyncProblem,
        Self::SyncSavedLocally,
        Self::SyncSavedLocallyOff,
        Self::Syringe,
        Self::SystemSecurityUpdate,
        Self::SystemSecurityUpdateGood,
        Self::SystemSecurityUpdateWarning,
        Self::SystemUpdate,
        Self::SystemUpdateAlt,
        Self::Tab,
        Self::TabClose,
        Self::TabCloseInactive,
        Self::TabCloseRight,
        Self::TabDuplicate,
        Self::TabGroup,
        Self::TabInactive,
        Self::TabMove,
        Self::TabNewRight,
        Self::TabRecent,
        Self::TabSearch,
        Self::TabUnselected,
        Self::TableBar,
        Self::TableChart,
        Self::TableChartView,
        Self::TableConvert,
        Self::TableEdit,
        Self::TableEye,
        Self::TableLamp,
        Self::TableLarge,
        Self::TableRestaurant,
        Self::TableRows,
        Self::TableRowsNarrow,
        Self::TableSign,
        Self::TableView,
        Self::Tablet,
        Self::TabletAndroid,
        Self::TabletCamera,
        Self::TabletMac,
        Self::Tabs,
        Self::Tactic,
        Self::Tag,
        Self::TagFaces,
        Self::TakeoutDining,
        Self::TakeoutDining2,
        Self::TamperDetectionOff,
        Self::TamperDetectionOn,
        Self::TapAndPlay,
        Self::Tapas,
        Self::Target,
        Self::TargetCheck,
        Self::Task,
        Self::TaskAlt,
        Self::TatamiSeat,
        Self::Taunt,
        Self::TaxiAlert,
        Self::TeamDashboard,
        Self::TempPreferencesCustom,
        Self::TempPreferencesEco,
        Self::TempleBuddhist,
        Self::TempleHindu,
        Self::Tenancy,
        Self::Terminal,
        Self::Terminal2,
        Self::TerminalAdd,
        Self::Terrain,
        Self::TextAd,
        Self::TextAdOff,
        Self::TextCompare,
        Self::TextDecrease,
        Self::TextFields,
        Self::TextFieldsAlt,
        Self::TextFormat,
        Self::TextIncrease,
        Self::TextRotateUp,
        Self::TextRotateVertical,
        Self::TextRotationAngledown,
        Self::TextRotationAngleup,
        Self::TextRotationDown,
        Self::TextRotationNone,
        Self::TextSelectEnd,
        Self::TextSelectJumpToBeginning,
        Self::TextSelectJumpToEnd,
        Self::TextSelectMoveBackCharacter,
        Self::TextSelectMoveBackWord,
        Self::TextSelectMoveDown,
        Self::TextSelectMoveForwardCharacter,
        Self::TextSelectMoveForwardWord,
        Self::TextSelectMoveUp,
        Self::TextSelectStart,
        Self::TextSnippet,
        Self::TextToSpeech,
        Self::TextUp,
        Self::Textsms,
        Self::Texture,
        Self::TextureAdd,
        Self::TextureMinus,
        Self::TheaterComedy,
        Self::Theaters,
        Self::Thermometer,
        Self::ThermometerAdd,
        Self::ThermometerAlert,
        Self::ThermometerGain,
        Self::ThermometerLoss,
        Self::ThermometerMinus,
        Self::Thermostat,
        Self::ThermostatArrowDown,
        Self::ThermostatArrowUp,
        Self::ThermostatAuto,
        Self::ThermostatCarbon,
        Self::ThingsToDo,
        Self::ThreadUnread,
        Self::ThreatIntelligence,
        Self::ThumbDown,
        Self::ThumbDownAlt,
        Self::ThumbDownFilled,
        Self::ThumbDownOff,
        Self::ThumbDownOffAlt,
        Self::ThumbUp,
        Self::ThumbUpAlt,
        Self::ThumbUpFilled,
        Self::ThumbUpOff,
        Self::ThumbUpOffAlt,
        Self::ThumbnailBar,
        Self::ThumbsUpDouble,
        Self::ThumbsUpDown,
        Self::Thunderstorm,
        Self::Tibia,
        Self::TibiaAlt,
        Self::TileLarge,
        Self::TileMedium,
        Self::TileSmall,
        Self::TiltArrowDown,
        Self::TiltArrowUp,
        Self::TimeAuto,
        Self::TimeToLeave,
        Self::Timelapse,
        Self::Timeline,
        Self::Timer,
        Self::Timer1,
        Self::Timer10,
        Self::Timer10Alt1,
        Self::Timer10Select,
        Self::Timer2,
        Self::Timer3,
        Self::Timer3Alt1,
        Self::Timer3Select,
        Self::Timer5,
        Self::Timer5Shutter,
        Self::TimerArrowDown,
        Self::TimerArrowUp,
        Self::TimerOff,
        Self::TimerPause,
        Self::TimerPlay,
        Self::TipsAndUpdates,
        Self::TireRepair,
        Self::Title,
        Self::Titlecase,
        Self::Toast,
        Self::Toc,
        Self::Today,
        Self::ToggleOff,
        Self::ToggleOn,
        Self::Token,
        Self::Toll,
        Self::Tonality,
        Self::Tonality2,
        Self::Toolbar,
        Self::ToolsFlatHead,
        Self::ToolsInstallationKit,
        Self::ToolsLadder,
        Self::ToolsLevel,
        Self::ToolsPhillips,
        Self::ToolsPliersWireStripper,
        Self::ToolsPowerDrill,
        Self::ToolsWrench,
        Self::Tooltip,
        Self::Tooltip2,
        Self::TopPanelClose,
        Self::TopPanelOpen,
        Self::Topic,
        Self::Tornado,
        Self::TotalDissolvedSolids,
        Self::TouchApp,
        Self::TouchDouble,
        Self::TouchDouble2,
        Self::TouchLong,
        Self::TouchTriple,
        Self::TouchpadMouse,
        Self::TouchpadMouseOff,
        Self::Tour,
        Self::Toys,
        Self::ToysAndGames,
        Self::ToysFan,
        Self::TrackChanges,
        Self::TrackpadInput,
        Self::TrackpadInput2,
        Self::TrackpadInput3,
        Self::Traffic,
        Self::TrafficJam,
        Self::TrailLength,
        Self::TrailLengthMedium,
        Self::TrailLengthShort,
        Self::Train,
        Self::Tram,
        Self::Transcribe,
        Self::TransferWithinAStation,
        Self::Transform,
        Self::Transgender,
        Self::TransitEnterexit,
        Self::TransitTicket,
        Self::TransitionChop,
        Self::TransitionDissolve,
        Self::TransitionFade,
        Self::TransitionPush,
        Self::TransitionSlide,
        Self::Translate,
        Self::TranslateIndic,
        Self::Transportation,
        Self::Travel,
        Self::TravelExplore,
        Self::TravelLuggageAndBags,
        Self::TrendingDown,
        Self::TrendingFlat,
        Self::TrendingUp,
        Self::TriangleCircle,
        Self::Trip,
        Self::TripOrigin,
        Self::Trolley,
        Self::TrolleyCableCar,
        Self::Trophy,
        Self::Troubleshoot,
        Self::Try,
        Self::Tsunami,
        Self::Tsv,
        Self::Tty,
        Self::Tune,
        Self::Tungsten,
        Self::TurnLeft,
        Self::TurnRight,
        Self::TurnSharpLeft,
        Self::TurnSharpRight,
        Self::TurnSlightLeft,
        Self::TurnSlightRight,
        Self::TurnedIn,
        Self::TurnedInNot,
        Self::Tv,
        Self::TvDisplays,
        Self::TvGen,
        Self::TvGuide,
        Self::TvNext,
        Self::TvOff,
        Self::TvOptionsEditChannels,
        Self::TvOptionsInputSettings,
        Self::TvRemote,
        Self::TvSignin,
        Self::TvWithAssistant,
        Self::TwoPager,
        Self::TwoPagerStore,
        Self::TwoWheeler,
        Self::TypeSpecimen,
        Self::UTurnLeft,
        Self::UTurnRight,
        Self::Udon,
        Self::UlnaRadius,
        Self::UlnaRadiusAlt,
        Self::Umbrella,
        Self::Unarchive,
        Self::Undereye,
        Self::UnfoldLess,
        Self::UnfoldLessDouble,
        Self::UnfoldMore,
        Self::UnfoldMoreDouble,
        Self::Ungroup,
        Self::UniversalCurrency,
        Self::UniversalCurrencyAlt,
        Self::UniversalLocal,
        Self::Unknown2,
        Self::Unknown5,
        Self::Unknown7,
        Self::UnknownDocument,
        Self::UnknownMed,
        Self::Unlicense,
        Self::UnpavedRoad,
        Self::Unpin,
        Self::Unpublished,
        Self::Unsubscribe,
        Self::Upcoming,
        Self::Update,
        Self::UpdateDisabled,
        Self::Upgrade,
        Self::UpiPay,
        Self::Upload2,
        Self::UploadFile,
        Self::Uppercase,
        Self::Urology,
        Self::Usb,
        Self::UsbOff,
        Self::UserAttributes,
        Self::Vaccines,
        Self::Vacuum,
        Self::Vacuum2,
        Self::Vacuum2On,
        Self::Valve,
        Self::VapeFree,
        Self::VapingRooms,
        Self::VariableAdd,
        Self::VariableInsert,
        Self::VariableRemove,
        Self::Variables,
        Self::Ventilator,
        Self::Verified,
        Self::VerifiedOff,
        Self::VerifiedUser,
        Self::VerticalAlignBottom,
        Self::VerticalAlignCenter,
        Self::VerticalAlignTop,
        Self::VerticalDistribute,
        Self::VerticalShades,
        Self::VerticalShadesClosed,
        Self::VerticalSplit,
        Self::Vibration,
        Self::VideoCall,
        Self::VideoCameraBack,
        Self::VideoCameraBackAdd,
        Self::VideoCameraFront,
        Self::VideoCameraFrontOff,
        Self::VideoChat,
        Self::VideoFile,
        Self::VideoFrameCopy,
        Self::VideoFrameSave,
        Self::VideoLabel,
        Self::VideoLibrary,
        Self::VideoSearch,
        Self::VideoSettings,
        Self::VideoStable,
        Self::VideoTemplate,
        Self::Videocam,
        Self::VideocamAlert,
        Self::VideocamOff,
        Self::VideogameAsset,
        Self::VideogameAssetOff,
        Self::ViewAgenda,
        Self::ViewApps,
        Self::ViewArray,
        Self::ViewCarousel,
        Self::ViewColumn,
        Self::ViewColumn2,
        Self::ViewComfy,
        Self::ViewComfyAlt,
        Self::ViewCompact,
        Self::ViewCompactAlt,
        Self::ViewCozy,
        Self::ViewDay,
        Self::ViewHeadline,
        Self::ViewInAr,
        Self::ViewInArNew,
        Self::ViewInArOff,
        Self::ViewKanban,
        Self::ViewList,
        Self::ViewModule,
        Self::ViewObjectTrack,
        Self::ViewQuilt,
        Self::ViewRealSize,
        Self::ViewSidebar,
        Self::ViewStream,
        Self::ViewTimeline,
        Self::ViewWeek,
        Self::Vignette,
        Self::Vignette2,
        Self::Villa,
        Self::Visibility,
        Self::VisibilityLock,
        Self::VisibilityOff,
        Self::VitalSigns,
        Self::Vitals,
        Self::Vo2Max,
        Self::VoiceChat,
        Self::VoiceChatOff,
        Self::VoiceOverOff,
        Self::VoiceSelection,
        Self::VoiceSelectionOff,
        Self::Voicemail,
        Self::Voicemail2,
        Self::Volcano,
        Self::VolumeDown,
        Self::VolumeDownAlt,
        Self::VolumeMute,
        Self::VolunteerActivism,
        Self::VotingChip,
        Self::VpnKey,
        Self::VpnKeyAlert,
        Self::VpnKeyOff,
        Self::VpnLock,
        Self::VpnLock2,
        Self::Vr180Create2d,
        Self::Vr180Create2dOff,
        Self::Vrpano,
        Self::WalkBike,
        Self::WallArt,
        Self::WallLamp,
        Self::Wallet,
        Self::Wallpaper,
        Self::WallpaperSlideshow,
        Self::WandShine,
        Self::WandStars,
        Self::Ward,
        Self::Warehouse,
        Self::WarningAmber,
        Self::WarningOff,
        Self::Wash,
        Self::Washoku,
        Self::Watch,
        Self::WatchAlert,
        Self::WatchArrow,
        Self::WatchArrowDown,
        Self::WatchButton,
        Self::WatchButtonPress,
        Self::WatchCheck,
        Self::WatchLater,
        Self::WatchLock,
        Self::WatchOff,
        Self::WatchScreentime,
        Self::WatchVibration,
        Self::WatchWake,
        Self::Water,
        Self::WaterBottle,
        Self::WaterBottleLarge,
        Self::WaterDamage,
        Self::WaterDo,
        Self::WaterDrop,
        Self::WaterDrops,
        Self::WaterEc,
        Self::WaterFull,
        Self::WaterHeater,
        Self::WaterLock,
        Self::WaterLoss,
        Self::WaterLux,
        Self::WaterMedium,
        Self::WaterOrp,
        Self::WaterPh,
        Self::WaterPump,
        Self::WaterVoc,
        Self::WaterfallChart,
        Self::Waves,
        Self::WavingHand,
        Self::WbAuto,
        Self::WbCloudy,
        Self::WbIncandescent,
        Self::WbIridescent,
        Self::WbShade,
        Self::WbSunny,
        Self::WbTwilight,
        Self::WbTwilight2,
        Self::Wc,
        Self::WeatherHail,
        Self::WeatherMix,
        Self::WeatherSnowy,
        Self::Web,
        Self::WebAsset,
        Self::WebAssetOff,
        Self::WebStories,
        Self::WebTraffic,
        Self::Webhook,
        Self::Weekend,
        Self::Weight,
        Self::West,
        Self::Whatshot,
        Self::Wheat,
        Self::WheelchairPickup,
        Self::WhereToVote,
        Self::WidgetMedium,
        Self::WidgetMenu,
        Self::WidgetSmall,
        Self::WidgetWidth,
        Self::Widgets,
        Self::Width,
        Self::WidthFull,
        Self::WidthNormal,
        Self::WidthWide,
        Self::Wifi,
        Self::Wifi1Bar,
        Self::Wifi2Bar,
        Self::WifiAdd,
        Self::WifiCalling,
        Self::WifiCalling1,
        Self::WifiCalling2,
        Self::WifiCalling3,
        Self::WifiCallingBar1,
        Self::WifiCallingBar2,
        Self::WifiCallingBar3,
        Self::WifiChannel,
        Self::WifiDevice,
        Self::WifiFind,
        Self::WifiHome,
        Self::WifiLock,
        Self::WifiNotification,
        Self::WifiOff,
        Self::WifiPassword,
        Self::WifiProtectedSetup,
        Self::WifiProxy,
        Self::WifiTethering,
        Self::WifiTetheringError,
        Self::WifiTetheringOff,
        Self::WindPower,
        Self::Window,
        Self::WindowClosed,
        Self::WindowOpen,
        Self::WindowSensor,
        Self::WindshieldDefrostAuto,
        Self::WindshieldDefrostFront,
        Self::WindshieldDefrostRear,
        Self::WindshieldHeatFront,
        Self::WineBar,
        Self::Woman,
        Self::Woman2,
        Self::Work,
        Self::WorkAlert,
        Self::WorkHistory,
        Self::WorkOff,
        Self::WorkOutline,
        Self::WorkUpdate,
        Self::Workflow,
        Self::WorkspacePremium,
        Self::Workspaces,
        Self::WorkspacesOutline,
        Self::WoundsInjuries,
        Self::WrapText,
        Self::Wrist,
        Self::WrongLocation,
        Self::Wysiwyg,
        Self::XCircle,
        Self::YCircle,
        Self::Yakitori,
        Self::Yard,
        Self::Yoshoku,
        Self::YourTrips,
        Self::YoutubeActivity,
        Self::YoutubeSearchedFor,
        Self::ZonePersonAlert,
        Self::ZonePersonIdle,
        Self::ZonePersonUrgent,
        Self::ZoomIn,
        Self::ZoomInMap,
        Self::ZoomOut,
        Self::ZoomOutMap,
    ];

    /// The number of leading variants that `tools/icon_tokens.txt` declares by hand.
    ///
    /// The rest of [`IconName::ALL`] is the Material Symbols catalogue. The curated tokens
    /// are declared first, so they are `ALL[..CURATED_TOKENS]`; `FALLBACK_GEOMETRY_ENTRIES`
    /// in `fallback.rs` is the same number, because the fallback carries geometry for exactly
    /// the tokens a control in this crate names directly.
    pub const CURATED_TOKENS: usize = 75;

    /// This icon's bundled outline data, indexed by declaration order.
    ///
    /// # Why the return is not an `Option`
    ///
    /// With the `data` feature on, every variant has data — the generator refuses to emit a
    /// table with a gap — so a token with no outline is a compile error rather than a runtime
    /// placeholder. A `None` here would be a lie about a value that cannot be absent.
    #[cfg(feature = "data")]
    pub fn data(self) -> IconData {
        use crate::data::ICON_DATA;
        // Indexed by the enum's declaration order, which is also `ICON_DATA`'s order: both
        // come from the same token list. The `debug_assert!` catches a reorder in a debug
        // build; `tests/icon_data_integrity_test.rs` checks the names in every build.
        let index = self as usize;
        debug_assert_eq!(
            ICON_DATA[index].name,
            self.as_str(),
            "ICON_DATA order must match IconName declaration order"
        );
        ICON_DATA[index]
    }

    /// [`Self::data`], available whether or not the feature is on.
    ///
    /// # Why two accessors rather than one returning `Option`
    ///
    /// The always-available spelling, so a draw path compiles in both states without a `cfg`
    /// at the call site. It is `Some` exactly when `data` is on, and `None` otherwise, since
    /// without the feature there is no table to index.
    #[cfg(feature = "data")]
    pub fn data_opt(self) -> Option<IconData> {
        Some(self.data())
    }

    /// [`Self::data`] for a build without the icon data: there is none, which is the honest
    /// answer rather than a fabricated entry.
    #[cfg(not(feature = "data"))]
    pub fn data_opt(self) -> Option<IconData> {
        None
    }
}
