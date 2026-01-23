//! Newtype wrappers for documentation metadata inputs.

use std::ops::Deref;

/// Generates a string-backed newtype with common trait implementations.
///
/// This macro generates:
/// - The struct definition with `#[derive(Debug, Clone, PartialEq, Eq)]`
/// - A `new(String) -> Self` constructor
/// - `Deref<Target = str>` implementation
/// - `From<String>` implementation
/// - `From<&str>` implementation
/// - `AsRef<str>` implementation
macro_rules! impl_string_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct $name(String);

        impl $name {
            #[expect(
                clippy::missing_const_for_fn,
                reason = "Newtype constructor is not used in const context."
            )]
            pub(crate) fn new(value: String) -> Self {
                Self(value)
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

impl_string_newtype!(AppName);
impl_string_newtype!(ConfigFileName);
