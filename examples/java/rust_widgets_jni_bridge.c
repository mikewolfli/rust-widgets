// JNI bridge implementation for Java access to rust_widgets C ABI.
#include <jni.h>
#include "../rust_widgets.h"

// JNI bridge skeleton delegating to the stable C ABI.

JNIEXPORT jint JNICALL Java_RustWidgets_bindingsApiVersion(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jint)rust_widgets_bindings_api_version();
}

JNIEXPORT jint JNICALL Java_RustWidgets_javaBindingStatus(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jint)rust_widgets_java_binding_status();
}

JNIEXPORT jint JNICALL Java_RustWidgets_jniSkeletonVersion(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jint)rust_widgets_java_jni_skeleton_version();
}

JNIEXPORT jint JNICALL Java_RustWidgets_setRenderAASamplesPerAxis(JNIEnv* env, jclass cls, jint samples) {
    (void)env;
    (void)cls;
    return (jint)rust_widgets_set_render_aa_samples_per_axis((unsigned int)samples);
}

JNIEXPORT jint JNICALL Java_RustWidgets_getRenderAASamplesPerAxis(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jint)rust_widgets_get_render_aa_samples_per_axis();
}

JNIEXPORT jint JNICALL Java_RustWidgets_setEmbeddedTargetFps(JNIEnv* env, jclass cls, jint fps) {
    (void)env;
    (void)cls;
    return (jint)rust_widgets_set_embedded_target_fps((unsigned int)fps);
}

JNIEXPORT jint JNICALL Java_RustWidgets_getEmbeddedTargetFps(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jint)rust_widgets_get_embedded_target_fps();
}

JNIEXPORT jlong JNICALL Java_RustWidgets_submitEmbeddedNoopTask(JNIEnv* env, jclass cls, jstring label) {
    (void)cls;
    const char* label_utf8 = (*env)->GetStringUTFChars(env, label, 0);
    const uint64_t task_id = rust_widgets_submit_embedded_noop_task(label_utf8);
    (*env)->ReleaseStringUTFChars(env, label, label_utf8);
    return (jlong)task_id;
}

JNIEXPORT jboolean JNICALL Java_RustWidgets_isEmbeddedEngineInitialized(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jboolean)rust_widgets_embedded_engine_is_initialized();
}

JNIEXPORT jboolean JNICALL Java_RustWidgets_isEmbeddedEngineRunning(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jboolean)rust_widgets_embedded_engine_is_running();
}

JNIEXPORT jlong JNICALL Java_RustWidgets_embeddedEngineFrameCount(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jlong)rust_widgets_embedded_engine_frame_count();
}

JNIEXPORT jlong JNICALL Java_RustWidgets_embeddedEnginePendingTaskCount(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jlong)rust_widgets_embedded_engine_pending_task_count();
}

JNIEXPORT jlong JNICALL Java_RustWidgets_embeddedEngineWindowCount(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jlong)rust_widgets_embedded_engine_window_count();
}

JNIEXPORT jlong JNICALL Java_RustWidgets_embeddedEngineButtonCount(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    return (jlong)rust_widgets_embedded_engine_button_count();
}

JNIEXPORT void JNICALL Java_RustWidgets_init(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    rust_widgets_init();
}

JNIEXPORT void JNICALL Java_RustWidgets_run(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    rust_widgets_run();
}

JNIEXPORT void JNICALL Java_RustWidgets_quit(JNIEnv* env, jclass cls) {
    (void)env;
    (void)cls;
    rust_widgets_quit();
}

JNIEXPORT jlong JNICALL Java_RustWidgets_createWindow(
    JNIEnv* env,
    jclass cls,
    jstring title,
    jint x,
    jint y,
    jint width,
    jint height
) {
    (void)cls;
    const char* title_utf8 = (*env)->GetStringUTFChars(env, title, 0);
    const uint64_t id = rust_widgets_create_window(
        title_utf8,
        (int)x,
        (int)y,
        (unsigned int)width,
        (unsigned int)height
    );
    (*env)->ReleaseStringUTFChars(env, title, title_utf8);
    return (jlong)id;
}

JNIEXPORT jlong JNICALL Java_RustWidgets_createButton(
    JNIEnv* env,
    jclass cls,
    jlong parent,
    jstring text,
    jint x,
    jint y,
    jint width,
    jint height
) {
    (void)cls;
    const char* text_utf8 = (*env)->GetStringUTFChars(env, text, 0);
    const uint64_t id = rust_widgets_create_button(
        (uint64_t)parent,
        text_utf8,
        (int)x,
        (int)y,
        (unsigned int)width,
        (unsigned int)height
    );
    (*env)->ReleaseStringUTFChars(env, text, text_utf8);
    return (jlong)id;
}

JNIEXPORT void JNICALL Java_RustWidgets_setWidgetText(JNIEnv* env, jclass cls, jlong widgetId, jstring text) {
    (void)cls;
    const char* text_utf8 = (*env)->GetStringUTFChars(env, text, 0);
    rust_widgets_set_widget_text((uint64_t)widgetId, text_utf8);
    (*env)->ReleaseStringUTFChars(env, text, text_utf8);
}

JNIEXPORT jstring JNICALL Java_RustWidgets_getWidgetText(JNIEnv* env, jclass cls, jlong widgetId) {
    (void)cls;
    const char* value = rust_widgets_get_widget_text((uint64_t)widgetId);
    if (value == NULL) {
        return (*env)->NewStringUTF(env, "");
    }
    jstring out = (*env)->NewStringUTF(env, value);
    rust_widgets_free_string(value);
    return out;
}
