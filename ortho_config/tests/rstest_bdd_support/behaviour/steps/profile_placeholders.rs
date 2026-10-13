//! Placeholder newtypes shared by the profile BDD steps (`profiles.feature`).
//!
//! `rstest_bdd` supplies every step argument as the string the feature file
//! wrote, so a step signature that takes `String` cannot say whether the
//! parameter it receives is a configuration key, a configuration value, a
//! profile name, or a CLI flag. These newtypes name that intent at the step
//! boundary, where the raw placeholder becomes a value the scenario actually
//! means.
//!
//! The four plain placeholders share one definition because their parsing rule
//! is identical — trim and unquote the Gherkin value. [`BareFlagName`] is hand
//! written because a value-less flag carries its own extra rule: the leading
//! `--` is part of the CLI spelling, not part of the flag's name.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use anyhow::Result;

use super::value_parsing::normalize_scalar;

/// Generates a placeholder newtype over normalized Gherkin text.
///
/// Each generated type dereferences to its inner string so call sites that
/// need a `&str` stay unadorned, while the type itself still prevents a
/// value, a key, and a profile name from being passed in each other's place.
/// It renders as that same string, so an assertion message can name the
/// placeholder it received without a hand-written `as_str()` at every site.
macro_rules! placeholder_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Default)]
        pub(crate) struct $name(String);

        impl $name {
            /// Borrows the normalized placeholder text.
            pub(crate) fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Ok(Self(normalize_scalar(value)))
            }
        }
    };
}

placeholder_newtype!(
    /// A configuration key named by a scenario placeholder.
    ConfigKey
);

placeholder_newtype!(
    /// A configuration value named by a scenario placeholder.
    ConfigValue
);

placeholder_newtype!(
    /// A profile name named by a scenario placeholder.
    ProfileName
);

placeholder_newtype!(
    /// One or more CLI flags supplied as a single scenario placeholder.
    FlagSpec
);

placeholder_newtype!(
    /// The name a feature file gives a profile selection source.
    ///
    /// Kept distinct from a profile name because the two appear side by side
    /// in `the selected profile is {profile} with source {source}`; the step
    /// that parses this one rejects any spelling other than `flag` and the
    /// environment forms.
    SourceName
);

/// Marks a profile entry as a table rather than a scalar.
///
/// A feature file can say a profile *contains* a `cmds` table, but a scalar
/// placeholder has nowhere to put that shape. This sentinel travels through
/// the recorded triple so the file builder can reconstruct the table; the
/// double underscores keep it from colliding with a real key or value.
pub(crate) const TABLE_SENTINEL: &str = "__table__";

/// A single value-less CLI flag named by a scenario placeholder.
///
/// The `--` prefix belongs to the point of use rather than the name, so it is
/// stripped here: a scenario writing `"--verbose"` and one writing `"verbose"`
/// must reach the same flag, and the bare flag's own arm rebuilds the `--`.
#[derive(Debug)]
pub(crate) struct BareFlagName(String);

impl BareFlagName {
    /// Borrows the flag name without its `--` prefix.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for BareFlagName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl FromStr for BareFlagName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let flag = normalize_scalar(value);
        let name = flag.strip_prefix("--").unwrap_or(&flag).to_owned();
        Ok(Self(name))
    }
}
