//! Unit tests for positional index resolution.

use super::*;

/// Parses clap declarations for a field list.
fn declarations(fields: &proc_macro2::TokenStream) -> (Vec<syn::Field>, Vec<FieldAttrs>) {
    let input: syn::DeriveInput = syn::parse2(quote::quote! {
        struct Fixture { #fields }
    })
    .expect("test fixture should parse");
    let syn::Data::Struct(data) = input.data else {
        panic!("test input must be a struct: {}", input.ident);
    };
    let parsed_fields: Vec<_> = data.fields.into_iter().collect();
    let attrs = parsed_fields
        .iter()
        .map(|field| {
            crate::derive::parse::parse_field_attrs(
                field,
                crate::derive::parse::ClapRenameAll::default(),
            )
        })
        .collect::<syn::Result<Vec<_>>>()
        .expect("test field attributes should parse");
    (parsed_fields, attrs)
}

#[test]
fn implicit_indices_follow_declaration_order() {
    let (fields, attrs) = declarations(&quote::quote! {
        #[arg(value_name = "SOURCE")]
        source: String,
        #[arg(long)]
        force: bool,
        #[arg(value_name = "DESTINATION")]
        destination: String
    });
    let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
    let source = resolved.get("source").expect("source resolves");
    let destination = resolved.get("destination").expect("destination resolves");
    assert_eq!(
        source.positional.as_ref().map(|value| value.index),
        Some(1),
        "the first implicit positional should receive index 1"
    );
    assert_eq!(
        destination.positional.as_ref().map(|value| value.index),
        Some(2),
        "the second implicit positional should receive index 2"
    );
    assert!(
        resolved
            .get("force")
            .is_some_and(|value| value.positional.is_none()),
        "a long flag must not receive a positional index"
    );
}

#[test]
fn explicit_indices_do_not_advance_the_implicit_counter() {
    let (fields, attrs) = declarations(&quote::quote! {
        #[arg(value_name = "FIRST")]
        first: String,
        #[arg(index = 5, value_name = "FIFTH")]
        fifth: String,
        #[arg(value_name = "SECOND")]
        second: String
    });
    let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
    assert_eq!(
        resolved
            .get("first")
            .and_then(|value| value.positional.as_ref())
            .map(|value| value.index),
        Some(1),
        "the first implicit positional should receive index 1"
    );
    assert_eq!(
        resolved
            .get("fifth")
            .and_then(|value| value.positional.as_ref())
            .map(|value| value.index),
        Some(5),
        "the explicit positional index should be preserved"
    );
    assert_eq!(
        resolved
            .get("second")
            .and_then(|value| value.positional.as_ref())
            .map(|value| value.index),
        Some(2),
        "an explicit index must not advance the implicit counter"
    );
}

#[test]
fn undeclared_fields_become_operands_in_a_declared_struct() {
    let (fields, attrs) = declarations(&quote::quote! {
        first: String,
        #[arg(value_name = "SECOND")]
        second: String
    });
    let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
    let first = resolved.get("first").expect("first resolves");
    assert_eq!(first.long, None, "clap derives no long flag");
    assert_eq!(first.value_name, "FIRST", "clap shouts the field name");
    assert_eq!(
        first.positional.as_ref().map(|value| value.index),
        Some(1),
        "the undeclared operand should take the first index"
    );
    assert_eq!(
        resolved
            .get("second")
            .and_then(|value| value.positional.as_ref())
            .map(|value| value.index),
        Some(2),
        "the declared operand should follow the undeclared one"
    );
}

#[test]
fn a_struct_without_declarations_resolves_no_arguments() {
    let (fields, attrs) = declarations(&quote::quote! {
        host: String,
        port: u16
    });
    let resolved = resolve_clap_args(&fields, &attrs).expect("fields should resolve");
    assert!(
        resolved.is_empty(),
        "a configuration-only struct keeps the generated surface"
    );
}

#[test]
fn inferred_operands_follow_claps_type_rules() {
    let (fields, attrs) = declarations(&quote::quote! {
        #[arg(long)]
        force: bool,
        target: Option<String>,
        files: Vec<String>,
        flag: bool
    });
    let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
    let target = resolved.get("target").expect("target resolves");
    assert_eq!(
        target.positional.as_ref().map(|value| value.index),
        Some(1),
        "an optional field is still a positional operand"
    );
    assert!(!target.required, "an Option field is not required");
    let files = resolved.get("files").expect("files resolves");
    assert_eq!(
        files.positional.as_ref().map(|value| value.index),
        Some(2),
        "a Vec field takes the next operand index"
    );
    assert!(
        files.multiple
            && files
                .positional
                .as_ref()
                .is_some_and(|value| value.variadic),
        "a Vec operand accepts several values"
    );
    assert!(
        !resolved.contains_key("flag"),
        "a bare bool cannot be a positional switch, so it keeps the generated surface"
    );
}

#[test]
fn subcommand_and_flatten_markers_are_not_arguments() {
    let (fields, attrs) = declarations(&quote::quote! {
        #[arg(long)]
        force: bool,
        #[command(subcommand)]
        command: Commands,
        #[command(flatten)]
        shared: Shared,
        target: String
    });
    let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
    assert!(
        !resolved.contains_key("command") && !resolved.contains_key("shared"),
        "markers own no argument of their own"
    );
    assert_eq!(
        resolved
            .get("target")
            .and_then(|value| value.positional.as_ref())
            .map(|value| value.index),
        Some(1),
        "only real arguments consume positional indices"
    );
}

#[test]
fn skipped_fields_receive_no_positional_index() {
    let (fields, attrs) = declarations(&quote::quote! {
        #[arg(skip)]
        hidden: String,
        #[arg(value_name = "INPUT")]
        input: String
    });
    let resolved = resolve_clap_args(&fields, &attrs).expect("declarations should resolve");
    assert!(
        resolved.get("hidden").is_some_and(|value| value.is_skipped),
        "an arg(skip) field should be marked skipped"
    );
    assert_eq!(
        resolved
            .get("input")
            .and_then(|value| value.positional.as_ref())
            .map(|value| value.index),
        Some(1),
        "a skipped field must not consume the first positional index"
    );
}
