//! Tests validating `normalise_cycle_key` behaviour across platforms.

use super::super::path::normalise_cycle_key;
use super::assert_normalise_cycle_key;
use anyhow::{Result, ensure};
use rstest::rstest;
use std::path::{Path, PathBuf};

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn normalise_cycle_key_is_noop_on_case_sensitive_platforms() -> Result<()> {
    let path = PathBuf::from("/tmp/Config.toml");
    let normalised = normalise_cycle_key(&path);
    let actual = normalised;
    ensure!(actual == path, "expected {path:?}, got {actual:?}");

    let unicode_mixed_case = PathBuf::from("/tmp/Café.toml");
    let unicode_upper_case = PathBuf::from("/tmp/CAFÉ.toml");
    ensure!(
        normalise_cycle_key(&unicode_mixed_case) == unicode_mixed_case,
        "unicode path normalisation changed value"
    );
    ensure!(
        normalise_cycle_key(&unicode_upper_case) == unicode_upper_case,
        "unicode uppercase path normalisation changed value"
    );

    let special_chars = PathBuf::from("/tmp/config-!@#.toml");
    ensure!(
        normalise_cycle_key(&special_chars) == special_chars,
        "special character path normalisation changed value"
    );

    let non_ascii = PathBuf::from("/tmp/конфиг.toml");
    ensure!(
        normalise_cycle_key(&non_ascii) == non_ascii,
        "Cyrillic path normalisation changed value"
    );
    Ok(())
}

#[rstest]
#[case::absolute_paths(
    r"C:\\Temp\\Config.toml",
    r"c:\\temp\\config.toml",
    "/tmp/Config.toml",
    "/tmp/config.toml"
)]
#[case::relative_paths(
    r".\\Temp\\Config.toml",
    r".\\temp\\config.toml",
    "./Temp/Config.toml",
    "./temp/config.toml"
)]
#[case::redundant_separators(
    r"C://Temp//Config.toml",
    r"c:\\temp\\config.toml",
    "/tmp//Nested//Config.toml",
    "/tmp/nested/config.toml"
)]
#[cfg_attr(
    not(any(windows, target_os = "macos")),
    ignore = "case-insensitive normalisation applies only on Windows and macOS"
)]
fn normalise_cycle_key_case_insensitive_scenarios(
    #[case] windows_input: &str,
    #[case] windows_expected: &str,
    #[case] unix_input: &str,
    #[case] unix_expected: &str,
) -> Result<()> {
    assert_normalise_cycle_key(windows_input, windows_expected, unix_input, unix_expected)?;
    Ok(())
}

#[cfg_attr(
    not(any(windows, target_os = "macos")),
    ignore = "case-insensitive normalisation applies only on Windows and macOS"
)]
#[test]
fn normalise_cycle_key_handles_unicode_and_special_characters() -> Result<()> {
    if cfg!(windows) {
        let unicode = PathBuf::from(r"C:\\Temp\\CAFÉ.toml");
        let special = PathBuf::from(r"C:\\Temp\\Config-!@#.toml");
        ensure!(
            normalise_cycle_key(&unicode) == Path::new(r"c:\\temp\\cafÉ.toml"),
            "unexpected unicode normalisation"
        );
        ensure!(
            normalise_cycle_key(&special) == Path::new(r"c:\\temp\\config-!@#.toml"),
            "unexpected special character normalisation"
        );
    } else {
        let unicode = PathBuf::from("/tmp/CAFÉ.toml");
        let special = PathBuf::from("/tmp/Config-!@#.toml");
        ensure!(
            normalise_cycle_key(&unicode) == Path::new("/tmp/café.toml"),
            "unexpected unicode normalisation"
        );
        ensure!(
            normalise_cycle_key(&special) == Path::new("/tmp/config-!@#.toml"),
            "unexpected special character normalisation"
        );
    }
    Ok(())
}
