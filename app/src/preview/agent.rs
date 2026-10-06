//! Runs agents' preview tool calls. Calls arrive from the MCP endpoint the browser tools are
//! served on, and only watch: agents list, open and look at previews.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use warp_browser::sites::{ApprovedSites, site_requiring_approval};
use warp_preview::agent::{OpenTarget, PreviewCommand, PreviewOutput, PreviewToolRequest};
use warp_preview::window::WindowEntry;
use warp_preview::{Source, WindowSource};
use warpui::r#async::Timer;
use warpui::{
    Entity, EntityId, ModelContext, SingletonEntity, TypedActionView, ViewHandle, WeakViewHandle,
};

use super::streams::{PreviewId, PreviewStatus, PreviewStreams};
use super::view::AppApproval;
use super::{PreviewPipView, PreviewRegistry, PreviewView};
use crate::browser::AgentApproval;
use crate::workspace::{WorkspaceAction, WorkspaceRegistry};

type ToolResult = Result<PreviewOutput, String>;
type Reply = async_channel::Sender<ToolResult>;

const APPROVED_SOURCES_FILE_NAME: &str = "preview-agent-sources.json";

/// How long `preview_open` waits for a first picture before replying without one.
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(8);
const FIRST_FRAME_POLL: Duration = Duration::from_millis(150);

/// Runs agents' preview tool calls on the main thread. Opening or looking at a page that is not
/// local, or at another app's window, waits until the user allows it where the preview shows.
pub struct PreviewAgent {
    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    requests_tx: async_channel::Sender<PreviewToolRequest>,
    /// Sites and apps the user always allows. Apps are keyed `app:<bundle id>`.
    approved: ApprovedSites,
    approved_path: Option<PathBuf>,
    /// Previews the user allowed agents to see once.
    allowed_previews: HashSet<PreviewId>,
    pending: Vec<PendingApproval>,
}

struct PendingApproval {
    key: String,
    holder: Holder,
    call: ApprovedCall,
    reply: Reply,
}

/// Where an approval prompt shows.
#[derive(Clone)]
enum Holder {
    Pane(WeakViewHandle<PreviewView>),
    PictureInPicture(WeakViewHandle<PreviewPipView>),
}

/// A call to run once approved.
enum ApprovedCall {
    Open(Source, WeakViewHandle<PreviewView>),
    Look(PreviewCommand, PreviewId),
}

/// What a call needs before it runs.
enum Access {
    Allowed,
    NeedsApproval { key: String, name: String },
}

impl PreviewAgent {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (requests_tx, requests_rx) = async_channel::unbounded();
        ctx.spawn_stream_local(requests_rx, Self::handle_request, |_, _| {});
        let approved_path = warp_core::paths::warp_home_config_dir()
            .map(|dir| dir.join(APPROVED_SOURCES_FILE_NAME));
        let approved = approved_path
            .as_deref()
            .map(ApprovedSites::load)
            .unwrap_or_default();
        Self {
            requests_tx,
            approved,
            approved_path,
            allowed_previews: HashSet::new(),
            pending: Vec::new(),
        }
    }

    /// Where the MCP endpoint sends preview tool calls.
    #[cfg(not(target_family = "wasm"))]
    pub fn requests(&self) -> async_channel::Sender<PreviewToolRequest> {
        self.requests_tx.clone()
    }

    /// Rejects the calls waiting for approval in a view that is going away.
    pub fn cancel_for_view(&mut self, view_id: EntityId, ctx: &mut ModelContext<Self>) {
        let (cancelled, pending) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition::<Vec<_>, _>(|pending| pending.holder.view_id() == view_id);
        self.pending = pending;
        for pending in cancelled {
            let _ = pending.reply.try_send(Err(
                "The preview closed before the user answered.".to_owned()
            ));
        }
        ctx.notify();
    }

    /// Applies the user's decision on `key` to every call waiting on it.
    pub fn resolve_approval(
        &mut self,
        key: &str,
        decision: AgentApproval,
        ctx: &mut ModelContext<Self>,
    ) {
        if decision == AgentApproval::Always {
            self.approved.approve(key.to_owned());
            if let Some(path) = &self.approved_path
                && let Err(err) = self.approved.save(path)
            {
                log::warn!("Failed to save sources approved for preview agents: {err:#}");
            }
        }
        let (resolved, pending) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition::<Vec<_>, _>(|pending| pending.key == key);
        self.pending = pending;
        for pending in resolved {
            pending.holder.clear_approval(ctx);
            match decision {
                AgentApproval::Deny => {
                    let _ = pending.reply.try_send(Err(format!(
                        "The user did not allow the agent to see {key}."
                    )));
                }
                AgentApproval::Once | AgentApproval::Always => match pending.call {
                    ApprovedCall::Open(source, view) => match view.upgrade(ctx) {
                        Some(view) => self.open(source, view, true, pending.reply, ctx),
                        None => {
                            let _ = pending
                                .reply
                                .try_send(Err("The preview pane closed".to_owned()));
                        }
                    },
                    ApprovedCall::Look(command, id) => {
                        self.allowed_previews.insert(id);
                        let _ = pending.reply.try_send(self.look(&command, id, ctx));
                    }
                },
            }
        }
    }

    fn handle_request(&mut self, request: PreviewToolRequest, ctx: &mut ModelContext<Self>) {
        let PreviewToolRequest { command, reply } = request;
        match command {
            PreviewCommand::Targets => {
                let previews = list_previews(ctx);
                ctx.spawn(
                    async { warp_preview::window::list_windows() },
                    move |_, windows, _| {
                        let _ = reply.try_send(Ok(PreviewOutput {
                            text: format_targets(&previews, windows),
                            jpeg: None,
                        }));
                    },
                );
            }
            PreviewCommand::Open(OpenTarget::Url(url)) => {
                let url = warp_browser::resolve_input(&url);
                let access = match site_requiring_approval(&url) {
                    Some(site) if !self.approved.allows(&site) => Access::NeedsApproval {
                        name: site.clone(),
                        key: site,
                    },
                    _ => Access::Allowed,
                };
                self.open_with_access(Source::Browser { url }, access, reply, ctx);
            }
            PreviewCommand::Open(OpenTarget::Window(window_id)) => {
                ctx.spawn(
                    async { warp_preview::window::list_windows() },
                    move |me, windows, ctx| {
                        let window = match windows {
                            Ok(windows) => windows
                                .into_iter()
                                .find(|entry| entry.source.window_id == window_id),
                            Err(err) => {
                                let _ = reply.try_send(Err(err.to_string()));
                                return;
                            }
                        };
                        let Some(window) = window else {
                            let _ = reply.try_send(Err(format!(
                                "No window {window_id}. Call preview_targets for current ids."
                            )));
                            return;
                        };
                        let access = me.window_access(&window.source);
                        me.open_with_access(Source::Window(window.source), access, reply, ctx);
                    },
                );
            }
            PreviewCommand::Screenshot { preview, .. } | PreviewCommand::Look { preview, .. } => {
                let streams = PreviewStreams::as_ref(ctx);
                let id = match preview {
                    Some(id) => Some(PreviewId(id)),
                    None => streams.last_used(),
                };
                let Some((id, source)) =
                    id.and_then(|id| streams.get(id).map(|preview| (id, preview.source.clone())))
                else {
                    let _ = reply.try_send(Err(
                        "No such preview. Call preview_targets, or preview_open to open one."
                            .to_owned(),
                    ));
                    return;
                };
                let access = if self.allowed_previews.contains(&id) {
                    Access::Allowed
                } else {
                    self.source_access(&source)
                };
                match access {
                    Access::Allowed => {
                        if let Some(step) = command.step() {
                            record_step(id, step, ctx);
                        }
                        let _ = reply.try_send(self.look(&command, id, ctx));
                    }
                    Access::NeedsApproval { key, name } => match holder_of(id, ctx) {
                        Some(holder) => {
                            holder.request_approval(
                                AppApproval {
                                    key: key.clone(),
                                    name,
                                },
                                ctx,
                            );
                            self.pending.push(PendingApproval {
                                key,
                                holder,
                                call: ApprovedCall::Look(command, id),
                                reply,
                            });
                        }
                        None => {
                            let _ = reply.try_send(Err(
                                "The preview isn't shown anywhere, so the user can't approve it."
                                    .to_owned(),
                            ));
                        }
                    },
                }
            }
        }
    }

    fn source_access(&self, source: &Source) -> Access {
        match source {
            Source::Browser { url } => match site_requiring_approval(url) {
                Some(site) if !self.approved.allows(&site) => Access::NeedsApproval {
                    name: site.clone(),
                    key: site,
                },
                _ => Access::Allowed,
            },
            Source::Window(window) => self.window_access(window),
        }
    }

    fn window_access(&self, window: &WindowSource) -> Access {
        let key = format!("app:{}", window.app_id);
        if self.approved.allows(&key) {
            Access::Allowed
        } else {
            Access::NeedsApproval {
                key,
                name: window.app_name.clone(),
            }
        }
    }

    fn open_with_access(
        &mut self,
        source: Source,
        access: Access,
        reply: Reply,
        ctx: &mut ModelContext<Self>,
    ) {
        let view = match preview_pane(ctx) {
            Ok(view) => view,
            Err(message) => {
                let _ = reply.try_send(Err(message));
                return;
            }
        };
        match access {
            Access::Allowed => self.open(source, view, false, reply, ctx),
            Access::NeedsApproval { key, name } => {
                let holder = Holder::Pane(view.downgrade());
                holder.request_approval(
                    AppApproval {
                        key: key.clone(),
                        name,
                    },
                    ctx,
                );
                self.pending.push(PendingApproval {
                    key,
                    holder,
                    call: ApprovedCall::Open(source, view.downgrade()),
                    reply,
                });
            }
        }
    }

    /// Opens `source` in `view` and replies with its first picture, or without one if none comes
    /// in time.
    fn open(
        &mut self,
        source: Source,
        view: ViewHandle<PreviewView>,
        approved_once: bool,
        reply: Reply,
        ctx: &mut ModelContext<Self>,
    ) {
        let step = PreviewCommand::Open(match &source {
            Source::Browser { url } => OpenTarget::Url(url.clone()),
            Source::Window(window) => OpenTarget::Window(window.window_id),
        })
        .step();
        let label = source.label();
        let id = PreviewStreams::handle(ctx).update(ctx, |streams, ctx| {
            let id = streams.open(source, ctx);
            streams.mark_used(id);
            id
        });
        if approved_once {
            self.allowed_previews.insert(id);
        }
        view.update(ctx, |view, ctx| {
            view.add_preview(id, ctx);
            if let Some(step) = step {
                view.record_agent_step(step, ctx);
            }
        });
        let intro = format!("Opened {label} as preview {id}.");
        reply_with_first_frame(id, intro, reply, FIRST_FRAME_TIMEOUT, ctx);
    }

    /// The picture, and for `preview_look` the recent log, of preview `id`.
    fn look(
        &self,
        command: &PreviewCommand,
        id: PreviewId,
        ctx: &mut ModelContext<Self>,
    ) -> ToolResult {
        PreviewStreams::handle(ctx).update(ctx, |streams, _| streams.mark_used(id));
        let streams = PreviewStreams::as_ref(ctx);
        let preview = streams
            .get(id)
            .ok_or_else(|| format!("Preview {id} closed"))?;
        let full_size = matches!(
            command,
            PreviewCommand::Screenshot {
                full_size: true,
                ..
            }
        );
        let jpeg = preview.latest_jpeg().and_then(|jpeg| {
            if full_size {
                Some(jpeg.to_vec())
            } else {
                small_picture(jpeg)
            }
        });
        let mut text = format!("Preview {id}: {}", preview.label());
        if let Some(status) = status_note(&preview.status) {
            text.push_str(&format!(" ({status})"));
        }
        if jpeg.is_none() {
            text.push_str(". No picture yet.");
        }
        if let PreviewCommand::Look { lines, .. } = command {
            let log: Vec<&String> = preview.recent_log(*lines).collect();
            if log.is_empty() {
                text.push_str("\nNo log lines.");
            } else {
                text.push_str("\nRecent log, oldest first:");
                for line in log {
                    text.push('\n');
                    text.push_str(line);
                }
            }
        }
        Ok(PreviewOutput { text, jpeg })
    }
}

impl Holder {
    fn view_id(&self) -> EntityId {
        match self {
            Holder::Pane(view) => view.id(),
            Holder::PictureInPicture(view) => view.id(),
        }
    }

    fn request_approval(&self, approval: AppApproval, ctx: &mut ModelContext<PreviewAgent>) {
        match self {
            Holder::Pane(view) => {
                if let Some(view) = view.upgrade(ctx) {
                    view.update(ctx, |view, ctx| view.request_approval(approval, ctx));
                }
            }
            Holder::PictureInPicture(view) => {
                if let Some(view) = view.upgrade(ctx) {
                    view.update(ctx, |view, ctx| view.request_approval(approval, ctx));
                }
            }
        }
    }

    fn clear_approval(&self, ctx: &mut ModelContext<PreviewAgent>) {
        match self {
            Holder::Pane(view) => {
                if let Some(view) = view.upgrade(ctx) {
                    view.update(ctx, |view, ctx| view.clear_approval(ctx));
                }
            }
            Holder::PictureInPicture(view) => {
                if let Some(view) = view.upgrade(ctx) {
                    view.update(ctx, |view, ctx| view.clear_approval(ctx));
                }
            }
        }
    }
}

/// Why a preview's picture may not be current.
fn status_note(status: &PreviewStatus) -> Option<String> {
    match status {
        PreviewStatus::Live => None,
        PreviewStatus::Starting => Some("starting".to_owned()),
        PreviewStatus::Minimized => {
            Some("the window is minimized, so the picture is old".to_owned())
        }
        PreviewStatus::NeedsChromium => {
            Some("waiting for the user to download Chromium on the preview's start page".to_owned())
        }
        PreviewStatus::NeedsPermission => {
            Some("waiting for the user to allow Screen Recording".to_owned())
        }
        PreviewStatus::Ended(reason) => Some(format!("ended: {reason}")),
    }
}

/// The pane, or picture-in-picture window, showing preview `id`.
fn holder_of(id: PreviewId, ctx: &ModelContext<PreviewAgent>) -> Option<Holder> {
    let registry = PreviewRegistry::as_ref(ctx);
    if let Some(view) = registry
        .views(ctx)
        .into_iter()
        .find(|view| view.as_ref(ctx).previews().contains(&id))
    {
        return Some(Holder::Pane(view.downgrade()));
    }
    registry
        .pictures_in_picture
        .iter()
        .find(|(_, pip)| pip.as_ref(ctx).previews().contains(&id))
        .map(|(_, pip)| Holder::PictureInPicture(pip.downgrade()))
}

fn record_step(id: PreviewId, step: String, ctx: &mut ModelContext<PreviewAgent>) {
    match holder_of(id, ctx) {
        Some(Holder::Pane(view)) => {
            if let Some(view) = view.upgrade(ctx) {
                view.update(ctx, |view, ctx| view.record_agent_step(step, ctx));
            }
        }
        Some(Holder::PictureInPicture(view)) => {
            if let Some(view) = view.upgrade(ctx) {
                view.update(ctx, |view, ctx| view.record_agent_step(step, ctx));
            }
        }
        None => {}
    }
}

/// The preview pane in the active window, opening one if there is none.
fn preview_pane(ctx: &mut ModelContext<PreviewAgent>) -> Result<ViewHandle<PreviewView>, String> {
    let window_id = ctx
        .windows()
        .active_window()
        .ok_or_else(|| "No Warp window is open".to_owned())?;
    let in_window = |ctx: &ModelContext<PreviewAgent>| {
        PreviewRegistry::as_ref(ctx)
            .views(ctx)
            .into_iter()
            .rev()
            .find(|view| view.window_id(ctx) == window_id)
    };
    if let Some(view) = in_window(ctx) {
        return Ok(view);
    }
    let workspace = WorkspaceRegistry::as_ref(ctx)
        .get(window_id, ctx)
        .ok_or_else(|| "No Warp window is open".to_owned())?;
    workspace.update(ctx, |workspace, ctx| {
        workspace.handle_action(
            &WorkspaceAction::OpenPreviewPane {
                previews: Vec::new(),
            },
            ctx,
        );
    });
    in_window(ctx).ok_or_else(|| "The preview pane did not open".to_owned())
}

/// Replies once preview `id` has a picture, or after `remaining` without one.
fn reply_with_first_frame(
    id: PreviewId,
    intro: String,
    reply: Reply,
    remaining: Duration,
    ctx: &mut ModelContext<PreviewAgent>,
) {
    let streams = PreviewStreams::as_ref(ctx);
    let preview = streams.get(id);
    let jpeg = preview
        .and_then(|preview| preview.latest_jpeg())
        .and_then(|jpeg| small_picture(jpeg));
    let waiting = preview.is_some_and(|preview| preview.status == PreviewStatus::Starting);
    if jpeg.is_some() || !waiting || remaining.is_zero() {
        let mut text = intro;
        match preview.and_then(|preview| status_note(&preview.status)) {
            Some(status) if jpeg.is_none() => {
                text.push_str(&format!(" It is {status}; call preview_look later."))
            }
            _ if jpeg.is_none() => text.push_str(" No picture yet; call preview_look shortly."),
            _ => text.push_str(" Call preview_look after changing something to see the result."),
        }
        let _ = reply.try_send(Ok(PreviewOutput { text, jpeg }));
        return;
    }
    ctx.spawn(Timer::after(FIRST_FRAME_POLL), move |_, _, ctx| {
        reply_with_first_frame(
            id,
            intro,
            reply,
            remaining.saturating_sub(FIRST_FRAME_POLL),
            ctx,
        )
    });
}

/// `jpeg` shrunk to the size tools return by default.
fn small_picture(jpeg: &[u8]) -> Option<Vec<u8>> {
    #[cfg(not(target_family = "wasm"))]
    {
        warp_preview::jpeg::shrink(jpeg, warp_preview::agent::SMALL_PICTURE_SIDE)
    }
    #[cfg(target_family = "wasm")]
    {
        Some(jpeg.to_vec())
    }
}

/// Open previews as (id, label, status) lines.
fn list_previews(ctx: &ModelContext<PreviewAgent>) -> Vec<String> {
    PreviewStreams::as_ref(ctx)
        .previews()
        .map(|preview| {
            let mut line = format!("- preview {}: {}", preview.id, preview.label());
            if let Some(status) = status_note(&preview.status) {
                line.push_str(&format!(" ({status})"));
            }
            line
        })
        .collect()
}

fn format_targets(
    previews: &[String],
    windows: Result<Vec<WindowEntry>, warp_preview::Error>,
) -> String {
    let mut text = String::new();
    if previews.is_empty() {
        text.push_str("No previews are open.\n");
    } else {
        text.push_str("Open previews:\n");
        for line in previews {
            text.push_str(line);
            text.push('\n');
        }
    }
    match windows {
        Ok(windows) if windows.is_empty() => text.push_str("\nNo other app windows are open.\n"),
        Ok(windows) => {
            text.push_str("\nApp windows (open one with preview_open window=<id>):\n");
            for window in windows {
                let title = if window.source.title.is_empty() {
                    String::new()
                } else {
                    format!(" \"{}\"", window.source.title)
                };
                let state = if window.on_screen { "" } else { ", minimized" };
                text.push_str(&format!(
                    "- window {}: {}{title} ({}x{}{state})\n",
                    window.source.window_id, window.source.app_name, window.width, window.height
                ));
            }
        }
        Err(warp_preview::Error::Unsupported) => {}
        Err(err) => text.push_str(&format!("\nApp windows are unavailable: {err}.\n")),
    }
    text.push_str(
        "\nAny URL, such as a local dev server, can be opened with preview_open url=<url>.",
    );
    text
}

impl Entity for PreviewAgent {
    type Event = ();
}

impl SingletonEntity for PreviewAgent {}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
