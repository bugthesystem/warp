use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use base64::Engine;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value, json};

use super::{BrowserCommand, ToolOutput, ToolRequest};

/// Path of the browser MCP endpoint on Warp's local HTTP server.
pub const MCP_PATH: &str = "/mcp/browser";

/// How long a tool call may wait before the agent gets an error. Calls on sites that are not local
/// wait for the user to approve the site, so this allows time for them to notice the prompt.
const TOOL_TIMEOUT: Duration = Duration::from_secs(300);

const SERVER_INSTRUCTIONS: &str = "Controls the browser panes in Warp. Call browser_read to see a \
page's text and its numbered interactive elements, then act on an element by its number with \
browser_click or browser_type. Element numbers change whenever browser_read runs. Use \
browser_console to check for errors after changing a page.";

/// Builds the router serving the browser MCP endpoint, which requires `token` as a bearer token.
/// Tool calls are sent to `requests`.
pub fn router(requests: async_channel::Sender<ToolRequest>, token: &str) -> axum::Router {
    let service = StreamableHttpService::new(
        move || {
            Ok(BrowserMcpServer {
                requests: requests.clone(),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    axum::Router::new()
        .nest_service(MCP_PATH, service)
        .layer(middleware::from_fn_with_state(
            Arc::<str>::from(token),
            require_token,
        ))
}

async fn require_token(
    State(token): State<Arc<str>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| value == &*token);
    if authorized {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Clone)]
struct BrowserMcpServer {
    requests: async_channel::Sender<ToolRequest>,
}

impl BrowserMcpServer {
    async fn run(&self, command: BrowserCommand) -> Result<ToolOutput, String> {
        let (reply, response) = tokio::sync::oneshot::channel();
        self.requests
            .send(ToolRequest { command, reply })
            .await
            .map_err(|_| "Warp is shutting down".to_owned())?;
        match tokio::time::timeout(TOOL_TIMEOUT, response).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("The browser pane closed before responding".to_owned()),
            Err(_) => Err("Timed out waiting for the page or for the user's approval".to_owned()),
        }
    }
}

impl ServerHandler for BrowserMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(SERVER_INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: tool_definitions(),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let args = request.arguments.unwrap_or_default();
        let result = match BrowserCommand::from_tool_call(&request.name, &args) {
            Ok(command) => self.run(command).await,
            Err(message) => Err(message),
        };
        let result = match result {
            Ok(ToolOutput::Text(text)) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Ok(ToolOutput::Png(png)) => CallToolResult::success(vec![ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(png),
                "image/png",
            )]),
            Err(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
        };
        Ok(result.into())
    }
}

fn tool_definitions() -> Vec<Tool> {
    let tab = json!({
        "type": "integer",
        "minimum": 0,
        "description": "Browser tab id from browser_tabs. Defaults to the most recently used tab."
    });
    let element = json!({
        "type": "integer",
        "minimum": 1,
        "description": "Element number from the latest browser_read."
    });
    vec![
        tool(
            "browser_open",
            "Open a URL in a new browser pane in Warp and make it the current tab.",
            json!({"url": {"type": "string", "description": "URL, host name or search terms."}}),
            &["url"],
        ),
        tool(
            "browser_tabs",
            "List open browser tabs with their ids, titles and URLs.",
            json!({}),
            &[],
        ),
        tool(
            "browser_navigate",
            "Load a URL in a browser tab.",
            json!({"tab": tab, "url": {"type": "string", "description": "URL, host name or search terms."}}),
            &["url"],
        ),
        tool(
            "browser_read",
            "Read a browser tab: its title, URL, text and numbered interactive elements.",
            json!({"tab": tab}),
            &[],
        ),
        tool(
            "browser_screenshot",
            "Take a PNG screenshot of what a browser tab shows.",
            json!({"tab": tab}),
            &[],
        ),
        tool(
            "browser_console",
            "Read a browser tab's console messages, uncaught errors and failed network requests \
             since the page loaded.",
            json!({
                "tab": tab,
                "clear": {"type": "boolean", "description": "Clear the messages after reading. Defaults to false."}
            }),
            &[],
        ),
        tool(
            "browser_click",
            "Click an interactive element by its number from browser_read.",
            json!({"tab": tab, "element": element}),
            &["element"],
        ),
        tool(
            "browser_type",
            "Replace the text of an input by its number from browser_read, optionally pressing Enter.",
            json!({
                "tab": tab,
                "element": element,
                "text": {"type": "string"},
                "submit": {"type": "boolean", "description": "Press Enter after typing. Defaults to false."}
            }),
            &["element", "text"],
        ),
    ]
}

fn tool(
    name: &'static str,
    description: &'static str,
    properties: Value,
    required: &[&str],
) -> Tool {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), json!("object"));
    schema.insert("properties".to_owned(), properties);
    schema.insert("required".to_owned(), json!(required));
    Tool::new(name, description, schema)
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
