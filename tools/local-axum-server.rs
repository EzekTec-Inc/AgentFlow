use agentflow::mcp::{McpClient, McpClientOptions};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
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

struct AppState {
    auth_token: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auth_token = std::env::var("AGENTFLOW_AUTH_TOKEN").unwrap_or_else(|_| {
        let token = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros());
        println!("WARNING: AGENTFLOW_AUTH_TOKEN not set.");
        println!("Generated temporary auth token: {}", token);
        token
    });

    let state = Arc::new(AppState { auth_token });

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/run", post(run_handler))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<RunRequest>,
) -> Result<Json<RunResponse>, (StatusCode, String)> {
    let auth_header = headers.get("authorization").and_then(|h| h.to_str().ok()).unwrap_or("");
    let expected = format!("Bearer {}", state.auth_token);
    
    if auth_header != expected {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()));
    }

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
            Ok(Json(RunResponse {
                status: "success".to_string(),
                tools_found: tools.len(),
                result: Some(
                    json!({"prompt": payload.prompt, "tools": tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>()}),
                ),
                error: None,
            }))
        }
        Err(e) => {
            eprintln!("Failed to connect to MCP server: {}", e);
            Ok(Json(RunResponse {
                status: "error".to_string(),
                tools_found: 0,
                result: None,
                error: Some(e.to_string()),
            }))
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
        
        let state = Arc::new(AppState { auth_token: "test-token".to_string() });
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer test-token".parse().unwrap());
        
        let res_wrapper = run_handler(State(state), headers, Json(req)).await;
        let res = res_wrapper.unwrap().0;
        
        assert_eq!(res.status, "error");
        assert!(res
            .error
            .as_ref()
            .unwrap()
            .contains("No such file or directory"));
    }
}