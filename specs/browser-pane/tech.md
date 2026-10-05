# Tech Spec: Embedded browser pane

**Issues:** [warpdotdev/warp#2164](https://github.com/warpdotdev/warp/issues/2164), [warpdotdev/warp#9194](https://github.com/warpdotdev/warp/issues/9194)

## Context

WarpUI draws each window onto one GPU surface, so a web page cannot be a WarpUI element. The pane instead attaches a native WKWebView as a child view of the window's content view and keeps it aligned with an area the pane leaves empty.

Relevant code:

- `crates/warpui/src/platform/mac/window.rs`: the macOS `Window` and its `WindowExt` trait, which now exposes `content_view()`.
- `crates/warpui_core/src/elements/gui/stack/save_position.rs` and `AppContext::element_position_by_id_at_last_frame` (`crates/warpui_core/src/core/app.rs`): record an element's rect after layout. The pane reads its content rect from here. A position saved `for_single_frame` is absent from a frame that did not draw the element.
- `AppContext::on_frame_drawn` (`crates/warpui_core/src/core/app.rs`): runs callbacks after each drawn frame. It held a single callback, already used by crash recovery; it now holds a list.
- `app/src/pane_group/pane/network_log_pane.rs`: the smallest existing `PaneContent` implementation, which `BrowserPane` copies.
- `app/src/pane_group/pane/mod.rs`: `IPaneType` and `PaneId` constructors.
- `app/src/app_state.rs`: `LeafContents`, including `is_persisted`.
- `app/src/ai/mcp/builtin.rs`: built-in MCP servers attached without user setup.

## Proposed changes

### `crates/warp_browser` (new)

Wraps [wry](https://github.com/tauri-apps/wry) 0.57.

- `window_parent(window_id, ctx)` returns the window's content view, wrapped so it implements `HasWindowHandle`.
- `WebView::new(parent, url, bounds, on_event)` builds a child web view. `set_bounds`, `set_visible`, `load_url`, `go_back`, `go_forward`, `reload`, `focus` and `focus_parent` delegate to wry. Bounds are logical pixels relative to the parent's top-left corner, which matches WarpUI's coordinate space.
- `WebViewEvent` reports title changes and page loads.
- `resolve_input` maps URL-field input to a URL or a search (product behavior 6).
- Other platforms compile an uninhabited stub with the same API, so callers type-check everywhere without `cfg`.

### App

1. **Spike (step 1):** a "Toggle browser spike" command attached a web view at a fixed rect to prove embedding worked. Step 2 replaced it.
2. **Pane (step 2):** `BrowserPane` (`app/src/pane_group/pane/browser_pane.rs`) wraps `BrowserView` (`app/src/browser/view.rs`), which renders an empty content area wrapped in a single-frame `SavePosition`. `BrowserViewRegistry` tracks open views, and a frame-drawn callback calls `sync_webviews` for the window just drawn. Each view reads its content rect, scales it by the zoom factor (`app/src/browser/geometry.rs`), and moves its web view there. The web view is created the first time the content area is drawn, recreated if the pane moves to another window, and hidden while the content area is not drawn. Page title and URL arrive as `WebViewEvent`s over a channel and show in the pane header. `WorkspaceAction::OpenBrowserPane { url }` opens the pane, and "Open browser pane" in the command palette dispatches it when the flag is on.
3. **Toolbar and tabs (steps 3–4):** a tab strip and a toolbar drawn in WarpUI above the content area. Each `BrowserTab` owns its own `WebView`; only the active tab's is placed, the rest are hidden. `⌘T` opens a tab while a browser pane has focus (a `FixedBinding` on the `BrowserView` context), and closing the last tab closes the pane. The URL field resolves input with `resolve_input`.
4. **Focus (step 5):** an initialization script posts an IPC message on `mousedown` in the page. The view then focuses its pane in Warp, keeping keyboard focus in the page. Focusing the URL field, switching tabs, or the view losing Warp focus calls `focus_parent`, which returns keyboard focus to Warp's host view. Warp's `NSWindow` already routes key equivalents that have a Warp binding to Warp before the first responder (`performKeyEquivalent:` in `crates/warpui/src/platform/mac/objc/window.m`), so Warp shortcuts work while a page has focus. Other key equivalents, such as copy and paste, reach the page.
5. **Overlays (step 6):** `Scene::is_overlaid` (`crates/warpui_core/src/scene.rs`) reports whether an overlay layer drew something that receives clicks over a rect. After each frame the view hides its web view while the content area is overlaid. Drawing a snapshot of the page in its place is a follow-up.
6. **Agent control (step 7):** `warp_browser::agent` defines the tools, the page scripts and an MCP server (rmcp, Streamable HTTP) mounted at `/mcp/browser` on Warp's existing local HTTP server (`crates/http_server`), behind a bearer token. The token is created once in Warp's home config directory (`browser-mcp-token`, owner-only) and reused, so MCP clients configured outside Warp, such as Claude Code, survive restarts; "Copy Claude Code setup for browser tools" copies the matching `claude mcp add` command. `BrowserAgent` (`app/src/browser/agent.rs`) receives calls over a channel and runs them on the main thread. `browser_read` numbers visible interactive elements with a `data-warp-agent-id` attribute; `browser_click` and `browser_type` act on those numbers; `browser_screenshot` uses WebKit's snapshot API. Click and type scripts first move an in-page agent cursor to the element and outline it, then act after `ACTION_DELAY`; the tool replies once the action has run. A capture script installed at document start records console messages, uncaught errors and failed `fetch`/XHR requests, which `browser_console` returns. The endpoint is attached as a built-in MCP server (`builtin::browser_mcp_installation`), so Warp's agent gets the tools without setup. Warp's own agent tools are defined server-side, so MCP is how a client-only change adds tools. The HTTP server's runtime enables timers as well as IO, because rmcp's session worker uses them, and `tools/list` sets `ttlMs: 0` and `cacheScope: private`, which clients on protocol 2026-07-28 require.
7. **Terminal links (step 8):** `TerminalView::open_link_url` sends local addresses (`warp_browser::is_local_address`) to `OpenBrowserPane` instead of the default browser.
8. **Site approval:** `warp_browser::sites` maps a URL to the site needing approval (`None` for local addresses and non-web schemes) and stores always-allowed sites plus the auto-approve mode in `browser-agent-sites.json`. Before running a command, `BrowserAgent::approval_needed` finds the site it targets: the URL for `browser_open` and `browser_navigate`, otherwise the tab's current URL. An unapproved site parks the request in `pending`, and the view shows the banner; `resolve_approval` runs or rejects every request waiting on that site. `browser_open` to an unapproved site opens a start-page tab first so the banner has somewhere to show.
9. **Start page and history:** `warp_browser::history` keeps visits (URL, title, time, whether an agent opened it) in `browser-history.json`, capped at 200, and groups them into local apps, opened by agents and recent. `BrowserHistoryModel` is the app singleton that records visits from `LoadFinished` and title events. A tab with an empty URL renders the start page in WarpUI instead of placing a web view. When a start page is activated or focused, `detect_local_servers` (`app/src/browser/local_servers.rs`) runs `lsof` off the main thread with a timeout, and `warp_browser::local_servers::parse_lsof_listeners` keeps loopback or wildcard listeners on unprivileged ports, skipping Warp's own process and a short list of non-web services.
10. **Restore:** `LeafContents::Browser(BrowserPaneSnapshot { tab_urls, active_tab_index })` is persisted in a new `browser_panes` table (migration `2026-10-05-000000_add_browser_panes`, keyed to `pane_leaves`, URLs stored as JSON). `BrowserView::snapshot` and `BrowserView::with_tabs` convert between a view and a snapshot; restore is skipped when the flag is off.
11. **Page mouse events:** Warp's `NSWindow` `sendEvent:` (`crates/warpui/src/platform/mac/objc/window.m`) sends mouse-up, drag and right-click events straight to the content view to keep pane drag-and-drop working. A sequence that starts over a native child view of the content view, such as a web view, now goes through AppKit's normal dispatch instead, so pages get drag selection and their context menu.
12. **Annotate and screenshot:** `warp_browser::annotation` holds `annotate_script(enabled)`, which installs or removes annotate mode in a page, and `PageAnnotation`, posted over IPC as `warp:annotation:<json>` (`WebViewEvent::Annotation`); `warp:annotate-exited` reports Escape. Shift-drag draws a box from `mousedown` to `mouseup`; the script stops `pointerdown` from propagating without cancelling it, because cancelling it suppresses the mouse events the drag needs. The view copies each note to the clipboard and adds it to `BrowserAgent`, which `browser_annotations` drains. The screenshot button reuses `WebView::snapshot_png` and writes the PNG to the clipboard. Confirmations are a toolbar notice cleared after two seconds rather than a toast, because a toast over the page would hide it.
13. **Agent activity:** `BrowserCommand::step` describes each page action. `BrowserAgent::execute` reports it to the view the command targets (`BrowserView::record_agent_step`), which keeps the last five steps and clears them four seconds after the latest, tracked by a generation counter. While steps are shown or agents are paused, the view draws the activity bar and wraps the content area in an accent border, which also shrinks the web view inside it. `BrowserAgent::set_paused` holds incoming page actions in `held` and replays them through `handle_request` on resume, so approval still applies; `stop` fails held and approval-waiting calls with a message telling the agent to wait for the user.

### Tradeoffs

- A native child view cannot be clipped or overdrawn by WarpUI, hence the snapshot fallback for overlays.
- Each WKWebView runs its own WebKit content process, so many open tabs cost memory. Unloading idle tabs is a follow-up.

## Testing and validation

- Unit tests: `resolve_input` and `is_local_address` (`crates/warp_browser/src/url_input_tests.rs`), tool argument parsing (`agent/command_tests.rs`), page formatting and script escaping (`agent/page_tests.rs`), `Scene::is_overlaid` (`crates/warpui_core/src/scene_tests.rs`) and `webview_bounds` (`app/src/browser/geometry_tests.rs`).
- The MCP endpoint end to end (`agent/server_tests.rs`): requests without the token are rejected, an MCP client lists the tools, calls are forwarded and answered, and bad arguments return tool errors.
- `ApprovedSites` and `site_requiring_approval` (`sites_tests.rs`), `BrowserHistory` grouping and capping (`history_tests.rs`), the token file (`agent/token_tests.rs`), and the tool list's cache hints (`agent/server_tests.rs`).
- The annotate script (element click, shift-drag area, Enter, Escape, page handlers not running) and the click script (cursor, outline, click, stored position) were exercised in Chromium with Playwright during development.
- `test_sqlite_round_trips_browser_pane_tabs` (`app/src/persistence/sqlite_tests.rs`) saves and restores a pane's tabs.
- macOS code in `warp_browser` is type-checked for `aarch64-apple-darwin` from Linux with a scratch crate, and the pane, tools and Claude Code connection were run by hand on a Mac.
- Integration tests under `crates/integration/src/test/browser_pane.rs` for the pane's WarpUI side. `test_open_browser_pane_splits_and_focuses` covers opening, splitting and focus. Outside macOS no web view is created, so these run on every CI platform.
- Manual checks on macOS for every product behavior, with a screen recording per step, as CONTRIBUTING.md requires for interactive changes.

## Follow-ups

- Windows (WebView2 child HWND from winit) and Linux (WebKitGTK; X11 only for child views).
- Trusted input: clicks and typing are synthesized in the page, which some sites ignore (`isTrusted` checks). Native `NSEvent`s sent to the WKWebView would fix this.
- A history sidebar, bookmarks and unloading idle tabs.
- Exporting the MCP URL and token into Warp terminal sessions, so Claude Code run inside Warp configures itself.
