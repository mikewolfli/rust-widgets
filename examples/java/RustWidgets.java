// Java JNI facade for the rust_widgets native binding.
public final class RustWidgets {
    static {
        // The host application is responsible for loading the native library.
        // Example: System.loadLibrary("rust_widgets");
    }

    private RustWidgets() {
    }

    // JNI skeleton methods mapped by examples/java/rust_widgets_jni_bridge.c.
    public static native int bindingsApiVersion();
    public static native int javaBindingStatus();
    public static native int jniSkeletonVersion();
    public static native int setRenderAASamplesPerAxis(int samples);
    public static native int getRenderAASamplesPerAxis();
    public static native int setEmbeddedTargetFps(int fps);
    public static native int getEmbeddedTargetFps();
    public static native long submitEmbeddedNoopTask(String label);
    public static native boolean isEmbeddedEngineInitialized();
    public static native boolean isEmbeddedEngineRunning();
    public static native long embeddedEngineFrameCount();
    public static native long embeddedEnginePendingTaskCount();
    public static native long embeddedEngineWindowCount();
    public static native long embeddedEngineButtonCount();

    public static native void init();
    public static native void run();
    public static native void quit();

    public static native long createWindow(String title, int x, int y, int width, int height);
    public static native long createButton(long parent, String text, int x, int y, int width, int height);

    public static native void setWidgetText(long widgetId, String text);
    public static native String getWidgetText(long widgetId);
}
