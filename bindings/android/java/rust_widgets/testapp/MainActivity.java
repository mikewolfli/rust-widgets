package rust_widgets.testapp;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.widget.LinearLayout;

import rust_widgets.RustWidgets;

/**
 * End-to-end integration test for the rust_widgets Android host bridge.
 *
 * <p>Runs on a device or emulator and exercises the real native path:
 * <ol>
 *   <li>Loads {@code librust_widgets.so} (via {@link RustWidgets}' static block).</li>
 *   <li>Initialises the JNI bridge ({@code nativeInit}).</li>
 *   <li>Attaches and detaches the Activity {@code Context}.</li>
 *   <li>Reports the bridge's integration status and method count, so a stale
 *       {@code .so} shows up as a diagnostic rather than a crash.</li>
 *   <li>Reports a window resize, the one fact only the host observes.</li>
 * </ol>
 *
 * <p>Results are written to logcat under the {@code RustWidgetsTest} tag so a CI
 * job can assert on them without a UI harness. The widget-level half of the probe
 * runs through {@code nativeWidgetSelfTest}, which creates a real window and
 * button, round-trips their text and geometry, toggles visibility and destroys
 * them — the same assertions the iOS probe makes through the C ABI, so neither
 * mobile platform is verified more shallowly than the other.
 *
 * <h3>What this test deliberately does not do</h3>
 *
 * <p>Earlier revisions created real {@code android.widget.*} views through
 * {@code nativeCreateButton} / {@code nativeCreateTextView} / … and mutated them
 * with {@code nativeSetViewText} / {@code nativeSetViewBounds}. Those entry points
 * no longer exist: the library paints every {@code WidgetKind} itself and the host
 * supplies a window plus a drawing surface, so there is no per-kind {@code create}
 * to call. The test was updated with the API — the stale declarations it used to
 * bind against meant every call raised {@code UnsatisfiedLinkError} at runtime.
 */
public class MainActivity extends Activity {

    private static final String TAG = "RustWidgetsTest";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        setContentView(root);

        boolean ok = runBridgeSmokeTest();
        Log.i(TAG, ok ? "RESULT: PASS" : "RESULT: FAIL");
    }

    @Override
    protected void onDestroy() {
        // The bridge holds a global reference to the Activity's Context. Releasing
        // it here is what lets the Activity be collected; skipping it leaks the
        // Activity across configuration changes.
        try {
            RustWidgets.nativeDetachContext();
            Log.i(TAG, "nativeDetachContext ok");
        } catch (UnsatisfiedLinkError e) {
            Log.e(TAG, "nativeDetachContext not linked: " + e.getMessage());
        }
        super.onDestroy();
    }

    /**
     * Drive the Rust JNI bridge and return whether every step succeeded.
     */
    private boolean runBridgeSmokeTest() {
        try {
            RustWidgets.nativeInit();
        } catch (UnsatisfiedLinkError e) {
            Log.e(TAG, "nativeInit not linked: " + e.getMessage());
            return false;
        }
        Log.i(TAG, "nativeInit ok");

        // The host hands over the Activity Context; the bridge stores a global
        // reference so it can resolve platform facilities on the library's behalf.
        //
        // `nativeAttachContext` returns an **int** (1/0), not a boolean: it mirrors the Rust
        // export's `jint`. Assigning it to a `boolean` is a compile error, which is how this
        // line was found — the build script used to discard javac's exit status, so the error
        // had been invisible and the failure appeared later as an uninstallable APK.
        int attached;
        try {
            attached = RustWidgets.nativeAttachContext(this);
        } catch (UnsatisfiedLinkError e) {
            Log.e(TAG, "nativeAttachContext not linked: " + e.getMessage());
            return false;
        }
        if (attached == 0) {
            Log.e(TAG, "nativeAttachContext refused the context");
            return false;
        }
        Log.i(TAG, "nativeAttachContext ok");

        // Status is a bit mask: bit 0 = VM stored, bit 1 = Context attached.
        int status = RustWidgets.nativeIntegrationStatus();
        Log.i(TAG, "nativeIntegrationStatus -> " + status);
        if ((status & 1) == 0) {
            Log.e(TAG, "integration status reports no JavaVM after nativeInit");
            return false;
        }
        if ((status & 2) == 0) {
            Log.e(TAG, "integration status reports no Context after nativeAttachContext");
            return false;
        }

        // A count mismatch means the .so predates this class; reporting it here is
        // the point of the entry point.
        int methodCount = RustWidgets.nativeMethodCount();
        Log.i(TAG, "nativeMethodCount -> " + methodCount);
        if (methodCount <= 0) {
            Log.e(TAG, "nativeMethodCount returned " + methodCount);
            return false;
        }

        // The window reports its own size changes. Drive them through the documented
        // entry point so a regression in the resize path is visible on-device.
        //
        // # What this asserts, and why
        //
        // The previous probe called `nativeNotifyResize(0L, 1080, 1920)` and only
        // logged the result. `0L` is not a window this library ever handed out, so the
        // call exercised **only** the rejection branch — a probe that passes when the
        // success branch is broken, and one that never asserted anything at all.
        //
        // The C ABI's contract (see `accept_host_resize` in `src/platform/android_jni.rs`)
        // is: a resize is accepted (`1`) when the id addresses a live window this backend
        // created, and refused (`0`) for an id it did not create. So the two halves are
        // asserted separately below:
        //   * an unknown id must be refused;
        //   * a non-positive id or size must be refused.
        //
        // The success half — resizing a window that was really created — needs the id
        // `rw_create_window` returned. This Activity's Android JNI surface
        // (`rust_widgets.RustWidgets`) deliberately exposes no `create_window` method
        // (the library paints every control itself, so window creation is reached
        // through the C ABI the host links against), and `nativeWidgetSelfTest` tears
        // down the window it creates without handing its id back. Obtaining a live id
        // here would therefore require adding a JNI creator to a file outside this
        // probe's scope, so the self-test's "window created" bit stands as the evidence
        // that a real window can be created on this device, and the assertions below
        // pin the refusal contract that this entry point does own.
        int refusedUnknown = RustWidgets.nativeNotifyResize(0L, 1080, 1920);
        int refusedBogus = RustWidgets.nativeNotifyResize(0xDEADBEEFL, 1080, 1920);
        int refusedSize = RustWidgets.nativeNotifyResize(1L, 0, 0);
        if (refusedUnknown != 0) {
            Log.e(TAG, "nativeNotifyResize accepted the unknown id 0 (expected refusal)");
            return false;
        }
        if (refusedBogus != 0) {
            Log.e(TAG, "nativeNotifyResize accepted an id the backend never created (expected refusal)");
            return false;
        }
        if (refusedSize != 0) {
            Log.e(TAG, "nativeNotifyResize accepted a non-positive size (expected refusal)");
            return false;
        }
        Log.i(TAG, "nativeNotifyResize refused unknown/stale/non-positive inputs as documented");

        // Everything above proves the JNI **plumbing**. This proves the library can create
        // and drive a control on this device, which is what an app depends on — and it is
        // the assertion the iOS probe has made through the C ABI since it was written,
        // while Android's widget path had no runtime evidence at all.
        return runWidgetSelfTest();
    }

    /**
     * Drive the widget-level self-test and report each bit, so a failure names its step.
     *
     * <p>The bit meanings are documented on the Rust entry point; logging them individually
     * is what turns "self test failed" into "the text did not round-trip".
     */
    private boolean runWidgetSelfTest() {
        int bits;
        try {
            bits = RustWidgets.nativeWidgetSelfTest();
        } catch (UnsatisfiedLinkError e) {
            Log.e(TAG, "nativeWidgetSelfTest not linked: " + e.getMessage());
            return false;
        }
        Log.i(TAG, "nativeWidgetSelfTest -> " + bits + " (0b" + Integer.toBinaryString(bits) + ")");

        final String[] steps = {
            "window created",
            "child button created",
            "text round-trips",
            "geometry round-trips",
            "visibility toggles",
            "both destroyed",
            "no native-menu over-claim",
            "backend names itself",
        };
        boolean ok = true;
        for (int i = 0; i < steps.length; i++) {
            boolean passed = (bits & (1 << i)) != 0;
            if (!passed) {
                ok = false;
            }
            Log.i(TAG, (passed ? "  [PASS] " : "  [FAIL] ") + steps[i]);
        }
        return ok;
    }
}
