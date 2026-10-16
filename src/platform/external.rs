// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared implementation of [`Platform::open_external`] for the desktop families.
//!
//! # Why one helper instead of one body per backend
//!
//! The three desktop families differ in exactly one thing: the name of the program that opens
//! a target (`open`, `xdg-open`, `cmd /C start`). The part that is easy to get wrong — refusing
//! a target the handler would read as its own option, and telling "no handler installed" apart
//! from "the request was refused" — is identical on all three. Keeping that part here means the
//! backends state their program name and nothing else (principles #36/#54).
//!
//! # What the validation is not
//!
//! It is not a shell-quoting defence. The target is passed as a single `argv` element and no
//! shell is involved, so a target containing spaces or quotes is simply one argument. What it
//! guards against is the handler's *own* option parser: `xdg-open -` and `xdg-open -- /path`
//! both fail with "unexpected option", so a target that begins with `-` must be refused here
//! rather than handed over.

use crate::platform::types::OpenError;
use std::path::Path;

/// Whether `program` can be found on this process's `PATH`.
///
/// # Why a `PATH` scan rather than a spawn
///
/// The question this answers is "is the host handler installed", and it must be answerable
/// **without side effects**. The spawn-based rule principle #38 prescribes for the print clients is
/// right when the probe is a precondition of the operation itself; here the probe *would be* the
/// operation — running `xdg-open`/`open` to see whether it exists hands a real target to a real
/// application. Reading `PATH` touches no process, which is exactly the "ask without opening" the
/// designer's request ([`Platform::has_external_handler`]) needs.
///
/// # Windows
///
/// `cmd` is `cmd.exe`, resolved through `PATHEXT`, so on Windows each directory is tried with the
/// bare name first and then with each `PATHEXT` suffix. `std::env::split_paths` supplies the
/// platform's own `PATH` separator, so no `:`/`;` assumption is made here (principle #36).
///
/// [`Platform::has_external_handler`]: crate::platform::Platform::has_external_handler
pub(crate) fn program_is_on_path(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        if dir.as_os_str().is_empty() {
            return false;
        }
        if is_executable_file(&dir.join(program)) {
            return true;
        }
        #[cfg(windows)]
        {
            if let Some(exts) = std::env::var_os("PATHEXT") {
                for ext in exts.to_string_lossy().split(';') {
                    if ext.is_empty() {
                        continue;
                    }
                    let mut name = std::string::String::from(program);
                    name.push_str(ext);
                    if is_executable_file(&dir.join(name)) {
                        return true;
                    }
                }
            }
        }
        false
    })
}

/// Whether `path` names an existing file the OS would consider runnable.
///
/// On Unix that means a regular file with an execute bit set; on Windows existence as a file is the
/// whole test (there is no execute bit, and `PATHEXT` resolution is handled by the caller).
fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Rejects targets the host handler would treat as its own option.
pub(crate) fn validate_target(target: &str) -> Result<(), OpenError> {
    if target.is_empty() || target.starts_with('-') {
        return Err(OpenError::InvalidTarget);
    }
    Ok(())
}

/// Spawns `program` with `prefix_args` (if any) followed by `target`, without waiting for it.
///
/// `prefix_args` exists for the one handler that needs them: Windows opens through
/// `cmd /C start "" <target>`, whose `start` builtin takes a window title before the target.
///
/// Returns as soon as the child exists: the handler is a long-lived GUI process, so waiting
/// would block the caller on the user's session. `Ok(())` therefore means "the handler was
/// started", not "the target was opened".
///
/// A spawn failure is reported as [`OpenError::NoHandler`]: the only realistic cause is that
/// `program` is not installed, and the handler's own exit status is not observable without
/// waiting, which this deliberately does not do.
pub(crate) fn spawn_handler(
    program: &str,
    prefix_args: &[&str],
    target: &str,
) -> Result<(), OpenError> {
    validate_target(target)?;
    let mut command = std::process::Command::new(program);
    command
        .args(prefix_args)
        .arg(target)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // The handler gets its own process group, which is what keeps the two ownership sets
    // disjoint in both directions: a host that ends "the process groups this session created"
    // does not kill the browser it asked the library to open, and the library does not put a
    // long-lived user application in the group the host will tear down. See
    // [`super::child_process`] for the boundary this states.
    super::child_process::in_own_group(&mut command);
    command.spawn().map(|_child| ()).map_err(|_error| OpenError::NoHandler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_option_shaped_or_empty_target_is_refused_before_spawning() {
        assert_eq!(validate_target(""), Err(OpenError::InvalidTarget));
        assert_eq!(validate_target("-"), Err(OpenError::InvalidTarget));
        assert_eq!(validate_target("--help"), Err(OpenError::InvalidTarget));
        assert_eq!(validate_target("https://example.com"), Ok(()));
        assert_eq!(validate_target("/tmp/a b.txt"), Ok(()));
    }

    /// A program that is not installed is reported as "no handler", not as success.
    #[test]
    fn a_missing_handler_is_reported_as_no_handler() {
        let missing = "/nonexistent/rw-open-handler-for-test";
        assert_eq!(spawn_handler(missing, &[], "/tmp/x"), Err(OpenError::NoHandler));
    }

    /// The target is validated before the program is even considered.
    #[test]
    fn validation_precedes_the_spawn_attempt() {
        let missing = "/nonexistent/rw-open-handler-for-test";
        assert_eq!(spawn_handler(missing, &[], "--version"), Err(OpenError::InvalidTarget));
    }

    /// A program that is present on `PATH` is reported present, and one that is not is not.
    ///
    /// The "present" program is per-platform: `true` on Unix, `cmd` on Windows. The
    /// `/nonexistent` name is deliberately absolute so it can never appear in any directory on
    /// `PATH`.
    #[test]
    fn the_handler_existence_query_answers_without_running_anything() {
        let present = if cfg!(windows) { "cmd" } else { "true" };
        assert!(program_is_on_path(present), "`{present}` is a standard PATH member on this host");
        assert!(
            !program_is_on_path("/nonexistent/rw-open-handler-for-test"),
            "an absolute path that does not exist must not be reported as a handler"
        );
    }

    /// A real program with a valid target is started, and the call returns without waiting
    /// for it. `true` is present on every Unix host CI runs on.
    #[cfg(unix)]
    #[test]
    fn a_present_handler_is_started_without_waiting() {
        assert_eq!(spawn_handler("true", &[], "some-target"), Ok(()));
    }
}
