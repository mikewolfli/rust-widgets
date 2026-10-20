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
     * <p><b>The return type is {@code int}, not {@code boolean}.</b> The Rust export
     * returns {@code jint} ({@code 1} on success, {@code 0} otherwise), and JNI return
     * types are ABI, not decoration: {@code jboolean} is one byte where {@code jint} is
     * four, so declaring {@code boolean} here made the JVM read one byte of a
     * four-byte return. On a little-endian device {@code 1} survives that read and
     * {@code 0} does not, so the failure would have appeared only when the call
     * *failed*. Compare against {@code != 0}.
     *
     * @return {@code 1} when the Context was accepted, {@code 0} otherwise
     */
    public static native int nativeAttachContext(android.content.Context context);

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
     * Report the bridge's integration state as a bit mask.
     *
     * <p>Intended for a host self-test or a crash-report breadcrumb. Bit 0 = the
     * {@code JavaVM} is stored; bit 1 = a {@code Context} is attached.
     *
     * <p><b>This returns {@code int}, not {@code String}.</b> The Rust export returns
     * {@code jint}. Declaring {@code String} here was the dangerous direction of the
     * same mistake: the JVM would take a small integer and dereference it as a
     * {@code jstring} pointer, which is an access violation rather than a wrong
     * number. It is what `tools/check_jni_signatures.sh` can now detect — it compared
     * parameter types and arity but bound the return type to an unused variable and
     * discarded it.
     *
     * @return a bit mask of the wired-up bridge features
     */
    public static native int nativeIntegrationStatus();

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

    // ---- Diagnostics ------------------------------------------------------

    /**
     * Run the on-device widget self-test and return a pass/fail bit mask.
     *
     * <p>The other entry points here prove the *plumbing*: the VM is stored, a Context is
     * attached, a resize is reported. This one proves the library can create a control,
     * change it and read it back **on this device** — which is what an application
     * depends on, and which no other check in this repository covers at runtime.
     *
     * <p>It is one entry point rather than a set of per-kind creators on purpose: the
     * library paints every {@code WidgetKind} itself, so the host supplies a window and a
     * drawing surface rather than one native {@code View} per kind.
     *
     * @return a bit mask; {@code 255} ({@code 0b11111111}) is a full pass. Bit 0 = window
     *         created, 1 = button created, 2 = text round-trips, 3 = geometry round-trips,
     *         4 = visibility toggles, 5 = both were destroyed, 6 = no native-menu over-claim,
     *         7 = the backend names itself
     */
    public static native int nativeWidgetSelfTest();

    /**
     * Run the on-device platform-services probe and return a **passed** bit mask.
     *
     * <p>The probe is the crate-wide `platform::diagnostics` run: every bit has three possible
     * outcomes — pass, skip (the backend honestly has no such concept) and fail. This method
     * carries the passed bits only; pair it with {@link #nativePlatformProbeFailures()} to tell a
     * skip from a failure.
     *
     * <p>Bit order (stable, shared by every backend): 0 custom clipboard MIME, 1 displays,
     * 2 window-to-display ownership, 3 display generation, 4 DPI scale, 5 system theme,
     * 6 text scale, 7 safe-area insets, 8 IME bridge, 9 accessibility bridge, 10 host path
     * picker, 11 external opening.
     */
    public static native int nativePlatformProbe();

    /**
     * The **failed** bits of the same platform-services probe.
     *
     * <p>A bit set here is a fact the backend claimed and could not deliver; a bit that is set in
     * neither this mask nor {@link #nativePlatformProbe()} is a skip, which is not a failure.
     * A run is acceptable when this mask is zero.
     */
    public static native int nativePlatformProbeFailures();

    // ---- Host input forwarding (BLUE28 B28.02) ----------------------------

    /**
     * Forward a touch/pointer event into the library's widget runtime.
     *
     * <p>Without this, a painted control is visible but never receives a press: the library has
     * the router and the widgets, and the host has the touch events, but nothing joins them.
     *
     * @param code 1 press, 2 release, 3 move, 4 enter, 5 leave — the same vocabulary the C ABI's
     *             {@code rw_dispatch_pointer_event} uses
     * @param x view-local x in pixels (the native side converts to logical pixels)
     * @param y view-local y in pixels
     * @param button 0 primary, 1 secondary (the library's mouse-button convention)
     * @return 1 when a window accepted the event, 0 otherwise
     */
    public static native int nativeDispatchPointerEvent(int code, int x, int y, int button);

    /**
     * Commit text from the input method into the focused widget.
     *
     * @param text the committed string
     * @return 1 when a focused widget accepted the text
     */
    public static native int nativeDispatchText(String text);

    /**
     * Forward a key event into the focused widget.
     *
     * @param keyCode Android's {@code KeyEvent.getKeyCode()}
     * @param modifiers Android's meta-state bitmask
     * @param pressed 1 for down, 0 for up
     * @return 1 when a focused widget accepted the event
     */
    public static native int nativeDispatchKeyEvent(int keyCode, int modifiers, int pressed);

    /**
     * Delivers a document-picker answer to the library's asynchronous path request.
     *
     * <p>The host calls this from {@code onActivityResult}: pass the resolved filesystem path, or
     * an empty string with {@code cancelled != 0} when the user dismissed the picker.
     *
     * @return 1 when the answer was queued
     */
    public static native int nativeInjectPathChoice(String path, int cancelled);

    /** Reports {@code Display.getRotation()} and {@code Configuration.orientation}. */
    public static native int nativeOnConfigurationChanged(int rotation, int orientation);

    /** Reports an Activity lifecycle transition: 0 active, 1 inactive, 2 background. */
    public static native int nativeOnLifecycleChanged(int state);
}
