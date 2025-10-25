//! Tests exercising optional flag handling and short-flag collisions.

use super::common::{OrthoResultExt, ConflictConfig, OptionConfig};
use anyhow::{ensure, Result};
use rstest::rstest;

#[rstest]
#[case::present(&["prog", "--maybe", "5"], Some(5))]
#[case::absent(&["prog"], None)]
fn parses_option_field(#[case] args: &[&str], #[case] expected: Option<u32>) -> Result<()> {
    let cfg = OptionConfig::load_from_iter(args.iter().copied()).to_anyhow()?;
    ensure!(cfg.maybe == expected, "expected maybe {:?}, got {:?}", expected, cfg.maybe);
    Ok(())
}

#[rstest]
fn resolves_short_flag_conflict() -> Result<()> {
    let cfg = ConflictConfig::load_from_iter(["prog", "-s", "one", "-S", "two"])
        .to_anyhow()?;
    ensure!(
        cfg.second.as_deref() == Some("one"),
        "expected second one, got {:?}",
        cfg.second
    );
    ensure!(
        cfg.sample.as_deref() == Some("two"),
        "expected sample two, got {:?}",
        cfg.sample
    );
    Ok(())
}
