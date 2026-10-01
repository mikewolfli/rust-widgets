#!/usr/bin/env bash
# Shared JDK discovery for the Android tooling.
#
# # Why this is a library and not a block in each script
#
# Two scripts drive the Android tools — `build_android_testapp.sh` (javac / d8 / apksigner /
# keytool) and `run_android_testapp.sh` (avdmanager) — and **both** need a JDK 17+. The
# resolution lived in the build script only, so the runner called `avdmanager` with whatever
# `java` happened to be on `PATH` and got:
#
#   This tool requires JDK 17 or later. Your version was detected as 11.0.32.1.
#
# That is a second copy of one rule waiting to happen (principle #101), and it already had: the
# build script's search order was fixed while the runner's was not. One function, sourced by
# both, is the shape that cannot drift.
#
# # What it exports
#
#   JDK_HOME  — a JDK root with `bin/javac`, `bin/java` and `bin/keytool`
#   JAVAC     — `$JDK_HOME/bin/javac`
#   JAVA      — `$JDK_HOME/bin/java`
#
# # Why `PATH` alone is not enough
#
# The Android command-line tools ship their own JBR precisely so a system JDK is not required,
# and a host commonly has an *older* `java` earlier on `PATH` (here: Homebrew's 11). So the
# bundled JBRs are checked **before** `PATH`, and the `PATH` fallback is version-checked with a
# message that names the fix rather than letting `d8` fail obscurely later.
#
# Sourced, not executed; call `rw_resolve_jdk` and check its return status.

# Resolves a usable JDK into JDK_HOME/JAVAC/JAVA.
#
# Returns non-zero (with a message on stderr) when nothing usable is found, so a caller under
# `set -e` aborts with the reason rather than continuing with an empty `JAVAC`.
rw_resolve_jdk() {
  local candidates=(
    "${JAVA_HOME:-}"
    "$HOME/Desktop/app/android-studio/jbr"                        # Linux Android Studio
    "/Applications/Android Studio.app/Contents/jbr/Contents/Home" # macOS Android Studio
  )
  local candidate
  JDK_HOME=""
  for candidate in "${candidates[@]}"; do
    if [[ -n "$candidate" && -x "$candidate/bin/javac" ]]; then
      JDK_HOME="$candidate"
      break
    fi
  done

  if [[ -z "$JDK_HOME" ]]; then
    # Take the *root* (one level above `bin`), so `keytool` resolves beside `javac`.
    local path_javac
    path_javac="$(command -v javac || true)"
    if [[ -n "$path_javac" ]]; then
      JDK_HOME="$(cd "$(dirname "$path_javac")/.." && pwd -P)"
    fi
  fi

  if [[ -z "$JDK_HOME" || ! -x "$JDK_HOME/bin/javac" ]]; then
    echo "error: no JDK found (checked JAVA_HOME, both Android Studio JBR locations, and PATH)." >&2
    echo "       The Android build tools need JDK 17+." >&2
    return 2
  fi

  JAVAC="$JDK_HOME/bin/javac"
  JAVA="$JDK_HOME/bin/java"

  # `avdmanager` enforces this itself and aborts with a message that does not name the fix, so
  # the check is made here where it can.
  local major
  major="$("$JAVA" -version 2>&1 | sed -nE 's/.*version "([0-9]+).*/\1/p' | head -1)"
  if [[ -n "$major" && "$major" -lt 17 ]]; then
    echo "error: the JDK at $JDK_HOME is version $major; the Android tools need 17+." >&2
    echo "       Set JAVA_HOME to a newer JDK, or install Android Studio (its bundled JBR qualifies)." >&2
    return 2
  fi

  export JDK_HOME JAVAC JAVA
  # `avdmanager` and the emulator both launch `java` by name, so the resolved JDK has to be the
  # one on `PATH` as well — otherwise this function's answer and the tools' answer disagree,
  # which is the same defect one level down.
  export PATH="$JDK_HOME/bin:$PATH"
  return 0
}
