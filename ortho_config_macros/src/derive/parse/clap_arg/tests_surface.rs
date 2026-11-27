//! Tests for clap declaration shapes, casing, and requiredness.

use super::{declaration, declaration_with};
use crate::derive::parse::ClapRenameAll;

#[test]
fn from_global_fields_have_no_field_local_argument() {
    let decl = declaration(&quote::quote! {
        #[arg(from_global)]
        verbose: bool
    });
    assert!(
        decl.is_skipped,
        "a from_global field owns no field-local clap argument"
    );
}

#[test]
fn empty_clap_attributes_declare_a_positional() {
    for tokens in [
        quote::quote! { #[arg()] input: String },
        quote::quote! { #[clap()] input: String },
    ] {
        let field = super::parse_field(&tokens);
        let decl = super::parse_clap_arg_decl(&field, ClapRenameAll::default())
            .expect("the declaration should parse")
            .expect("an empty attribute still declares the argument");
        assert!(
            decl.long.is_none() && decl.short.is_none(),
            "an empty attribute leaves the argument positional"
        );
        assert!(
            decl.positional.is_some(),
            "an empty attribute should carry positional metadata"
        );
    }
}

#[test]
fn an_unreadable_id_falls_back_to_the_generated_surface() {
    let field = super::parse_field(&quote::quote! {
        #[arg(id = ARG_ID, long)]
        host: String
    });
    assert!(
        super::parse_clap_arg_decl(&field, ClapRenameAll::default())
            .expect("the declaration should parse")
            .is_none(),
        "an unreadable id should leave the declaration unmodelled"
    );
}

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
fn unreadable_num_args_falls_back_to_the_generated_surface() {
    let literal = declaration(&quote::quote! {
        #[arg(long, num_args = 0..=1)]
        colour: String
    });
    assert!(
        literal.value_optional,
        "a literal 0..=1 range makes the value optional"
    );

    for tokens in [
        quote::quote! { #[arg(long, num_args = MIN..=1)] colour: String },
        quote::quote! { #[arg(long, num_args = LIMIT)] colour: Option<Option<String>> },
    ] {
        let field = super::parse_field(&tokens);
        assert!(
            super::parse_clap_arg_decl(&field, ClapRenameAll::default())
                .expect("the declaration should parse")
                .is_none(),
            "an unreadable num_args should leave the declaration unmodelled"
        );
    }
}

#[test]
fn default_missing_value_does_not_make_a_bounded_value_optional() {
    let bounded = declaration(&quote::quote! {
        #[arg(long, num_args = 1, default_missing_value = "auto")]
        colour: Option<String>
    });
    assert!(
        !bounded.value_optional,
        "a one-value bound rejects the bare flag even with a missing-value default"
    );

    let optional = declaration(&quote::quote! {
        #[arg(long, num_args = 0..=1, default_missing_value = "auto")]
        colour: Option<String>
    });
    assert!(
        optional.value_optional,
        "a zero-minimum bound keeps the bare flag valid"
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
fn help_actions_take_no_value() {
    let help_short = declaration(&quote::quote! {
        #[arg(long, action = clap::ArgAction::HelpShort)]
        usage: Option<String>
    });
    assert!(
        !help_short.takes_value,
        "HelpShort displays help without accepting a value"
    );

    let help_long = declaration(&quote::quote! {
        #[arg(long, action = clap::ArgAction::HelpLong)]
        usage: Option<String>
    });
    assert!(
        !help_long.takes_value,
        "HelpLong displays help without accepting a value"
    );
}

#[test]
fn option_vec_repetition_survives_a_single_value_bound() {
    let repeated = declaration(&quote::quote! {
        #[arg(long, num_args = 1)]
        tags: Vec<String>
    });
    assert!(
        repeated.multiple,
        "clap still infers ArgAction::Append for a Vec option"
    );

    let explicit_set = declaration(&quote::quote! {
        #[arg(long, num_args = 1, action = clap::ArgAction::Set)]
        tags: Vec<String>
    });
    assert!(
        !explicit_set.multiple,
        "an explicit Set action stops repeated occurrences"
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
