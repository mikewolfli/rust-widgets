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
     * Print a file through the system {@code PrintManager} (BLUE28 B28.27).
     *
     * <p>Called from Rust's {@code Platform::spawn_print_job}. A
     * {@code PrintDocumentAdapter} is a Java class and cannot be created from Rust, so the
     * adapter lives here: it reports one page and streams the job file's bytes (the PDF or
     * image the library produced) to the destination the print UI selected. The system print
     * dialog is {@code PrintManager.print}, so a returned {@code 1} means the job really
     * entered the platform queue.
     *
     * @return {@code 1} when the print UI was launched, {@code 0} when there is no print
     *         service, no context, the file is missing, or the framework refused the job.
     */
    public static int printFile(android.content.Context context, String path) {
        if (context == null || path == null) {
            return 0;
        }
        final java.io.File file = new java.io.File(path);
        if (!file.isFile()) {
            return 0;
        }
        try {
            android.print.PrintManager manager =
                    (android.print.PrintManager) context.getSystemService(
                            android.content.Context.PRINT_SERVICE);
            if (manager == null) {
                return 0;
            }
            android.print.PrintDocumentAdapter adapter =
                    new android.print.PrintDocumentAdapter() {
                        @Override
                        public void onLayout(
                                android.print.PrintAttributes oldAttributes,
                                android.print.PrintAttributes newAttributes,
                                android.os.CancellationSignal cancellationSignal,
                                LayoutResultCallback callback,
                                android.os.Bundle extras) {
                            if (cancellationSignal != null && cancellationSignal.isCanceled()) {
                                callback.onLayoutCancelled();
                                return;
                            }
                            android.print.PrintDocumentInfo info =
                                    new android.print.PrintDocumentInfo.Builder(file.getName())
                                            .setContentType(
                                                    android.print.PrintDocumentInfo
                                                            .CONTENT_TYPE_DOCUMENT)
                                            .setPageCount(1)
                                            .build();
                            callback.onLayoutFinished(info, true);
                        }

                        @Override
                        public void onWrite(
                                android.print.PageRange[] pages,
                                android.os.ParcelFileDescriptor destination,
                                android.os.CancellationSignal cancellationSignal,
                                WriteResultCallback callback) {
                            try (java.io.InputStream in = new java.io.FileInputStream(file);
                                    java.io.OutputStream out =
                                            new java.io.FileOutputStream(
                                                    destination.getFileDescriptor())) {
                                byte[] buffer = new byte[8192];
                                int read;
                                while ((read = in.read(buffer)) > 0) {
                                    out.write(buffer, 0, read);
                                }
                                out.flush();
                                callback.onWriteFinished(
                                        new android.print.PageRange[] {
                                            android.print.PageRange.ALL_PAGES
                                        });
                            } catch (java.io.IOException e) {
                                callback.onWriteFailed(e.getMessage());
                            }
                        }
                    };
            android.print.PrintAttributes attributes =
                    new android.print.PrintAttributes.Builder().build();
            manager.print(file.getName(), adapter, attributes);
            return 1;
        } catch (RuntimeException e) {
            return 0;
        }
    }

    /**
     * Hand the host {@code Activity}'s {@code Context} to the bridge.
     *
     * <p>The bridge stores a global reference to it and drops the reference in
     * {@link #nativeDetachContext()}. Passing the same context twice replaces the
     * stored reference rather than leaking the previous one.
     *
     * @return {@code 1} when the context was accepted, {@code 0} otherwise — an
     *         {@code int}, matching the Rust export's {@code jint} and its documented
     *         {@code 1}/{@code 0} contract. Declaring {@code boolean} made the JVM read
     *         one byte of a four-byte return (see `RustWidgetsAndroid.java` for the full
     *         reason); compare against {@code != 0}.
     */
    public static native int nativeAttachContext(android.content.Context context);

    /**
     * Release the stored {@code Activity} {@code Context}.
     *
     * <p>Call from {@code Activity.onDestroy}. Dropping the global reference is
     * what allows the activity to be collected; without this the reference keeps
     * it reachable across configuration changes.
     */
    public static native void nativeDetachContext();

    /**
     * Ask the host to launch the system document picker, filtered to {@code mimeType}.
     *
     * <p>The picker is started with {@code ACTION_OPEN_DOCUMENT} and {@code setType},
     * so the argument is a **MIME type** — {@code "*&#47;*"}, {@code "image&#47;*"},
     * {@code "application&#47;pdf"} — not a URI to open. An earlier revision of this
     * declaration named the parameter {@code uri} and documented it as "open {@code uri}
     * in a document viewer", which would have made every caller pass a path that the
     * picker then used as a type filter — a picker that filters on garbage looks like a
     * picker with no results.
     *
     * <p>(The slashes above are HTML entities for the same reason the sibling file's
     * header records: a raw <code>*&#47;*</code> would close this Javadoc block at the
     * separator and stop the file compiling. That is not hypothetical — it is the exact
     * error this comment produced the first time it was written.)
     *
     * <p>The result arrives at the host's own {@code onActivityResult}, because the host
     * owns the request code.
     *
     * @param mimeType the MIME type filter, e.g. {@code "*&#47;*"}
     * @return {@code 1} when the picker was launched, {@code 0} otherwise (no Activity,
     *         or the launch failed) — an {@code int}, matching the Rust export
     */
    public static native int nativeOpenDocument(String mimeType);

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

    /**
     * Run the on-device widget self-test and return a pass/fail bit mask.
     *
     * <p>Every other entry point here proves the *plumbing* — the VM is stored, a Context
     * is attached, a resize is reported. This one proves the library can actually create a
     * control, change it and read it back **on this device**, which is what a host app
     * depends on. It creates a window and a button, round-trips the button's text and
     * geometry, toggles its visibility, and destroys both.
     *
     * <p>It is deliberately **one** entry point rather than a set of per-kind creators:
     * the library paints every {@code WidgetKind} itself, so the host supplies a window and
     * a drawing surface rather than one native {@code View} per kind. Adding
     * {@code nativeCreateButton} back just to test would re-introduce the API the project
     * removed.
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

    /**
     * Forward a touch/pointer event into the library's widget runtime (BLUE28 B28.02).
     *
     * @param code 1 press, 2 release, 3 move, 4 enter, 5 leave
     * @param x view-local x in pixels
     * @param y view-local y in pixels
     * @param button 0 primary, 1 secondary
     * @return 1 when a window accepted the event
     */
    public static native int nativeDispatchPointerEvent(int code, int x, int y, int button);

    /** Commit text from the input method into the focused widget. */
    public static native int nativeDispatchText(String text);

    /** Forward a key event into the focused widget. */
    public static native int nativeDispatchKeyEvent(int keyCode, int modifiers, int pressed);

    /** Deliver a document-picker result to the asynchronous path request. */
    public static native int nativeInjectPathChoice(String path, int cancelled);

    /**
     * Reports {@code Display.getRotation()} and {@code Configuration.orientation}.
     *
     * <p>Rotation is {@code Surface.ROTATION_*}; orientation is
     * {@code Configuration.ORIENTATION_*} ({@code 1} portrait, {@code 2} landscape) or
     * {@code 0}. The library uses it for the mobile orientation API and to re-run layout
     * after a device rotation.
     */
    public static native int nativeOnConfigurationChanged(int rotation, int orientation);

    /**
     * Reports an Activity lifecycle transition: {@code 0} active, {@code 1} inactive,
     * {@code 2} background. The library stops its animation clock for the last two.
     */
    public static native int nativeOnLifecycleChanged(int state);
}
