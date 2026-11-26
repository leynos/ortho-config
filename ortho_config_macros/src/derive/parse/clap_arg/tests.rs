//! Unit tests for clap declaration parsing.

use super::{ClapArgDecl, ClapPositionalDecl, parse_clap_arg_decl};

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
    let field = parse_field(tokens);
    parse_clap_arg_decl(&field)
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
    assert_eq!(
        flag_decl.required,
        Some(false),
        "a switch is never required"
    );

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
        parse_clap_arg_decl(&flattened)
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
