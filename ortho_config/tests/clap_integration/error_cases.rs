//! Error handling scenarios for CLI parsing.

use super::common::{with_jail, OrthoError, OptionConfig, RequiredConfig, TestConfig};
use anyhow::{anyhow, ensure, Result};
use rstest::rstest;

#[rstest]
#[case::unknown_flag(&["prog", "--bogus"])]
#[case::duplicate_flag(&["prog", "--sample-value", "foo", "--sample-value", "bar"])]
fn rejects_cli_parsing_errors(#[case] args: &[&str]) -> Result<()> {
    let err = match TestConfig::load_from_iter(args.iter().copied()) {
        Ok(cfg) => return Err(anyhow!("expected CLI parsing error, got config {:?}", cfg)),
        Err(err) => err,
    };
    ensure!(
        matches!(&*err, OrthoError::CliParsing(_)),
        "expected CLI parsing error, got {:?}",
        err
    );
    Ok(())
}

#[rstest]
fn option_field_rejects_invalid_value() -> Result<()> {
    let err = match OptionConfig::load_from_iter(["prog", "--maybe", "notanumber"]) {
        Ok(cfg) => {
            return Err(anyhow!(
                "expected CLI parsing failure, got config {:?}",
                cfg
            ));
        }
        Err(err) => err,
    };
    ensure!(
        matches!(&*err, OrthoError::CliParsing(_)),
        "expected CLI parsing error, got {:?}",
        err
    );
    Ok(())
}

#[rstest]
fn missing_required_field_surfaces_merge_error() -> Result<()> {
    with_jail(|_| match RequiredConfig::load_from_iter(["prog"]) {
        Ok(cfg) => Err(anyhow!(
            "expected merge error for missing config, got {:?}",
            cfg
        )),
        Err(err) => {
            ensure!(
                matches!(&*err, OrthoError::Merge { .. }),
                "expected merge error, got {:?}",
                err
            );
            Ok(())
        }
    })
}
