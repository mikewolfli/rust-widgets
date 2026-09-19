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
 * job can assert on them without a UI harness.
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
        boolean attached;
        try {
            attached = RustWidgets.nativeAttachContext(this);
        } catch (UnsatisfiedLinkError e) {
            Log.e(TAG, "nativeAttachContext not linked: " + e.getMessage());
            return false;
        }
        if (!attached) {
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

        // The window reports its own size changes. Drive one through the documented
        // entry point so a regression in the resize path is visible on-device.
        int resized = RustWidgets.nativeNotifyResize(0L, 1080, 1920);
        Log.i(TAG, "nativeNotifyResize -> " + resized);

        return true;
    }
}
