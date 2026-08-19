const MAIN_RS: &str = include_str!("../src/main.rs");

#[test]
fn frozen_codex_registry_is_still_live_gopher_only() {
    assert!(MAIN_RS.contains("for tool in &manifest.tools"));
    assert!(MAIN_RS.contains("name: tool_name"));
    assert!(MAIN_RS.contains("description: tool.description.clone()"));
    assert!(MAIN_RS.contains("input_schema: tool.input_schema.clone()"));

    for scripting_tool in [
        "fl_scripting_search",
        "fl_scripting_describe",
        "fl_scripting_call",
    ] {
        assert!(
            !MAIN_RS.contains(scripting_tool),
            "frozen ghost-fl-agent registry source unexpectedly contains {scripting_tool}"
        );
    }
}
