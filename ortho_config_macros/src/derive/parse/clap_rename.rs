//! Container-level clap casing applied to derived argument names.
//!
//! `#[command(rename_all = "...")]` changes the spelling clap derives for a
//! bare `#[arg(long)]` or `#[arg(short)]`. Documentation must use the cased
//! spelling, because clap rejects the uncased flag. Literal `long`/`short`
//! spellings and an assigned `id` are already verbatim and are not re-cased.

use heck::{ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};

/// One of clap's argument-name casing styles.
///
/// The default matches `clap_derive`'s kebab-case rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ClapRenameAll {
    /// Kebab-case, clap's default.
    #[default]
    KebabCase,
    /// All lowercase with word boundaries removed.
    Lower,
    /// All uppercase with word boundaries removed.
    Upper,
    /// Lower camel case.
    LowerCamel,
    /// Upper camel (Pascal) case.
    UpperCamel,
    /// Lowercase words joined by underscores.
    Snake,
    /// Uppercase words joined by underscores.
    ShoutySnake,
    /// The field name exactly as written.
    Verbatim,
}

impl ClapRenameAll {
    /// Parses a casing name using clap's own normalisation.
    ///
    /// clap upper-camel-cases and lowercases the value before matching, so
    /// `"snake_case"`, `"snake-case"` and `"SnakeCase"` agree. Values clap
    /// rejects return `None`; the derive then keeps the default rather than
    /// rejecting an attribute clap may interpret differently.
    #[must_use]
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.to_upper_camel_case().to_lowercase().as_str() {
            "camel" | "camelcase" => Some(Self::LowerCamel),
            "kebab" | "kebabcase" => Some(Self::KebabCase),
            "pascal" | "pascalcase" => Some(Self::UpperCamel),
            "screamingsnake" | "screamingsnakecase" => Some(Self::ShoutySnake),
            "snake" | "snakecase" => Some(Self::Snake),
            "lower" | "lowercase" => Some(Self::Lower),
            "upper" | "uppercase" => Some(Self::Upper),
            "verbatim" | "verbatimcase" => Some(Self::Verbatim),
            _ => None,
        }
    }

    /// Applies this casing to a field name.
    #[must_use]
    pub(crate) fn apply(self, name: &str) -> String {
        match self {
            Self::KebabCase => name.to_kebab_case(),
            Self::LowerCamel => name.to_lower_camel_case(),
            Self::UpperCamel => name.to_upper_camel_case(),
            Self::Snake => name.to_snake_case(),
            Self::ShoutySnake => name.to_shouty_snake_case(),
            Self::Lower => name.to_snake_case().replace('_', ""),
            Self::Upper => name.to_shouty_snake_case().replace('_', ""),
            Self::Verbatim => name.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for casing normalisation and application.

    use super::ClapRenameAll;

    #[test]
    fn clap_style_names_are_normalised() {
        for (name, expected) in [
            ("camelCase", ClapRenameAll::LowerCamel),
            ("kebab-case", ClapRenameAll::KebabCase),
            ("PascalCase", ClapRenameAll::UpperCamel),
            ("SCREAMING_SNAKE_CASE", ClapRenameAll::ShoutySnake),
            ("snake_case", ClapRenameAll::Snake),
            ("lower", ClapRenameAll::Lower),
            ("UPPER", ClapRenameAll::Upper),
            ("verbatim", ClapRenameAll::Verbatim),
        ] {
            assert_eq!(
                ClapRenameAll::parse(name),
                Some(expected),
                "casing name {name} should parse"
            );
        }
    }

    #[test]
    fn casing_styles_rewrite_the_field_name() {
        assert_eq!(
            ClapRenameAll::Snake.apply("outputFile"),
            "output_file",
            "snake case should join words with underscores"
        );
        assert_eq!(
            ClapRenameAll::Lower.apply("output_file"),
            "outputfile",
            "lower case should remove word boundaries"
        );
        assert_eq!(
            ClapRenameAll::Upper.apply("output_file"),
            "OUTPUTFILE",
            "upper case should remove word boundaries"
        );
        assert_eq!(
            ClapRenameAll::Verbatim.apply("output_file"),
            "output_file",
            "verbatim should keep the source spelling"
        );
    }
}
