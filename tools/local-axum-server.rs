use agentflow::mcp::{McpClient, McpClientOptions};
use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Deserialize)]
struct RunRequest {
    prompt: String,
    server_cmd: String,
    server_args: Vec<String>,
}

#[derive(Serialize)]
struct RunResponse {
    status: String,
    tools_found: usize,
    result: Option<Value>,
    error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/run", post(run_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_handler(Json(payload): Json<RunRequest>) -> Json<RunResponse> {
    let mut process_cmd = tokio::process::Command::new(&payload.server_cmd);
    for arg in &payload.server_args {
        process_cmd.arg(arg);
    }

    match McpClient::spawn_stdio(
        process_cmd,
        McpClientOptions {
            client_name: "local-axum-server".into(),
            client_version: "0.1.0".into(),
        },
    )
    .await
    {
        Ok(client) => {
            let tools = client.list_tools().await.unwrap_or_default();
            // Just return the tools found for now as a basic integration proof
            Json(RunResponse {
                status: "success".to_string(),
                tools_found: tools.len(),
                result: Some(json!({"prompt": payload.prompt, "tools": tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>()})),
                error: None,
            })
        }
        Err(e) => {
            eprintln!("Failed to connect to MCP server: {}", e);
            Json(RunResponse {
                status: "error".to_string(),
                tools_found: 0,
                result: None,
                error: Some(e.to_string()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_handler_invalid_command() {
        let req = RunRequest {
            prompt: "test".to_string(),
            server_cmd: "non_existent_command_12345".to_string(),
            server_args: vec![],
        };
        let res = run_handler(Json(req)).await;
        assert_eq!(res.status, "error");
        assert!(res.error.as_ref().unwrap().contains("No such file or directory"));
    }
}