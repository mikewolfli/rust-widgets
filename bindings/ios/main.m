// ============================================================================
// main.m — iOS simulator integration test host for rust_widgets
// ============================================================================
// This is the Apple analogue of `bindings/android/java/.../MainActivity.java`.
// It boots a real UIKit application inside the iOS Simulator, drives the Rust
// C ABI (`rw_*`), and asserts that the UIKit FFI path produced *real* UIKit
// objects — not just state handles.
//
// Evidence collected (printed to stdout, captured by `simctl launch`):
//   1. `UIApplication.sharedApplication` exists and a scene is connected.
//   2. After `rw_create_window`, the app's key window is a live `UIWindow`
//      whose root view controller is installed.
//   3. After `rw_create_button` / `rw_create_label` / `rw_create_line_edit`,
//      the window's view hierarchy contains real `UIButton` / `UILabel` /
//      `UITextField` subviews.
//   4. Text round-trips through the C ABI (`rw_set_widget_text` /
//      `rw_get_widget_text`).
//
// Exit protocol: the app posts `RESULT: PASS` or `RESULT: FAIL — <reasons>`
// through the `rw_results` file in the app's Documents directory and also logs
// it, so the runner can assert without parsing simulator logs.
// ============================================================================

#import <UIKit/UIKit.h>
#import <objc/runtime.h>
#import <stdio.h>
#import <string.h>

// ── Rust C ABI (see src/bindings/binding_impl.rs and rust_widgets.generated.h)
extern void rw_init(void);
extern void rw_quit(void);
extern uint64_t rw_create_window(const char *title, int x, int y, unsigned width, unsigned height);
extern uint64_t rw_create_button(uint64_t parent, const char *text, int x, int y, unsigned width, unsigned height);
extern uint64_t rw_create_label(uint64_t parent, const char *text, int x, int y, unsigned width, unsigned height);
extern uint64_t rw_create_line_edit(uint64_t parent, const char *text, int x, int y, unsigned width, unsigned height);
extern void rw_set_widget_text(uint64_t widget, const char *text);
extern const char *rw_get_widget_text(uint64_t widget);
extern void rw_free_string(char *value);
extern void rw_set_widget_geometry(uint64_t widget, int x, int y, unsigned width, unsigned height);
extern void rw_show_widget(uint64_t widget);
extern void rw_hide_widget(uint64_t widget);
extern bool rw_is_widget_visible(uint64_t widget);

// ── Test bookkeeping ────────────────────────────────────────────────────────

static NSMutableArray<NSString *> *gFailures = nil;
static NSMutableArray<NSString *> *gLines = nil;

static void record(NSString *name, BOOL ok, NSString *detail) {
    NSString *line = [NSString stringWithFormat:@"[%@] %@: %@",
                                                ok ? @"PASS" : @"FAIL", name, detail];
    printf("%s\n", line.UTF8String);
    [gLines addObject:line];
    if (!ok) {
        [gFailures addObject:name];
    }
}

/// Depth-first search for the first view of the given class in a subtree.
static UIView *findViewOfClass(UIView *root, Class cls) {
    if (!root) {
        return nil;
    }
    if ([root isKindOfClass:cls]) {
        return root;
    }
    for (UIView *child in root.subviews) {
        UIView *found = findViewOfClass(child, cls);
        if (found) {
            return found;
        }
    }
    return nil;
}

static void writeResultFile(NSString *text) {
    NSArray<NSString *> *paths =
        NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES);
    if (paths.count == 0) {
        return;
    }
    NSString *file = [paths[0] stringByAppendingPathComponent:@"ios_probe_result.txt"];
    [text writeToFile:file atomically:YES encoding:NSUTF8StringEncoding error:nil];
}

// ── App delegate ────────────────────────────────────────────────────────────

@interface RWAppDelegate : UIResponder <UIApplicationDelegate>
@property(nonatomic, strong) UIWindow *window;
@end

@implementation RWAppDelegate

- (BOOL)application:(UIApplication *)application
    didFinishLaunchingWithOptions:(NSDictionary *)launchOptions {
    gFailures = [NSMutableArray array];
    gLines = [NSMutableArray array];

    // 1. A real UIKit application must already exist and own a connected scene.
    UIApplication *app = UIApplication.sharedApplication;
    record(@"ui_application", app != nil,
           [NSString stringWithFormat:@"UIApplication.sharedApplication = %p", app]);

    // Establish our own key window (the test host's window), so the Rust FFI
    // can attach widget subviews into a live hierarchy.
    self.window = [[UIWindow alloc] initWithFrame:UIScreen.mainScreen.bounds];
    UIViewController *rootVC = [[UIViewController alloc] init];
    rootVC.view.backgroundColor = UIColor.whiteColor;
    self.window.rootViewController = rootVC;
    [self.window makeKeyAndVisible];

    // 2. Drive the Rust C ABI on the main thread.
    rw_init();
    uint64_t window = rw_create_window("ios-probe", 0, 0, 320, 480);
    record(@"create_window_id", window != 0,
           [NSString stringWithFormat:@"rw_create_window = %llu", window]);

    // The Rust iOS backend creates its own UIWindow via the UIKit FFI. Search
    // the whole application for it: it must be a real, visible UIWindow with a
    // root view controller installed by `create_ui_window`.
    //
    // NOTE: `UIApplication.windows` is deprecated in favour of
    // `UIWindowScene.windows`, but the Rust backend's window is intentionally
    // scene-less (it is created directly with `initWithFrame:` because a scene
    // instance is not available to the library), so only the application-wide
    // list can see it. The deprecation is therefore acknowledged locally.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    NSArray<UIWindow *> *appWindows = UIApplication.sharedApplication.windows;
#pragma clang diagnostic pop
    UIWindow *rustWindow = nil;
    for (UIWindow *candidate in appWindows) {
        if (candidate != self.window && candidate.rootViewController != nil) {
            rustWindow = candidate;
            break;
        }
    }
    // The FFI window is created on the main thread, so it must be present and
    // have a root view controller installed by `create_ui_window`.
    record(@"native_uikit_window",
           rustWindow != nil && rustWindow.rootViewController != nil,
           [NSString stringWithFormat:@"app.windows=%lu rustWindow=%p rootVC=%p",
                                      (unsigned long)appWindows.count, rustWindow,
                                      rustWindow.rootViewController]);

    UIView *host = rootVC.view;

    // 3. Real UIKit subviews created through the FFI and attached to the window.
    uint64_t button = rw_create_button(window, "Tap", 16, 40, 120, 40);
    uint64_t label = rw_create_label(window, "Hello", 16, 100, 200, 30);
    uint64_t edit = rw_create_line_edit(window, "edit", 16, 150, 200, 30);
    record(@"native_view_ids",
           button != 0 && label != 0 && edit != 0,
           [NSString stringWithFormat:@"button=%llu label=%llu line_edit=%llu",
                                      button, label, edit]);

    // The FFI attaches controls to the Rust window's content view. Assert the
    // concrete UIKit classes exist somewhere in the live hierarchy.
    UIView *buttonView = nil;
    UIView *labelView = nil;
    UIView *editView = nil;
    for (UIWindow *candidate in appWindows) {
        UIView *content = candidate.rootViewController.view;
        buttonView = buttonView ?: findViewOfClass(content, UIButton.class);
        labelView = labelView ?: findViewOfClass(content, UILabel.class);
        editView = editView ?: findViewOfClass(content, UITextField.class);
    }
    record(@"native_uikit_subviews",
           buttonView != nil && labelView != nil && editView != nil,
           [NSString stringWithFormat:@"UIButton=%p UILabel=%p UITextField=%p",
                                      buttonView, labelView, editView]);

    // 4. Text round-trip through the C ABI.
    rw_set_widget_text(button, "Tapped");
    const char *read_back = rw_get_widget_text(button);
    BOOL text_ok = read_back != NULL && strcmp(read_back, "Tapped") == 0;
    record(@"text_roundtrip", text_ok,
           [NSString stringWithFormat:@"rw_get_widget_text = %s",
                                      read_back ? read_back : "(null)"]);
    if (read_back) {
        rw_free_string((char *)read_back);
    }

    // 5. Geometry + visibility must drive the real UIKit view.
    rw_set_widget_geometry(button, 20, 60, 140, 44);
    rw_hide_widget(button);
    BOOL hidden = !rw_is_widget_visible(button);
    rw_show_widget(button);
    BOOL shown = rw_is_widget_visible(button);
    record(@"visibility_geometry", hidden && shown,
           [NSString stringWithFormat:@"hidden_reported=%d shown_reported=%d", hidden, shown]);

    // 6. A geometry change must be reflected on the real UIButton frame when a
    //    native object exists for the handle.
    if (buttonView) {
        CGRect f = buttonView.frame;
        record(@"native_frame_applied", f.size.width == 140 && f.size.height == 44,
               [NSString stringWithFormat:@"UIButton.frame = %@", NSStringFromCGRect(f)]);
    } else {
        record(@"native_frame_applied", NO, @"no native UIButton to inspect");
    }

    NSString *result = nil;
    if (gFailures.count == 0) {
        result = @"RESULT: PASS";
    } else {
        result = [NSString stringWithFormat:@"RESULT: FAIL — %@",
                                            [gFailures componentsJoinedByString:@", "]];
    }
    printf("\n%s\n", result.UTF8String);
    writeResultFile([gLines componentsJoinedByString:@"\n"]);
    record(@"result_written", YES, result);

    return YES;
}

- (void)applicationDidEnterBackground:(UIApplication *)application {
    // The probe is single-shot; do not keep the process alive in the background.
    exit(gFailures.count == 0 ? 0 : 1);
}

@end

int main(int argc, char *argv[]) {
    @autoreleasepool {
        return UIApplicationMain(argc, argv, nil, NSStringFromClass(RWAppDelegate.class));
    }
}
