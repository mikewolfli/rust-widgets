// Java embedded-engine demo for FPS control and runtime stats.
public final class RustWidgetsEmbeddedEngineDemo {
    private RustWidgetsEmbeddedEngineDemo() {
    }

    public static void main(String[] args) {
        // Ensure rust_widgets native library is loadable from java.library.path.
        System.loadLibrary("rust_widgets");
        int abiVersion = RustWidgets.bindingsApiVersion();

        RustWidgets.init();

        int appliedFps = RustWidgets.setEmbeddedTargetFps(90);
        int targetFps = RustWidgets.getEmbeddedTargetFps();
        long taskId = RustWidgets.submitEmbeddedNoopTask("java-embedded-noop");

        long window = RustWidgets.createWindow("Embedded Engine Java Demo", 120, 120, 480, 300);
        long button = RustWidgets.createButton(window, "OK", 24, 24, 96, 36);

        boolean initialized = RustWidgets.isEmbeddedEngineInitialized();
        boolean running = RustWidgets.isEmbeddedEngineRunning();
        long frameCount = RustWidgets.embeddedEngineFrameCount();
        long pendingTaskCount = RustWidgets.embeddedEnginePendingTaskCount();
        long windowCount = RustWidgets.embeddedEngineWindowCount();
        long buttonCount = RustWidgets.embeddedEngineButtonCount();

        System.out.println("DEMO_PROFILE=embedded");
        System.out.println("ABI_VERSION=" + abiVersion);
        System.out.println("TARGET_FPS=" + targetFps);
        System.out.println("APPLIED_FPS=" + appliedFps);
        System.out.println("TASK_ID=" + taskId);
        System.out.println("WINDOW_ID=" + window);
        System.out.println("BUTTON_ID=" + button);
        System.out.println("ENGINE_INITIALIZED=" + (initialized ? 1 : 0));
        System.out.println("ENGINE_RUNNING=" + (running ? 1 : 0));
        System.out.println("FRAME_COUNT=" + frameCount);
        System.out.println("PENDING_TASK_COUNT=" + pendingTaskCount);
        System.out.println("WINDOW_COUNT=" + windowCount);
        System.out.println("BUTTON_COUNT=" + buttonCount);

        RustWidgets.quit();
    }
}