//! Binds the `ortho_config` behavioural feature files to the step registry.

use crate::fixtures::{
    CliDefaultContext, CollectionContext, ComposerContext, DocsContext, ErrorContext,
    ExtendsContext, FlattenContext, LocalizerContext, MergeErrorContext, RulesContext,
    SubcommandContext, binary_name, cli_default_context, collection_context, composer_context,
    context, docs_context, error_context, extends_context, flatten_context, merge_error_context,
    rules_context, subcommand_context,
};
use rstest_bdd_macros::scenarios;

scenarios!(
    "tests/features/cli_default_as_absent.feature",
    fixtures = [cli_default_context: CliDefaultContext]
);
scenarios!(
    "tests/features/cli_precedence.feature",
    fixtures = [rules_context: RulesContext]
);
scenarios!(
    "tests/features/collection_merge.feature",
    fixtures = [collection_context: CollectionContext]
);
scenarios!(
    "tests/features/config_path.feature",
    fixtures = [rules_context: RulesContext]
);
scenarios!(
    "tests/features/csv_env.feature",
    fixtures = [rules_context: RulesContext]
);
scenarios!(
    "tests/features/docs_ir.feature",
    fixtures = [docs_context: DocsContext]
);
scenarios!(
    "tests/features/error_aggregation.feature",
    fixtures = [error_context: ErrorContext]
);
scenarios!(
    "tests/features/extends.feature",
    fixtures = [extends_context: ExtendsContext]
);
scenarios!(
    "tests/features/flatten.feature",
    fixtures = [flatten_context: FlattenContext]
);
scenarios!(
    "tests/features/ignore_patterns.feature",
    fixtures = [rules_context: RulesContext]
);
scenarios!(
    "tests/features/subcommand.feature",
    fixtures = [subcommand_context: SubcommandContext]
);
scenarios!(
    "tests/features/localizer.feature",
    fixtures = [context: LocalizerContext]
);
scenarios!(
    "tests/features/merge_composer.feature",
    fixtures = [
        composer_context: ComposerContext,
        rules_context: RulesContext,
        binary_name: &'static str
    ]
);
scenarios!(
    "tests/features/merge_error_routing.feature",
    fixtures = [merge_error_context: MergeErrorContext]
);
