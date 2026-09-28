//! Compiles public enum defaults and recursive documentation metadata.

use ortho_config::agent_context::{InteractionMode, MutationEffect};
use ortho_config::docs::{
    DocMetadata, FieldMetadata, HeadingIds, ORTHO_DOCS_IR_VERSION, SectionsMetadata, ValueType,
};

fn main() {
    let _interaction_mode = InteractionMode::default();
    let _mutation_effect = MutationEffect::default();

    let nested_value_type = ValueType::List {
        of: Box::new(ValueType::Map {
            of: Box::new(ValueType::String),
        }),
    };
    let field = FieldMetadata {
        name: "nested".to_owned(),
        help_id: "app.field.nested".to_owned(),
        long_help_id: None,
        value: Some(nested_value_type),
        default: None,
        required: false,
        deprecated: None,
        cli: None,
        env: None,
        file: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    };

    let leaf = empty_metadata("leaf");
    let child = DocMetadata {
        subcommands: vec![leaf],
        ..empty_metadata("child")
    };
    let _root = DocMetadata {
        fields: vec![field],
        subcommands: vec![child],
        ..empty_metadata("root")
    };
}

fn empty_metadata(app_name: &str) -> DocMetadata {
    DocMetadata {
        ir_version: ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: app_name.to_owned(),
        bin_name: None,
        about_id: format!("{app_name}.about"),
        synopsis_id: None,
        sections: SectionsMetadata {
            headings_ids: HeadingIds {
                name: "heading.name".to_owned(),
                synopsis: "heading.synopsis".to_owned(),
                description: "heading.description".to_owned(),
                options: "heading.options".to_owned(),
                environment: "heading.environment".to_owned(),
                files: "heading.files".to_owned(),
                precedence: "heading.precedence".to_owned(),
                exit_status: "heading.exit-status".to_owned(),
                examples: "heading.examples".to_owned(),
                see_also: "heading.see-also".to_owned(),
                commands: None,
            },
            discovery: None,
            precedence: None,
            examples: Vec::new(),
            links: Vec::new(),
            notes: Vec::new(),
        },
        fields: Vec::new(),
        subcommands: Vec::new(),
        windows: None,
    }
}
