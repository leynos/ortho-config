//! The directory walk that decides whether entries survive an enumeration error.
//!
//! `read_listing` is the only code that turns a directory that fails part-way
//! into a listing the scan can still use, and its behaviour is not reachable
//! from a fake: the fake hands over a listing that is already complete or
//! already partial, so a mutation that discarded the entries read before a
//! fault would survive every test in `lock_holder_tests`. Driving
//! `collect_until_fault` directly is what pins the rule where it is decided.
//!
//! Split from the procfs module's own file because the cap counts every line a
//! module declares, and the reader plus these cases no longer fit together.

use anyhow::Result;

use super::lock_procfs::collect_until_fault;

/// Verifies a listing that fails part-way keeps every entry it already read.
///
/// This is the defect the rule exists for. Directory iteration can fail after
/// it has started, and the entries it yielded are real observations; a listing
/// that returned only the error would discard a descriptor that may be the one
/// explaining a hold. The fault must still be reported, so the scan is not
/// passed off as complete.
///
/// The fault is placed after two entries rather than immediately, so a reader
/// that kept only the *first* entry, or only those before some other cutoff,
/// fails here too.
#[test]
fn a_fault_keeps_every_entry_read_before_it() -> Result<()> {
    use anyhow::ensure;

    let mut script = ["fd 7", "fd 8", "boom"].into_iter();
    let collected = collect_until_fault(
        || match script.next()? {
            "boom" => Some(Err("Stale file handle")),
            name => Some(Ok(name)),
        },
        |error| format!("/proc/1/fd: an entry could not be read ({error})"),
    );

    ensure!(
        collected.items == ["fd 7", "fd 8"],
        "every entry read before the fault must survive it, or the report \
         discards evidence it gathered; got {:?}",
        collected.items,
    );
    let fault = collected
        .fault
        .as_deref()
        .expect("a listing that stopped early must report why");
    ensure!(
        fault.contains("Stale file handle"),
        "the fault must carry the error that caused it: {fault}",
    );
    ensure!(
        fault.contains("/proc/1/fd"),
        "the fault must name the directory it was reading: {fault}",
    );
    Ok(())
}

/// Verifies a listing that runs to the end reports no fault.
///
/// The counterpart that keeps the test above from passing vacuously: if a
/// reader returned `Some(fault)` unconditionally, or treated the end of the
/// listing as an error, the retention case would still see its two entries and
/// the scan would report every complete listing as partial.
#[test]
fn a_listing_that_finishes_reports_no_fault() -> Result<()> {
    use anyhow::ensure;

    let mut script = ["fd 3"].into_iter();
    let collected = collect_until_fault(
        || script.next().map(Ok::<_, &str>),
        |error: &str| error.to_owned(),
    );

    ensure!(
        collected.items == ["fd 3"],
        "the entries read must all survive a listing that finished: {:?}",
        collected.items,
    );
    ensure!(
        collected.fault.is_none(),
        "a listing that reached the end has no fault to report: {:?}",
        collected.fault,
    );
    Ok(())
}
