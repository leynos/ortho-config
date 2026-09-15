//! Type introspection helpers.
//!
//! These utilities perform shallow inspection of `syn::Type` values to
//! recognise wrapper types such as `Option<T>` and collection containers such
//! as `Vec<T>` and `BTreeMap<K, V>`.

use syn::{GenericArgument, PathArguments, Type};

/// The outer shape supported when clap's string default is inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClapDefaultValueShape {
    Scalar,
    Option,
    Vec,
}

/// The leaf type and outer shape needed to reproduce a clap string default.
#[derive(Clone)]
pub(crate) struct ClapDefaultValueType {
    pub shape: ClapDefaultValueShape,
    pub leaf: Type,
}

/// Extract the first type argument from a `PathArguments` container.
fn extract_first_type_argument(args: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(angle_args) = args else {
        return None;
    };
    let first = angle_args.args.first()?;
    let GenericArgument::Type(inner) = first else {
        return None;
    };
    Some(inner)
}

/// Returns the generic parameter if `ty` is the provided wrapper.
///
/// The check is shallow: it inspects only the outermost path and supports
/// common fully-qualified forms like `std::option::Option<T>`. The function is
/// not recursive.
fn type_inner<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    let Type::Path(p) = ty else {
        return None;
    };

    // Grab the final two segments (if available) to match paths such as
    // `std::option::Option<T>` or `crate::option::Option<T>` without caring
    // about the full prefix.
    let mut segs = p.path.segments.iter().rev();
    let last = segs.next()?;
    if last.ident != wrapper {
        return None;
    }

    // Ignore the parent segment so crate-relative forms such as
    // `crate::option::Option<T>` and custom module paths match.
    let _ = segs.next();

    extract_first_type_argument(&last.arguments)
}

/// Returns the inner type if `ty` is `Option<T>`.
///
/// This uses [`type_inner`], which is **not recursive**. It only inspects the
/// outermost layer, so `Option<Vec<T>>` yields `Vec<T>` rather than `T`.
pub(crate) fn option_inner(ty: &Type) -> Option<&Type> {
    type_inner(ty, "Option")
}

/// Extracts the element type `T` if `ty` is `Vec<T>`.
///
/// Used internally by the derive macro to identify vector fields that
/// require special append merge logic.
pub(crate) fn vec_inner(ty: &Type) -> Option<&Type> {
    type_inner(ty, "Vec")
}

/// Classifies a field type for parser-faithful clap `default_value` inference.
///
/// Only scalars, `Option<T>`, and `Vec<T>` are supported. Nested wrappers are
/// deliberately rejected because a single string default cannot faithfully
/// reconstruct their clap value shape without mirroring more clap internals.
pub(crate) fn clap_default_value_type(ty: &Type) -> Result<ClapDefaultValueType, &'static str> {
    let mut leaf = ty;
    let mut shape = None;

    loop {
        let next = option_inner(leaf)
            .map(|inner| (ClapDefaultValueShape::Option, inner))
            .or_else(|| vec_inner(leaf).map(|inner| (ClapDefaultValueShape::Vec, inner)));
        let Some((wrapper, inner)) = next else {
            break;
        };
        if shape.replace(wrapper).is_some() {
            return Err("nested `Option` and `Vec` wrappers");
        }
        leaf = inner;
    }

    if btree_map_inner(leaf).is_some() || hash_map_inner(leaf).is_some() {
        return Err("map fields");
    }
    if matches!(leaf, Type::Infer(_)) {
        return Err("an unresolved leaf type");
    }

    Ok(ClapDefaultValueType {
        shape: shape.unwrap_or(ClapDefaultValueShape::Scalar),
        leaf: leaf.clone(),
    })
}

/// Extracts the key and value types if `ty` is `BTreeMap<K, V>`.
///
/// The helper mirrors [`vec_inner`], matching both plain and fully-qualified
/// paths where the final segment is `BTreeMap`.
pub(crate) fn btree_map_inner(ty: &Type) -> Option<(&Type, &Type)> {
    map_inner(ty, "BTreeMap")
}

/// Extracts the key and value types if `ty` is `HashMap<K, V>`.
///
/// The helper mirrors [`btree_map_inner`], matching both plain and
/// fully-qualified paths where the final segment is `HashMap`.
pub(crate) fn hash_map_inner(ty: &Type) -> Option<(&Type, &Type)> {
    map_inner(ty, "HashMap")
}

/// Shared helper to extract key/value types from map-like containers.
fn map_inner<'a>(ty: &'a Type, wrapper: &str) -> Option<(&'a Type, &'a Type)> {
    let Type::Path(p) = ty else {
        return None;
    };
    let mut segs = p.path.segments.iter().rev();
    let last = segs.next()?;
    if last.ident != wrapper {
        return None;
    }
    let _ = segs.next();
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    let mut type_args = args.args.iter().filter_map(|arg| match arg {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    });
    Some((type_args.next()?, type_args.next()?))
}
