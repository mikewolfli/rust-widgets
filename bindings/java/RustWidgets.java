package io.github.rustwidgets;

/**
 * Pure Java binding for the {@code rust_widgets} native GUI library.
 *
 * <p>This class loads {@code librust_widgets} (the shared library built from the
 * Rust crate) and exposes a clean, idiomatic Java API over its native functions.
 *
 * <h2>Usage</h2>
 * <pre>{@code
 *   RustWidgets.init();
 *   long win = RustWidgets.createWindow("Hello", 100, 100, 640, 480);
 *   long btn = RustWidgets.createButton(win, "Click Me", 10, 10, 120, 32);
 *   RustWidgets.showWidget(win);
 *   RustWidgets.run();
 * }</pre>
 *
 * <p>Widget IDs are {@code long} values (0 = invalid). All methods throw
 * {@link IllegalStateException} on invalid widget IDs or native failures.
 *
 * <p>Thread safety: the native library expects all calls from the main (EDT)
 * thread unless documented otherwise.
 */
public final class RustWidgets {

    // ---- Constants --------------------------------------------------------

    /** Sentinel value representing an invalid / absent widget ID. */
    public static final long INVALID_WIDGET_ID = 0L;

    // Capability bit flags (mirrors platform::Capabilities).
    /** Platform supports DPI scaling. */
    public static final int CAP_DPI_SCALING       = 1 << 0;
    /** Platform supports IME (input method editor). */
    public static final int CAP_IME               = 1 << 1;
    /** Platform supports accessibility. */
    public static final int CAP_ACCESSIBILITY     = 1 << 2;
    /** Platform supports native menus. */
    public static final int CAP_NATIVE_MENU       = 1 << 3;
    /** Platform supports typed widget trigger events. */
    public static final int CAP_TYPED_WIDGET_TRIGGER = 1 << 4;

    // Widget trigger kind codes (mirrors WidgetTriggerKind).
    /** Unknown / unspecified trigger. */
    public static final int TRIGGER_UNKNOWN          = 0;
    /** Primary activation (click, toggle, etc.). */
    public static final int TRIGGER_CLICKED          = 1;
    /** Value changed (text input, slider, etc.). */
    public static final int TRIGGER_VALUE_CHANGED    = 2;
    /** Selection changed (combo box, list box, etc.). */
    public static final int TRIGGER_SELECTION_CHANGED = 3;
    /** Widget / window closed. */
    public static final int TRIGGER_CLOSED           = 4;

    // ---- Static initialiser -----------------------------------------------

    static {
        System.loadLibrary("rust_widgets");
    }

    // ---- Private constructor (utility class) ------------------------------

    private RustWidgets() {
        throw new AssertionError("No instances");
    }

    // ======================================================================
    //  Lifecycle
    // ======================================================================

    /**
     * Initialise the native library. Must be called once before any other
     * method. Calls after the first are no-ops.
     *
     * @throws IllegalStateException if the native library cannot be initialised
     */
    public static void init() {
        nativeInit();
    }

    /**
     * Run the native event loop (blocks the calling thread). Call after
     * creating and showing at least one window.
     */
    public static void run() {
        nativeRun();
    }

    /**
     * Signal the native event loop to quit. Typically called from a button
     * callback / trigger handler.
     */
    public static void quit() {
        nativeQuit();
    }

    // ======================================================================
    //  Widget creation
    // ======================================================================

    /**
     * Create a top-level window.
     *
     * @param title  window title
     * @param x      initial horizontal position
     * @param y      initial vertical position
     * @param width  content area width (px)
     * @param height content area height (px)
     * @return the widget ID (always &gt; 0 on success)
     * @throws IllegalArgumentException if {@code title} is null
     */
    public static long createWindow(String title, int x, int y, int width, int height) {
        if (title == null) throw new IllegalArgumentException("title must not be null");
        return nativeCreateWindow(title, x, y, width, height);
    }

    /**
     * Create a push-button child widget.
     *
     * @param parent parent widget ID
     * @param text   button label
     * @param x      x-offset relative to parent
     * @param y      y-offset relative to parent
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createButton(long parent, String text, int x, int y, int width, int height) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(parent);
        return nativeCreateButton(parent, text, x, y, width, height);
    }

    /**
     * Create a checkbox.
     *
     * @param parent parent widget ID
     * @param text   checkbox label
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createCheckbox(long parent, String text, int x, int y, int width, int height) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(parent);
        return nativeCreateCheckbox(parent, text, x, y, width, height);
    }

    /**
     * Create a single-line text input.
     *
     * @param parent parent widget ID
     * @param text   initial text
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createLineEdit(long parent, String text, int x, int y, int width, int height) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(parent);
        return nativeCreateLineEdit(parent, text, x, y, width, height);
    }

    /**
     * Create a static label.
     *
     * @param parent parent widget ID
     * @param text   label text
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createLabel(long parent, String text, int x, int y, int width, int height) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(parent);
        return nativeCreateLabel(parent, text, x, y, width, height);
    }

    /**
     * Create a radio button.
     *
     * @param parent parent widget ID
     * @param text   radio button label
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createRadioButton(long parent, String text, int x, int y, int width, int height) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(parent);
        return nativeCreateRadioButton(parent, text, x, y, width, height);
    }

    /**
     * Create a horizontal slider.
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createSlider(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateSlider(parent, x, y, width, height);
    }

    /**
     * Create a progress bar.
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createProgressBar(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateProgressBar(parent, x, y, width, height);
    }

    /**
     * Create a combo-box (drop-down list).
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createComboBox(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateComboBox(parent, x, y, width, height);
    }

    /**
     * Create a list box.
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createListBox(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateListBox(parent, x, y, width, height);
    }

    /**
     * Create an invisible panel / container.
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createPanel(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreatePanel(parent, x, y, width, height);
    }

    /**
     * Create a spin-box (numeric stepper).
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createSpinBox(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateSpinBox(parent, x, y, width, height);
    }

    /**
     * Create a list / tree view.
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createListView(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateListView(parent, x, y, width, height);
    }

    /**
     * Create a scrollable area.
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createScrollArea(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateScrollArea(parent, x, y, width, height);
    }

    /**
     * Create a tool bar.
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createToolBar(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateToolBar(parent, x, y, width, height);
    }

    /**
     * Create a menu bar.
     *
     * @param parent parent widget ID
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createMenuBar(long parent, int x, int y, int width, int height) {
        checkWidgetId(parent);
        return nativeCreateMenuBar(parent, x, y, width, height);
    }

    /**
     * Create a status bar.
     *
     * @param parent parent widget ID
     * @param text   status bar text
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createStatusBar(long parent, String text, int x, int y, int width, int height) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(parent);
        return nativeCreateStatusBar(parent, text, x, y, width, height);
    }

    // ---- Dialog creation --------------------------------------------------

    /**
     * Create a message-box dialog.
     *
     * @param parent parent widget ID (may be {@link #INVALID_WIDGET_ID})
     * @param title  dialog title
     * @param text   dialog message body
     * @param x      x-offset
     * @param y      y-offset
     * @param width  dialog width
     * @param height dialog height
     * @return the widget ID
     */
    public static long createMessageBox(long parent, String title, String text,
                                        int x, int y, int width, int height) {
        if (title == null) throw new IllegalArgumentException("title must not be null");
        if (text == null) throw new IllegalArgumentException("text must not be null");
        return nativeCreateMessageBox(parent, title, text, x, y, width, height);
    }

    /**
     * Create a file-open / file-save dialog.
     *
     * @param parent parent widget ID (may be {@link #INVALID_WIDGET_ID})
     * @param title  dialog title
     * @param x      x-offset
     * @param y      y-offset
     * @param width  dialog width
     * @param height dialog height
     * @return the widget ID
     */
    public static long createFileDialog(long parent, String title,
                                        int x, int y, int width, int height) {
        if (title == null) throw new IllegalArgumentException("title must not be null");
        return nativeCreateFileDialog(parent, title, x, y, width, height);
    }

    /**
     * Create a color-picker dialog.
     *
     * @param parent parent widget ID (may be {@link #INVALID_WIDGET_ID})
     * @param title  dialog title
     * @param x      x-offset
     * @param y      y-offset
     * @param width  dialog width
     * @param height dialog height
     * @return the widget ID
     */
    public static long createColorDialog(long parent, String title,
                                         int x, int y, int width, int height) {
        if (title == null) throw new IllegalArgumentException("title must not be null");
        return nativeCreateColorDialog(parent, title, x, y, width, height);
    }

    /**
     * Create a font-picker dialog.
     *
     * @param parent parent widget ID (may be {@link #INVALID_WIDGET_ID})
     * @param title  dialog title
     * @param x      x-offset
     * @param y      y-offset
     * @param width  dialog width
     * @param height dialog height
     * @return the widget ID
     */
    public static long createFontDialog(long parent, String title,
                                        int x, int y, int width, int height) {
        if (title == null) throw new IllegalArgumentException("title must not be null");
        return nativeCreateFontDialog(parent, title, x, y, width, height);
    }

    // ---- Menu creation ----------------------------------------------------

    /**
     * Create a menu (child of a menu bar or another menu).
     *
     * @param parent parent widget ID (menu bar or existing menu)
     * @param text   menu label
     * @param x      x-offset
     * @param y      y-offset
     * @param width  widget width
     * @param height widget height
     * @return the widget ID
     */
    public static long createMenu(long parent, String text, int x, int y, int width, int height) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(parent);
        return nativeCreateMenu(parent, text, x, y, width, height);
    }

    /**
     * Attach a menu bar to a window.
     *
     * @param window  window widget ID
     * @param menuBar menu bar widget ID
     * @return true on success
     */
    public static boolean attachMenuBarToWindow(long window, long menuBar) {
        checkWidgetId(window);
        checkWidgetId(menuBar);
        return nativeAttachMenuBarToWindow(window, menuBar);
    }

    /**
     * Add an item to a menu.
     *
     * @param parentMenu the menu widget ID
     * @param text       item label
     * @param shortcut   keyboard shortcut string (e.g. {@code "Ctrl+S"}), or empty
     * @return the menu-item widget ID
     */
    public static long menuAddItem(long parentMenu, String text, String shortcut) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        if (shortcut == null) shortcut = "";
        checkWidgetId(parentMenu);
        return nativeMenuAddItem(parentMenu, text, shortcut);
    }

    // ======================================================================
    //  Widget manipulation
    // ======================================================================

    /**
     * Show a previously hidden widget.
     *
     * @param widgetId target widget ID
     */
    public static void showWidget(long widgetId) {
        checkWidgetId(widgetId);
        nativeShowWidget(widgetId);
    }

    /**
     * Hide a widget.
     *
     * @param widgetId target widget ID
     */
    public static void hideWidget(long widgetId) {
        checkWidgetId(widgetId);
        nativeHideWidget(widgetId);
    }

    /**
     * Set the text of a widget (label, button, line-edit, etc.).
     *
     * @param widgetId target widget ID
     * @param text     new text
     */
    public static void setWidgetText(long widgetId, String text) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(widgetId);
        nativeSetWidgetText(widgetId, text);
    }

    /**
     * Get the current text of a widget.
     *
     * @param widgetId target widget ID
     * @return widget text (never null)
     */
    public static String getWidgetText(long widgetId) {
        checkWidgetId(widgetId);
        return nativeGetWidgetText(widgetId);
    }

    /**
     * Enable or disable a widget.
     *
     * @param widgetId target widget ID
     * @param enabled  {@code true} to enable, {@code false} to disable
     */
    public static void setWidgetEnabled(long widgetId, boolean enabled) {
        checkWidgetId(widgetId);
        nativeSetWidgetEnabled(widgetId, enabled);
    }

    /**
     * Check whether a widget is enabled.
     *
     * @param widgetId target widget ID
     * @return true if enabled
     */
    public static boolean isWidgetEnabled(long widgetId) {
        checkWidgetId(widgetId);
        return nativeIsWidgetEnabled(widgetId);
    }

    /**
     * Set the geometry (position and size) of a widget.
     *
     * @param widgetId target widget ID
     * @param x        new x-offset
     * @param y        new y-offset
     * @param width    new width
     * @param height   new height
     */
    public static void setWidgetGeometry(long widgetId, int x, int y, int width, int height) {
        checkWidgetId(widgetId);
        nativeSetWidgetGeometry(widgetId, x, y, width, height);
    }

    // ======================================================================
    //  Combo-box items
    // ======================================================================

    /**
     * Add an item to a combo-box.
     *
     * @param comboBox combo-box widget ID
     * @param text     item text
     * @return true on success
     */
    public static boolean comboBoxAddItem(long comboBox, String text) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(comboBox);
        return nativeComboBoxAddItem(comboBox, text);
    }

    /**
     * Remove all items from a combo-box.
     *
     * @param comboBox combo-box widget ID
     * @return true on success
     */
    public static boolean comboBoxClearItems(long comboBox) {
        checkWidgetId(comboBox);
        return nativeComboBoxClearItems(comboBox);
    }

    /**
     * Set the current (selected) index of a combo-box.
     *
     * @param comboBox combo-box widget ID
     * @param index    zero-based index
     * @return true on success
     */
    public static boolean comboBoxSetCurrentIndex(long comboBox, int index) {
        checkWidgetId(comboBox);
        return nativeComboBoxSetCurrentIndex(comboBox, index);
    }

    /**
     * Get the current (selected) index of a combo-box.
     *
     * @param comboBox combo-box widget ID
     * @return zero-based index, or -1 if nothing selected
     */
    public static int comboBoxCurrentIndex(long comboBox) {
        checkWidgetId(comboBox);
        return nativeComboBoxCurrentIndex(comboBox);
    }

    /**
     * Get the number of items in a combo-box.
     *
     * @param comboBox combo-box widget ID
     * @return item count
     */
    public static int comboBoxItemCount(long comboBox) {
        checkWidgetId(comboBox);
        return nativeComboBoxItemCount(comboBox);
    }

    /**
     * Get the text of a combo-box item at the given index.
     *
     * @param comboBox combo-box widget ID
     * @param index    zero-based index
     * @return item text (never null)
     */
    public static String comboBoxItemText(long comboBox, int index) {
        checkWidgetId(comboBox);
        return nativeComboBoxItemText(comboBox, index);
    }

    // ======================================================================
    //  List-box items
    // ======================================================================

    /**
     * Add an item to a list box.
     *
     * @param listBox list-box widget ID
     * @param text    item text
     * @return true on success
     */
    public static boolean listBoxAddItem(long listBox, String text) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        checkWidgetId(listBox);
        return nativeListBoxAddItem(listBox, text);
    }

    /**
     * Remove an item from a list box by index.
     *
     * @param listBox list-box widget ID
     * @param index   zero-based index
     * @return true on success
     */
    public static boolean listBoxRemoveItem(long listBox, int index) {
        checkWidgetId(listBox);
        return nativeListBoxRemoveItem(listBox, index);
    }

    /**
     * Remove all items from a list box.
     *
     * @param listBox list-box widget ID
     * @return true on success
     */
    public static boolean listBoxClearItems(long listBox) {
        checkWidgetId(listBox);
        return nativeListBoxClearItems(listBox);
    }

    /**
     * Set the current (selected) index of a list box.
     *
     * @param listBox list-box widget ID
     * @param index   zero-based index
     * @return true on success
     */
    public static boolean listBoxSetCurrentIndex(long listBox, int index) {
        checkWidgetId(listBox);
        return nativeListBoxSetCurrentIndex(listBox, index);
    }

    /**
     * Get the current (selected) index of a list box.
     *
     * @param listBox list-box widget ID
     * @return zero-based index, or -1 if nothing selected
     */
    public static int listBoxCurrentIndex(long listBox) {
        checkWidgetId(listBox);
        return nativeListBoxCurrentIndex(listBox);
    }

    /**
     * Get the number of items in a list box.
     *
     * @param listBox list-box widget ID
     * @return item count
     */
    public static int listBoxItemCount(long listBox) {
        checkWidgetId(listBox);
        return nativeListBoxItemCount(listBox);
    }

    /**
     * Get the text of a list-box item at the given index.
     *
     * @param listBox list-box widget ID
     * @param index   zero-based index
     * @return item text (never null)
     */
    public static String listBoxItemText(long listBox, int index) {
        checkWidgetId(listBox);
        return nativeListBoxItemText(listBox, index);
    }

    // ======================================================================
    //  Events
    // ======================================================================

    /**
     * Poll for a simple widget-triggered event (widget ID only).
     *
     * @return the widget ID that was triggered, or {@link #INVALID_WIDGET_ID}
     *         if no event is pending
     */
    public static long pollWidgetTriggered() {
        return nativePollWidgetTriggered();
    }

    /**
     * A typed widget-trigger event carrying a widget ID and a trigger kind.
     */
    public static final class TriggerEvent {
        private final long widgetId;
        private final int  kind;

        TriggerEvent(long widgetId, int kind) {
            this.widgetId = widgetId;
            this.kind     = kind;
        }

        /** The widget that was triggered. */
        public long widgetId() { return widgetId; }

        /** The trigger kind code ({@link #TRIGGER_CLICKED}, etc.). */
        public int kind() { return kind; }

        @Override
        public String toString() {
            return "TriggerEvent{widgetId=" + widgetId + ", kind=" + kind + "}";
        }
    }

    /**
     * Poll for the next typed widget-trigger event.
     *
     * @return a {@link TriggerEvent}, or {@code null} if no event is pending
     */
    public static TriggerEvent pollWidgetTriggerEvent() {
        long packed = nativePollWidgetTriggerEvent();
        if (packed == 0) {
            return null;
        }
        int kindCode  = (int) (packed >>> 32);
        long widgetId = packed & 0xFFFF_FFFFL;
        return new TriggerEvent(widgetId, kindCode);
    }

    /**
     * Poll for a menu-triggered event.
     *
     * @return the menu-item widget ID that was triggered, or
     *         {@link #INVALID_WIDGET_ID} if no event is pending
     */
    public static long pollMenuTriggered() {
        return nativePollMenuTriggered();
    }

    // ======================================================================
    //  Clipboard
    // ======================================================================

    /**
     * Set the system clipboard text.
     *
     * @param text text to place on the clipboard
     * @return true on success
     */
    public static boolean setClipboardText(String text) {
        if (text == null) throw new IllegalArgumentException("text must not be null");
        return nativeSetClipboardText(text);
    }

    /**
     * Get the current system clipboard text.
     *
     * @return clipboard text (never null; empty if unavailable)
     */
    public static String getClipboardText() {
        return nativeGetClipboardText();
    }

    // ======================================================================
    //  Platform information
    // ======================================================================

    /**
     * Get the name of the active native backend.
     *
     * @return backend name (e.g. {@code "native-gtk"}, {@code "native-win32"})
     */
    public static String backendName() {
        return nativeBackendName();
    }

    /**
     * Get a bitmask of platform capabilities.
     *
     * @return bitmask of {@link #CAP_DPI_SCALING CAP_*} flags
     */
    public static int platformCapabilities() {
        return nativePlatformCapabilities();
    }

    /**
     * Get the C ABI / binding API version number.
     *
     * @return version number
     */
    public static int bindingsApiVersion() {
        return nativeBindingsApiVersion();
    }

    // ======================================================================
    //  String memory management (advanced)
    // ======================================================================

    /**
     * Free a string pointer allocated by the native library.
     * Only needed when working with raw pointer-based APIs.
     *
     * @param ptr the native pointer to free (0 is safe)
     */
    public static void freeString(long ptr) {
        nativeFreeString(ptr);
    }

    // ======================================================================
    //  Native method declarations (private — called via public API)
    // ======================================================================

    // Lifecycle
    private static native void nativeInit();
    private static native void nativeRun();
    private static native void nativeQuit();

    // Widget creation
    private static native long nativeCreateWindow(String title, int x, int y, int width, int height);
    private static native long nativeCreateButton(long parent, String text, int x, int y, int width, int height);
    private static native long nativeCreateCheckbox(long parent, String text, int x, int y, int width, int height);
    private static native long nativeCreateLineEdit(long parent, String text, int x, int y, int width, int height);
    private static native long nativeCreateLabel(long parent, String text, int x, int y, int width, int height);
    private static native long nativeCreateRadioButton(long parent, String text, int x, int y, int width, int height);
    private static native long nativeCreateSlider(long parent, int x, int y, int width, int height);
    private static native long nativeCreateProgressBar(long parent, int x, int y, int width, int height);
    private static native long nativeCreateComboBox(long parent, int x, int y, int width, int height);
    private static native long nativeCreateListBox(long parent, int x, int y, int width, int height);
    private static native long nativeCreatePanel(long parent, int x, int y, int width, int height);
    private static native long nativeCreateSpinBox(long parent, int x, int y, int width, int height);
    private static native long nativeCreateListView(long parent, int x, int y, int width, int height);
    private static native long nativeCreateScrollArea(long parent, int x, int y, int width, int height);
    private static native long nativeCreateToolBar(long parent, int x, int y, int width, int height);
    private static native long nativeCreateMenuBar(long parent, int x, int y, int width, int height);
    private static native long nativeCreateStatusBar(long parent, String text, int x, int y, int width, int height);

    // Dialogs
    private static native long nativeCreateMessageBox(long parent, String title, String text,
                                                      int x, int y, int width, int height);
    private static native long nativeCreateFileDialog(long parent, String title,
                                                      int x, int y, int width, int height);
    private static native long nativeCreateColorDialog(long parent, String title,
                                                       int x, int y, int width, int height);
    private static native long nativeCreateFontDialog(long parent, String title,
                                                     int x, int y, int width, int height);

    // Menus
    private static native long nativeCreateMenu(long parent, String text, int x, int y, int width, int height);
    private static native boolean nativeAttachMenuBarToWindow(long window, long menuBar);
    private static native long nativeMenuAddItem(long parentMenu, String text, String shortcut);
    private static native long nativePollMenuTriggered();

    // Widget manipulation
    private static native void nativeShowWidget(long widgetId);
    private static native void nativeHideWidget(long widgetId);
    private static native void nativeSetWidgetText(long widgetId, String text);
    private static native String nativeGetWidgetText(long widgetId);
    private static native void nativeSetWidgetEnabled(long widgetId, boolean enabled);
    private static native boolean nativeIsWidgetEnabled(long widgetId);
    private static native void nativeSetWidgetGeometry(long widgetId, int x, int y, int width, int height);

    // Combo box
    private static native boolean nativeComboBoxAddItem(long comboBox, String text);
    private static native boolean nativeComboBoxClearItems(long comboBox);
    private static native boolean nativeComboBoxSetCurrentIndex(long comboBox, int index);
    private static native int nativeComboBoxCurrentIndex(long comboBox);
    private static native int nativeComboBoxItemCount(long comboBox);
    private static native String nativeComboBoxItemText(long comboBox, int index);

    // List box
    private static native boolean nativeListBoxAddItem(long listBox, String text);
    private static native boolean nativeListBoxRemoveItem(long listBox, int index);
    private static native boolean nativeListBoxClearItems(long listBox);
    private static native boolean nativeListBoxSetCurrentIndex(long listBox, int index);
    private static native int nativeListBoxCurrentIndex(long listBox);
    private static native int nativeListBoxItemCount(long listBox);
    private static native String nativeListBoxItemText(long listBox, int index);

    // Events
    private static native long nativePollWidgetTriggered();
    private static native long nativePollWidgetTriggerEvent();

    // Clipboard
    private static native boolean nativeSetClipboardText(String text);
    private static native String nativeGetClipboardText();

    // Platform
    private static native String nativeBackendName();
    private static native int nativePlatformCapabilities();
    private static native int nativeBindingsApiVersion();

    // Memory
    private static native void nativeFreeString(long ptr);

    // ======================================================================
    //  Internal helpers
    // ======================================================================

    private static void checkWidgetId(long id) {
        if (id == INVALID_WIDGET_ID) {
            throw new IllegalArgumentException("Widget ID must not be 0 (INVALID_WIDGET_ID)");
        }
    }
}
