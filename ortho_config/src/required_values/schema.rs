//! Requirement schema declared by a configuration type.
//!
//! A schema is a tree of key nodes. Each node records whether its absence is an
//! error, the aliases its loader accepts, and, for nested objects, the
//! requirements of its children. The schema is pure data: it never reads the
//! environment, the process arguments, or any configuration file.

use std::fmt;

/// Whether an absent node is an error or merely deactivates its subtree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// The loader requires the value, so its absence must be reported.
    Required,
    /// The loader tolerates absence, so the subtree is not inspected.
    Optional,
}

/// The children of a requirement node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaChildren {
    /// A leaf requirement that holds a scalar, collection, or map value.
    Leaf,
    /// A nested object requirement with its own declared children.
    Nested(Vec<SchemaField>),
}

/// One declared configuration requirement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaField {
    /// The canonical key this loader reads, after Serde renaming.
    pub key: String,
    /// Alternative keys the loader accepts for the same value.
    pub aliases: Vec<String>,
    /// Whether absence of this node is an error.
    pub activation: Activation,
    /// The node's children, or [`SchemaChildren::Leaf`].
    pub children: SchemaChildren,
}

impl SchemaField {
    /// Build one requirement node.
    ///
    /// A leaf passes [`SchemaChildren::Leaf`]; an object passes
    /// [`SchemaChildren::Nested`] with its declared children.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{Activation, SchemaChildren, SchemaField};
    ///
    /// let leaf = SchemaField::new("port", Vec::new(), Activation::Required, SchemaChildren::Leaf);
    /// assert_eq!(leaf.key, "port");
    /// ```
    #[must_use]
    pub fn new(
        key: impl Into<String>,
        aliases: Vec<String>,
        activation: Activation,
        children: SchemaChildren,
    ) -> Self {
        Self {
            key: key.into(),
            aliases,
            activation,
            children,
        }
    }

    /// Mark this node and every descendant optional.
    ///
    /// A flattened `Option<Child>` is all-or-nothing to Serde: if any part of
    /// the child fails to deserialize, the whole child resolves to `None`
    /// instead of raising a missing-value error. Demoting only the child's
    /// immediate fields would still report a missing leaf inside a nested
    /// object, so the whole composed subtree is demoted.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{Activation, SchemaChildren, SchemaField};
    ///
    /// let child = SchemaField::new("host", Vec::new(), Activation::Required, SchemaChildren::Leaf);
    /// let parent = SchemaField::new(
    ///     "database",
    ///     Vec::new(),
    ///     Activation::Required,
    ///     SchemaChildren::Nested(vec![child]),
    /// );
    /// let demoted = parent.optional_subtree();
    /// assert_eq!(demoted.activation, Activation::Optional);
    /// assert!(matches!(demoted.children, SchemaChildren::Nested(_)));
    /// ```
    #[must_use]
    pub fn optional_subtree(mut self) -> Self {
        self.activation = Activation::Optional;
        if let SchemaChildren::Nested(children) = self.children {
            self.children = SchemaChildren::Nested(
                children
                    .into_iter()
                    .map(Self::optional_subtree)
                    .collect::<Vec<_>>(),
            );
        }
        self
    }
}

/// Root-level facts needed to describe truthful supply routes.
///
/// These are collected once per configuration type. Per-field command-line
/// hints exist only for fields whose canonical key still matches the identifier
/// the generated parser uses, so a hint is never offered for a key the parser
/// cannot spell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteHints {
    /// Environment-variable prefix already normalized to end in `_`.
    pub prefix: Option<String>,
    /// Long flags the generated parser accepts for top-level fields.
    pub cli_flags: Vec<CliFlagHint>,
}

/// The command-line spelling of one top-level requirement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliFlagHint {
    /// Canonical key the flag populates.
    pub key: String,
    /// The long flag, including its leading dashes.
    pub flag: String,
    /// Whether the flag expects a value.
    pub takes_value: bool,
}

/// A type that can describe its own required configuration values.
///
/// The generated implementation only records declared fields; it does not
/// infer requirements from runtime data. This keeps enumeration deterministic
/// and independent of any particular configuration file.
pub trait RequiredConfigSchema {
    /// Collect this type's requirements in declaration order.
    ///
    /// `stack` guards against recursive types; an implementation must call
    /// [`RequiredSchemaStack::enter`] before composing children and
    /// [`RequiredSchemaStack::leave`] afterwards.
    ///
    /// # Errors
    ///
    /// Returns [`RequiredSchemaError::CyclicSchema`] when the type reappears in
    /// its own composition chain.
    fn collect_required_schema(
        stack: &mut RequiredSchemaStack,
    ) -> Result<Vec<SchemaField>, RequiredSchemaError>;

    /// Describe the routes this type's loader really accepts.
    #[must_use]
    fn required_route_hints() -> RouteHints {
        RouteHints::default()
    }
}

/// Failure to compose a requirement schema.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequiredSchemaError {
    /// A type reappeared inside its own composition chain.
    CyclicSchema {
        /// Type names in composition order, ending with the repeated type.
        chain: Vec<&'static str>,
    },
}

impl fmt::Display for RequiredSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CyclicSchema { chain } => {
                write!(
                    formatter,
                    "recursive required-value schema: {}",
                    chain.join(" -> ")
                )
            }
        }
    }
}

impl std::error::Error for RequiredSchemaError {}

/// Guards schema composition against recursive types.
///
/// The stack keys on `std::any::type_name`, so it is independent of instanti-
/// ation details that cannot appear in a name. A type may legitimately appear
/// twice in a tree at different depths; only an occurrence inside its own
/// ancestor chain is a cycle.
#[derive(Clone, Debug, Default)]
pub struct RequiredSchemaStack {
    /// Type names from the root to the innermost type being composed.
    chain: Vec<&'static str>,
}

impl RequiredSchemaStack {
    /// Build an empty stack.
    #[must_use]
    pub const fn new() -> Self {
        Self { chain: Vec::new() }
    }

    /// Record that `T` is now being composed.
    ///
    /// # Errors
    ///
    /// Returns [`RequiredSchemaError::CyclicSchema`] when `T` is already an
    /// ancestor, because composing it again cannot terminate.
    pub fn enter<T: ?Sized>(&mut self) -> Result<(), RequiredSchemaError> {
        let name = std::any::type_name::<T>();
        if self.chain.contains(&name) {
            let mut chain = self.chain.clone();
            chain.push(name);
            return Err(RequiredSchemaError::CyclicSchema { chain });
        }
        self.chain.push(name);
        Ok(())
    }

    /// Record that composition of the innermost type has finished.
    pub fn leave<T: ?Sized>(&mut self) {
        let name = std::any::type_name::<T>();
        if self.chain.last() == Some(&name) {
            self.chain.pop();
        }
    }

    /// Return the type names currently being composed.
    #[must_use]
    pub fn chain(&self) -> &[&'static str] {
        &self.chain
    }
}
