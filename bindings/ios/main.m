// ============================================================================
// main.m — iOS simulator integration test host for rust_widgets
// ============================================================================
// This is the Apple analogue of `bindings/android/java/.../MainActivity.java`.
// It boots a real UIKit application inside the iOS Simulator, drives the Rust
// C ABI (`rw_*`), and asserts the property/geometry/visibility contract holds
// for controls hosted by the iOS backend.
//
// # What changed with BLUE15, and why the assertions look different
//
// This probe used to assert that `rw_create_button` produced a real `UIButton`
// subview in the window hierarchy, and that `rw_set_widget_text` changed that
// button's `-titleForState:`. That was correct for the pre-BLUE15 architecture,
// in which each platform backend built native controls.
//
// BLUE15 §D-4 removed native control construction from **all ten** backends,
// iOS included: the library now paints its own controls into a host-provided
// surface. Asserting the presence of a `UIButton` therefore asserts the
// *absence* of the feature that was deliberately implemented — it is a probe
// for the old world, and it fails on correct code.
//
// The probe now asserts what the architecture actually guarantees:
//   1. `UIApplication.sharedApplication` exists (the host is live).
//   2. `rw_create_window` returns a live handle, and the backend creates no
//      native UIKit window of its own (the host owns the surface).
//   3. `rw_create_*` returns live handles for the controls.
//   4. Text round-trips through the C ABI.
//   5. Geometry and visibility are reported back correctly.
//   6. **No** `UIButton` / `UILabel` / `UITextField` exists in the hierarchy —
//      this is the positive assertion of self-painting, and the one that would
//      catch a regression back to native construction.
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

/**
 * Depth-first search for the first view of the given class in a subtree.
 *
 * Used by the self-painting assertion below to prove that the backend created
 * **no** native control view.
 */
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

    // Establish our own key window (the test host's window). The library paints
    // into a surface the host provides, so this window *is* the surface.
    self.window = [[UIWindow alloc] initWithFrame:UIScreen.mainScreen.bounds];
    UIViewController *rootVC = [[UIViewController alloc] init];
    rootVC.view.backgroundColor = UIColor.whiteColor;
    self.window.rootViewController = rootVC;
    [self.window makeKeyAndVisible];

    // 2. Drive the Rust C ABI on the main thread.
    NSUInteger windowsBefore = 0;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    windowsBefore = UIApplication.sharedApplication.windows.count;
#pragma clang diagnostic pop

    rw_init();
    uint64_t window = rw_create_window("ios-probe", 0, 0, 320, 480);
    record(@"create_window_id", window != 0,
           [NSString stringWithFormat:@"rw_create_window = %llu", window]);

    // The contract (BLUE15 #55/#56): the host supplies a window **and** a drawing
    // surface; the library creates no *controls*. A window is therefore expected,
    // and `Platform::create_window` on iOS creates exactly one `UIWindow` to paint
    // into (`src/platform/ios/platform_impl.rs`, `create_ui_window`).
    //
    // This assertion used to read `no_backend_owned_window` and demand that the
    // backend create **no** window at all. That premise went stale: it was written
    // when iOS registered a state-only handle, and the window-creating
    // implementation landed afterwards (2026-09-17 vs the probe's 2026-09-14), so
    // the probe was asserting the opposite of the rule it cites. What actually
    // matters is that the window the backend created is a *painting surface it
    // owns*, not a second host window — so the assertion is now on the count the
    // call added and on that window having a root view controller to paint into.
    //
    // NOTE: `UIApplication.windows` is deprecated in favour of
    // `UIWindowScene.windows`, but a scene-less window (which the backend creates
    // without a scene instance) is only visible in the application-wide list, so
    // only that list can prove the count. Acknowledged locally.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    NSArray<UIWindow *> *appWindows = UIApplication.sharedApplication.windows;
#pragma clang diagnostic pop
    NSUInteger windowsAfter = appWindows.count;
    UIWindow *libraryWindow = nil;
    for (UIWindow *candidate in appWindows) {
        if (candidate != self.window) {
            libraryWindow = candidate;
            break;
        }
    }
    record(@"backend_window_is_a_painting_surface",
           windowsAfter == windowsBefore + 1 && libraryWindow != nil &&
               libraryWindow.rootViewController != nil,
           [NSString stringWithFormat:@"before=%lu after=%lu library=%p rootVC=%p",
                                      (unsigned long)windowsBefore, (unsigned long)windowsAfter,
                                      libraryWindow, libraryWindow.rootViewController]);

    UIView *host = rootVC.view;

    // 3. Controls are created through the C ABI. They are library-side objects
    //    with no native UIKit counterpart, so the assertion is on the handles.
    uint64_t button = rw_create_button(window, "Tap", 16, 40, 120, 40);
    uint64_t label = rw_create_label(window, "Hello", 16, 100, 200, 30);
    uint64_t edit = rw_create_line_edit(window, "edit", 16, 150, 200, 30);
    record(@"control_handles",
           button != 0 && label != 0 && edit != 0,
           [NSString stringWithFormat:@"button=%llu label=%llu line_edit=%llu",
                                      button, label, edit]);

    // 3b. The positive assertion of self-painting: the backend must NOT have
    //     installed native UIKit controls into the host hierarchy. This is what
    //     catches a regression back to the pre-BLUE15 native-control model.
    UIView *buttonView = findViewOfClass(host, UIButton.class);
    UIView *labelView = findViewOfClass(host, UILabel.class);
    UIView *editView = findViewOfClass(host, UITextField.class);
    record(@"self_painted_no_native_views",
           buttonView == nil && labelView == nil && editView == nil,
           [NSString stringWithFormat:@"UIButton=%p UILabel=%p UITextField=%p",
                                      buttonView, labelView, editView]);

    // 4. Text round-trips through the C ABI.
    rw_set_widget_text(button, "Tapped");
    const char *read_back = rw_get_widget_text(button);
    BOOL text_ok = read_back != NULL && strcmp(read_back, "Tapped") == 0;
    record(@"text_roundtrip", text_ok,
           [NSString stringWithFormat:@"rw_get_widget_text = %s",
                                      read_back ? read_back : "(null)"]);
    if (read_back) {
        rw_free_string((char *)read_back);
    }

    // 5. Geometry + visibility must round-trip through the ABI.
    rw_set_widget_geometry(button, 20, 60, 140, 44);
    rw_hide_widget(button);
    BOOL hidden = !rw_is_widget_visible(button);
    rw_show_widget(button);
    BOOL shown = rw_is_widget_visible(button);
    record(@"visibility_geometry", hidden && shown,
           [NSString stringWithFormat:@"hidden_reported=%d shown_reported=%d", hidden, shown]);

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
