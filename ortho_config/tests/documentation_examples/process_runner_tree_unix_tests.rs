//! Regressions for transient Linux `/proc` entries.

use super::read_proc_stat;
use std::io;

#[test]
fn vanished_proc_entries_are_ignored() {
    let errors = [
        io::Error::from(io::ErrorKind::NotFound),
        io::Error::from_raw_os_error(nix::libc::ESRCH),
    ];

    for error in errors {
        let result = read_proc_stat(|| Err(error));
        assert!(result.is_ok_and(|observed_stat| observed_stat.is_none()));
    }
}

#[test]
fn unexpected_proc_errors_are_preserved() {
    let stat = read_proc_stat(|| Err(io::Error::from(io::ErrorKind::PermissionDenied)));
    assert!(stat.is_err_and(|error| error.kind() == io::ErrorKind::PermissionDenied));
}
