//! MCP protocol-level tests: boot the actual server over an in-process
//! duplex transport and invoke tools via the JSON-RPC wire protocol. This is
//! the only way to validate the #[tool_router] / #[tool_handler] macro wiring.

mod common;

use rmcp::model::CallToolRequestParam;
use rmcp::service::ServiceExt;
use serde_json::json;

/// Boot a client connected to an RpgService over an in-process duplex.
/// Returns (client, _server_handle) — the handle MUST be held alive for the
/// duration of the test or the server task is dropped and the transport closes.
macro_rules! boot_client {
    () => {{
        let service = common::service_for_scenario();
        let (server_transport, client_transport) = tokio::io::duplex(8192);
        let server_handle = tokio::spawn(async move {
            let server = service.serve(server_transport).await.expect("server serve");
            server.waiting().await.expect("server waiting");
        });
        let client = ().serve(client_transport).await.expect("client serve");
        (client, server_handle)
    }};
}

#[tokio::test]
async fn protocol_list_tools_returns_all_expected() {
    let (client, _handle) = boot_client!();

    let tools = client
        .list_tools(Default::default())
        .await
        .expect("list_tools ok");

    let tool_names: Vec<String> = tools
        .tools
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    println!("Registered tools ({}): {tool_names:?}", tool_names.len());

    for expected in &[
        "encode_repo",
        "search_nodes",
        "get_node_details",
        "explore_graph",
        "get_callers",
        "get_callees",
        "get_impact",
        "get_source",
        "get_feature_tree",
        "get_ffi_bindings",
        "semantic_search",
        "find_node_at_location",
        "get_architecture_overview",
    ] {
        assert!(
            tool_names.iter().any(|n| n == expected),
            "tool '{expected}' should be registered"
        );
    }
    assert!(
        tool_names.len() >= 18,
        "should have at least 18 tools, got {}",
        tool_names.len()
    );
}

#[tokio::test]
async fn protocol_call_search_nodes_over_wire() {
    let (client, _handle) = boot_client!();

    let result = client
        .call_tool(CallToolRequestParam {
            name: "search_nodes".into(),
            arguments: json!({ "query": "PaymentProcessor" })
                .as_object()
                .cloned(),
        })
        .await
        .expect("call_tool ok");

    assert!(!result.is_error.unwrap_or(false), "should not be an error");
    let j = common::result_json(&result);
    let nodes = j["nodes"].as_array().expect("nodes array");
    assert!(
        nodes.iter().any(|n| n["name"].as_str() == Some("PaymentProcessor")),
        "should find PaymentProcessor over the wire"
    );
}

#[tokio::test]
async fn protocol_call_graph_summary_over_wire() {
    let (client, _handle) = boot_client!();

    let result = client
        .call_tool(CallToolRequestParam {
            name: "get_graph_summary".into(),
            arguments: None,
        })
        .await
        .expect("call_tool ok");

    let j = common::result_json(&result);
    assert!(
        j["total_nodes"].as_u64().unwrap_or(0) > 0,
        "summary should have nodes over the wire"
    );
}

#[tokio::test]
async fn protocol_invalid_tool_returns_error() {
    let (client, _handle) = boot_client!();

    let result = client
        .call_tool(CallToolRequestParam {
            name: "nonexistent_tool".into(),
            arguments: None,
        })
        .await;

    assert!(
        result.is_err(),
        "calling a nonexistent tool should return an error"
    );
}

#[tokio::test]
async fn protocol_invalid_params_returns_error() {
    let (client, _handle) = boot_client!();

    let result = client
        .call_tool(CallToolRequestParam {
            name: "search_nodes".into(),
            arguments: json!({}).as_object().cloned(),
        })
        .await;

    assert!(
        result.is_err(),
        "missing required param should return an error"
    );
}

#[tokio::test]
async fn protocol_server_instructions_present() {
    let (client, _handle) = boot_client!();

    let info = client.peer_info().expect("peer info");
    let instructions = info.instructions.as_ref().expect("instructions present");
    assert!(
        !instructions.is_empty(),
        "instructions should be non-empty"
    );
    let instr_str = instructions.to_string();
    assert!(
        instr_str.contains("search_nodes") || instr_str.contains("explore_graph"),
        "instructions should guide on tool usage"
    );
}
