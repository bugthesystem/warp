# Tech Spec: Embedded browser pane

**Issue:** none (fork feature)

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
2. **Pane (step 2):** `BrowserPane` (`app/src/pane_group/pane/browser_pane.rs`) wraps `BrowserView` (`app/src/browser/view.rs`), which renders an empty content area wrapped in a single-frame `SavePosition`. `BrowserViewRegistry` tracks open views, and a frame-drawn callback calls `sync_webviews` for the window just drawn. Each view reads its content rect, scales it by the zoom factor (`app/src/browser/geometry.rs`), and moves its web view there. The web view is created the first time the content area is drawn, recreated if the pane moves to another window, and hidden while the content area is not drawn. Page title and URL arrive as `WebViewEvent`s over a channel and show in the pane header. `WorkspaceAction::OpenBrowserPane { url }` opens the pane, and "Open browser pane" in the command palette dispatches it when the flag is on. `LeafContents::Browser` is not persisted.
3. **Toolbar and tabs (steps 3–4):** a tab strip and a toolbar drawn in WarpUI above the content area. Each `BrowserTab` owns its own `WebView`; only the active tab's is placed, the rest are hidden. `⌘T` opens a tab while a browser pane has focus (a `FixedBinding` on the `BrowserView` context), and closing the last tab closes the pane. The URL field resolves input with `resolve_input`.
4. **Focus (step 5):** an initialization script posts an IPC message on `mousedown` in the page. The view then focuses its pane in Warp, keeping keyboard focus in the page. Focusing the URL field, switching tabs, or the view losing Warp focus calls `focus_parent`, which returns keyboard focus to Warp's host view. Warp's `NSWindow` already routes key equivalents that have a Warp binding to Warp before the first responder (`performKeyEquivalent:` in `crates/warpui/src/platform/mac/objc/window.m`), so Warp shortcuts work while a page has focus. Other key equivalents, such as copy and paste, reach the page.
5. **Overlays (step 6):** `Scene::is_overlaid` (`crates/warpui_core/src/scene.rs`) reports whether an overlay layer drew something that receives clicks over a rect. After each frame the view hides its web view while the content area is overlaid. Drawing a snapshot of the page in its place is a follow-up.
6. **Agent control (step 7):** `warp_browser::agent` defines the tools, the page scripts and an MCP server (rmcp, Streamable HTTP) mounted at `/mcp/browser` on Warp's existing local HTTP server (`crates/http_server`), behind a bearer token generated per launch. `BrowserAgent` (`app/src/browser/agent.rs`) receives calls over a channel and runs them on the main thread. `browser_read` numbers visible interactive elements with a `data-warp-agent-id` attribute; `browser_click` and `browser_type` act on those numbers; `browser_screenshot` uses WebKit's snapshot API. The endpoint is attached as a built-in MCP server (`builtin::browser_mcp_installation`), so Warp's agent gets the tools without setup and its MCP permission settings decide which calls need approval. Warp's own agent tools are defined server-side, so MCP is how a client-only change adds tools.
7. **Terminal links (step 8):** `TerminalView::open_link_url` sends local addresses (`warp_browser::is_local_address`) to `OpenBrowserPane` instead of the default browser.

### Tradeoffs

- A native child view cannot be clipped or overdrawn by WarpUI, hence the snapshot fallback for overlays.
- Each WKWebView runs its own WebKit content process, so many open tabs cost memory. Unloading idle tabs is a follow-up.

## Testing and validation

- Unit tests: `resolve_input` and `is_local_address` (`crates/warp_browser/src/url_input_tests.rs`), tool argument parsing (`agent/command_tests.rs`), page formatting and script escaping (`agent/page_tests.rs`), `Scene::is_overlaid` (`crates/warpui_core/src/scene_tests.rs`) and `webview_bounds` (`app/src/browser/geometry_tests.rs`).
- The MCP endpoint end to end (`agent/server_tests.rs`): requests without the token are rejected, an MCP client lists the tools, calls are forwarded and answered, and bad arguments return tool errors.
- macOS code in `warp_browser` is type-checked for `aarch64-apple-darwin` from Linux with a scratch crate; it has not been run on a Mac.
- Integration tests under `crates/integration/src/test/browser_pane.rs` for the pane's WarpUI side. `test_open_browser_pane_splits_and_focuses` covers opening, splitting and focus. Outside macOS no web view is created, so these run on every CI platform.
- Manual checks on macOS for every product behavior, with a screen recording per step, as CONTRIBUTING.md requires for interactive changes.

## Follow-ups

- Windows (WebView2 child HWND from winit) and Linux (WebKitGTK; X11 only for child views).
- Restoring tabs, history, bookmarks, the new-tab page and unloading idle tabs.
