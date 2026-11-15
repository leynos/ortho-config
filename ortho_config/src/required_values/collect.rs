//! Pure collector that walks a JSON value against a requirement schema.
//!
//! The collector never reports a key the loader cannot actually read, and
//! never reports a value that is present. A present value — including `null`
//! and an empty collection — counts as supplied, because the loader accepts it
//! and any domain validation is a different error.

use serde_json::{Map, Value};

use crate::error::{ConfigPath, MissingRequiredValue, MissingRequiredValues, SupplyRoute};

use super::schema::{Activation, CliFlagHint, RouteHints, SchemaChildren, SchemaField};

/// The loader namespaces every route is expressed through.
///
/// A top-level load reads `PREFIX_key` variables and keys at the file root,
/// which is the default. A subcommand load reads `PREFIX_CMDS_<NAME>_key`
/// variables and the `[cmds.<name>]` file section instead, so its routes must
/// be expressed through that scope or they would name values the loader never
/// reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RouteScope {
    /// Environment prefix prepended verbatim to every variable name.
    pub(crate) env_prefix: Option<String>,
    /// File section every configuration-file route is nested under.
    pub(crate) file_section: Vec<String>,
}

impl RouteScope {
    /// Build the scope implied by the type's own route hints.
    pub(crate) fn from_hints(hints: &RouteHints) -> Self {
        Self {
            env_prefix: hints.prefix.clone(),
            file_section: Vec::new(),
        }
    }

    /// Build the scope one selected subcommand's loader reads.
    pub(crate) fn subcommand(env_prefix: String, name: &str) -> Self {
        Self {
            env_prefix: Some(env_prefix),
            file_section: vec![String::from("cmds"), name.to_owned()],
        }
    }

    /// Render the configuration-file path for one observed key path.
    fn file_path(&self, route_path: &ConfigPath) -> ConfigPath {
        ConfigPath::new(
            self.file_section
                .iter()
                .cloned()
                .chain(route_path.segments().iter().cloned()),
        )
    }
}

/// The two route sources one collection pass reads through.
///
/// The hints describe the type — container flags and their arity — while the
/// scope describes the loader that will consume them. They are always
/// consulted together for every reported path, so they travel together rather
/// than as two parallel arguments.
#[derive(Clone, Copy)]
pub(crate) struct RouteContext<'a> {
    /// Container CLI hints applied to every reported path.
    pub(crate) hints: &'a RouteHints,
    /// Namespaces the loader reads those routes through.
    pub(crate) scope: &'a RouteScope,
}

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
    let scope = RouteScope::from_hints(hints);
    collect_missing_scoped(
        fields,
        root,
        RouteContext {
            hints,
            scope: &scope,
        },
        program,
    )
}

/// Collect every absent required value through an explicit route scope.
pub(crate) fn collect_missing_scoped(
    fields: &[SchemaField],
    root: &Value,
    routes: RouteContext<'_>,
    program: Option<String>,
) -> Option<MissingRequiredValues> {
    let object = root.as_object()?;
    let mut collector = Collector::new(routes);
    collector.collect_level(fields, object);
    MissingRequiredValues::try_new(program, collector.missing)
}

/// Recursion state for one collection pass.
///
/// The current path and the accumulating report travel together, so each step
/// only has to name the node it visits.
struct Collector<'a> {
    /// Route sources applied to every reported path.
    routes: RouteContext<'a>,
    /// Canonical path of the node currently being visited.
    path: ConfigPath,
    /// Key spellings the merged value actually holds for the same node.
    ///
    /// The canonical path names the requirement; these segments spell the
    /// keys an accepted alias actually used. Routes must follow the observed
    /// spelling, because a file or environment key that repeats the merged
    /// object under a second spelling makes the loader reject the result as a
    /// duplicate field instead of repairing the absence.
    route_path: ConfigPath,
    /// Requirements reported so far, in declaration order.
    missing: Vec<MissingRequiredValue>,
}

impl<'a> Collector<'a> {
    /// Start a pass at the root path with no requirements reported yet.
    const fn new(routes: RouteContext<'a>) -> Self {
        Self {
            routes,
            path: ConfigPath::root(),
            route_path: ConfigPath::root(),
            missing: Vec::new(),
        }
    }

    /// Visit one object level, reporting requirements the object does not
    /// satisfy.
    fn collect_level(&mut self, fields: &[SchemaField], object: &Map<String, Value>) {
        for field in fields {
            match lookup(object, field) {
                Some((key, value)) => self.collect_present(field, key, value),
                None => self.collect_absent(field),
            }
        }
    }

    /// Descend into a present value when the node declares nested requirements.
    fn collect_present(&mut self, field: &SchemaField, key: &str, value: &Value) {
        let SchemaChildren::Nested(children) = &field.children else {
            return;
        };
        // A present non-object cannot be enumerated field by field; the
        // deserializer reports that type error with the real offending value.
        let Some(object) = value.as_object() else {
            return;
        };
        self.path.push(&field.key);
        self.route_path.push(key);
        self.collect_level(children, object);
        self.path.pop();
        self.route_path.pop();
    }

    /// Report an absent node when its absence is an error.
    fn collect_absent(&mut self, field: &SchemaField) {
        if field.activation == Activation::Optional {
            return;
        }
        self.path.push(&field.key);
        self.route_path.push(&field.key);
        self.report_absent_node(field);
        self.path.pop();
        self.route_path.pop();
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
            self.route_path.push(&child.key);
            self.report_absent_node(child);
            self.path.pop();
            self.route_path.pop();
        }
        if self.missing.len() == before {
            self.push_report(true);
        }
    }

    /// Append one report entry for the current path, with the routes the loader
    /// accepts.
    fn push_report(&mut self, expects_object: bool) {
        let routes = routes_for(&self.path, &self.route_path, expects_object, self.routes);
        self.missing.push(MissingRequiredValue::new(
            self.path.clone(),
            routes,
            expects_object,
        ));
    }
}

/// Find the first key the loader accepts for `field`, with that spelling.
fn lookup<'a>(object: &'a Map<String, Value>, field: &SchemaField) -> Option<(&'a str, &'a Value)> {
    std::iter::once(field.key.as_str())
        .chain(field.aliases.iter().map(String::as_str))
        .find_map(|key| {
            object
                .get_key_value(key)
                .map(|(matched, value)| (matched.as_str(), value))
        })
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
///
/// `path` is the canonical identity used for the report and the CLI hint;
/// `route_path` is the spelling the merged value actually uses, which the
/// environment and file routes must repeat.
fn routes_for(
    path: &ConfigPath,
    route_path: &ConfigPath,
    expects_object: bool,
    routes: RouteContext<'_>,
) -> Vec<SupplyRoute> {
    let mut report = Vec::new();
    if let Some(hint) = cli_hint_for(path, expects_object, routes.hints) {
        report.push(SupplyRoute::CommandLine {
            flag: hint.flag.clone(),
            takes_value: hint.takes_value,
        });
    }
    if let Some(variable) = environment_variable(route_path, routes.scope.env_prefix.as_deref()) {
        report.push(SupplyRoute::Environment { variable });
    }
    report.push(SupplyRoute::ConfigurationFile {
        path: routes.scope.file_path(route_path),
        expects_object,
    });
    report
}

/// Render the environment variable the configured provider reads, if any.
///
/// The provider trims each variable name, strips the prefix, uppercases the
/// remainder, splits it on `__`, and lowercases every part. A route is offered
/// only when those transformations leave the key path unchanged; otherwise the
/// advertised variable would populate a different key and the suggestion would
/// be false.
///
/// Both the segments and the path they form are checked, because the provider
/// splits the *joined* name: a segment edge can merge with the separator and
/// produce a path no individual segment check can predict.
fn environment_variable(path: &ConfigPath, prefix: Option<&str>) -> Option<String> {
    let segments = path.segments();
    if !segments.iter().all(|segment| is_expressible(segment)) {
        return None;
    }
    let joined = segments.join("__");
    if !provider_reads_the_same_path(&joined, segments) {
        return None;
    }
    let variable = joined.to_ascii_uppercase();
    Some(match prefix {
        Some(configured) => format!("{configured}{variable}"),
        None => variable,
    })
}

/// Replay the provider's split transform and require it to reproduce `path`.
///
/// The injected provider replaces *every* `__` in the joined name with a dot,
/// then lowercases. An underscore at a segment edge therefore merges with the
/// separator: joining `database_` and `host` gives `database___host`, which the
/// provider reads as `database` plus `_host`. Advertising `APP_DATABASE___HOST`
/// for `database_.host` would send the user to a variable that supplies a
/// different key, so such a path gets no environment route at all.
fn provider_reads_the_same_path(joined: &str, segments: &[String]) -> bool {
    joined
        .replace("__", ".")
        .split('.')
        .eq(segments.iter().map(String::as_str))
}

/// Report whether one path segment survives the provider's key transforms.
///
/// A literal dot nests instead of naming one key, an embedded `__` splits into
/// two segments, and surrounding whitespace is trimmed. The final lowercase
/// transform rewrites any ASCII uppercase letter.
///
/// Two byte values are rejected for a different reason: they cannot appear in
/// a variable *name* at all. Every process environment treats `=` as the
/// separator between name and value, so `APP_A=B` names `APP_A` and carries
/// `B`; a NUL terminates the C string the platform call receives. Advertising
/// either spelling would name a variable no loader can ever observe.
fn is_expressible(segment: &str) -> bool {
    !segment.is_empty()
        && segment == segment.trim()
        && !segment.contains('.')
        && !segment.contains("__")
        && !segment
            .bytes()
            .any(|byte| byte.is_ascii_uppercase() || byte == b'=' || byte == 0)
}
