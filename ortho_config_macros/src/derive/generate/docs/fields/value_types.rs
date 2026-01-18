//! Value type inference and formatting for documentation metadata.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

use crate::derive::parse::{btree_map_inner, option_inner, vec_inner};

#[derive(Clone)]
pub(super) enum ValueTypeModel {
    String,
    Integer { bits: u8, signed: bool },
    Float { bits: u8 },
    Bool,
    Duration,
    Path,
    IpAddr,
    Hostname,
    Url,
    Enum { variants: Vec<String> },
    List { of: Box<ValueTypeModel> },
    Map { of: Box<ValueTypeModel> },
    Custom { name: String },
}

impl ValueTypeModel {
    fn tokens(&self) -> TokenStream {
        match self {
            Self::String => quote! { ortho_config::docs::ValueType::String },
            Self::Integer { bits, signed } => quote! {
                ortho_config::docs::ValueType::Integer { bits: #bits, signed: #signed }
            },
            Self::Float { bits } => quote! {
                ortho_config::docs::ValueType::Float { bits: #bits }
            },
            Self::Bool => quote! { ortho_config::docs::ValueType::Bool },
            Self::Duration => quote! { ortho_config::docs::ValueType::Duration },
            Self::Path => quote! { ortho_config::docs::ValueType::Path },
            Self::IpAddr => quote! { ortho_config::docs::ValueType::IpAddr },
            Self::Hostname => quote! { ortho_config::docs::ValueType::Hostname },
            Self::Url => quote! { ortho_config::docs::ValueType::Url },
            Self::Enum { variants } => {
                let values = variants
                    .iter()
                    .map(|value| {
                        let lit = syn::LitStr::new(value, proc_macro2::Span::call_site());
                        quote! { String::from(#lit) }
                    })
                    .collect::<Vec<_>>();
                quote! {
                    ortho_config::docs::ValueType::Enum { variants: vec![ #( #values ),* ] }
                }
            }
            Self::List { of } => {
                let inner = of.tokens();
                quote! {
                    ortho_config::docs::ValueType::List { of: Box::new(#inner) }
                }
            }
            Self::Map { of } => {
                let inner = of.tokens();
                quote! {
                    ortho_config::docs::ValueType::Map { of: Box::new(#inner) }
                }
            }
            Self::Custom { name } => {
                let lit = syn::LitStr::new(name, proc_macro2::Span::call_site());
                quote! {
                    ortho_config::docs::ValueType::Custom { name: String::from(#lit) }
                }
            }
        }
    }
}

pub(super) fn value_type_tokens(value: Option<ValueTypeModel>) -> TokenStream {
    value.map_or_else(
        || quote! { None },
        |model| {
            let inner = model.tokens();
            quote! { Some(#inner) }
        },
    )
}

pub(super) fn enum_variants(value_type: &ValueTypeModel) -> Option<&Vec<String>> {
    match value_type {
        ValueTypeModel::Enum { variants } => Some(variants),
        ValueTypeModel::List { of } => enum_variants(of),
        _ => None,
    }
}

pub(super) fn parse_value_type_override(raw: &str) -> ValueTypeModel {
    let trimmed = raw.trim();
    if let Some(inner) = parse_wrapped(trimmed, "list") {
        return ValueTypeModel::List {
            of: Box::new(parse_value_type_override(inner)),
        };
    }
    if let Some(inner) = parse_wrapped(trimmed, "map") {
        return ValueTypeModel::Map {
            of: Box::new(parse_value_type_override(inner)),
        };
    }
    if let Some(inner) = parse_wrapped(trimmed, "enum") {
        let variants = split_list(inner);
        return ValueTypeModel::Enum { variants };
    }

    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "string" | "str" => ValueTypeModel::String,
        "bool" | "boolean" => ValueTypeModel::Bool,
        "duration" => ValueTypeModel::Duration,
        "path" | "pathbuf" => ValueTypeModel::Path,
        "ip" | "ipaddr" | "ipaddress" | "ipv4" | "ipv6" => ValueTypeModel::IpAddr,
        "hostname" | "host" => ValueTypeModel::Hostname,
        "url" | "uri" => ValueTypeModel::Url,
        "enum" => ValueTypeModel::Enum {
            variants: Vec::new(),
        },
        "usize" => ValueTypeModel::Integer {
            bits: target_pointer_bits(),
            signed: false,
        },
        "isize" => ValueTypeModel::Integer {
            bits: target_pointer_bits(),
            signed: true,
        },
        _ => parse_numeric_override(trimmed).unwrap_or_else(|| ValueTypeModel::Custom {
            name: trimmed.to_owned(),
        }),
    }
}

fn parse_numeric_override(raw: &str) -> Option<ValueTypeModel> {
    match raw {
        "u8" => Some(ValueTypeModel::Integer {
            bits: 8,
            signed: false,
        }),
        "u16" => Some(ValueTypeModel::Integer {
            bits: 16,
            signed: false,
        }),
        "u32" => Some(ValueTypeModel::Integer {
            bits: 32,
            signed: false,
        }),
        "u64" => Some(ValueTypeModel::Integer {
            bits: 64,
            signed: false,
        }),
        "u128" => Some(ValueTypeModel::Integer {
            bits: 128,
            signed: false,
        }),
        "i8" => Some(ValueTypeModel::Integer {
            bits: 8,
            signed: true,
        }),
        "i16" => Some(ValueTypeModel::Integer {
            bits: 16,
            signed: true,
        }),
        "i32" => Some(ValueTypeModel::Integer {
            bits: 32,
            signed: true,
        }),
        "i64" => Some(ValueTypeModel::Integer {
            bits: 64,
            signed: true,
        }),
        "i128" => Some(ValueTypeModel::Integer {
            bits: 128,
            signed: true,
        }),
        "f32" => Some(ValueTypeModel::Float { bits: 32 }),
        "f64" => Some(ValueTypeModel::Float { bits: 64 }),
        _ => None,
    }
}

fn parse_wrapped<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    if let Some(after_prefix) = value.strip_prefix(prefix) {
        let trimmed = after_prefix.trim_start();
        if let Some(inner) = trimmed.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
            return Some(inner.trim());
        }
        if let Some(inner) = trimmed.strip_prefix(':') {
            return Some(inner.trim());
        }
        if let Some(inner) = trimmed.strip_prefix('<').and_then(|v| v.strip_suffix('>')) {
            return Some(inner.trim());
        }
    }
    None
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn infer_value_type(ty: &Type) -> Option<ValueTypeModel> {
    let base_type = unwrap_reference(ty);

    if let Some(inner) = option_inner(base_type) {
        return infer_value_type(inner);
    }

    if let Some(inner) = vec_inner(base_type) {
        let list_item = infer_value_type(inner).unwrap_or_else(|| custom_type(inner));
        return Some(ValueTypeModel::List {
            of: Box::new(list_item),
        });
    }

    if let Some((_, value_type)) = btree_map_inner(base_type).or_else(|| hash_map_inner(base_type))
    {
        let map_value = infer_value_type(value_type).unwrap_or_else(|| custom_type(value_type));
        return Some(ValueTypeModel::Map {
            of: Box::new(map_value),
        });
    }

    infer_scalar_type(base_type)
}

pub(super) fn is_multi_value(ty: &Type) -> bool {
    let inner = option_inner(ty).unwrap_or(ty);
    vec_inner(inner).is_some()
}

fn infer_scalar_type(ty: &Type) -> Option<ValueTypeModel> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let ident = type_path.path.segments.last()?.ident.to_string();
    match ident.as_str() {
        "String" | "str" => Some(ValueTypeModel::String),
        "bool" => Some(ValueTypeModel::Bool),
        "u8" => Some(ValueTypeModel::Integer {
            bits: 8,
            signed: false,
        }),
        "u16" => Some(ValueTypeModel::Integer {
            bits: 16,
            signed: false,
        }),
        "u32" => Some(ValueTypeModel::Integer {
            bits: 32,
            signed: false,
        }),
        "u64" => Some(ValueTypeModel::Integer {
            bits: 64,
            signed: false,
        }),
        "u128" => Some(ValueTypeModel::Integer {
            bits: 128,
            signed: false,
        }),
        "usize" => Some(ValueTypeModel::Integer {
            bits: target_pointer_bits(),
            signed: false,
        }),
        "i8" => Some(ValueTypeModel::Integer {
            bits: 8,
            signed: true,
        }),
        "i16" => Some(ValueTypeModel::Integer {
            bits: 16,
            signed: true,
        }),
        "i32" => Some(ValueTypeModel::Integer {
            bits: 32,
            signed: true,
        }),
        "i64" => Some(ValueTypeModel::Integer {
            bits: 64,
            signed: true,
        }),
        "i128" => Some(ValueTypeModel::Integer {
            bits: 128,
            signed: true,
        }),
        "isize" => Some(ValueTypeModel::Integer {
            bits: target_pointer_bits(),
            signed: true,
        }),
        "f32" => Some(ValueTypeModel::Float { bits: 32 }),
        "f64" => Some(ValueTypeModel::Float { bits: 64 }),
        "Duration" => Some(ValueTypeModel::Duration),
        "Path" | "PathBuf" => Some(ValueTypeModel::Path),
        "IpAddr" | "Ipv4Addr" | "Ipv6Addr" => Some(ValueTypeModel::IpAddr),
        "Hostname" => Some(ValueTypeModel::Hostname),
        "Url" => Some(ValueTypeModel::Url),
        _ => Some(custom_type(ty)),
    }
}

fn custom_type(ty: &Type) -> ValueTypeModel {
    let name = type_ident_name(ty).unwrap_or_else(|| String::from("value"));
    ValueTypeModel::Custom { name }
}

fn type_ident_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|seg| seg.ident.to_string()),
        Type::Reference(reference) => type_ident_name(&reference.elem),
        _ => None,
    }
}

fn unwrap_reference(ty: &Type) -> &Type {
    if let Type::Reference(reference) = ty {
        reference.elem.as_ref()
    } else {
        ty
    }
}

fn hash_map_inner(ty: &Type) -> Option<(&Type, &Type)> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let mut segments = type_path.path.segments.iter().rev();
    let last = segments.next()?;
    if last.ident != "HashMap" {
        return None;
    }
    let _ = segments.next();
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    let mut type_args = args.args.iter().filter_map(|arg| match arg {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    });
    Some((type_args.next()?, type_args.next()?))
}

const TARGET_POINTER_BITS: u8 = if cfg!(target_pointer_width = "64") {
    64
} else if cfg!(target_pointer_width = "32") {
    32
} else {
    16
};

const fn target_pointer_bits() -> u8 {
    TARGET_POINTER_BITS
}
