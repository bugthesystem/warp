//! Notes the user pins to page elements for agents, and the page script that collects them.

use serde::Deserialize;

use crate::PAGE_ACTION_PREFIX;

/// The page action that asks Warp to send the saved notes to the agent.
pub const SEND_NOTES_ACTION: &str = "send-notes";

/// Prefix of the message the annotate script posts for each note, followed by the note as JSON.
pub const ANNOTATION_MESSAGE_PREFIX: &str = "warp:annotation:";

/// Message the annotate script posts when the user leaves annotate mode from the page.
pub const ANNOTATE_EXITED_MESSAGE: &str = "warp:annotate-exited";

/// A note the user wrote about one element of a page.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct PageAnnotation {
    pub url: String,
    /// A CSS selector that matched the element when it was picked.
    pub selector: String,
    /// The element's tag and visible name, such as `button "Sign in"`.
    pub element: String,
    /// The start of the element's HTML.
    pub html: String,
    pub note: String,
}

impl PageAnnotation {
    /// Parses a note posted by the annotate script.
    pub fn parse(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// Formats notes as text for an agent, or for pasting into one.
pub fn format_annotations(annotations: &[PageAnnotation]) -> String {
    if annotations.is_empty() {
        return "No annotations. The user can add some with Annotate in the browser pane's \
                toolbar."
            .to_owned();
    }
    let mut output = String::from("Browser annotations:\n");
    for (index, annotation) in annotations.iter().enumerate() {
        let PageAnnotation {
            url,
            selector,
            element,
            html,
            note,
        } = annotation;
        output.push_str(&format!(
            "\n{number}. {note}\n   Element: {element}\n   Selector: {selector}\n   Page: {url}\n   HTML: {html}\n",
            number = index + 1,
        ));
    }
    output
}

/// A script that turns annotate mode on or off in a page. While it is on, a banner explains the
/// mode, hovering outlines elements with their name and size, and clicking one, or shift-dragging
/// a box around an area, opens a note card. Each saved note is posted to Warp and the spot keeps
/// an outline and a numbered badge. Escape closes the card, then leaves annotate mode, as does the
/// banner's Done button.
pub fn annotate_script(enabled: bool) -> String {
    ANNOTATE_SCRIPT
        .replace("__ENABLED__", if enabled { "true" } else { "false" })
        .replace("__ANNOTATION_PREFIX__", ANNOTATION_MESSAGE_PREFIX)
        .replace("__EXITED__", ANNOTATE_EXITED_MESSAGE)
        .replace("__PAGE_ACTION_PREFIX__", PAGE_ACTION_PREFIX)
        .replace("__SEND_NOTES__", SEND_NOTES_ACTION)
}

const ANNOTATE_SCRIPT: &str = r##"(() => {
  const enabled = __ENABLED__;
  const existing = window.__warpAnnotate;
  if (!enabled) { if (existing) existing.stop(); return; }
  if (existing) return;

  const accent = "#7c5cff";
  const tint = alpha => `rgba(124,92,255,${alpha})`;
  const font = "-apple-system,BlinkMacSystemFont,system-ui,sans-serif";
  const z = "2147483646";
  const ui = new Set();
  const own = el => { for (let node = el; node; node = node.parentElement) if (ui.has(node)) return true; return false; };
  const make = (tag, css, parent) => {
    const el = document.createElement(tag);
    el.style.cssText = css;
    if (parent) parent.appendChild(el);
    else { el.setAttribute("aria-hidden", "true"); ui.add(el); document.documentElement.appendChild(el); }
    return el;
  };
  // Web Animations rather than a stylesheet, which a page's Content-Security-Policy can block.
  const enter = (el, from) => el.animate([{ opacity: 0, transform: from }, { opacity: 1, transform: "none" }],
    { duration: 180, easing: "cubic-bezier(.2,.8,.2,1)" });
  const leave = el => { ui.delete(el); el.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 120 }).onfinish = () => el.remove(); };
  const onPage = rect => ({ left: rect.left + scrollX, top: rect.top + scrollY, width: rect.width, height: rect.height });
  const place = (el, rect, pad) => Object.assign(el.style, {
    left: `${rect.left - pad}px`, top: `${rect.top - pad}px`,
    width: `${rect.width + pad * 2}px`, height: `${rect.height + pad * 2}px`,
  });

  const describe = el => {
    const name = (el.getAttribute("aria-label") || el.innerText || el.value || el.alt || el.title || "")
      .trim().replace(/\s+/g, " ").slice(0, 80);
    const tag = el.tagName.toLowerCase();
    return name ? `${tag} "${name}"` : tag;
  };
  const shortName = el => {
    const tag = el.tagName.toLowerCase();
    if (el.id) return `${tag}#${el.id}`;
    const cls = [...el.classList][0];
    return cls ? `${tag}.${cls}` : tag;
  };
  const selectorFor = el => {
    const parts = [];
    for (let node = el; node && node.nodeType === 1 && parts.length < 5; node = node.parentElement) {
      if (node.id) { parts.unshift(`#${CSS.escape(node.id)}`); break; }
      let part = node.tagName.toLowerCase() + [...node.classList].slice(0, 2).map(c => `.${CSS.escape(c)}`).join("");
      const parent = node.parentElement;
      if (parent) {
        const same = [...parent.children].filter(child => child.tagName === node.tagName);
        if (same.length > 1) part += `:nth-of-type(${same.indexOf(node) + 1})`;
      }
      parts.unshift(part);
    }
    return parts.join(" > ");
  };

  const previousCursor = document.documentElement.style.cursor;
  document.documentElement.style.cursor = "crosshair";

  const banner = make("div", `position:fixed;top:12px;left:50%;transform:translateX(-50%);z-index:${z};`
    + `display:flex;align-items:center;gap:10px;padding:7px 8px 7px 12px;border-radius:999px;`
    + `background:rgba(22,20,30,.94);border:1px solid ${tint(.55)};box-shadow:0 8px 28px rgba(0,0,0,.35);`
    + `color:#f4f3f8;font:500 12px/18px ${font};white-space:nowrap;cursor:default`);
  const dot = make("span", `width:8px;height:8px;border-radius:50%;background:${accent};box-shadow:0 0 0 0 ${tint(.6)}`, banner);
  dot.animate([{ boxShadow: `0 0 0 0 ${tint(.6)}` }, { boxShadow: `0 0 0 7px ${tint(0)}` }],
    { duration: 1400, iterations: Infinity, easing: "ease-out" });
  make("span", "", banner).textContent = "Click an element or Shift-drag an area to leave a note";
  const kbd = make("span", `padding:0 6px;border-radius:4px;border:1px solid rgba(255,255,255,.2);color:#b9b6c8;font-size:11px`, banner);
  kbd.textContent = "Esc";
  const done = make("button", `all:unset;cursor:pointer;padding:3px 10px;border-radius:999px;background:${accent};`
    + `color:#fff;font:600 12px/18px ${font}`, banner);
  done.textContent = "Done";
  enter(banner, "translate(-50%, -8px)");

  const hover = make("div", `position:fixed;z-index:${z};pointer-events:none;border-radius:6px;opacity:0;`
    + `border:1.5px solid ${accent};background:${tint(.08)};box-shadow:0 0 0 4px ${tint(.14)};`
    + `transition:left 90ms ease-out,top 90ms ease-out,width 90ms ease-out,height 90ms ease-out,opacity 120ms`);
  const tag = make("div", `position:fixed;z-index:${z};pointer-events:none;opacity:0;padding:1px 7px;border-radius:5px;`
    + `background:${accent};color:#fff;font:600 11px/17px ${font};white-space:nowrap;`
    + `box-shadow:0 2px 8px rgba(0,0,0,.25);transition:left 90ms ease-out,top 90ms ease-out,opacity 120ms`);
  const box = make("div", `position:fixed;z-index:${z};pointer-events:none;display:none;border-radius:6px;`
    + `border:1.5px dashed ${accent};background:${tint(.12)}`);
  const hideHover = () => { hover.style.opacity = "0"; tag.style.opacity = "0"; };

  let editor = null;
  let selection = null;
  let count = 0;
  const select = rect => {
    const el = make("div", `position:absolute;z-index:${z};pointer-events:none;border-radius:7px;`
      + `border:2px solid ${accent};background:${tint(.08)};box-shadow:0 0 0 5px ${tint(.18)}`);
    place(el, onPage(rect), 3);
    enter(el, "scale(.98)");
    return el;
  };
  const cancel = () => {
    if (editor) { leave(editor); editor = null; }
    if (selection) { leave(selection); selection = null; }
  };

  const elementTarget = el => ({ el, rect: el.getBoundingClientRect(), description: describe(el) });
  const areaTarget = rect => {
    const el = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2) || document.body;
    const size = `${Math.round(rect.width)}×${Math.round(rect.height)}`;
    return { el, rect, description: `area ${size} at (${Math.round(rect.left)}, ${Math.round(rect.top)}) around ${describe(el)}` };
  };

  let toast = null;
  const confirm = number => {
    if (toast) leave(toast);
    toast = make("div", `position:fixed;bottom:18px;left:50%;transform:translateX(-50%);z-index:${z};`
      + `display:flex;align-items:center;gap:10px;padding:7px 8px 7px 12px;border-radius:999px;`
      + `background:rgba(22,20,30,.94);border:1px solid ${tint(.55)};box-shadow:0 8px 28px rgba(0,0,0,.35);`
      + `color:#f4f3f8;font:500 12px/18px ${font};white-space:nowrap`);
    const check = make("span", `color:#7ee2a8;font-weight:700`, toast);
    check.textContent = "✓";
    make("span", "", toast).textContent = `Note ${number} saved. Your agent can read it, or send it now.`;
    const send = make("button", `all:unset;cursor:pointer;padding:3px 10px;border-radius:999px;background:${accent};`
      + `color:#fff;font:600 12px/18px ${font}`, toast);
    send.textContent = "Send to agent";
    send.addEventListener("click", () => {
      window.ipc.postMessage("__PAGE_ACTION_PREFIX__" + "__SEND_NOTES__");
      if (toast) { leave(toast); toast = null; }
    });
    enter(toast, "translate(-50%, 8px)");
    const shown = toast;
    setTimeout(() => { if (toast === shown) { leave(toast); toast = null; } }, 6000);
  };

  const save = (target, note, error) => {
    try {
      window.ipc.postMessage("__ANNOTATION_PREFIX__" + JSON.stringify({
        url: location.href,
        selector: selectorFor(target.el),
        element: target.description,
        html: target.el.outerHTML.replace(/\s+/g, " ").slice(0, 400),
        note,
      }));
    } catch (err) {
      error.textContent = `Couldn't send the note to Warp: ${err}`;
      return;
    }
    count += 1;
    confirm(count);
    const pinned = selection;
    selection = null;
    Object.assign(pinned.style, { borderWidth: "1.5px", background: "transparent", boxShadow: "none", borderColor: tint(.75) });
    const page = onPage(target.rect);
    const badge = make("div", `position:absolute;z-index:${z};pointer-events:none;min-width:20px;height:20px;`
      + `padding:0 6px;box-sizing:border-box;border-radius:10px;background:${accent};color:#fff;`
      + `font:700 11px/20px ${font};text-align:center;box-shadow:0 2px 8px rgba(0,0,0,.3);`
      + `left:${page.left - 10}px;top:${page.top - 10}px`);
    badge.textContent = String(count);
    badge.animate([{ transform: "scale(.3)" }, { transform: "scale(1.15)" }, { transform: "scale(1)" }],
      { duration: 260, easing: "ease-out" });
    leave(editor);
    editor = null;
  };

  const openEditor = target => {
    cancel();
    hideHover();
    selection = select(target.rect);
    const page = onPage(target.rect);
    const width = 320;
    const left = Math.max(scrollX + 8, Math.min(page.left, scrollX + innerWidth - width - 8));
    const below = target.rect.bottom + 150 < innerHeight;
    const top = below ? page.top + page.height + 10 : Math.max(scrollY + 8, page.top - 150);
    editor = make("div", `position:absolute;z-index:${z};left:${left}px;top:${top}px;width:${width}px;`
      + `box-sizing:border-box;padding:10px 12px;border-radius:12px;background:rgba(22,20,30,.97);`
      + `border:1px solid ${tint(.6)};box-shadow:0 12px 32px rgba(0,0,0,.4);color:#f4f3f8;`
      + `font:13px/1.45 ${font};cursor:auto`);
    const header = make("div", "display:flex;align-items:center;gap:8px;margin-bottom:6px", editor);
    const chip = make("span", `flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;`
      + `color:#b4a8ff;font:600 11px/16px ${font}`, header);
    chip.textContent = target.description;
    const close = make("button", "all:unset;cursor:pointer;color:#8a879a;font:16px/16px sans-serif;padding:0 2px", header);
    close.textContent = "×";
    close.addEventListener("click", cancel);
    const field = make("textarea", "display:block;width:100%;box-sizing:border-box;resize:none;border:none;"
      + "outline:none;background:transparent;color:inherit;font:inherit;padding:0", editor);
    field.placeholder = "What should the agent change here?";
    field.rows = 2;
    const footer = make("div", "display:flex;align-items:center;justify-content:space-between;margin-top:8px", editor);
    make("span", `color:#8a879a;font:11px/16px ${font}`, footer).textContent = "↵ add · Esc cancel";
    const add = make("button", `all:unset;cursor:pointer;padding:3px 12px;border-radius:999px;background:${accent};`
      + `color:#fff;font:600 12px/18px ${font};opacity:.45;transition:opacity 120ms`, footer);
    add.textContent = "Add note";
    const error = make("div", `color:#ff8c8c;font:11px/16px ${font};margin-top:6px`, editor);
    const submit = () => { const note = field.value.trim(); if (note) save(target, note, error); else field.focus(); };
    // Pressing a card button must not take focus from the note field.
    for (const button of [add, close]) button.addEventListener("mousedown", event => event.preventDefault());
    field.addEventListener("input", () => { add.style.opacity = field.value.trim() ? "1" : ".45"; });
    field.addEventListener("keydown", event => {
      if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); submit(); }
    });
    add.addEventListener("click", submit);
    enter(editor, "translateY(-6px)");
    field.focus();
  };

  let dragStart = null;
  let dragged = false;
  const dragRect = event => new DOMRect(Math.min(dragStart.x, event.clientX), Math.min(dragStart.y, event.clientY),
    Math.abs(event.clientX - dragStart.x), Math.abs(event.clientY - dragStart.y));
  const swallow = event => { if (!own(event.target)) { event.preventDefault(); event.stopPropagation(); } };
  // Cancelling pointerdown would also suppress the mousedown and mouseup that drags rely on.
  const isolate = event => { if (!own(event.target)) event.stopPropagation(); };
  const onDown = event => {
    if (own(event.target)) return;
    swallow(event);
    if (event.shiftKey) { cancel(); dragStart = { x: event.clientX, y: event.clientY }; }
  };
  const onUp = event => {
    if (own(event.target) && !dragStart) return;
    swallow(event);
    if (!dragStart) return;
    const rect = dragRect(event);
    dragStart = null;
    box.style.display = "none";
    if (rect.width > 8 && rect.height > 8) { dragged = true; openEditor(areaTarget(rect)); }
  };
  const onMove = event => {
    if (dragStart) {
      hideHover();
      box.style.display = "block";
      place(box, dragRect(event), 0);
      return;
    }
    if (editor || own(event.target)) { hideHover(); return; }
    const rect = event.target.getBoundingClientRect();
    place(hover, rect, 2);
    hover.style.opacity = "1";
    tag.textContent = `${shortName(event.target)}  ${Math.round(rect.width)}×${Math.round(rect.height)}`;
    Object.assign(tag.style, {
      left: `${Math.max(4, rect.left - 2)}px`,
      top: `${rect.top > 26 ? rect.top - 24 : rect.bottom + 6}px`,
      opacity: "1",
    });
  };
  const onClick = event => {
    if (own(event.target)) return;
    swallow(event);
    if (dragged) { dragged = false; return; }
    openEditor(elementTarget(event.target));
  };
  const onKey = event => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    if (editor) cancel();
    else exit();
  };
  const listeners = [["mousemove", onMove], ["click", onClick], ["mousedown", onDown], ["mouseup", onUp],
    ["pointerdown", isolate], ["keydown", onKey], ["scroll", hideHover]];
  for (const [type, listener] of listeners) window.addEventListener(type, listener, true);
  const stop = () => {
    for (const [type, listener] of listeners) window.removeEventListener(type, listener, true);
    for (const el of ui) el.remove();
    ui.clear();
    document.documentElement.style.cursor = previousCursor;
    delete window.__warpAnnotate;
  };
  const exit = () => { stop(); window.ipc.postMessage("__EXITED__"); };
  done.addEventListener("click", exit);
  window.__warpAnnotate = { stop };
})()"##;

#[cfg(test)]
#[path = "annotation_tests.rs"]
mod tests;
