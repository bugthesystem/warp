# Tech Spec: Embedded browser pane

**Issue:** none (fork feature)

## Context

WarpUI draws each window onto one GPU surface, so a web page cannot be a WarpUI element. The pane instead attaches a native WKWebView as a child view of the window's content view and keeps it aligned with an area the pane leaves empty.

Relevant code:

- `crates/warpui/src/platform/mac/window.rs`: the macOS `Window` and its `WindowExt` trait, which now exposes `content_view()`.
- `crates/warpui_core/src/elements/gui/stack/save_position.rs` and `AppContext::element_position_by_id_at_last_frame` (`crates/warpui_core/src/core/app.rs`): record an element's rect after layout. The pane reads its content rect from here.
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

1. **Spike (step 1):** `BrowserSpike` (`app/src/browser/spike.rs`) and the "Toggle browser spike" command palette entry attach a web view at a fixed rect. This proves embedding works and is removed once the pane exists.
2. **Pane (steps 2–4):** `BrowserPane` and `BrowserView` render a tab strip and toolbar, plus an empty area wrapped in `SavePosition`. `BrowserTabs` owns one `WebView` per tab. After each frame the view reads its rect, calls `set_bounds` when it changed, and hides the web view when there is no rect. `LeafContents::Browser` is not persisted.
3. **Focus (step 5):** clicking the page area calls `focus`. Losing pane focus calls `focus_parent`. Reserved shortcuts are checked as key equivalents before the web view handles them.
4. **Overlays (step 6):** a per-window overlay count. While it is non-zero, or an overlay intersects the pane, the web view is hidden and its last snapshot is drawn in its place.
5. **Agent control (step 7):** a local MCP server on `127.0.0.1` (Streamable HTTP, launch token) exposes `browser_open`, `browser_tabs`, `browser_navigate`, `browser_read`, `browser_screenshot`, `browser_click` and `browser_type`. It is registered as a built-in MCP server. Warp's own agent tools are defined server-side, so MCP is how a client-only change adds tools.
6. **Terminal links (step 8):** a modifier-click on a `localhost` link dispatches `OpenBrowserPane { url }`.

### Tradeoffs

- A native child view cannot be clipped or overdrawn by WarpUI, hence the snapshot fallback for overlays.
- Each WKWebView runs its own WebKit content process, so many open tabs cost memory. Unloading idle tabs is a follow-up.

## Testing and validation

- Unit tests: `resolve_input` (`crates/warp_browser/src/url_input_tests.rs`). The geometry and visibility rules in step 2 live in plain functions with their own tests.
- Integration tests under `crates/integration/` for the pane's WarpUI side (opening, splitting, tab strip state), with the web view stubbed out where no display is available.
- Manual checks on macOS for every product behavior, with a screen recording per step, as CONTRIBUTING.md requires for interactive changes.

## Follow-ups

- Windows (WebView2 child HWND from winit) and Linux (WebKitGTK; X11 only for child views).
- Restoring tabs, history, bookmarks, the new-tab page and unloading idle tabs.
