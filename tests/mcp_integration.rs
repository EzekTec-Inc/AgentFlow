use agentflow::mcp::{McpClient, McpClientOptions};
use serde_json::json;

#[cfg(feature = "mcp")]
#[tokio::test]
async fn mcp_client_server_round_trip_covers_schema_success_and_validation_error() {
    let mut command = tokio::process::Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--example",
        "mcp-server",
        "--features",
        "mcp skills",
    ]);

    let client = McpClient::spawn_stdio(
        command,
        McpClientOptions {
            client_name: "agentflow-mcp-integration-test".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    )
    .await
    .expect("spawn MCP stdio server");

    assert_eq!(client.server_name(), Some("GoA_Research_Server"));
    assert_eq!(client.server_version(), Some("0.2.0"));

    let tools = client.list_tools().await.expect("list MCP tools");
    assert!(tools.iter().any(|tool| tool.name == "crawl_goa_url"));
    assert!(tools.iter().any(|tool| tool.name == "render_report_summary"));

    let crawl_tool = tools
        .iter()
        .find(|tool| tool.name == "crawl_goa_url")
        .expect("crawl_goa_url tool present");
    assert_eq!(crawl_tool.input_schema["type"], json!("object"));
    assert_eq!(crawl_tool.input_schema["properties"]["url"]["type"], json!("string"));
    assert_eq!(crawl_tool.input_schema["required"], json!(["url"]));
    assert_eq!(crawl_tool.input_schema["additionalProperties"], json!(false));

    let missing_arg = client
        .call_tool("crawl_goa_url", json!({}))
        .await
        .expect("validation error result");
    assert_eq!(missing_arg.is_error, Some(true));
    let missing_arg_payload =
        serde_json::to_string(&missing_arg.content).expect("serialize missing arg content");
    assert!(missing_arg_payload.contains("Missing required input(s) for tool 'crawl_goa_url': url"));

    let wrong_type = client
        .call_tool("crawl_goa_url", json!({ "url": 123 }))
        .await
        .expect("wrong type validation result");
    assert_eq!(wrong_type.is_error, Some(true));
    let wrong_type_payload =
        serde_json::to_string(&wrong_type.content).expect("serialize wrong type content");
    assert!(wrong_type_payload.contains(
        "Invalid input type(s) for tool 'crawl_goa_url'; expected string for: url (number)"
    ));

    let success = client
        .call_tool(
            "render_report_summary",
            json!({ "title": "GoA Design System Report" }),
        )
        .await
        .expect("successful render_report_summary call");
    assert_ne!(success.is_error, Some(true));
    let success_payload = serde_json::to_value(&success.content).expect("serialize success content");
    let success_payload_text = success_payload.to_string();
    assert!(success_payload_text.contains("Report ready: GoA Design System Report"));
    assert!(success_payload_text.contains("exitCode"));

    client.shutdown().await.expect("shutdown MCP client");
}
