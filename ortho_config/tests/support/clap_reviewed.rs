//! Reviewed source-of-truth data for the supported `clap` error inventory.
//!
//! Both the matrix inventory test and the pre-publication coverage gate read
//! this module, so the reviewed total cannot drift between them. When a new
//! `clap_builder` variant is reviewed, update this constant and
//! [`CLAP_ERROR_IDS`](ortho_config::CLAP_ERROR_IDS) together; the matrix test
//! fails to compile if only one side moves.

/// Total `ErrorKind` variants declared by the supported `clap_builder` 4.6.7.
pub(crate) const CLAP_ERROR_KIND_COUNT: usize = 17;
