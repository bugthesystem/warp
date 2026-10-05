# Product Spec: Embedded browser pane

**Issue:** none (fork feature)
**Figma:** none provided; the reference is Cursor's browser pane

## Summary

Add a browser pane that splits next to terminal and agent panes, holds several browser tabs, and can be driven by an agent. It ships behind `FeatureFlag::BrowserPane` and supports macOS only.

## Problem

Developers running a dev server or reading docs alongside a terminal session switch to a separate browser window. Agents working in Warp cannot see or operate the page they just changed, so they cannot verify UI work on their own.

## Goals

- Open a browser pane in the current tab, alongside terminal and agent panes.
- Several browser tabs per pane, with back, forward, reload and a URL/search field.
- Warp's agent, and agents run from a Warp terminal, can open, read, screenshot and operate pages in the pane.
- Opening a `localhost` link from the terminal shows it in the pane.

## Non-goals

- Windows and Linux support (follow-ups; the code stays platform-neutral where it can).
- History sidebar, bookmarks, a new-tab page and restoring tabs after a restart (later versions).
- DevTools outside debug builds.
- Replacing the user's default browser for links outside the browser pane.

## Behavior

1. With `FeatureFlag::BrowserPane` disabled, or on a platform other than macOS, no browser pane entry point is shown and no web view is created.
2. "Open browser pane" in the command palette splits the active pane and opens a browser pane with one tab.
3. The page is drawn exactly inside the pane's content area. It follows the pane when splits are resized, the window is resized or maximized, or the display scale changes.
4. When the pane's Warp tab is not visible, the page is not visible. Returning to the tab shows it again in place.
5. Warp's command palette, menus, modals and tooltips always appear above the page, never under it.
6. Typing in the URL field and pressing Enter:
   1. loads the input as typed when it is a URL with an `http`, `https`, `file`, `about` or `data` scheme;
   2. loads `localhost`, `*.localhost`, `127.0.0.1`, `0.0.0.0` and `[::1]` addresses over `http`;
   3. loads other host names (such as `warp.dev` or `docs.rs/wry`) over `https`;
   4. otherwise searches Google for the input;
   5. loads `about:blank` when the input is empty.
7. The URL field shows the page's current URL, and the pane header shows the page title.
8. Back, forward and reload act on the active browser tab only.
9. `+` in the tab strip opens a new browser tab. With the pane focused, `⌘T` does the same.
10. Switching browser tabs shows the selected tab's page with its scroll position unchanged. Only one tab's page is visible at a time.
11. Closing a browser tab or the pane frees its pages.
12. Clicking the page gives it keyboard focus. Focusing any other pane takes keyboard focus back to Warp.
13. While the page has focus, Warp's pane-navigation and window shortcuts still work.
14. Cookies and logins persist across restarts.
15. Agent tools only reach tabs in browser panes, and each action happens in a tab the user can see.
16. An agent opening a site that is not a local address (per 6.2) needs the user's approval.
17. A modifier-click on a `localhost` link in the terminal opens it in a browser pane.

## Success criteria

Each behavior above can be checked by hand on a Mac. Behavior 6 is covered by unit tests.
