//! Pure collector that walks a JSON value against a requirement schema.
//!
//! The collector never reports a key the loader cannot actually read, and
//! never reports a value that is present. A present value — including `null`
//! and an empty collection — counts as supplied, because the loader accepts it
//! and any domain validation is a different error.

use serde_json::{Map, Value};

use crate::error::{ConfigPath, MissingRequiredValue, MissingRequiredValues, SupplyRoute};

use super::schema::{Activation, CliFlagHint, RouteHints, SchemaChildren, SchemaField};

/// Collect every absent required value, or `None` when none is missing.
///
/// A root that is not a JSON object is left to the deserializer: it produces a
/// type error that already names the problem, and inventing per-field reports
/// for it would be misleading.
pub(crate) fn collect_missing(
    fields: &[SchemaField],
    root: &Value,
    hints: &RouteHints,
    program: Option<String>,
) -> Option<MissingRequiredValues> {
    let object = root.as_object()?;
    let mut collector = Collector::new(hints);
    collector.collect_level(fields, object);
    MissingRequiredValues::try_new(program, collector.missing)
}

/// Recursion state for one collection pass.
///
/// The current path and the accumulating report travel together, so each step
/// only has to name the node it visits.
struct Collector<'a> {
    /// Container routes and CLI hints applied to every reported path.
    hints: &'a RouteHints,
    /// Path of the node currently being visited.
    path: ConfigPath,
    /// Requirements reported so far, in declaration order.
    missing: Vec<MissingRequiredValue>,
}

impl<'a> Collector<'a> {
    /// Start a pass at the root path with no requirements reported yet.
    const fn new(hints: &'a RouteHints) -> Self {
        Self {
            hints,
            path: ConfigPath::root(),
            missing: Vec::new(),
        }
    }

    /// Visit one object level, reporting requirements the object does not
    /// satisfy.
    fn collect_level(&mut self, fields: &[SchemaField], object: &Map<String, Value>) {
        for field in fields {
            match lookup(object, field) {
                Some(value) => self.collect_present(field, value),
                None => self.collect_absent(field),
            }
        }
    }

    /// Descend into a present value when the node declares nested requirements.
    fn collect_present(&mut self, field: &SchemaField, value: &Value) {
        let SchemaChildren::Nested(children) = &field.children else {
            return;
        };
        // A present non-object cannot be enumerated field by field; the
        // deserializer reports that type error with the real offending value.
        let Some(object) = value.as_object() else {
            return;
        };
        self.path.push(&field.key);
        self.collect_level(children, object);
        self.path.pop();
    }

    /// Report an absent node when its absence is an error.
    fn collect_absent(&mut self, field: &SchemaField) {
        if field.activation == Activation::Optional {
            return;
        }
        self.path.push(&field.key);
        self.report_absent_node(field);
        self.path.pop();
    }

    /// Report an absent required node, descending into required descendants.
    ///
    /// An absent object activates its required descendants, so the report names
    /// each one. When it has no required descendants the object itself is
    /// reported, and the route tells the user to declare the table.
    fn report_absent_node(&mut self, field: &SchemaField) {
        let SchemaChildren::Nested(children) = &field.children else {
            self.push_report(false);
            return;
        };
        let before = self.missing.len();
        for child in children
            .iter()
            .filter(|child| child.activation == Activation::Required)
        {
            self.path.push(&child.key);
            self.report_absent_node(child);
            self.path.pop();
        }
        if self.missing.len() == before {
            self.push_report(true);
        }
    }

    /// Append one report entry for the current path, with the routes the loader
    /// accepts.
    fn push_report(&mut self, expects_object: bool) {
        let routes = routes_for(&self.path, expects_object, self.hints);
        self.missing.push(MissingRequiredValue::new(
            self.path.clone(),
            routes,
            expects_object,
        ));
    }
}

/// Find the first key the loader accepts for `field`.
fn lookup<'a>(object: &'a Map<String, Value>, field: &SchemaField) -> Option<&'a Value> {
    std::iter::once(&field.key)
        .chain(field.aliases.iter())
        .find_map(|key| object.get(key))
}

/// Find the CLI hint for a reported path, when the parser can consume it.
///
/// Only a non-object top-level key has a flag: an object is supplied through
/// its children, and a nested key has no flag of its own.
fn cli_hint_for<'a>(
    path: &ConfigPath,
    expects_object: bool,
    hints: &'a RouteHints,
) -> Option<&'a CliFlagHint> {
    if expects_object || path.len() != 1 {
        return None;
    }
    let key = path.last()?;
    hints.cli_flags.iter().find(|hint| hint.key == key)
}

/// Build the truthful route list for one missing requirement.
fn routes_for(path: &ConfigPath, expects_object: bool, hints: &RouteHints) -> Vec<SupplyRoute> {
    let mut routes = Vec::new();
    if let Some(hint) = cli_hint_for(path, expects_object, hints) {
        routes.push(SupplyRoute::CommandLine {
            flag: hint.flag.clone(),
            takes_value: hint.takes_value,
        });
    }
    if let Some(variable) = environment_variable(path, hints.prefix.as_deref()) {
        routes.push(SupplyRoute::Environment { variable });
    }
    routes.push(SupplyRoute::ConfigurationFile {
        path: path.clone(),
        expects_object,
    });
    routes
}

/// Render the environment variable the configured provider reads, if any.
///
/// The provider nests dotted keys, so a segment containing a literal dot has
/// no expressible variable and the route is withheld rather than guessed.
fn environment_variable(path: &ConfigPath, prefix: Option<&str>) -> Option<String> {
    if path.segments().iter().any(|segment| segment.contains('.')) {
        return None;
    }
    let joined = path.segments().join("__").to_ascii_uppercase();
    Some(match prefix {
        Some(configured) => format!("{configured}{joined}"),
        None => joined,
    })
}
