package rust_widgets;

/**
 * JNI declarations for the Android bridge.
 *
 * <p>These methods bind to the Rust {@code #[no_mangle]} exports in
 * {@code src/platform/android_jni.rs}. The symbol prefix is derived from this
 * package and class: {@code Java_rust_1widgets_RustWidgets_<method>}, where
 * {@code _1} is the JNI escape for {@code _} in the package name.
 *
 * <p>Unlike the cross-platform C-ABI binding ({@code io.github.rustwidgets.RustWidgets}),
 * this class is Android-only and carries the JavaVM/Context handshake the Android
 * host needs.
 *
 * <h2>Why there are no {@code nativeCreate*} methods</h2>
 *
 * <p>This class used to declare {@code nativeCreateButton}, {@code nativeCreateTextView},
 * and five siblings, each constructing a real {@code android.widget.*} view, plus the
 * matching {@code nativeSetView*} mutators and {@code nativeDestroyView}. Those were
 * removed together with the Rust-side view construction (BLUE15 §D-4): the library no
 * longer builds native {@code View}s on any platform, so the Android backend paints
 * its own controls like every other backend. A Java method whose Rust symbol no longer
 * exists would fail at the first call with {@code UnsatisfiedLinkError}, which is worse
 * than its absence — hence the deletion rather than a deprecated stub.
 *
 * <p>Controls are now created through the cross-platform entry point
 * ({@code RustWidgets.create}); this class only owns the platform handshake that has to
 * happen before any control exists.
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

    // ---- Platform handshake -----------------------------------------------

    /**
     * Store the JavaVM so later JNI calls can attach the calling thread.
     *
     * <p>Call once after loading the library, before creating any control.
     */
    public static native void nativeInit();

    /**
     * Hand the host {@code Context} to the Rust-side bridge.
     *
     * <p>Once stored, the backend can resolve Android system services (the
     * document picker, logging) without a per-call Java entry point.
     *
     * @return {@code true} when the Context was accepted
     */
    public static native boolean nativeAttachContext(android.content.Context context);

    /**
     * Drop the stored {@code Context}.
     *
     * <p>Call from {@code Activity.onDestroy} so the bridge does not keep the host
     * {@code Activity} alive past its own lifetime.
     */
    public static native void nativeDetachContext();

    // ---- Layout -----------------------------------------------------------

    /**
     * Report the host window's new client size, in pixels.
     *
     * <p>Android has no window-resize callback the library can subscribe to without
     * owning the {@code Activity}: the change is delivered to the host's own
     * {@code View.OnLayoutChangeListener} or {@code Activity.onConfigurationChanged}. So
     * the host, which is the only party that observes it, reports it here — the same
     * contract the desktop backends implement from their toolkit's callback.
     *
     * <p>Reporting re-runs that window's layout, so its children follow the new size
     * instead of keeping the geometry they were given for the old one. A host that never
     * calls this still works: its window keeps the size it was created with.
     *
     * @param windowId the id {@code rw_create_window} returned; a non-positive value is
     *                 refused
     * @param width    the new client width, in pixels; must be positive
     * @param height   the new client height, in pixels; must be positive
     * @return {@code 1} when the resize was accepted, {@code 0} when it was refused (an
     *         unknown {@code windowId} or a non-positive size)
     */
    public static native int nativeNotifyResize(long windowId, int width, int height);

    // ---- Diagnostics ------------------------------------------------------

    /**
     * Route Rust {@code log} output through Android's {@code logcat}.
     *
     * <p>Idempotent: a second call is a no-op rather than installing a second
     * logger.
     */
    public static native void nativeInstallLogging();

    /**
     * Report how many JNI methods the Rust bridge exports.
     *
     * <p>Used by the host's self-test to detect a stale {@code .so} loaded against a
     * newer Java binding: a mismatch means the two halves were built from different
     * revisions.
     *
     * @return the exported method count
     */
    public static native int nativeMethodCount();

    /**
     * Report the bridge's integration state as a human-readable string.
     *
     * <p>Intended for a host self-test or a crash-report breadcrumb.
     *
     * @return a description of which bridge features are wired up
     */
    public static native String nativeIntegrationStatus();

    // ---- Platform actions -------------------------------------------------

    /**
     * Launch the system document picker ({@code ACTION_OPEN_DOCUMENT}).
     *
     * <p>The picker result is delivered to the host Activity's own
     * {@code onActivityResult} / result launcher; the bridge does not intercept it.
     * Requires {@link #nativeAttachContext} to have been called with a live Context.
     *
     * @param mimeType the MIME type filter, e.g. {@code *&#47;*} (a literal slash — the
     *                 spelling is broken with an entity because a raw separator would
     *                 terminate this comment early and stop the file from compiling)
     * @return {@code 1} on success, {@code 0} on failure
     */
    public static native int nativeOpenDocument(String mimeType);
}
