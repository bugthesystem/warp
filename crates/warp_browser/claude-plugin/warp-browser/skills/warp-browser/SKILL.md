---
name: warp-browser
description: Use Warp's browser and preview panes to see and check web pages and other apps next to the terminal. Use when changing a web UI, checking a page running on localhost, reproducing a bug in the browser, checking a game, engine or simulator window, or when the user mentions their browser notes, annotations or a page or preview they are looking at in Warp.
---

# Warp browser pane

Warp shows a browser pane next to this terminal, and its `warp-browser` tools act in it while the
user watches: a visible agent cursor moves to each element before clicking or typing.

## Check UI changes yourself

After changing a web UI, verify it in the browser instead of asking the user to look:

1. `browser_open` the page (for example the dev server's `http://localhost:5173`), or
   `browser_navigate` the current tab, then wait for it to load.
2. `browser_read` to see the page text and its numbered interactive elements.
3. `browser_console` to catch errors and failed requests the change caused.
4. `browser_screenshot` when the look matters (layout, spacing, colour).
5. Fix what is wrong and check again. Report what you saw, not what you expected.

## Act on the page

- `browser_click` and `browser_type` take element numbers from the latest `browser_read`; numbers
  change on every read, so read again after the page changes.
- Input is native, so pages treat it as a person's. Do not submit forms that spend money, send
  messages or change accounts unless the user asked for that.

## The user's notes

The user can pin notes to elements or areas with Annotate in the pane's toolbar. When they mention
notes, annotations, "what I marked" or "this button", call `browser_annotations`: each note has the
element, a CSS selector, the page URL and the element's HTML, which lead you to the code to change.

## Approval and control

- Local addresses need no approval. On other sites the pane may ask the user first; the call waits
  for their answer. If it is denied, do not retry that site.
- The user can pause or stop you from the pane. If a call says they stopped you, do not use the
  browser again until they ask.

## Previews

When the `preview_*` tools are available, Warp can also show a page in a headless browser, or
another app's window such as a game engine, a running game or a simulator, live in a preview pane.

1. `preview_targets` lists open previews and other apps' windows with their ids.
2. `preview_open` with `url`, `window` or `device` (an iOS simulator by name, run without its
   own window) shows it and replies with its first picture.
3. After changing code or a scene, `preview_look` returns the current picture and the recent log
   (console messages and errors for pages) in one call. `preview_screenshot` returns only the
   picture, full size on request.
4. `preview_click`, `preview_drag`, `preview_scroll`, `preview_type` and `preview_key` act on the
   preview like the user's pointer and keyboard, without moving the user's pointer or raising the
   window. Positions are pixels in the picture `preview_look` returns (its reply says the size),
   and each replies with a picture of the result. Click a field before typing into it.

Change scenes and assets through the engine's own MCP server or the shell when it has one, then
look again; use the input tools for what only the UI can do. Warp asks the user before you see another app's window or a page that is
not local; if they deny it, do not retry.
