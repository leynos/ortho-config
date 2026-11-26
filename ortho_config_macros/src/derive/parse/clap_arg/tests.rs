//! Unit tests for clap declaration parsing.

use super::{ClapArgDecl, ClapPositionalDecl, parse_clap_arg_decl};
use crate::derive::parse::ClapRenameAll;

/// Parses a field written as a test token stream.
fn parse_field(tokens: &proc_macro2::TokenStream) -> syn::Field {
    let syn::DeriveInput { ident, data, .. } = syn::parse2(quote::quote! {
        struct Fixture { #tokens }
    })
    .expect("test fixture should parse");
    let syn::Data::Struct(struct_data) = data else {
        panic!("test input must be a struct: {ident}");
    };
    struct_data
        .fields
        .into_iter()
        .next()
        .expect("test fixture should contain one field")
}

/// Parses the one clap declaration declared by a test field.
fn declaration(tokens: &proc_macro2::TokenStream) -> ClapArgDecl {
    declaration_with(tokens, ClapRenameAll::default())
}

/// Parses one clap declaration under an explicit container casing rule.
fn declaration_with(tokens: &proc_macro2::TokenStream, rename_all: ClapRenameAll) -> ClapArgDecl {
    let field = parse_field(tokens);
    parse_clap_arg_decl(&field, rename_all)
        .expect("declaration should parse")
        .expect("declaration should be present")
}

#[test]
fn bare_long_uses_the_kebab_case_field_name() {
    let decl = declaration(&quote::quote! {
        #[arg(long)]
        log_level: String
    });
    assert_eq!(
        decl.long.as_deref(),
        Some("log-level"),
        "a bare long should use the kebab-case field name"
    );
    assert_eq!(
        decl.short, None,
        "a long-only declaration must not invent a short flag"
    );
    assert_eq!(
        decl.value_name, "LOG_LEVEL",
        "clap derives the value name with upper snake case"
    );
    assert!(
        decl.positional.is_none(),
        "a long flag is not a positional operand"
    );
}

#[test]
fn explicit_spellings_and_positionals_are_preserved() {
    let proposal_decl = declaration(&quote::quote! {
        #[arg(value_name = "PROPOSAL")]
        proposal: String
    });
    assert_eq!(
        proposal_decl.long, None,
        "a positional must not advertise a long flag"
    );
    assert_eq!(
        proposal_decl.short, None,
        "a positional must not advertise a short flag"
    );
    assert_eq!(
        proposal_decl.value_name, "PROPOSAL",
        "the declared value name must be preserved"
    );
    assert!(
        proposal_decl.takes_value,
        "a positional operand consumes a value"
    );
    assert_eq!(
        proposal_decl.positional,
        Some(ClapPositionalDecl {
            index: None,
            variadic: false,
            var_arg: false,
            last: false,
        }),
        "a field without long or short should be positional"
    );

    let check_decl = declaration(&quote::quote! {
        #[arg(long, value_name = "COMMAND")]
        check: Option<String>
    });
    assert_eq!(
        check_decl.long.as_deref(),
        Some("check"),
        "the declared long flag should be preserved"
    );
    assert_eq!(
        check_decl.short, None,
        "the declaration did not supply a short flag"
    );
    assert_eq!(
        check_decl.value_name, "COMMAND",
        "the declared value name should be preserved"
    );
    assert!(check_decl.takes_value, "an option flag consumes a value");
    assert!(
        check_decl.positional.is_none(),
        "a long flag is not a positional operand"
    );
}

#[test]
fn explicit_id_controls_default_spellings() {
    let decl = declaration(&quote::quote! {
        #[arg(id = "preamble", long)]
        phrase: String
    });
    assert_eq!(
        decl.long.as_deref(),
        Some("preamble"),
        "clap translates a bare long from the assigned id verbatim"
    );
    assert_eq!(
        decl.value_name, "preamble",
        "clap translates the value name from the assigned id verbatim"
    );
}

#[test]
fn bool_flags_are_switches_but_optional_bools_take_values() {
    let flag_decl = declaration(&quote::quote! {
        #[arg(long)]
        force: bool
    });
    assert!(
        !flag_decl.takes_value,
        "a bare bool is a clap switch with no value"
    );
    assert!(!flag_decl.required, "a switch is never required");

    let optional_decl = declaration(&quote::quote! {
        #[arg(long)]
        force: Option<bool>
    });
    assert!(
        optional_decl.takes_value,
        "clap treats Option<bool> as a value-taking option"
    );
}

#[test]
fn num_args_ranges_control_variadic_and_optional_values() {
    let optional_decl = declaration(&quote::quote! {
        #[arg(long, num_args = 0..=1)]
        colour: String
    });
    assert!(
        optional_decl.takes_value,
        "num_args 0..=1 still accepts a value"
    );
    assert!(
        optional_decl.value_optional,
        "num_args 0..=1 makes the value optional"
    );

    let variadic_decl = declaration(&quote::quote! {
        #[arg(num_args = 1..)]
        paths: Vec<String>
    });
    assert!(
        variadic_decl.multiple,
        "a positional Vec accepts repeated values"
    );
    assert!(
        variadic_decl.positional.is_some_and(|value| value.variadic),
        "num_args 1.. makes the positional variadic"
    );
}

#[test]
fn skip_and_flatten_are_not_documented_as_arguments() {
    let skipped_decl = declaration(&quote::quote! {
        #[arg(skip)]
        hidden: String
    });
    assert!(
        skipped_decl.is_skipped,
        "an arg(skip) field must not be documented as CLI surface"
    );

    let flattened = parse_field(&quote::quote! {
        #[command(flatten)]
        common: CommonArgs
    });
    assert!(
        parse_clap_arg_decl(&flattened, ClapRenameAll::default())
            .expect("flatten should parse")
            .is_none(),
        "a flattened container is not one clap argument"
    );
}

#[test]
fn explicit_index_and_trailing_settings_are_recorded() {
    let decl = declaration(&quote::quote! {
        #[arg(index = 2, trailing_var_arg, last)]
        input: String
    });
    let positional = decl.positional.expect("positional metadata");
    assert_eq!(
        positional.index,
        Some(2),
        "the explicit positional index should be recorded"
    );
    assert!(
        positional.var_arg,
        "trailing_var_arg should be recorded independently"
    );
    assert!(positional.last, "last should be recorded independently");
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
}
