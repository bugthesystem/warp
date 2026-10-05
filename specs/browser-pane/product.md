# Product Spec: Embedded browser pane

**Issues:** [warpdotdev/warp#2164](https://github.com/warpdotdev/warp/issues/2164), [warpdotdev/warp#9194](https://github.com/warpdotdev/warp/issues/9194) (duplicate of #2164)
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
- History sidebar and bookmarks (later versions).
- DevTools outside debug builds.
- Replacing the user's default browser for links outside the browser pane.

## Behavior

1. With `FeatureFlag::BrowserPane` disabled, or on a platform other than macOS, no browser pane entry point is shown and no web view is created.
2. "Open browser pane" in the command palette, or `⌘⇧B`, splits the active pane and opens a browser pane with one tab.
3. The page is drawn exactly inside the pane's content area. It follows the pane when splits are resized, the window is resized or maximized, or the display scale changes.
4. When the pane's Warp tab is not visible, the page is not visible. Returning to the tab shows it again in place.
5. Warp's command palette, menus, modals and tooltips always appear above the page, never under it. While one covers the page, the page area is hidden.
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
16. Agents act on local addresses (per 6.2) without asking. Before an agent opens, reads or operates a page on any other site, the pane shows a banner naming the site with "Allow once", "Always allow" and "Deny", and the agent waits for the answer. "Always allow" is remembered for that site across restarts; `www.` is ignored when matching sites. Warp's own agent additionally goes through its MCP permission settings.
17. `⌘`-clicking a link to a local address (per 6.2) in terminal output opens it in a browser pane. Other links open in the default browser as before.
18. Before an agent clicks or types, a visible cursor moves to the element and the element is outlined, so the user can follow along. The user's own mouse pointer is never moved.
19. Agents can read a tab's console messages, uncaught errors and failed network requests.
20. "Copy Claude Code setup for browser tools" in the command palette copies a command that gives Claude Code the same browser tools. The setup keeps working after Warp restarts.
21. The toolbar always shows the agent approval mode as "Agent: Ask" or "Agent: Auto". Clicking it, or "Toggle browser agent auto-approve" in the command palette, switches mode. In Auto, agents act on every site without a banner. The mode persists across restarts.
22. A new tab shows a start page instead of a web page. It lists local apps visited recently, sites opened by agents, and other recent sites, most recent first, each with its title and address. Clicking one loads it in that tab.
23. Visited pages, their titles and whether an agent opened them are kept across restarts, up to the 200 most recent.
24. After Warp restarts, each browser pane comes back with its tabs at the URLs they last showed, and the same tab active. Tabs showing the start page come back as start pages.
25. Above its other sections, the start page (behavior 22) lists servers other processes on this machine are listening on (loopback or all interfaces, port 1024 and up), with the process name and `localhost` URL, so a dev server just started in a terminal is one click away. The list refreshes whenever a start page is shown or focused. macOS services such as AirPlay and common databases are left out.

## Success criteria

Each behavior above can be checked by hand on a Mac. Behaviors 6, 16 (site matching), 22 (grouping) and 23 are covered by unit tests, 24 by a persistence round-trip test, and 25 (parsing and filtering) by unit tests.
