package rust_widgets;

/**
 * JNI declarations for the Android native-view bridge.
 *
 * <p>These methods bind to the Rust {@code #[no_mangle]} exports in
 * {@code src/platform/android_jni.rs}. The symbol prefix is derived from this
 * package and class: {@code Java_rust_1widgets_RustWidgets_<method>}, where
 * {@code _1} is the JNI escape for {@code _} in the package name.
 *
 * <p>Unlike the cross-platform C-ABI binding ({@code io.github.rustwidgets.RustWidgets}),
 * this class creates and manipulates real Android {@code View} objects. It is
 * therefore only meaningful on Android, where the host {@code Activity} passes
 * its {@code Context} in.
 *
 * <p>Signature parity with the Rust side is enforced by
 * {@code tools/check_jni_signatures.py}; run it after editing either side.
 */
public final class RustWidgets {

    static {
        System.loadLibrary("rust_widgets");
    }

    private RustWidgets() {
        throw new AssertionError("No instances");
    }

    /**
     * Store the JavaVM so later JNI calls can attach the calling thread.
     *
     * <p>Call once after loading the library, before creating any view.
     */
    public static native void nativeInit();

    // ---- View creation ----------------------------------------------------

    /** Create an {@code android.widget.Button}. Returns the native handle, or 0. */
    public static native long nativeCreateButton(
            android.content.Context context, String text,
            int x, int y, int w, int h);

    /** Create an {@code android.widget.TextView}. Returns the native handle, or 0. */
    public static native long nativeCreateTextView(
            android.content.Context context, String text,
            int x, int y, int w, int h);

    /** Create an {@code android.widget.EditText}. Returns the native handle, or 0. */
    public static native long nativeCreateEditText(
            android.content.Context context, String text,
            int x, int y, int w, int h);

    /** Create an {@code android.widget.CheckBox}. Returns the native handle, or 0. */
    public static native long nativeCreateCheckBox(
            android.content.Context context, String text,
            int x, int y, int w, int h);

    /** Create an {@code android.widget.RadioButton}. Returns the native handle, or 0. */
    public static native long nativeCreateRadioButton(
            android.content.Context context, String text,
            int x, int y, int w, int h);

    /** Create an {@code android.widget.ProgressBar}. Returns the native handle, or 0. */
    public static native long nativeCreateProgressBar(
            android.content.Context context,
            int x, int y, int w, int h);

    /** Create an {@code android.widget.SeekBar}. Returns the native handle, or 0. */
    public static native long nativeCreateSeekBar(
            android.content.Context context,
            int x, int y, int w, int h);

    // ---- View mutation ----------------------------------------------------

    /** Set the text of a view created by one of the {@code nativeCreate*} methods. */
    public static native void nativeSetViewText(long nativePtr, String text);

    /** Set the bounds of a view created by one of the {@code nativeCreate*} methods. */
    public static native void nativeSetViewBounds(
            long nativePtr, int x, int y, int w, int h);

    /** Show or hide a view ({@code View.VISIBLE} / {@code View.GONE}). */
    public static native void nativeSetViewVisibility(long nativePtr, boolean visible);

    /** Enable or disable a view. */
    public static native void nativeSetViewEnabled(long nativePtr, boolean enabled);

    /** Release the global reference held for a view. */
    public static native void nativeDestroyView(long nativePtr);

    // ---- Rust-driven creation (Rust→Java direction) -----------------------

    /**
     * Hand the host {@code Context} to the Rust-side view factory.
     *
     * <p>Once stored, the Rust {@code AndroidPlatform} backend creates real
     * Android views directly (without a per-call Java entry point).
     *
     * @return {@code true} when the Context was accepted
     */
    public static native boolean nativeAttachContext(android.content.Context context);

    /**
     * Run the Rust-side create path for every native widget kind.
     *
     * @return the number of widgets created; negative values indicate the
     *         bridge was not ready ({@code -1}) or a specific step failed
     */
    public static native int nativeSelfTestKinds();

    /**
     * Exercise the Rust-side dialog path (create / update / dismiss / show).
     *
     * @return {@code 1} on success, negative on failure
     */
    public static native int nativeSelfTestDialog();

    /**
     * Exercise the Rust-side file-dialog path: create a file dialog through the
     * backend and launch the system document picker
     * ({@code ACTION_OPEN_DOCUMENT}) on the stored Activity.
     *
     * <p>The picker result is delivered to the host Activity's own
     * {@code onActivityResult} / result launcher; the bridge does not intercept
     * it.
     *
     * @return {@code 1} on success, negative on failure
     */
    public static native int nativeSelfTestFileDialog();
}
