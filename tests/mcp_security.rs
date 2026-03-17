#![cfg(feature = "mcp")]
use agentflow::mcp::{McpClient, McpClientOptions};
use serde_json::json;

#[tokio::test]
async fn mcp_server_blocks_shell_based_skill_tools() {
    let mut command = tokio::process::Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "--example",
        "mcp_server_blocked_shell",
        "--features",
        "mcp skills",
    ]);

    let client = McpClient::spawn_stdio(
        command,
        McpClientOptions {
            client_name: "agentflow-mcp-shell-block-test".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    )
    .await
    .expect("spawn MCP stdio server");

    let result = client
        .call_tool("run_shell", json!({ "payload": "hello" }))
        .await
        .expect("blocked tool result");

    assert_eq!(result.is_error, Some(true));
    let payload = serde_json::to_string(&result.content).expect("serialize content");
    assert!(payload.contains("blocked shell command 'bash'"));

    client.shutdown().await.expect("shutdown MCP client");
}
