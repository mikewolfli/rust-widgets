package io.github.rustwidgets;

/**
 * Low-level JNI native method declarations for the {@code rust_widgets} library.
 *
 * <p>This class mirrors the JNI naming convention and provides public static
 * native methods that match the Rust JNI bridge functions in
 * {@code src/bindings/java_jni.rs}.
 *
 * <p>Use this class when you need direct access to the native layer without the
 * validation/convenience wrappers in {@link RustWidgets}.
 *
 * <p>All methods are {@code public static} and call the same JNI entry points
 * as the private methods in {@link RustWidgets}. Both classes load
 * {@code librust_widgets} via their static initialisers.
 */
public final class RustWidgetsJNI {

    static {
        System.loadLibrary("rust_widgets");
    }

    private RustWidgetsJNI() {
        throw new AssertionError("No instances");
    }

    // ======================================================================
    //  Lifecycle
    // ======================================================================

    public static native void nativeInit();
    public static native void nativeRun();
    public static native void nativeQuit();

    // ======================================================================
    //  Widget Creation
    // ======================================================================

    public static native long nativeCreateWindow(String title, int x, int y, int width, int height);
    public static native long nativeCreateButton(long parent, String text, int x, int y, int width, int height);
    public static native long nativeCreateCheckbox(long parent, String text, int x, int y, int width, int height);
    public static native long nativeCreateLineEdit(long parent, String text, int x, int y, int width, int height);
    public static native long nativeCreateLabel(long parent, String text, int x, int y, int width, int height);
    public static native long nativeCreateRadioButton(long parent, String text, int x, int y, int width, int height);
    public static native long nativeCreateSlider(long parent, int x, int y, int width, int height);
    public static native long nativeCreateProgressBar(long parent, int x, int y, int width, int height);
    public static native long nativeCreateComboBox(long parent, int x, int y, int width, int height);
    public static native long nativeCreateListBox(long parent, int x, int y, int width, int height);
    public static native long nativeCreatePanel(long parent, int x, int y, int width, int height);
    public static native long nativeCreateSpinBox(long parent, int x, int y, int width, int height);
    public static native long nativeCreateListView(long parent, int x, int y, int width, int height);
    public static native long nativeCreateScrollArea(long parent, int x, int y, int width, int height);
    public static native long nativeCreateToolBar(long parent, int x, int y, int width, int height);
    public static native long nativeCreateMenuBar(long parent, int x, int y, int width, int height);
    public static native long nativeCreateStatusBar(long parent, String text, int x, int y, int width, int height);

    // ---- Dialog creation --------------------------------------------------

    public static native long nativeCreateMessageBox(long parent, String title, String text,
                                                     int x, int y, int width, int height);
    public static native long nativeCreateFileDialog(long parent, String title,
                                                     int x, int y, int width, int height);
    public static native long nativeCreateColorDialog(long parent, String title,
                                                      int x, int y, int width, int height);
    public static native long nativeCreateFontDialog(long parent, String title,
                                                     int x, int y, int width, int height);

    // ---- Menus ------------------------------------------------------------

    public static native long nativeCreateMenu(long parent, String text, int x, int y, int width, int height);
    public static native boolean nativeAttachMenuBarToWindow(long window, long menuBar);
    public static native long nativeMenuAddItem(long parentMenu, String text, String shortcut);
    public static native long nativePollMenuTriggered();

    // ======================================================================
    //  Widget manipulation
    // ======================================================================

    public static native void nativeShowWidget(long widgetId);
    public static native void nativeHideWidget(long widgetId);
    public static native void nativeSetWidgetText(long widgetId, String text);
    public static native String nativeGetWidgetText(long widgetId);
    public static native void nativeSetWidgetEnabled(long widgetId, boolean enabled);
    public static native boolean nativeIsWidgetEnabled(long widgetId);
    public static native void nativeSetWidgetGeometry(long widgetId, int x, int y, int width, int height);

    // ======================================================================
    //  Combo box
    // ======================================================================

    public static native boolean nativeComboBoxAddItem(long comboBox, String text);
    public static native boolean nativeComboBoxClearItems(long comboBox);
    public static native boolean nativeComboBoxSetCurrentIndex(long comboBox, int index);
    public static native int nativeComboBoxCurrentIndex(long comboBox);
    public static native int nativeComboBoxItemCount(long comboBox);
    public static native String nativeComboBoxItemText(long comboBox, int index);

    // ======================================================================
    //  List box
    // ======================================================================

    public static native boolean nativeListBoxAddItem(long listBox, String text);
    public static native boolean nativeListBoxRemoveItem(long listBox, int index);
    public static native boolean nativeListBoxClearItems(long listBox);
    public static native boolean nativeListBoxSetCurrentIndex(long listBox, int index);
    public static native int nativeListBoxCurrentIndex(long listBox);
    public static native int nativeListBoxItemCount(long listBox);
    public static native String nativeListBoxItemText(long listBox, int index);

    // ======================================================================
    //  Events
    // ======================================================================

    public static native long nativePollWidgetTriggered();
    public static native long nativePollWidgetTriggerEvent();

    // ======================================================================
    //  Clipboard
    // ======================================================================

    public static native boolean nativeSetClipboardText(String text);
    public static native String nativeGetClipboardText();

    // ======================================================================
    //  Platform information
    // ======================================================================

    public static native String nativeBackendName();
    public static native int nativePlatformCapabilities();
    public static native int nativeBindingsApiVersion();

    // ======================================================================
    //  String memory management
    // ======================================================================

    public static native void nativeFreeString(long ptr);
}
