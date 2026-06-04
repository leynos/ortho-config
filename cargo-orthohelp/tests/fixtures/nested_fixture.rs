//! Nested documentation fixture for integration tests.

mod nested_fixture_impl;

use cargo_orthohelp::ir::{
    LocalizedDocMetadata, LocalizedExample, LocalizedFieldMetadata, LocalizedHeadings,
    LocalizedSectionsMetadata,
};
use cargo_orthohelp::schema::{CliMetadata, DefaultValue, EnvMetadata, ValueType, WindowsMetadata};

nested_fixture_impl::define_nested_fixture!();
