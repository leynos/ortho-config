//! Tests for clap declaration shapes, casing, and requiredness.

use super::{declaration, declaration_with};
use crate::derive::parse::ClapRenameAll;

#[test]
fn container_casing_rewrites_only_derived_spellings() {
    let cased = declaration_with(
        &quote::quote! {
            #[arg(long, short)]
            output_file: String
        },
        ClapRenameAll::Snake,
    );
    assert_eq!(
        cased.long.as_deref(),
        Some("output_file"),
        "snake_case casing should reach a derived long flag"
    );
    assert_eq!(
        cased.short,
        Some('o'),
        "snake_case casing should also reach a derived short flag"
    );
    assert_eq!(
        cased.value_name, "OUTPUT_FILE",
        "clap keeps the value name in upper snake case regardless of casing"
    );

    let literal = declaration_with(
        &quote::quote! {
            #[arg(long = "output-file")]
            output_file: String
        },
        ClapRenameAll::Snake,
    );
    assert_eq!(
        literal.long.as_deref(),
        Some("output-file"),
        "a literal long spelling must stay verbatim"
    );

    let assigned = declaration_with(
        &quote::quote! {
            #[arg(id = "out_file", long)]
            output_file: String
        },
        ClapRenameAll::Snake,
    );
    assert_eq!(
        assigned.long.as_deref(),
        Some("out_file"),
        "an assigned id is a literal name and must not be re-cased"
    );
}

#[test]
fn requiredness_follows_clap_not_serde_defaults() {
    let required = declaration(&quote::quote! {
        #[arg(long)]
        host: String
    });
    assert!(
        required.required,
        "a value-taking clap argument with no clap default is required"
    );

    let optional = declaration(&quote::quote! {
        #[arg(long)]
        host: Option<String>
    });
    assert!(!optional.required, "an Option argument is never required");

    let collection = declaration(&quote::quote! {
        #[arg(long)]
        hosts: Vec<String>
    });
    assert!(!collection.required, "a Vec argument is never required");

    let with_default = declaration(&quote::quote! {
        #[arg(long, default_value = "localhost")]
        host: String
    });
    assert!(
        !with_default.required,
        "a clap default makes the argument optional"
    );
}

#[test]
fn nested_option_accepts_a_bare_flag() {
    let decl = declaration(&quote::quote! {
        #[arg(long)]
        colour: Option<Option<String>>
    });
    assert!(
        decl.takes_value,
        "Option<Option<T>> still accepts a value when present"
    );
    assert!(
        decl.value_optional,
        "clap infers num_args(0..=1) for Option<Option<T>>"
    );
    assert!(!decl.required, "Option<Option<T>> is never required");
}

#[test]
fn unreadable_range_bounds_do_not_claim_an_optional_value() {
    let literal = declaration(&quote::quote! {
        #[arg(long, num_args = 0..=1)]
        colour: String
    });
    assert!(
        literal.value_optional,
        "a literal 0..=1 range makes the value optional"
    );

    let lower_bound = declaration(&quote::quote! {
        #[arg(long, num_args = MIN..=1)]
        colour: String
    });
    assert!(
        !lower_bound.value_optional,
        "a non-literal lower bound must not be assumed to admit zero values"
    );
}

#[test]
fn parenthesised_values_are_accepted() {
    let decl = declaration(&quote::quote! {
        #[arg(long, num_args(2), default_value_t = 5)]
        count: u32
    });
    assert_eq!(
        decl.long.as_deref(),
        Some("count"),
        "the long flag should survive a parenthesised sibling"
    );
    assert!(
        decl.multiple,
        "num_args(2) should mark the argument multi-valued"
    );

    let trailing_comma = declaration(&quote::quote! {
        #[arg(long, num_args(2,))]
        count: u32
    });
    assert!(
        trailing_comma.multiple,
        "clap accepts a trailing comma in a parenthesised value"
    );
}

#[test]
fn os_string_defaults_make_an_argument_optional() {
    let decl = declaration(&quote::quote! {
        #[arg(long, default_value_os_t = std::ffi::OsString::from("out"))]
        output: std::ffi::OsString
    });
    assert!(
        !decl.required,
        "default_value_os_t is a clap default, so the argument is optional"
    );
}

#[test]
fn qualified_vec_paths_stay_scalar_like_claps_own_rule() {
    let decl = declaration(&quote::quote! {
        #[arg(long)]
        items: std::vec::Vec<String>
    });
    assert!(
        !decl.multiple,
        "clap treats a qualified Vec path as a scalar"
    );
    assert!(
        decl.required,
        "a scalar value-taking argument with no clap default is required"
    );
}
