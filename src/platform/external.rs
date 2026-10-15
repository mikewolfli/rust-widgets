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

    /// A real program with a valid target is started, and the call returns without waiting
    /// for it. `true` is present on every Unix host CI runs on.
    #[cfg(unix)]
    #[test]
    fn a_present_handler_is_started_without_waiting() {
        assert_eq!(spawn_handler("true", &[], "some-target"), Ok(()));
    }
}
