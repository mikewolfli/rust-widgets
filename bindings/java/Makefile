# Makefile for the RustWidgets Java/JNI bindings
#
# Targets:
#   build       — compile Java sources into .class files
#   run         — compile + run the demo (needs the native library on LD_LIBRARY_PATH)
#   demo        — alias for run
#   clean       — remove generated .class files
#   rebuild     — clean + build
#   check       — check that javac is available
#
# Prerequisites:
#   - JDK 17+  (javac, java)
#   - librust_widgets.so / .dylib / .dll built with `cargo build --features jni`
#
# To run:
#   cd bindings/java
#   make demo RUST_LIB=path/to/target/debug

JAVA_HOME        ?= $(shell dirname $$(dirname $$(readlink -f $$(which javac) 2>/dev/null)) 2>/dev/null)
JAVAC             = $(JAVA_HOME)/bin/javac
JAVA              = $(JAVA_HOME)/bin/java
JAVADOC           = $(JAVA_HOME)/bin/javadoc

SRC_DIR           = .
BUILD_DIR         = build
PACKAGE_DIR       = io/github/rustwidgets

# Java source files
SOURCES           = RustWidgets.java RustWidgetsJNI.java RustWidgetsDemo.java

# Path to the native library directory (override via RUST_LIB or environment)
RUST_LIB         ?= ../../target/debug

# Classpath
CLASSPATH         = $(BUILD_DIR)

# JVM flags: point to the directory containing librust_widgets.so
JVM_FLAGS         = -Djava.library.path=$(RUST_LIB) -cp $(CLASSPATH)

.PHONY: all build run demo clean rebuild check

all: build

# ---- Check prerequisites ------------------------------------------------

check:
	@command -v $(JAVAC) >/dev/null 2>&1 || { \
		echo "Error: javac not found. Set JAVA_HOME or install a JDK."; \
		exit 1; \
	}
	@echo "JAVA_HOME: $(JAVA_HOME)"
	@echo "JAVAC:     $(JAVAC)"

# ---- Compile Java sources -----------------------------------------------

build: check
	@mkdir -p $(BUILD_DIR)
	$(JAVAC) -d $(BUILD_DIR) $(SOURCES)
	@echo "--- Build complete: classes in $(BUILD_DIR)/$(PACKAGE_DIR)/ ---"
	@ls -1 $(BUILD_DIR)/$(PACKAGE_DIR)/*.class

# ---- Run demo -----------------------------------------------------------

run: build
	@echo "--- Running RustWidgetsDemo (native lib in $(RUST_LIB)) ---"
	$(JAVA) $(JVM_FLAGS) io.github.rustwidgets.RustWidgetsDemo

demo: run

# ---- Generate Javadoc ---------------------------------------------------

javadoc: build
	@mkdir -p $(BUILD_DIR)/javadoc
	$(JAVADOC) -d $(BUILD_DIR)/javadoc -sourcepath $(SRC_DIR) -subpackages io.github.rustwidgets
	@echo "--- Javadoc generated at $(BUILD_DIR)/javadoc/index.html ---"

# ---- Clean --------------------------------------------------------------

clean:
	rm -rf $(BUILD_DIR)
	@echo "--- Cleaned ---"

rebuild: clean build

# ---- Help ---------------------------------------------------------------

help:
	@echo "Targets:"
	@echo "  build    — compile Java classes"
	@echo "  run      — build + run demo (set RUST_LIB to the .so/.dylib dir)"
	@echo "  demo     — alias for run"
	@echo "  clean    — delete build output"
	@echo "  rebuild  — clean + build"
	@echo ""
	@echo "Variables:"
	@echo "  RUST_LIB — path to directory with librust_widgets.so (default: ../../target/debug)"
	@echo "  JAVA_HOME — JDK installation root (auto-detected if possible)"
