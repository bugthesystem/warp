//! Runs agents' browser tool calls against browser panes. Calls arrive from the browser MCP
//! endpoint on Warp's local HTTP server.

use std::sync::{Arc, Mutex};

use serde::Deserialize;
use warp_browser::agent::{self, BrowserCommand, ToolOutput, ToolRequest};
use warpui::{Entity, ModelContext, SingletonEntity, TypedActionView, ViewHandle};

use super::{BrowserView, BrowserViewRegistry};
use crate::workspace::{WorkspaceAction, WorkspaceRegistry};

type ToolResult = Result<ToolOutput, String>;

pub struct BrowserAgent {
    requests_tx: async_channel::Sender<ToolRequest>,
}

impl BrowserAgent {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (requests_tx, requests_rx) = async_channel::unbounded();
        ctx.spawn_stream_local(requests_rx, Self::handle_request, |_, _| {});
        Self { requests_tx }
    }

    /// The router serving the browser MCP endpoint.
    pub fn router(&self) -> axum::Router {
        agent::router(self.requests_tx.clone())
    }

    fn handle_request(&mut self, request: ToolRequest, ctx: &mut ModelContext<Self>) {
        let ToolRequest { command, reply } = request;
        let reply = move |result: ToolResult| {
            // The agent stops waiting after a timeout, which drops the receiver.
            let _ = reply.send(result);
        };

        match command {
            BrowserCommand::Open { url } => reply(open_tab(url, ctx)),
            BrowserCommand::ListTabs => reply(Ok(ToolOutput::Text(list_tabs(ctx)))),
            BrowserCommand::Navigate { tab, url } => {
                reply(with_tab(tab, ctx).map(|(tab_id, view)| {
                    let url = warp_browser::resolve_input(&url);
                    view.update(ctx, |view, ctx| view.load_url(url.clone(), ctx));
                    ToolOutput::Text(format!(
                        "Loading {url} in tab {tab_id}. Call browser_read once it loads."
                    ))
                }))
            }
            BrowserCommand::Read { tab } => {
                evaluate_in_tab(tab, &agent::read_page_script(), ctx, move |result| {
                    reply(
                        result
                            .and_then(|json| agent::format_page_snapshot(&json))
                            .map(ToolOutput::Text),
                    )
                })
            }
            BrowserCommand::Screenshot { tab } => match webview_tab(tab, ctx) {
                Ok(view) => view.read(ctx, |view, _| {
                    if let Some(webview) = view.webview() {
                        webview.snapshot_png(move |png| {
                            reply(
                                png.map(ToolOutput::Png)
                                    .ok_or_else(|| "WebKit could not take a screenshot".to_owned()),
                            )
                        });
                    }
                }),
                Err(message) => reply(Err(message)),
            },
            BrowserCommand::Click { tab, element } => {
                evaluate_in_tab(tab, &agent::click_script(element), ctx, move |result| {
                    reply(action_result(result, || {
                        format!("Clicked element {element}.")
                    }))
                })
            }
            BrowserCommand::Type {
                tab,
                element,
                text,
                submit,
            } => evaluate_in_tab(
                tab,
                &agent::type_script(element, &text, submit),
                ctx,
                move |result| {
                    reply(action_result(result, || {
                        format!("Typed into element {element}.")
                    }))
                },
            ),
        }
    }
}

impl Entity for BrowserAgent {
    type Event = ();
}

impl SingletonEntity for BrowserAgent {}

fn open_tab(url: String, ctx: &mut ModelContext<BrowserAgent>) -> ToolResult {
    let workspace = ctx
        .windows()
        .active_window()
        .and_then(|window_id| WorkspaceRegistry::as_ref(ctx).get(window_id, ctx))
        .ok_or_else(|| "No Warp window is open".to_owned())?;
    let url = warp_browser::resolve_input(&url);
    workspace.update(ctx, |workspace, ctx| {
        workspace.handle_action(
            &WorkspaceAction::OpenBrowserPane {
                url: Some(url.clone()),
            },
            ctx,
        );
    });
    let tab_id = BrowserViewRegistry::as_ref(ctx)
        .current_tab_id()
        .ok_or_else(|| "The browser pane did not open".to_owned())?;
    Ok(ToolOutput::Text(format!(
        "Opened {url} in tab {tab_id}. Call browser_read once it loads."
    )))
}

fn list_tabs(ctx: &ModelContext<BrowserAgent>) -> String {
    let registry = BrowserViewRegistry::as_ref(ctx);
    let tabs = registry.tabs(ctx);
    if tabs.is_empty() {
        return "No browser tabs are open. Use browser_open to open one.".to_owned();
    }
    let current = registry.current_tab_id();
    tabs.iter()
        .map(|(tab_id, view)| {
            let view = view.as_ref(ctx);
            let marker = if Some(*tab_id) == current {
                " (current)"
            } else {
                ""
            };
            format!(
                "{tab_id}: {} - {}{marker}",
                view.title().unwrap_or("Untitled"),
                view.url()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Resolves `tab` and makes it the current tab.
fn with_tab(
    tab: Option<u64>,
    ctx: &mut ModelContext<BrowserAgent>,
) -> Result<(u64, ViewHandle<BrowserView>), String> {
    let (tab_id, view) = BrowserViewRegistry::as_ref(ctx)
        .resolve(tab, ctx)
        .ok_or_else(|| match tab {
            Some(tab_id) => format!("No browser tab {tab_id}. Call browser_tabs to list them."),
            None => "No browser tabs are open. Use browser_open to open one.".to_owned(),
        })?;
    BrowserViewRegistry::handle(ctx).update(ctx, |registry, _| registry.set_current_tab(tab_id));
    Ok((tab_id, view))
}

/// Resolves `tab`, requiring that its web view exists.
fn webview_tab(
    tab: Option<u64>,
    ctx: &mut ModelContext<BrowserAgent>,
) -> Result<ViewHandle<BrowserView>, String> {
    let (tab_id, view) = with_tab(tab, ctx)?;
    if view.as_ref(ctx).webview().is_some() {
        Ok(view)
    } else {
        Err(format!(
            "Tab {tab_id} is not showing yet. Make sure its pane is visible and try again."
        ))
    }
}

fn evaluate_in_tab(
    tab: Option<u64>,
    script: &str,
    ctx: &mut ModelContext<BrowserAgent>,
    on_result: impl FnOnce(Result<String, String>) + Send + 'static,
) {
    let view = match webview_tab(tab, ctx) {
        Ok(view) => view,
        Err(message) => return on_result(Err(message)),
    };
    view.read(ctx, |view, _| {
        let Some(webview) = view.webview() else {
            return;
        };
        let on_result = Arc::new(Mutex::new(Some(on_result)));
        let for_callback = on_result.clone();
        let evaluated = webview.evaluate(script, move |json| {
            if let Some(on_result) = take(&for_callback) {
                on_result(Ok(json));
            }
        });
        if let Err(err) = evaluated
            && let Some(on_result) = take(&on_result)
        {
            on_result(Err(format!("Could not run the script in the page: {err}")));
        }
    });
}

/// Takes a one-shot callback out of a slot shared between a success and a failure path.
fn take<F>(slot: &Mutex<Option<F>>) -> Option<F> {
    slot.lock().ok()?.take()
}

#[derive(Deserialize)]
struct ActionOutcome {
    ok: bool,
    error: Option<String>,
}

/// Turns the JSON an action script returns into a tool result.
fn action_result(
    result: Result<String, String>,
    success_message: impl FnOnce() -> String,
) -> ToolResult {
    let outcome: ActionOutcome = serde_json::from_str(&result?)
        .map_err(|err| format!("Unexpected result from the page: {err}"))?;
    if outcome.ok {
        Ok(ToolOutput::Text(success_message()))
    } else {
        Err(outcome
            .error
            .unwrap_or_else(|| "The page rejected the action".to_owned()))
    }
}
