//! A reviewed total that disagrees with the shipped matrix must not compile.
//!
//! `CLAP_ERROR_IDS` and the reviewed total are tied together by
//! `const_assert_eq!` in the matrix suite, so a row added or dropped without a
//! human review of the resolved `clap_builder` source fails the build. This
//! fixture asserts the same macro against a drifted total, which is what that
//! failure looks like: the comparison is a constant, so the diagnostic is
//! `E0080` and no runtime test could have been silently skipped instead.

use ortho_config::CLAP_ERROR_IDS;
use static_assertions::const_assert_eq;

// The shipped matrix holds 17 rows. 18 is the total a reviewer would leave
// behind after adding a row without reviewing the resolved source.
const_assert_eq!(CLAP_ERROR_IDS.len(), 18);

fn main() {}