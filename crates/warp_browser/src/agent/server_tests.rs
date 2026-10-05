use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use rmcp::ServiceExt as _;
use rmcp::model::{CacheScope, CallToolRequestParams, ClientConfig};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use serde_json::json;

use super::{MCP_PATH, router};
use crate::agent::{ToolOutput, ToolRequest};

const TOKEN: &str = "test-token";

async fn serve() -> (String, async_channel::Receiver<ToolRequest>) {
    let (requests_tx, requests_rx) = async_channel::unbounded();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}{MCP_PATH}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router(requests_tx, TOKEN)).await });
    (url, requests_rx)
}

async fn connect(url: &str) -> RunningService<RoleClient, ClientConfig> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap(),
    );
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    let transport = StreamableHttpClientTransport::with_client(
        client,
        StreamableHttpClientTransportConfig::with_uri(url.to_owned()),
    );
    ClientConfig::default().serve(transport).await.unwrap()
}

#[tokio::test]
async fn rejects_requests_without_the_session_token() {
    let (url, _requests) = serve().await;

    let response = reqwest::Client::new()
        .post(&url)
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn lists_the_browser_tools() {
    let (url, _requests) = serve().await;
    let client = connect(&url).await;

    let tools = client.list_all_tools().await.unwrap();

    let names: Vec<_> = tools.iter().map(|tool| tool.name.to_string()).collect();
    assert_eq!(
        names,
        [
            "browser_open",
            "browser_tabs",
            "browser_navigate",
            "browser_read",
            "browser_screenshot",
            "browser_console",
            "browser_annotations",
            "browser_click",
            "browser_type",
        ]
    );
}

#[tokio::test]
async fn marks_the_tool_list_as_private_and_uncached() {
    let (url, _requests) = serve().await;
    let client = connect(&url).await;

    let result = client.list_tools(None).await.unwrap();

    assert_eq!(result.ttl_ms, Some(0));
    assert_eq!(result.cache_scope, Some(CacheScope::Private));
}

#[tokio::test]
async fn forwards_tool_calls_and_returns_the_reply() {
    let (url, requests) = serve().await;
    tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        let echo = format!("{:?}", request.command);
        let _ = request.reply.send(Ok(ToolOutput::Text(echo)));
    });
    let client = connect(&url).await;

    let result = client
        .call_tool(
            CallToolRequestParams::new("browser_click")
                .with_arguments(json!({"tab": 2, "element": 5}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();

    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        result.content[0].as_text().unwrap().text,
        "Click { tab: Some(2), element: 5 }"
    );
}

#[tokio::test]
async fn reports_invalid_arguments_as_a_tool_error() {
    let (url, _requests) = serve().await;
    let client = connect(&url).await;

    let result = client
        .call_tool(CallToolRequestParams::new("browser_click"))
        .await
        .unwrap();

    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.content[0].as_text().unwrap().text,
        "`element` is required"
    );
}
