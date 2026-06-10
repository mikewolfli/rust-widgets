package io.github.rustwidgets;

/**
 * Demonstration and integration test for the {@code rust_widgets} Java/JNI binding.
 *
 * <p>This demo creates a window with several widgets, exercises the API, and
 * then cleans up. Run with:
 * <pre>{@code
 *   # From the bindings/java/ directory:
 *   make demo
 * }</pre>
 *
 * <p>It uses {@link RustWidgets} directly. To test the low-level
 * {@link RustWidgetsJNI} API, substitute calls as needed.
 */
public final class RustWidgetsDemo {

    private RustWidgetsDemo() {
        throw new AssertionError("No instances");
    }

    /**
     * Run the demo.
     *
     * @param args ignored
     */
    public static void main(String[] args) {
        System.out.println("=== RustWidgets Java Demo ===");

        // ---- Platform info ----
        System.out.println("Bindings API version: " + RustWidgets.bindingsApiVersion());
        System.out.println("Backend name: " + RustWidgets.backendName());
        int caps = RustWidgets.platformCapabilities();
        System.out.println("Platform capabilities: 0x" + Integer.toHexString(caps));
        if ((caps & RustWidgets.CAP_DPI_SCALING) != 0) {
            System.out.println("  - DPI scaling supported");
        }
        if ((caps & RustWidgets.CAP_IME) != 0) {
            System.out.println("  - IME supported");
        }
        if ((caps & RustWidgets.CAP_NATIVE_MENU) != 0) {
            System.out.println("  - Native menus supported");
        }

        // ---- Lifecycle ----
        RustWidgets.init();
        System.out.println("Library initialised.");

        // ---- Create a window ----
        long window = RustWidgets.createWindow("RustWidgets Java Demo", 100, 100, 640, 480);
        System.out.println("Created window id=" + window);

        // ---- Create widgets ----
        long label = RustWidgets.createLabel(window, "Hello from Java!", 10, 10, 200, 24);
        System.out.println("Created label id=" + label);

        long button = RustWidgets.createButton(window, "Click Me", 10, 40, 120, 32);
        System.out.println("Created button id=" + button);

        long checkbox = RustWidgets.createCheckbox(window, "Enable feature", 10, 80, 160, 24);
        System.out.println("Created checkbox id=" + checkbox);

        long slider = RustWidgets.createSlider(window, 10, 110, 200, 24);
        System.out.println("Created slider id=" + slider);

        long comboBox = RustWidgets.createComboBox(window, 10, 140, 160, 28);
        System.out.println("Created combo-box id=" + comboBox);

        long listBox = RustWidgets.createListBox(window, 10, 180, 160, 100);
        System.out.println("Created list-box id=" + listBox);

        // ---- Exercise widget manipulation ----
        RustWidgets.setWidgetText(label, "Updated label text");
        String text = RustWidgets.getWidgetText(label);
        System.out.println("Label text: \"" + text + "\"");

        RustWidgets.setWidgetEnabled(button, false);
        System.out.println("Button enabled: " + RustWidgets.isWidgetEnabled(button));
        RustWidgets.setWidgetEnabled(button, true);

        // ---- Combo box items ----
        RustWidgets.comboBoxAddItem(comboBox, "Option A");
        RustWidgets.comboBoxAddItem(comboBox, "Option B");
        RustWidgets.comboBoxAddItem(comboBox, "Option C");
        RustWidgets.comboBoxSetCurrentIndex(comboBox, 0);
        System.out.println("Combo box items: " + RustWidgets.comboBoxItemCount(comboBox));
        System.out.println("  Item 0: " + RustWidgets.comboBoxItemText(comboBox, 0));
        System.out.println("  Current index: " + RustWidgets.comboBoxCurrentIndex(comboBox));

        // ---- List box items ----
        RustWidgets.listBoxAddItem(listBox, "Item 1");
        RustWidgets.listBoxAddItem(listBox, "Item 2");
        RustWidgets.listBoxAddItem(listBox, "Item 3");
        System.out.println("List box items: " + RustWidgets.listBoxItemCount(listBox));
        for (int i = 0; i < RustWidgets.listBoxItemCount(listBox); i++) {
            System.out.println("  [" + i + "] " + RustWidgets.listBoxItemText(listBox, i));
        }

        // ---- Clipboard ----
        RustWidgets.setClipboardText("Hello from rust_widgets!");
        String clip = RustWidgets.getClipboardText();
        System.out.println("Clipboard: \"" + clip + "\"");

        // ---- Show window and run ----
        RustWidgets.showWidget(window);
        System.out.println("=== Starting event loop (close window to stop) ===");

        // Simple polling event loop
        long lastPoll = System.currentTimeMillis();
        while (true) {
            RustWidgets.TriggerEvent event = RustWidgets.pollWidgetTriggerEvent();
            if (event != null) {
                System.out.println("Event: widgetId=" + event.widgetId()
                    + ", kind=" + kindName(event.kind()));

                // If the button was clicked, quit
                if (event.widgetId() == button && event.kind() == RustWidgets.TRIGGER_CLICKED) {
                    System.out.println("Button clicked — quitting.");
                    RustWidgets.quit();
                    break;
                }
            }

            // Also check simple polling
            long triggered = RustWidgets.pollWidgetTriggered();
            if (triggered != RustWidgets.INVALID_WIDGET_ID) {
                System.out.println("Triggered (simple): widgetId=" + triggered);
            }

            // Small sleep to avoid busy-looping
            try {
                Thread.sleep(16); // ~60fps polling rate
            } catch (InterruptedException ignored) {
                Thread.currentThread().interrupt();
                break;
            }

            // Timeout after 30 seconds for automated testing
            if (System.currentTimeMillis() - lastPoll > 30_000) {
                System.out.println("Timeout reached.");
                break;
            }
        }

        System.out.println("=== Demo complete ===");
    }

    /** Map trigger kind code to a human-readable name. */
    private static String kindName(int kind) {
        switch (kind) {
            case RustWidgets.TRIGGER_CLICKED:           return "CLICKED";
            case RustWidgets.TRIGGER_VALUE_CHANGED:     return "VALUE_CHANGED";
            case RustWidgets.TRIGGER_SELECTION_CHANGED: return "SELECTION_CHANGED";
            case RustWidgets.TRIGGER_CLOSED:            return "CLOSED";
            default:                                    return "UNKNOWN(" + kind + ")";
        }
    }
}
