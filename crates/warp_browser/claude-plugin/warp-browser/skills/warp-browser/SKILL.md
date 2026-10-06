---
name: warp-browser
description: Use Warp's browser pane to see and check web pages next to the terminal. Use when changing a web UI, checking a page running on localhost, reproducing a bug in the browser, or when the user mentions their browser notes, annotations or a page they are looking at in Warp.
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
