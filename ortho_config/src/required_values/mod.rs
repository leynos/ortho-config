//! Enumerating absent required configuration values.
//!
//! The derive macro emits a [`RequiredConfigSchema`] implementation beside the
//! generated merge code. After every layer is merged, the generated `finish`
//! calls [`ensure_no_missing_required_values`] on the merged JSON value. When
//! requirements are absent, one [`MissingRequiredValues`](crate::MissingRequiredValues)
//! report names all of them together with the routes that can supply each one.
//!
//! Enumeration is best effort by design: it covers declared fields whose
//! requiredness is decidable from attribute metadata. It does not attempt to
//! predict the behaviour of a handwritten `Deserialize`, an unmarked
//! `#[serde(flatten)]`, or an enum payload.
//!
//! # Examples
//!
//! ```rust
//! use ortho_config::{
//!     Activation, RequiredConfigSchema, RequiredSchemaError, RequiredSchemaStack, SchemaChildren,
//!     SchemaField,
//! };
//!
//! struct Service;
//!
//! impl RequiredConfigSchema for Service {
//!     fn collect_required_schema(
//!         stack: &mut RequiredSchemaStack,
//!     ) -> Result<Vec<SchemaField>, RequiredSchemaError> {
//!         stack.enter::<Self>()?;
//!         let fields = vec![SchemaField::new(
//!             "endpoint",
//!             Vec::new(),
//!             Activation::Required,
//!             SchemaChildren::Leaf,
//!         )];
//!         stack.leave::<Self>();
//!         Ok(fields)
//!     }
//! }
//!
//! let schema = Service::collect_required_schema(&mut RequiredSchemaStack::new())
//!     .expect("schema composition cannot cycle for a leaf field");
//! assert_eq!(schema.len(), 1, "one declared field yields one schema entry");
//! ```

mod collect;
mod env_route;
mod schema;

pub(crate) use collect::RouteScope;

pub use schema::{
    Activation, CliFlagHint, RequiredConfigSchema, RequiredSchemaError, RequiredSchemaStack,
    RouteHints, SchemaChildren, SchemaField,
};

use std::sync::Arc;

use serde_json::Value;

use crate::OrthoResult;
use crate::error::{MissingRequiredValues, OrthoError};

/// The validation key used when schema composition itself fails.
const SCHEMA_ERROR_KEY: &str = "required_values_schema";

/// Report every absent required value in `value`, or succeed when none is.
///
/// `program` labels the report so a user knows which program is complaining;
/// generated code supplies the invoked program name and falls back to the type
/// name when the process arguments are unavailable.
///
/// # Errors
///
/// Returns [`OrthoError::MissingRequiredValues`] when at least one declared
/// requirement is absent, and [`OrthoError::Validation`] when the schema itself
/// is recursive. A value that is not a JSON object is not diagnosed here.
///
/// # Examples
///
/// ```rust
/// use ortho_config::{
///     Activation, RequiredConfigSchema, RequiredSchemaError, RequiredSchemaStack, SchemaChildren,
///     SchemaField,
/// };
/// # use ortho_config::ensure_no_missing_required_values;
/// # use serde_json::json;
///
/// struct Endpoint;
///
/// impl RequiredConfigSchema for Endpoint {
///     fn collect_required_schema(
///         stack: &mut RequiredSchemaStack,
///     ) -> Result<Vec<SchemaField>, RequiredSchemaError> {
///         stack.enter::<Self>()?;
///         stack.leave::<Self>();
///         Ok(vec![SchemaField::new(
///             "url",
///             Vec::new(),
///             Activation::Required,
///             SchemaChildren::Leaf,
///         )])
///     }
/// }
///
/// let absent = json!({});
/// let error = ensure_no_missing_required_values::<Endpoint>(&absent, "demo")
///     .expect_err("an empty object cannot satisfy a required field");
/// assert!(
///     error.to_string().contains("Missing required configuration"),
///     "the report must name the problem it detected"
/// );
/// assert!(
///     ensure_no_missing_required_values::<Endpoint>(&json!({"url": "x"}), "demo").is_ok(),
///     "a supplied required value must pass"
/// );
/// ```
pub fn ensure_no_missing_required_values<T: RequiredConfigSchema>(
    value: &Value,
    program: &str,
) -> OrthoResult<()> {
    let fields = schema_of::<T>()?;
    let hints = T::required_route_hints();
    collect::collect_missing(&fields, value, &hints, Some(program.to_owned()))
        .map_or_else(|| Ok(()), |report| Err(missing_values_failure(report)))
}

/// Report every absent required value through a loader-specific route scope.
///
/// A selected subcommand reads `PREFIX_CMDS_<NAME>_*` variables and the
/// `[cmds.<name>]` file section rather than the struct's own prefix and root.
/// Its scope keeps the advertised routes reproducible with the loader that
/// raised the error; the CLI hints still come from the derived schema.
///
/// The scoped loaders build their figment from file, environment, and
/// command-line layers, and never run the declaring type's generated defaults
/// struct. A declared default is therefore not a supply layer for this reading,
/// and the root's [`Activation::DefaultBacked`] fields are required here. A
/// default clap already materialises arrives through the command-line layer, so
/// only a genuinely absent field is reported.
pub(crate) fn ensure_no_missing_required_values_scoped<T: RequiredConfigSchema>(
    value: &Value,
    program: &str,
    scope: &RouteScope,
) -> OrthoResult<()> {
    let fields: Vec<SchemaField> = schema_of::<T>()?
        .into_iter()
        .map(SchemaField::require_default_backed)
        .collect();
    let hints = T::required_route_hints();
    let routes = collect::RouteContext {
        hints: &hints,
        scope,
    };
    collect::collect_missing_scoped(&fields, value, routes, Some(program.to_owned()))
        .map_or_else(|| Ok(()), |report| Err(missing_values_failure(report)))
}

/// Compose `T`'s requirement schema, reporting a cyclic composition as a
/// validation error.
fn schema_of<T: RequiredConfigSchema>() -> Result<Vec<SchemaField>, Arc<OrthoError>> {
    let mut stack = RequiredSchemaStack::new();
    T::collect_required_schema(&mut stack).map_err(|error| schema_failure(&error))
}

/// Build the public error for an enumerated missing-value report.
fn missing_values_failure(report: MissingRequiredValues) -> Arc<OrthoError> {
    Arc::new(OrthoError::MissingRequiredValues(Box::new(report)))
}

/// Convert a schema composition failure into a validation error.
fn schema_failure(error: &RequiredSchemaError) -> Arc<OrthoError> {
    Arc::new(OrthoError::Validation {
        key: SCHEMA_ERROR_KEY.to_owned(),
        message: error.to_string(),
    })
}

/// Derive a program label from the process arguments.
///
/// The file name of `argv[0]` is used when it has one, because it matches what
/// a user typed; a bare program name is used unchanged. An empty or
/// unrepresentable first argument yields `None`, and the caller falls back to
/// its own label.
///
/// # Examples
///
/// ```rust
/// use std::ffi::OsString;
/// use ortho_config::program_label_from_args;
///
/// let args = vec![OsString::from("/usr/local/bin/demo")];
/// assert_eq!(
///     program_label_from_args(&args).as_deref(),
///     Some("demo"),
///     "the file name of argv[0] is the label a user typed"
/// );
/// assert_eq!(
///     program_label_from_args(&[]),
///     None,
///     "no first argument means no label"
/// );
/// ```
#[must_use]
pub fn program_label_from_args(args: &[std::ffi::OsString]) -> Option<String> {
    let first = args.first()?;
    let label = std::path::Path::new(first)
        .file_name()
        .and_then(|name| name.to_str())
        .or_else(|| first.to_str())?;
    if label.is_empty() {
        None
    } else {
        Some(label.to_owned())
    }
}

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;
