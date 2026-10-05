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
3. **Toolbar and tabs (steps 3–4):** a toolbar and a tab strip drawn in WarpUI above the content area. `BrowserTabs` owns one `WebView` per tab.
4. **Focus (step 5):** clicking the page area calls `focus`. Losing pane focus calls `focus_parent`. Reserved shortcuts are checked as key equivalents before the web view handles them.
5. **Overlays (step 6):** a per-window overlay count. While it is non-zero, or an overlay intersects the pane, the web view is hidden and its last snapshot is drawn in its place.
6. **Agent control (step 7):** a local MCP server on `127.0.0.1` (Streamable HTTP, launch token) exposes `browser_open`, `browser_tabs`, `browser_navigate`, `browser_read`, `browser_screenshot`, `browser_click` and `browser_type`. It is registered as a built-in MCP server. Warp's own agent tools are defined server-side, so MCP is how a client-only change adds tools.
7. **Terminal links (step 8):** a modifier-click on a `localhost` link dispatches `OpenBrowserPane { url }`.

### Tradeoffs

- A native child view cannot be clipped or overdrawn by WarpUI, hence the snapshot fallback for overlays.
- Each WKWebView runs its own WebKit content process, so many open tabs cost memory. Unloading idle tabs is a follow-up.

## Testing and validation

- Unit tests: `resolve_input` (`crates/warp_browser/src/url_input_tests.rs`) and `webview_bounds` (`app/src/browser/geometry_tests.rs`).
- Integration tests under `crates/integration/src/test/browser_pane.rs` for the pane's WarpUI side. `test_open_browser_pane_splits_and_focuses` covers opening, splitting and focus. Outside macOS no web view is created, so these run on every CI platform.
- Manual checks on macOS for every product behavior, with a screen recording per step, as CONTRIBUTING.md requires for interactive changes.

## Follow-ups

- Windows (WebView2 child HWND from winit) and Linux (WebKitGTK; X11 only for child views).
- Restoring tabs, history, bookmarks, the new-tab page and unloading idle tabs.
