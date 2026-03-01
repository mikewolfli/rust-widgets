# rust_widgets Java JNI examples

This directory contains Java JNI facade and runnable demo sources.

## Files

- `RustWidgets.java`: JNI native method declarations.
- `rust_widgets_jni_bridge.c`: JNI bridge implementation mapping to the C ABI.
- `RustWidgetsEmbeddedEngineDemo.java`: embedded engine control/stats demo.

## Compile Java demo

```bash
javac examples/java/RustWidgets.java examples/java/RustWidgetsEmbeddedEngineDemo.java
```

## Run Java demo (macOS)

Build the Rust dynamic library first:

```bash
cargo build
```

Run with class path and dynamic library path:

```bash
java -cp examples/java -Djava.library.path=target/debug RustWidgetsEmbeddedEngineDemo
```
