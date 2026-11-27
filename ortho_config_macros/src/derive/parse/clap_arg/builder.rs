//! Accumulation and resolution of one field's clap argument declaration.
//!
//! The builder consumes nested `arg`/`clap` keys in source order, then resolves
//! them against the field's clap argument id and Rust type. Values that cannot
//! be read statically are ignored rather than rejected: clap accepts
//! expressions, and the `OrthoConfig` derive must not reject a valid clap
//! attribute it cannot interpret.

use syn::ext::IdentExt;
use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{Expr, Lit, Type};

use super::super::{ClapRenameAll, discard_unknown};
use super::resolvers::{self, NameChoice, RepetitionFlags, ShortChoice};
use super::value_syntax::{
    ValueRange, action_name, argument, clap_option_inner, is_bare_bool, is_default_value_key,
    is_vec_field, parse_bool, parse_lit_str, parse_usize, parse_value_range, single_char,
};
use super::{ClapArgDecl, ClapPositionalDecl};

/// Accumulates one field's clap declaration until the field type is known.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag records an independent clap key or parser state."
)]
#[derive(Debug, Default)]
pub(super) struct ClapArgBuilder {
    /// Whether any clap argument attribute was present.
    seen: bool,
    /// Long spelling choice, if `long` was declared.
    long: Option<NameChoice>,
    /// Short spelling choice, if `short` was declared.
    short: Option<ShortChoice>,
    /// Explicit value placeholder.
    value_name: Option<String>,
    /// Explicit positional index.
    index: Option<u16>,
    /// Explicit value-count range.
    num_args: Option<ValueRange>,
    /// Whether a clap default value was declared.
    has_default: bool,
    /// Explicit action-derived value consumption.
    takes_value: Option<bool>,
    /// Whether the declared action appends repeated values.
    action_appends: bool,
    /// Whether the declared action replaces rather than accumulates values.
    action_set: bool,
    /// Whether the declared action counts repeated occurrences.
    action_counts: bool,
    /// Whether the declared action consumes a value, before arity overrides.
    action_takes_value: Option<bool>,
    /// Whether clap hides the argument.
    hide: bool,
    /// Explicit requiredness.
    required: Option<bool>,
    /// Whether the argument consumes trailing values without `--`.
    var_arg: bool,
    /// Whether the argument requires `--`.
    last: bool,
    /// Whether clap skips the field.
    skipped: bool,
    /// Whether the field is a flattened container rather than one argument.
    flatten: bool,
    /// Whether the field selects a subcommand.
    subcommand: bool,
    /// Whether a value could not be understood and the declaration must be
    /// ignored in favour of the generated surface.
    unparsed: bool,
}

impl ClapArgBuilder {
    /// Reports whether the declaration cannot describe one source field.
    pub(super) const fn should_ignore(&self) -> bool {
        !self.seen || self.unparsed || self.flatten || self.subcommand
    }

    /// Records that a clap argument attribute was present.
    ///
    /// An empty `#[arg()]` has no keys to consume, but it still declares the
    /// argument; clap treats it as positional.
    pub(super) const fn mark_present(&mut self) {
        self.seen = true;
    }

    /// Records one nested key from an `arg` or `clap` attribute.
    pub(super) fn consume(&mut self, meta: &ParseNestedMeta) -> syn::Result<()> {
        self.seen = true;
        if self.consume_argument_key(meta)? {
            return Ok(());
        }
        if self.consume_value_key(meta)? {
            return Ok(());
        }
        if self.consume_marker(meta)? {
            return Ok(());
        }
        discard_unknown(meta)
    }

    /// Records a key that shapes how clap parses the argument.
    fn consume_argument_key(&mut self, meta: &ParseNestedMeta) -> syn::Result<bool> {
        if meta.path.is_ident("long") {
            self.set_long(meta)?;
        } else if meta.path.is_ident("short") {
            self.set_short(meta)?;
        } else if meta.path.is_ident("value_name") {
            self.set_value_name(meta)?;
        } else if meta.path.is_ident("index") {
            self.set_index(meta)?;
        } else if meta.path.is_ident("num_args") {
            self.set_num_args(meta)?;
        } else if meta.path.is_ident("action") {
            self.set_action(meta)?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    /// Records a key that supplies a value, visibility, or arity setting.
    fn consume_value_key(&mut self, meta: &ParseNestedMeta) -> syn::Result<bool> {
        if is_default_value_key(meta) {
            self.has_default = true;
            discard_unknown(meta)?;
        } else {
            return self.consume_toggle_key(meta);
        }
        Ok(true)
    }

    /// Records a boolean toggle that describes the argument.
    fn consume_toggle_key(&mut self, meta: &ParseNestedMeta) -> syn::Result<bool> {
        if meta.path.is_ident("hide") {
            self.hide = self.bool_argument(meta)?;
        } else if meta.path.is_ident("required") {
            self.required = self.optional_bool_argument(meta)?;
        } else if meta.path.is_ident("trailing_var_arg") {
            self.var_arg = self.bool_argument(meta)?;
        } else if meta.path.is_ident("last") {
            self.last = self.bool_argument(meta)?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    /// Records a marker that removes the field from ordinary argument metadata.
    fn consume_marker(&mut self, meta: &ParseNestedMeta) -> syn::Result<bool> {
        if meta.path.is_ident("skip") {
            self.skipped = true;
        } else if meta.path.is_ident("flatten") {
            self.flatten = true;
        } else if meta.path.is_ident("subcommand") || meta.path.is_ident("external_subcommand") {
            self.subcommand = true;
        } else {
            return Ok(false);
        }
        discard_unknown(meta)?;
        Ok(true)
    }

    /// Records the `long` key and its optional literal spelling.
    fn set_long(&mut self, meta: &ParseNestedMeta) -> syn::Result<()> {
        match argument(meta)? {
            None => self.long = Some(NameChoice::Derived),
            Some(Expr::Lit(lit)) => match lit.lit {
                Lit::Str(value) => self.long = Some(NameChoice::Explicit(value.value())),
                _ => self.unparsed = true,
            },
            Some(_) => self.unparsed = true,
        }
        Ok(())
    }

    /// Records the `short` key and its optional literal character.
    fn set_short(&mut self, meta: &ParseNestedMeta) -> syn::Result<()> {
        match argument(meta)? {
            None => self.short = Some(ShortChoice::Derived),
            Some(Expr::Lit(lit)) => match lit.lit {
                Lit::Char(value) => self.short = Some(ShortChoice::Explicit(value.value())),
                Lit::Str(value) => match single_char(&value.value()) {
                    Some(character) => self.short = Some(ShortChoice::Explicit(character)),
                    None => self.unparsed = true,
                },
                _ => self.unparsed = true,
            },
            Some(_) => self.unparsed = true,
        }
        Ok(())
    }

    /// Reads a boolean key, falling back to the generated surface when the
    /// declared value cannot be read statically.
    fn bool_argument(&mut self, meta: &ParseNestedMeta) -> syn::Result<bool> {
        if let Some(value) = parse_bool(meta)? {
            return Ok(value);
        }
        self.unparsed = true;
        Ok(false)
    }

    /// Reads an optional boolean key, marking unreadable values as unparsed.
    fn optional_bool_argument(&mut self, meta: &ParseNestedMeta) -> syn::Result<Option<bool>> {
        if let Some(value) = parse_bool(meta)? {
            return Ok(Some(value));
        }
        self.unparsed = true;
        Ok(None)
    }

    /// Records the explicit `value_name` placeholder.
    fn set_value_name(&mut self, meta: &ParseNestedMeta) -> syn::Result<()> {
        match parse_lit_str(meta)? {
            Some(value) => self.value_name = Some(value),
            None => self.unparsed = true,
        }
        Ok(())
    }

    /// Records an explicit positional index.
    fn set_index(&mut self, meta: &ParseNestedMeta) -> syn::Result<()> {
        match parse_usize(meta)? {
            Some(index) => {
                self.index = Some(u16::try_from(index).map_err(|_| {
                    syn::Error::new(
                        meta.path.span(),
                        "clap positional index exceeds the documentation IR's u16 range",
                    )
                })?);
            }
            None => self.unparsed = true,
        }
        Ok(())
    }

    /// Records a `num_args` range when it is written literally.
    ///
    /// A range the derive cannot read statically — a constant count, or a
    /// bound that is not a literal integer — leaves the declaration unmodelled,
    /// so the field falls back to the documented generated surface rather than
    /// advertising an arity the parser may reject.
    fn set_num_args(&mut self, meta: &ParseNestedMeta) -> syn::Result<()> {
        let range = parse_value_range(meta)?;
        if range.is_none() {
            self.unparsed = true;
        }
        self.num_args = range;
        Ok(())
    }

    /// Records whether an explicit action consumes values.
    fn set_action(&mut self, meta: &ParseNestedMeta) -> syn::Result<()> {
        let Some(action) = argument(meta)? else {
            self.unparsed = true;
            return Ok(());
        };
        let Some(name) = action_name(&action) else {
            self.unparsed = true;
            return Ok(());
        };
        // clap keeps only the final action, so a later declaration replaces
        // every action-derived flag rather than adding to it.
        self.takes_value = None;
        self.action_appends = false;
        self.action_set = false;
        self.action_counts = false;
        self.action_takes_value = None;
        match name.as_str() {
            "SetTrue" | "SetFalse" | "Help" | "HelpShort" | "HelpLong" | "Version" => {
                self.takes_value = Some(false);
                self.action_takes_value = Some(false);
            }
            "Count" => {
                self.takes_value = Some(false);
                self.action_takes_value = Some(false);
                self.action_counts = true;
            }
            "Set" => {
                self.takes_value = Some(true);
                self.action_takes_value = Some(true);
                self.action_set = true;
            }
            "Append" => {
                self.takes_value = Some(true);
                self.action_takes_value = Some(true);
                self.action_appends = true;
            }
            _ => self.unparsed = true,
        }
        Ok(())
    }

    /// Resolves the declaration against the field's type and clap argument id.
    pub(super) fn finish(
        self,
        field: &syn::Field,
        rename_all: ClapRenameAll,
    ) -> syn::Result<ClapArgDecl> {
        let explicit_id = super::super::clap_attrs::clap_arg_id(field)?;
        let field_ident = field.ident.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(field, "tuple fields cannot declare clap arguments")
        })?;
        let raw_field = field_ident.unraw().to_string();
        let (long, short) = resolvers::spellings(resolvers::SpellingInputs {
            long: self.long.as_ref(),
            short: self.short.as_ref(),
            explicit_id: explicit_id.as_deref(),
            raw_field: &raw_field,
            rename_all,
        });
        let value_name = resolvers::value_name(
            self.value_name.as_deref(),
            explicit_id.as_deref(),
            &raw_field,
        );
        let is_positional = long.is_none() && short.is_none();
        let is_vec = is_vec_field(&field.ty);
        let num_args = self.num_args;
        let takes_value = self.resolve_takes_value(is_positional, &field.ty);
        let value_optional = resolvers::value_optional(takes_value, num_args, &field.ty);
        let multiple = resolvers::multiple(
            RepetitionFlags {
                action_appends: self.action_appends,
                action_set: self.action_set,
                action_counts: self.action_counts,
                is_vec,
                is_positional,
            },
            num_args,
        );
        let variadic = is_positional && resolvers::variadic(is_vec, num_args);
        let positional = is_positional.then_some(ClapPositionalDecl {
            index: self.index,
            variadic,
            var_arg: self.var_arg,
            last: self.last,
        });

        Ok(ClapArgDecl {
            long,
            short,
            value_name,
            positional,
            takes_value,
            value_optional,
            multiple,
            hide_in_help: self.hide,
            required: self.resolve_required(&field.ty),
            is_skipped: self.skipped,
        })
    }

    /// Resolves whether the argument consumes a value.
    fn resolve_takes_value(&self, is_positional: bool, ty: &Type) -> bool {
        if is_positional {
            return true;
        }
        if let Some(range) = self.num_args {
            return range.max > 0;
        }
        if let Some(value) = self.takes_value {
            return value;
        }
        !is_bare_bool(ty)
    }

    /// Resolves clap's requiredness from explicit and inferred signals.
    ///
    /// An argument with no clap default is required, except that `Option` and
    /// `Vec` shapes and the value-free actions clap infers for a bare boolean
    /// are never required. Requiredness therefore does not follow an
    /// `num_args` arity override. The configuration loader's Serde defaults do
    /// not relax a clap declaration, so a declared argument must not fall back
    /// to those when it is missing a value.
    fn resolve_required(&self, ty: &Type) -> bool {
        if let Some(required) = self.required {
            return required;
        }
        if self.has_default {
            return false;
        }
        if self.action_takes_value == Some(false) {
            return false;
        }
        if self.action_takes_value.is_none() && is_bare_bool(ty) {
            return false;
        }
        !(is_vec_field(ty) || clap_option_inner(ty).is_some())
    }
}
