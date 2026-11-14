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
//! predict the behaviour of a hand-written `Deserialize`, an unmarked
//! `#[serde(flatten)]`, or an enum payload.
//!
//! # Examples
//!
//! ```rust
//! use ortho_config::{
//!     Activation, RequiredConfigSchema, RequiredSchemaError, RequiredSchemaStack, SchemaField,
//! };
//!
//! struct Service;
//!
//! impl RequiredConfigSchema for Service {
//!     fn collect_required_schema(
//!         stack: &mut RequiredSchemaStack,
//!     ) -> Result<Vec<SchemaField>, RequiredSchemaError> {
//!         stack.enter::<Self>()?;
//!         let fields = vec![SchemaField::leaf("endpoint", Vec::new(), Activation::Required)];
//!         stack.leave::<Self>();
//!         Ok(fields)
//!     }
//! }
//!
//! let schema = Service::collect_required_schema(&mut RequiredSchemaStack::new())
//!     .expect("schema composition cannot cycle for a leaf field");
//! assert_eq!(schema.len(), 1);
//! ```

mod collect;
mod schema;

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
///     Activation, RequiredConfigSchema, RequiredSchemaError, RequiredSchemaStack, SchemaField,
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
///         Ok(vec![SchemaField::leaf("url", Vec::new(), Activation::Required)])
///     }
/// }
///
/// let absent = json!({});
/// let error = ensure_no_missing_required_values::<Endpoint>(&absent, "demo")
///     .expect_err("an empty object cannot satisfy a required field");
/// assert!(error.to_string().contains("Missing required configuration"));
/// assert!(ensure_no_missing_required_values::<Endpoint>(&json!({"url": "x"}), "demo").is_ok());
/// ```
pub fn ensure_no_missing_required_values<T: RequiredConfigSchema>(
    value: &Value,
    program: &str,
) -> OrthoResult<()> {
    let mut stack = RequiredSchemaStack::new();
    let fields = T::collect_required_schema(&mut stack).map_err(|error| schema_failure(&error))?;
    let hints = T::required_route_hints();
    collect::collect_missing(&fields, value, &hints, Some(program.to_owned()))
        .map_or_else(|| Ok(()), |report| Err(missing_values_failure(report)))
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
/// assert_eq!(program_label_from_args(&args).as_deref(), Some("demo"));
/// assert_eq!(program_label_from_args(&[]), None);
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
