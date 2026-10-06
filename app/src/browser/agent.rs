//! Runs agents' browser tool calls against browser panes. Calls arrive from the browser MCP
//! endpoint on Warp's local HTTP server.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::Deserialize;
use warp_browser::agent::{self, BrowserCommand, PagePoint, ToolOutput, ToolRequest};
use warp_browser::annotation::{PageAnnotation, format_annotations};
use warp_browser::claude_plugin;
use warp_browser::sites::{ApprovedSites, site_requiring_approval};
use warpui::r#async::Timer;

use crate::features::FeatureFlag;
use warpui::{
    Entity, EntityId, ModelContext, SingletonEntity, TypedActionView, ViewHandle, WeakViewHandle,
};

use super::{AgentApproval, BrowserView, BrowserViewRegistry};
use crate::workspace::{WorkspaceAction, WorkspaceRegistry};

type ToolResult = Result<ToolOutput, String>;

const TOKEN_FILE_NAME: &str = "browser-mcp-token";

/// The bearer token the browser MCP endpoint requires. It is stored in Warp's home config
/// directory so MCP clients set up outside Warp, such as Claude Code, keep working across launches.
pub fn mcp_token() -> &'static str {
    static TOKEN: OnceLock<String> = OnceLock::new();
    TOKEN.get_or_init(|| {
        let Some(path) =
            warp_core::paths::warp_home_config_dir().map(|dir| dir.join(TOKEN_FILE_NAME))
        else {
            return agent::new_token();
        };
        agent::load_or_create_token(&path).unwrap_or_else(|err| {
            log::warn!("Failed to store the browser MCP token; using one for this launch: {err:#}");
            agent::new_token()
        })
    })
}

pub fn mcp_url() -> String {
    format!(
        "http://127.0.0.1:{}{}",
        http_server::HttpServer::port(),
        agent::MCP_PATH
    )
}

/// Variables every Warp terminal gets, so Claude Code's warp-browser plugin reaches the browser
/// tools without setup.
pub fn terminal_env_vars() -> Vec<(OsString, OsString)> {
    if !FeatureFlag::BrowserPane.is_enabled() || !warp_browser::is_supported() {
        return Vec::new();
    }
    vec![
        (claude_plugin::URL_ENV.into(), mcp_url().into()),
        (claude_plugin::TOKEN_ENV.into(), mcp_token().into()),
    ]
}

/// Writes the warp-browser Claude Code plugin into Warp's config directory and returns the shell
/// command that installs it.
pub fn write_claude_code_plugin() -> std::io::Result<String> {
    let dir = warp_core::paths::warp_home_config_dir()
        .ok_or_else(|| std::io::Error::other("Warp has no config directory"))?
        .join(CLAUDE_PLUGIN_DIR_NAME);
    claude_plugin::write_marketplace(&dir)?;
    Ok(claude_plugin::install_command(&dir))
}

const CLAUDE_PLUGIN_DIR_NAME: &str = "claude-plugin";

const APPROVED_SITES_FILE_NAME: &str = "browser-agent-sites.json";

/// Runs agents' browser tool calls on the main thread. Calls that act on, or open, a site that is
/// not local wait until the user approves the site in the browser pane.
pub struct BrowserAgent {
    requests_tx: async_channel::Sender<ToolRequest>,
    approved_sites: ApprovedSites,
    approved_sites_path: Option<PathBuf>,
    pending: Vec<PendingApproval>,
    /// Notes the user pinned to page elements that no agent has read yet.
    annotations: Vec<PageAnnotation>,
    /// Whether the user paused agents, which holds their page actions until they resume.
    paused: bool,
    /// Calls that arrived while paused.
    held: Vec<ToolRequest>,
}

/// A tool call waiting for the user to approve `site`.
struct PendingApproval {
    site: String,
    view: WeakViewHandle<BrowserView>,
    command: BrowserCommand,
    reply: Reply,
}

const STOPPED_MESSAGE: &str = "The user stopped the agent in the browser pane. Do not use the \
browser again until they ask you to.";

type Reply = tokio::sync::oneshot::Sender<ToolResult>;

/// How long the page gets to react to a click or typing before the tool replies.
const SETTLE_DELAY: Duration = Duration::from_millis(150);

/// Ends an action's reply when it fell back to input from inside the page.
const IN_PAGE_NOTE: &str =
    " from inside the page, as native input was unavailable. Some pages ignore such input.";

impl BrowserAgent {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (requests_tx, requests_rx) = async_channel::unbounded();
        ctx.spawn_stream_local(requests_rx, Self::handle_request, |_, _| {});
        let approved_sites_path =
            warp_core::paths::warp_home_config_dir().map(|dir| dir.join(APPROVED_SITES_FILE_NAME));
        let approved_sites = approved_sites_path
            .as_deref()
            .map(ApprovedSites::load)
            .unwrap_or_default();
        Self {
            requests_tx,
            approved_sites,
            approved_sites_path,
            pending: Vec::new(),
            annotations: Vec::new(),
            paused: false,
            held: Vec::new(),
        }
    }

    /// The router serving the browser MCP endpoint.
    pub fn router(&self) -> axum::Router {
        agent::router(self.requests_tx.clone(), mcp_token())
    }

    /// How many notes no agent has read yet.
    pub fn unread_annotations(&self) -> usize {
        self.annotations.len()
    }

    /// Removes and returns the notes no agent has read yet.
    pub fn take_annotations(&mut self, ctx: &mut ModelContext<Self>) -> Vec<PageAnnotation> {
        ctx.notify();
        std::mem::take(&mut self.annotations)
    }

    /// Keeps a note for the next `browser_annotations` call.
    pub fn add_annotation(&mut self, annotation: PageAnnotation, ctx: &mut ModelContext<Self>) {
        self.annotations.push(annotation);
        ctx.notify();
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Pauses or resumes agents' page actions. Resuming runs the calls that waited.
    pub fn set_paused(&mut self, paused: bool, ctx: &mut ModelContext<Self>) {
        self.paused = paused;
        if !paused {
            for request in std::mem::take(&mut self.held) {
                self.handle_request(request, ctx);
            }
        }
        ctx.notify();
    }

    /// Rejects every call that is waiting, whether paused or waiting for approval, and resumes.
    pub fn stop(&mut self, ctx: &mut ModelContext<Self>) {
        self.paused = false;
        for request in std::mem::take(&mut self.held) {
            let _ = request.reply.send(Err(STOPPED_MESSAGE.to_owned()));
        }
        for pending in std::mem::take(&mut self.pending) {
            if let Some(view) = pending.view.upgrade(ctx) {
                view.update(ctx, |view, ctx| view.clear_agent_approval(&pending.site, ctx));
            }
            let _ = pending.reply.send(Err(STOPPED_MESSAGE.to_owned()));
        }
        ctx.notify();
    }

    /// Rejects the calls waiting for approval in a browser view that was closed.
    pub fn cancel_for_view(&mut self, view_id: EntityId, ctx: &mut ModelContext<Self>) {
        let (cancelled, pending) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition::<Vec<_>, _>(|pending| pending.view.id() == view_id);
        self.pending = pending;
        for pending in cancelled {
            let _ = pending.reply.send(Err(
                "The browser pane was closed before the user answered. Call browser_open to open \
                 a new one."
                    .to_owned(),
            ));
        }
        ctx.notify();
    }

    /// Whether agents may use any site without asking.
    pub fn auto_approve(&self) -> bool {
        self.approved_sites.auto_approve()
    }

    /// Turns auto-approval on or off. Turning it on lets every waiting call go ahead.
    pub fn toggle_auto_approve(&mut self, ctx: &mut ModelContext<Self>) {
        let auto_approve = !self.approved_sites.auto_approve();
        self.approved_sites.set_auto_approve(auto_approve);
        self.save_approved_sites();
        if auto_approve {
            let sites: Vec<String> = self
                .pending
                .iter()
                .map(|pending| pending.site.clone())
                .collect();
            for site in sites {
                self.resolve_approval(&site, AgentApproval::Once, ctx);
            }
        }
        ctx.notify();
    }

    fn save_approved_sites(&self) {
        if let Some(path) = &self.approved_sites_path
            && let Err(err) = self.approved_sites.save(path)
        {
            log::warn!("Failed to save sites approved for the browser agent: {err:#}");
        }
    }

    /// Applies the user's decision for `site` to every call waiting on it.
    pub fn resolve_approval(
        &mut self,
        site: &str,
        decision: AgentApproval,
        ctx: &mut ModelContext<Self>,
    ) {
        if decision == AgentApproval::Always {
            self.approved_sites.approve(site.to_owned());
            self.save_approved_sites();
        }
        let (resolved, pending) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition::<Vec<_>, _>(|pending| pending.site == site);
        self.pending = pending;
        for pending in resolved {
            if let Some(view) = pending.view.upgrade(ctx) {
                view.update(ctx, |view, ctx| view.clear_agent_approval(&pending.site, ctx));
            }
            match decision {
                AgentApproval::Once | AgentApproval::Always => {
                    self.execute(pending.command, pending.reply, ctx)
                }
                AgentApproval::Deny => {
                    let _ = pending.reply.send(Err(format!(
                        "The user did not allow the agent to use {site}."
                    )));
                }
            }
        }
    }

    fn handle_request(&mut self, request: ToolRequest, ctx: &mut ModelContext<Self>) {
        // With no browser pane open there is no Resume button, so nothing is held.
        if self.paused
            && request.command.step().is_some()
            && BrowserViewRegistry::as_ref(ctx)
                .resolve(None, ctx)
                .is_some()
        {
            self.held.push(request);
            ctx.notify();
            return;
        }
        let ToolRequest { command, reply } = request;
        match self.approval_needed(command, ctx) {
            Ok(Approval::NotNeeded(command)) => self.execute(command, reply, ctx),
            Ok(Approval::Needed {
                site,
                view,
                command,
            }) => {
                view.update(ctx, |view, ctx| {
                    view.request_agent_approval(site.clone(), ctx)
                });
                self.pending.push(PendingApproval {
                    site,
                    view: view.downgrade(),
                    command,
                    reply,
                });
            }
            Err(message) => {
                let _ = reply.send(Err(message));
            }
        }
    }

    /// Works out whether `command` needs the user's approval for a site. A call that opens a
    /// site becomes a navigation of a new, empty tab, so the approval prompt has a place to show.
    fn approval_needed(
        &self,
        command: BrowserCommand,
        ctx: &mut ModelContext<Self>,
    ) -> Result<Approval, String> {
        let target_site = match &command {
            BrowserCommand::ListTabs | BrowserCommand::Annotations => None,
            BrowserCommand::Open { url } | BrowserCommand::Navigate { url, .. } => {
                site_requiring_approval(&warp_browser::resolve_input(url))
            }
            BrowserCommand::Read { .. }
            | BrowserCommand::Screenshot { .. }
            | BrowserCommand::Console { .. }
            | BrowserCommand::Click { .. }
            | BrowserCommand::Type { .. } => BrowserViewRegistry::as_ref(ctx)
                .resolve(command.tab(), ctx)
                .and_then(|(tab_id, view)| {
                    view.as_ref(ctx)
                        .tab_url(tab_id)
                        .and_then(site_requiring_approval)
                }),
        };
        let Some(site) = target_site.filter(|site| !self.approved_sites.allows(site)) else {
            return Ok(Approval::NotNeeded(command));
        };

        let (command, view) = match command {
            BrowserCommand::Open { url } => {
                let (tab_id, view) = open_tab(None, ctx)?;
                (
                    BrowserCommand::Navigate {
                        tab: Some(tab_id),
                        url,
                    },
                    view,
                )
            }
            command @ (BrowserCommand::ListTabs
            | BrowserCommand::Annotations
            | BrowserCommand::Navigate { .. }
            | BrowserCommand::Read { .. }
            | BrowserCommand::Screenshot { .. }
            | BrowserCommand::Console { .. }
            | BrowserCommand::Click { .. }
            | BrowserCommand::Type { .. }) => {
                let (_, view) = with_tab(command.tab(), ctx)?;
                (command, view)
            }
        };
        Ok(Approval::Needed {
            site,
            view,
            command,
        })
    }

    fn execute(&mut self, command: BrowserCommand, reply: Reply, ctx: &mut ModelContext<Self>) {
        let reply = move |result: ToolResult| {
            // The agent stops waiting after a timeout, which drops the receiver.
            let _ = reply.send(result);
        };

        // A tab opened for the agent shows the step once it exists.
        let step = command.step();
        if let Some(step) = step.clone()
            && !matches!(command, BrowserCommand::Open { .. })
            && let Some((_, view)) = BrowserViewRegistry::as_ref(ctx).resolve(command.tab(), ctx)
        {
            view.update(ctx, |view, ctx| view.record_agent_step(step, ctx));
        }

        match command {
            BrowserCommand::Annotations => {
                let annotations = std::mem::take(&mut self.annotations);
                ctx.notify();
                reply(Ok(ToolOutput::Text(format_annotations(&annotations))));
            }
            BrowserCommand::Open { url } => reply(
                open_tab(Some(warp_browser::resolve_input(&url)), ctx).map(|(tab_id, view)| {
                    if let Some(step) = step {
                        view.update(ctx, |view, ctx| view.record_agent_step(step, ctx));
                    }
                    ToolOutput::Text(format!(
                        "Opened {url} in tab {tab_id}. Call browser_read once it loads."
                    ))
                }),
            ),
            BrowserCommand::ListTabs => reply(Ok(ToolOutput::Text(list_tabs(ctx)))),
            BrowserCommand::Navigate { tab, url } => {
                reply(with_tab(tab, ctx).map(|(tab_id, view)| {
                    let url = warp_browser::resolve_input(&url);
                    view.update(ctx, |view, ctx| {
                        view.load_url(tab_id, url.clone(), true, ctx)
                    });
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
                Ok((tab_id, view)) => view.read(ctx, |view, _| {
                    if let Some(webview) = view.tab_webview(tab_id) {
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
            BrowserCommand::Console { tab, clear } => {
                evaluate_in_tab(tab, &agent::console_script(clear), ctx, move |result| {
                    reply(
                        result
                            .and_then(|json| agent::format_console_messages(&json))
                            .map(ToolOutput::Text),
                    )
                })
            }
            BrowserCommand::Click { tab, element } => {
                point_then(tab, element, ctx, move |pointed, ctx| {
                    let (tab_id, view, point) = match pointed {
                        Ok(pointed) => pointed,
                        Err(message) => return reply(Err(message)),
                    };
                    if native_click(tab_id, &view, point, ctx) {
                        let done = format!("Clicked element {element}.");
                        reply_after_settling(Ok(ToolOutput::Text(done)), reply, ctx);
                        return;
                    }
                    evaluate_in_tab(
                        Some(tab_id),
                        &agent::click_now_script(element),
                        ctx,
                        move |result| {
                            reply(action_result(result, || {
                                format!("Clicked element {element}{IN_PAGE_NOTE}")
                            }))
                        },
                    );
                })
            }
            BrowserCommand::Type {
                tab,
                element,
                text,
                submit,
            } => point_then(tab, element, ctx, move |pointed, ctx| {
                let (tab_id, view, point) = match pointed {
                    Ok(pointed) => pointed,
                    Err(message) => return reply(Err(message)),
                };
                // Clicking first focuses the field the way a person's click would.
                native_click(tab_id, &view, point, ctx);
                evaluate_then(
                    Some(tab_id),
                    &agent::prepare_type_script(element),
                    ctx,
                    move |prepared, ctx| {
                        if let Err(message) =
                            prepared.and_then(|(_, _, json)| action_result(Ok(json), String::new))
                        {
                            return reply(Err(message));
                        }
                        let typed = view.read(ctx, |view, _| {
                            view.tab_webview(tab_id)
                                .is_some_and(|webview| webview.type_text(&text, submit))
                        });
                        if typed {
                            let done = format!("Typed into element {element}.");
                            reply_after_settling(Ok(ToolOutput::Text(done)), reply, ctx);
                            return;
                        }
                        evaluate_in_tab(
                            Some(tab_id),
                            &agent::type_now_script(element, &text, submit),
                            ctx,
                            move |result| {
                                reply(action_result(result, || {
                                    format!("Typed into element {element}{IN_PAGE_NOTE}")
                                }))
                            },
                        );
                    },
                );
            }),
        }
    }
}

impl Entity for BrowserAgent {
    type Event = ();
}

impl SingletonEntity for BrowserAgent {}

enum Approval {
    NotNeeded(BrowserCommand),
    Needed {
        site: String,
        view: ViewHandle<BrowserView>,
        command: BrowserCommand,
    },
}

/// Opens a tab showing `url`, or the new-tab page, in the current browser pane when it is in the
/// active window, and in a new browser pane otherwise.
fn open_tab(
    url: Option<String>,
    ctx: &mut ModelContext<BrowserAgent>,
) -> Result<(u64, ViewHandle<BrowserView>), String> {
    let window_id = ctx
        .windows()
        .active_window()
        .ok_or_else(|| "No Warp window is open".to_owned())?;

    let current_view = BrowserViewRegistry::as_ref(ctx)
        .resolve(None, ctx)
        .map(|(_, view)| view)
        .filter(|view| view.window_id(ctx) == window_id);
    if let Some(view) = current_view {
        let tab_id = view.update(ctx, |view, ctx| view.open_tab(url, true, ctx));
        return Ok((tab_id, view));
    }

    let workspace = WorkspaceRegistry::as_ref(ctx)
        .get(window_id, ctx)
        .ok_or_else(|| "No Warp window is open".to_owned())?;
    workspace.update(ctx, |workspace, ctx| {
        workspace.handle_action(&WorkspaceAction::OpenBrowserPane { url }, ctx);
    });
    let (tab_id, view) = BrowserViewRegistry::as_ref(ctx)
        .resolve(None, ctx)
        .ok_or_else(|| "The browser pane did not open".to_owned())?;
    view.update(ctx, |view, _| view.mark_agent_driven(tab_id));
    Ok((tab_id, view))
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
                view.tab_title(*tab_id).unwrap_or("Untitled"),
                view.tab_url(*tab_id).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Resolves `tab` and brings it to the front of its pane, so the user sees what the agent does.
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
    view.update(ctx, |view, ctx| view.select_tab(tab_id, ctx));
    Ok((tab_id, view))
}

/// Resolves `tab`, requiring that its web view exists.
fn webview_tab(
    tab: Option<u64>,
    ctx: &mut ModelContext<BrowserAgent>,
) -> Result<(u64, ViewHandle<BrowserView>), String> {
    let (tab_id, view) = with_tab(tab, ctx)?;
    if view.as_ref(ctx).tab_webview(tab_id).is_some() {
        Ok((tab_id, view))
    } else {
        Err(format!(
            "Tab {tab_id} is not showing yet. It appears once its pane is visible; try again."
        ))
    }
}

fn evaluate_in_tab(
    tab: Option<u64>,
    script: &str,
    ctx: &mut ModelContext<BrowserAgent>,
    on_result: impl FnOnce(Result<String, String>) + Send + 'static,
) {
    let (tab_id, view) = match webview_tab(tab, ctx) {
        Ok(tab) => tab,
        Err(message) => return on_result(Err(message)),
    };
    view.read(ctx, |view, _| {
        let Some(webview) = view.tab_webview(tab_id) else {
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

/// The page runs clicks and typing once the agent cursor reaches the element, so wait for that
/// before replying; otherwise the agent could read the page before the action happened.
/// Runs `script` in the page of `tab` and calls `then` on the main thread with the tab, its view
/// and the script's result.
fn evaluate_then(
    tab: Option<u64>,
    script: &str,
    ctx: &mut ModelContext<BrowserAgent>,
    then: impl FnOnce(Evaluated, &mut ModelContext<BrowserAgent>) + 'static,
) {
    let (tab_id, view) = match webview_tab(tab, ctx) {
        Ok(tab) => tab,
        Err(message) => return then(Err(message), ctx),
    };
    let (result_tx, result_rx) = futures::channel::oneshot::channel();
    evaluate_in_tab(Some(tab_id), script, ctx, move |result| {
        let _ = result_tx.send(result);
    });
    ctx.spawn(
        async move {
            result_rx
                .await
                .unwrap_or_else(|_| Err("The page did not answer".to_owned()))
        },
        move |_, result, ctx| then(result.map(|json| (tab_id, view, json)), ctx),
    );
}

/// Glides the agent cursor to the element `element` and, once it arrives, calls `then` with the
/// point it reached.
fn point_then(
    tab: Option<u64>,
    element: u64,
    ctx: &mut ModelContext<BrowserAgent>,
    then: impl FnOnce(Pointed, &mut ModelContext<BrowserAgent>) + 'static,
) {
    evaluate_then(
        tab,
        &agent::point_script(element),
        ctx,
        move |result, ctx| match result
            .and_then(|(tab_id, view, json)| Ok((tab_id, view, agent::page_point(&json)?)))
        {
            Ok(pointed) => {
                ctx.spawn(Timer::after(agent::ACTION_DELAY), move |_, _, ctx| {
                    then(Ok(pointed), ctx)
                });
            }
            Err(message) => then(Err(message), ctx),
        },
    );
}

/// Shows the agent cursor pressing at `point` and clicks there with native input. Returns whether
/// the native click was sent.
fn native_click(
    tab_id: u64,
    view: &ViewHandle<BrowserView>,
    point: PagePoint,
    ctx: &ModelContext<BrowserAgent>,
) -> bool {
    view.read(ctx, |view, _| {
        view.tab_webview(tab_id).is_some_and(|webview| {
            let _ = webview.evaluate(&agent::press_script(point.x, point.y), |_| {});
            webview.click_at(point.x, point.y)
        })
    })
}

/// Replies once the page has had a moment to react to the action.
fn reply_after_settling(
    result: ToolResult,
    reply: impl FnOnce(ToolResult) + 'static,
    ctx: &mut ModelContext<BrowserAgent>,
) {
    ctx.spawn(Timer::after(SETTLE_DELAY), move |_, _, _| reply(result));
}

/// Takes a one-shot callback out of a slot shared between a success and a failure path.
fn take<F>(slot: &Mutex<Option<F>>) -> Option<F> {
    slot.lock().ok()?.take()
}

/// A tab, its view, and the result of a script run in its page.
type Evaluated = Result<(u64, ViewHandle<BrowserView>, String), String>;

/// A tab, its view, and the point the agent cursor reached in its page.
type Pointed = Result<(u64, ViewHandle<BrowserView>, PagePoint), String>;

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
