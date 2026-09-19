package rust_widgets;

/**
 * JNI declarations for the Android host bridge.
 *
 * <p>These methods bind to the Rust {@code #[no_mangle]} exports in
 * {@code src/platform/android_jni.rs}. The symbol prefix is derived from this
 * package and class: {@code Java_rust_1widgets_RustWidgets_<method>}, where
 * {@code _1} is the JNI escape for the {@code _} in {@code rust_widgets}.
 *
 * <p><b>This class does not create Android {@code View} objects.</b> The library
 * paints every {@code WidgetKind} itself and the host supplies a window plus a
 * drawing surface, so there is no per-kind {@code create} entry point. What the
 * bridge carries instead is what Rust cannot obtain by itself:
 *
 * <ul>
 *   <li>the {@code JavaVM}, captured in {@link #nativeInit()}, so the bridge can
 *       attach threads;
 *   <li>the host {@code Activity}'s {@code Context}, so platform facilities (the
 *       document picker) can be resolved on the library's behalf;
 *   <li>host-observed facts Rust cannot subscribe to, such as a window resize.
 * </ul>
 *
 * <p>Signature parity with the Rust side is enforced by
 * {@code tools/check_jni_signatures.sh}; run it after editing either side. The
 * class previously declared {@code nativeCreateButton} / {@code nativeSetViewText}
 * / … and a set of {@code nativeSelfTest*} helpers, all of which the Rust side had
 * already removed — every one of those calls raised {@code UnsatisfiedLinkError}
 * at runtime while the signature gate reported success, because the gate never
 * parsed this file. Both are fixed.
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
     * <p>Call once after loading the library, before any other bridge call.
     */
    public static native void nativeInit();

    /**
     * Hand the host {@code Activity}'s {@code Context} to the bridge.
     *
     * <p>The bridge stores a global reference to it and drops the reference in
     * {@link #nativeDetachContext()}. Passing the same context twice replaces the
     * stored reference rather than leaking the previous one.
     *
     * @return {@code true} when the context was accepted
     */
    public static native boolean nativeAttachContext(android.content.Context context);

    /**
     * Release the stored {@code Activity} {@code Context}.
     *
     * <p>Call from {@code Activity.onDestroy}. Dropping the global reference is
     * what allows the activity to be collected; without this the reference keeps
     * it reachable across configuration changes.
     */
    public static native void nativeDetachContext();

    /**
     * Ask the host to open {@code uri} in a document viewer.
     *
     * <p>An {@code Activity} operation with no library-side equivalent, so the
     * bridge forwards it and reports whether the host accepted it.
     *
     * @return {@code true} when the host launched the intent
     */
    public static native boolean nativeOpenDocument(String uri);

    /**
     * Report that the host window's client area is now {@code width} x
     * {@code height} pixels.
     *
     * <p>Android has no window-resize callback the library can subscribe to
     * without owning the {@code Activity}, so the host — the only party that
     * observes the change, via {@code OnLayoutChangeListener} or
     * {@code onConfigurationChanged} — reports it here. Reporting re-runs the
     * window's layout, so its children follow the new size.
     *
     * @param windowId the id returned by {@code rw_create_window}
     * @return {@code 1} when the resize was accepted, {@code 0} when refused
     */
    public static native int nativeNotifyResize(long windowId, int width, int height);

    /**
     * Integration status of the bridge, as a bit mask.
     *
     * <p>Bit 0 = the {@code JavaVM} is stored; bit 1 = a {@code Context} is
     * attached. The host uses it to show a diagnostic instead of guessing why a
     * platform call failed.
     */
    public static native int nativeIntegrationStatus();

    /**
     * Number of JNI entry points this bridge exports.
     *
     * <p>A host compares it with the count its Java class was compiled against,
     * which turns a stale {@code .so} into a start-up diagnostic rather than a
     * crash at the first call.
     */
    public static native int nativeMethodCount();

    /**
     * Install the logcat logger without capturing the VM.
     *
     * <p>Lets a host route Rust diagnostics to logcat even when it never calls the
     * JNI bridge — for example a host that only renders frames the library
     * produced.
     */
    public static native void nativeInstallLogging();
}
