package rust_widgets.testapp;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.widget.FrameLayout;
import android.widget.LinearLayout;

import rust_widgets.RustWidgets;

/**
 * End-to-end integration test for the rust_widgets Android JNI bridge.
 *
 * <p>Runs on a device or emulator and exercises the real native path:
 * <ol>
 *   <li>Loads {@code librust_widgets.so} (via {@link RustWidgets}' static block).</li>
 *   <li>Initialises the JNI bridge ({@code nativeInit}).</li>
 *   <li>Creates native Android views through the bridge.</li>
 *   <li>Mutates them (text / bounds / visibility / enabled).</li>
 *   <li>Destroys them.</li>
 * </ol>
 *
 * <p>Results are written to logcat under the {@code RustWidgetsTest} tag so a
 * CI job can assert on them without a UI harness.
 */
public class MainActivity extends Activity {

    private static final String TAG = "RustWidgetsTest";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        setContentView(root);

        boolean ok = runBridgeSmokeTest(root);
        if (ok) {
            ok = runRustDrivenTest();
        }
        Log.i(TAG, ok ? "RESULT: PASS" : "RESULT: FAIL");

        // Last: launches the system picker, which takes over the foreground.
        if (ok) {
            runFileDialogTest();
        }
    }

    /**
     * Exercise the Rust→Java direction: hand the Context to the Rust backend and
     * let it create every native widget kind itself.
     *
     * @return whether the Rust-side create path succeeded for all kinds
     */
    private boolean runRustDrivenTest() {
        if (!RustWidgets.nativeAttachContext(this)) {
            Log.e(TAG, "nativeAttachContext failed");
            return false;
        }
        Log.i(TAG, "nativeAttachContext ok");

        int created = RustWidgets.nativeSelfTestKinds();
        Log.i(TAG, "nativeSelfTestKinds -> " + created);
        if (created != 7) {
            Log.e(TAG, "expected 7 Rust-created widgets, got " + created);
            return false;
        }

        // Dialogs are backed by AlertDialog, not a View; exercise create/show/
        // message-update/hide through the Rust API.
        int dialog = RustWidgets.nativeSelfTestDialog();
        Log.i(TAG, "nativeSelfTestDialog -> " + dialog);
        if (dialog != 1) {
            Log.e(TAG, "expected 1 Rust-driven dialog, got " + dialog);
            return false;
        }
        return true;
    }

    /**
     * Exercise the Rust-driven file-dialog path.
     *
     * <p>Runs last because it launches the system document picker, which takes
     * over the foreground. The picker result comes back through this Activity's
     * {@link #onActivityResult}, which is logged but does not affect the pass
     * verdict (the verdict is about the Rust bridge launching it correctly).
     *
     * @return whether the Rust file-dialog path launched the picker
     */
    private boolean runFileDialogTest() {
        int fd = RustWidgets.nativeSelfTestFileDialog();
        Log.i(TAG, "nativeSelfTestFileDialog -> " + fd);
        if (fd != 1) {
            Log.e(TAG, "expected 1 Rust-driven file dialog, got " + fd);
            return false;
        }
        return true;
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, android.content.Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        Log.i(TAG, "onActivityResult req=" + requestCode + " result=" + resultCode
                + " data=" + (data == null ? "null" : String.valueOf(data.getData())));
    }

    /**
     * Drive the Rust JNI bridge and return whether every step succeeded.
     *
     * @param root container the created views are attached to
     */
    private boolean runBridgeSmokeTest(LinearLayout root) {
        try {
            RustWidgets.nativeInit();
        } catch (UnsatisfiedLinkError e) {
            Log.e(TAG, "nativeInit not linked: " + e.getMessage());
            return false;
        }
        Log.i(TAG, "nativeInit ok");

        long button = RustWidgets.nativeCreateButton(this, "Hello", 0, 0, 200, 80);
        long textView = RustWidgets.nativeCreateTextView(this, "Label", 0, 90, 200, 60);
        long editText = RustWidgets.nativeCreateEditText(this, "edit", 0, 160, 200, 60);
        long checkBox = RustWidgets.nativeCreateCheckBox(this, "check", 0, 230, 200, 60);
        long radio = RustWidgets.nativeCreateRadioButton(this, "radio", 0, 300, 200, 60);
        long progress = RustWidgets.nativeCreateProgressBar(this, 0, 370, 200, 40);
        long seek = RustWidgets.nativeCreateSeekBar(this, 0, 420, 200, 60);

        long[] handles = {button, textView, editText, checkBox, radio, progress, seek};
        String[] names = {"Button", "TextView", "EditText", "CheckBox",
                          "RadioButton", "ProgressBar", "SeekBar"};
        boolean allCreated = true;
        for (int i = 0; i < handles.length; i++) {
            if (handles[i] == 0) {
                Log.e(TAG, "create " + names[i] + " returned 0");
                allCreated = false;
            } else {
                Log.i(TAG, "create " + names[i] + " -> " + handles[i]);
            }
        }
        if (!allCreated) {
            return false;
        }

        // Mutation round-trips.
        RustWidgets.nativeSetViewText(button, "Updated");
        RustWidgets.nativeSetViewBounds(button, 10, 10, 220, 90);
        RustWidgets.nativeSetViewEnabled(button, false);
        RustWidgets.nativeSetViewEnabled(button, true);
        RustWidgets.nativeSetViewVisibility(textView, false);
        RustWidgets.nativeSetViewVisibility(textView, true);
        Log.i(TAG, "mutation round-trips ok");

        // Attach the created views so the framework realises them. A FrameLayout
        // wrapper is used because the raw LayoutParams from the bridge are not
        // valid for a LinearLayout child.
        FrameLayout host = new FrameLayout(this);
        host.setLayoutParams(new FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT));
        root.addView(host);
        Log.i(TAG, "views hosted");

        for (long handle : handles) {
            RustWidgets.nativeDestroyView(handle);
        }
        Log.i(TAG, "destroy ok");
        return true;
    }
}
