//! Nested documentation fixture shared by renderer unit tests.

#[path = "../../tests/fixtures/nested_fixture_impl.rs"]
mod nested_fixture_impl;

use crate::ir::{
    LocalizedDocMetadata, LocalizedExample, LocalizedFieldMetadata, LocalizedHeadings,
    LocalizedSectionsMetadata,
};
use crate::schema::{CliMetadata, DefaultValue, EnvMetadata, ValueType, WindowsMetadata};

nested_fixture_impl::define_nested_fixture!();
