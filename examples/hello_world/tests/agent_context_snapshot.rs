//! Snapshot coverage for the illustrative `hello_world` agent context.

use hello_world::cli::context::render_agent_context_json;
use insta::assert_snapshot;

#[test]
fn context_agent_context_json_snapshot() {
    let json = render_agent_context_json().expect("agent context should serialize");

    assert_snapshot!("context_agent_context_json", json);
}
