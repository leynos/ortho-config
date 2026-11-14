//! Token emission for the optional `output` IR block.
//!
//! Kept in its own module so `sections.rs` and `generate/docs/mod.rs` stay
//! under the repository's 400-line ceiling.
//!
//! The block is emitted from the parser's validated model without inventing a
//! value: an undeclared leaf stays `None`, so the wire form omits it and the
//! policy rule can tell "not declared" from "declared false". Enum leaves
//! reuse the renderer's token helper, which converts a `snake_case` token
//! generically; the parser is the gate that rejects invalid spellings.

use proc_macro2::TokenStream;
use quote::quote;

use crate::derive::parse::{DocStructAttrs, ExitClassAttr, OutputAttrs};

use super::renderer::{bool_option_tokens, enum_option_tokens};

/// Builds the token stream for the optional `output` IR block.
///
/// Returns `None` when no `output(...)` declaration exists. An explicit
/// `output()` declaration yields `Some(OutputMetadata)` whose every leaf is
/// undeclared: it makes no capability claim, and the wire form is `{}`.
pub(super) fn build_output_metadata(doc: &DocStructAttrs, krate: &TokenStream) -> TokenStream {
    let Some(output) = doc.output.as_ref() else {
        return quote! { None };
    };
    block_tokens(output, krate)
}

/// Emits `Some(OutputMetadata { .. })` for one declared block.
fn block_tokens(output: &OutputAttrs, krate: &TokenStream) -> TokenStream {
    let supports_json = bool_option_tokens(output.supports_json);
    let stdout = enum_option_tokens(output.stdout.as_deref(), "StdoutPurpose", krate);
    let stderr_diagnostics = bool_option_tokens(output.stderr_diagnostics);
    let exit_classes = output
        .exit_classes
        .iter()
        .map(|class| class_tokens(class, krate))
        .collect::<Vec<_>>();

    quote! {
        Some(#krate::docs::OutputMetadata {
            supports_json: #supports_json,
            stdout: #stdout,
            stderr_diagnostics: #stderr_diagnostics,
            exit_classes: vec![ #( #exit_classes ),* ],
        })
    }
}

/// Emits one `ExitClass` entry in author order.
///
/// Both members are string literals rather than Fluent keys: a class name is a
/// stable protocol identifier and its meaning is transported verbatim, so
/// neither may vary with a locale.
fn class_tokens(class: &ExitClassAttr, krate: &TokenStream) -> TokenStream {
    let name = syn::LitStr::new(&class.name, proc_macro2::Span::call_site());
    let meaning = syn::LitStr::new(&class.meaning, proc_macro2::Span::call_site());
    quote! {
        #krate::docs::ExitClass {
            name: String::from(#name),
            meaning: String::from(#meaning),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Focused checks on the emitted block shape.

    use super::*;
    use crate::derive::parse::DocStructAttrs;

    /// Emits the block for the supplied declaration, if any.
    fn emitted(output: Option<OutputAttrs>) -> String {
        let mut doc = DocStructAttrs::default();
        doc.output = output;
        build_output_metadata(&doc, &quote! { ortho_config }).to_string()
    }

    /// An absent declaration emits `None` rather than an empty record, so the
    /// undeclared state is preserved on the wire.
    #[test]
    fn absent_declaration_emits_none() {
        assert_eq!(
            emitted(None).trim(),
            "None",
            "an absent output declaration must not emit a record"
        );
    }

    /// An explicit empty declaration emits a record with every leaf undeclared.
    #[test]
    fn empty_declaration_emits_empty_record() {
        let tokens = emitted(Some(OutputAttrs::default()));
        assert!(
            tokens.contains("Some (ortho_config :: docs :: OutputMetadata"),
            "empty declaration should emit a record: {tokens}"
        );
        assert!(
            tokens.contains("supports_json : None")
                && tokens.contains("stdout : None")
                && tokens.contains("stderr_diagnostics : None"),
            "every leaf should stay undeclared: {tokens}"
        );
        assert!(
            tokens.contains("exit_classes : vec ! []"),
            "an empty class vector should stay empty: {tokens}"
        );
    }

    /// Declared leaves map to their typed IR values, and classes keep author
    /// order rather than being sorted or deduplicated here.
    #[test]
    fn declared_leaves_and_classes_are_emitted_in_order() {
        let tokens = emitted(Some(OutputAttrs {
            supports_json: Some(true),
            stdout: Some("data".to_owned()),
            stderr_diagnostics: Some(false),
            exit_classes: vec![
                ExitClassAttr {
                    name: "usage".to_owned(),
                    meaning: "The invocation was invalid".to_owned(),
                },
                ExitClassAttr {
                    name: "success".to_owned(),
                    meaning: "The command completed".to_owned(),
                },
            ],
        }));
        assert!(
            tokens.contains("supports_json : Some (true)"),
            "declared JSON support: {tokens}"
        );
        assert!(
            tokens.contains("stdout : Some (ortho_config :: docs :: StdoutPurpose :: Data)"),
            "declared stdout purpose: {tokens}"
        );
        assert!(
            tokens.contains("stderr_diagnostics : Some (false)"),
            "declared false must survive: {tokens}"
        );
        let usage = tokens.find("usage").expect("usage class should be emitted");
        let success = tokens
            .find("success")
            .expect("success class should be emitted");
        assert!(
            usage < success,
            "classes should keep author order: {tokens}"
        );
    }
}
