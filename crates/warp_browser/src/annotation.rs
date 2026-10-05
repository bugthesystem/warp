//! Notes the user pins to page elements for agents, and the page script that collects them.

use serde::Deserialize;

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

/// A script that turns annotate mode on or off in a page. While it is on, hovering outlines
/// elements; clicking one, or shift-dragging a box around an area, opens a note field. Each saved
/// note is posted to Warp and marked with a numbered badge. Escape closes the note field, then
/// leaves annotate mode.
pub fn annotate_script(enabled: bool) -> String {
    format!(
        r##"(() => {{
  const enabled = {enabled};
  const existing = window.__warpAnnotate;
  if (!enabled) {{ if (existing) existing.stop(); return; }}
  if (existing) return;
  const accent = "#7c5cff";
  const ui = new Set();
  const own = el => {{ for (let node = el; node; node = node.parentElement) if (ui.has(node)) return true; return false; }};
  const layer = css => {{
    const el = document.createElement("div");
    el.setAttribute("aria-hidden", "true");
    el.style.cssText = "position:fixed;z-index:2147483646;" + css;
    document.documentElement.appendChild(el);
    ui.add(el);
    return el;
  }};
  const hover = layer(`pointer-events:none;border:2px dashed ${{accent}};border-radius:6px;background:rgba(124,92,255,.08);display:none`);
  const badges = [];
  let editor = null;
  let count = 0;
  const previousCursor = document.documentElement.style.cursor;
  document.documentElement.style.cursor = "crosshair";

  const describe = el => {{
    const name = (el.getAttribute("aria-label") || el.innerText || el.value || el.alt || el.title || "")
      .trim().replace(/\s+/g, " ").slice(0, 80);
    const tag = el.tagName.toLowerCase();
    return name ? `${{tag}} "${{name}}"` : tag;
  }};
  const selectorFor = el => {{
    const parts = [];
    for (let node = el; node && node.nodeType === 1 && parts.length < 5; node = node.parentElement) {{
      if (node.id) {{ parts.unshift(`#${{CSS.escape(node.id)}}`); break; }}
      let part = node.tagName.toLowerCase();
      const classes = [...node.classList].slice(0, 2).map(c => `.${{CSS.escape(c)}}`).join("");
      part += classes;
      const parent = node.parentElement;
      if (parent) {{
        const same = [...parent.children].filter(child => child.tagName === node.tagName);
        if (same.length > 1) part += `:nth-of-type(${{same.indexOf(node) + 1}})`;
      }}
      parts.unshift(part);
    }}
    return parts.join(" > ");
  }};
  const closeEditor = () => {{ if (editor) {{ ui.delete(editor); editor.remove(); editor = null; }} }};
  const elementTarget = el => ({{ el, rect: el.getBoundingClientRect(), description: describe(el) }});
  const areaTarget = rect => {{
    const el = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2)
      || document.body;
    const size = `${{Math.round(rect.width)}}×${{Math.round(rect.height)}}`;
    const at = `(${{Math.round(rect.left)}}, ${{Math.round(rect.top)}})`;
    return {{ el, rect, description: `area ${{size}} at ${{at}} around ${{describe(el)}}` }};
  }};
  const openEditor = ({{ el, rect, description }}) => {{
    closeEditor();
    const width = 300;
    const left = Math.max(8, Math.min(rect.left, innerWidth - width - 8));
    const top = rect.bottom + 120 < innerHeight ? rect.bottom + 8 : Math.max(8, rect.top - 112);
    editor = layer(`left:${{left}}px;top:${{top}}px;width:${{width}}px;padding:10px;border-radius:10px;`
      + `background:#1c1b22;border:1px solid ${{accent}};box-shadow:0 8px 24px rgba(0,0,0,.35);`
      + `font:13px/1.4 -apple-system,system-ui,sans-serif;color:#f4f3f8;cursor:auto`);
    const label = document.createElement("div");
    label.textContent = description;
    label.style.cssText = "color:#a99bff;font-size:11px;margin-bottom:6px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis";
    const field = document.createElement("textarea");
    field.placeholder = "What should the agent change here?";
    field.rows = 2;
    field.style.cssText = "width:100%;box-sizing:border-box;resize:none;border:none;outline:none;"
      + "background:transparent;color:inherit;font:inherit";
    const hint = document.createElement("div");
    hint.textContent = "Enter to add · Esc to cancel";
    hint.style.cssText = "color:#8a879a;font-size:11px;margin-top:4px";
    editor.append(label, field, hint);
    field.addEventListener("keydown", event => {{
      if (event.key === "Enter" && !event.shiftKey) {{
        event.preventDefault();
        const note = field.value.trim();
        if (!note) return;
        window.ipc.postMessage("{ANNOTATION_MESSAGE_PREFIX}" + JSON.stringify({{
          url: location.href,
          selector: selectorFor(el),
          element: description,
          html: el.outerHTML.replace(/\s+/g, " ").slice(0, 400),
          note,
        }}));
        count += 1;
        const badge = layer(`pointer-events:none;left:${{rect.left - 10}}px;top:${{rect.top - 10}}px;`
          + `min-width:20px;height:20px;padding:0 5px;box-sizing:border-box;border-radius:10px;`
          + `background:${{accent}};color:#fff;font:600 11px/20px -apple-system,system-ui,sans-serif;`
          + `text-align:center;box-shadow:0 2px 6px rgba(0,0,0,.3)`);
        badge.textContent = String(count);
        badges.push(badge);
        closeEditor();
      }}
    }});
    field.focus();
  }};

  const box = layer(`pointer-events:none;border:2px solid ${{accent}};border-radius:4px;background:rgba(124,92,255,.12);display:none`);
  let dragStart = null;
  let dragged = false;
  const dragRect = event => {{
    const left = Math.min(dragStart.x, event.clientX);
    const top = Math.min(dragStart.y, event.clientY);
    return new DOMRect(left, top, Math.abs(event.clientX - dragStart.x), Math.abs(event.clientY - dragStart.y));
  }};
  const onDown = event => {{
    if (own(event.target)) return;
    swallow(event);
    if (event.shiftKey) {{
      closeEditor();
      dragStart = {{ x: event.clientX, y: event.clientY }};
    }}
  }};
  const onUp = event => {{
    if (own(event.target) && !dragStart) return;
    swallow(event);
    if (!dragStart) return;
    const rect = dragRect(event);
    dragStart = null;
    box.style.display = "none";
    if (rect.width > 8 && rect.height > 8) {{
      dragged = true;
      openEditor(areaTarget(rect));
    }}
  }};
  const onMove = event => {{
    if (dragStart) {{
      const rect = dragRect(event);
      hover.style.display = "none";
      Object.assign(box.style, {{
        display: "block",
        left: `${{rect.left}}px`, top: `${{rect.top}}px`,
        width: `${{rect.width}}px`, height: `${{rect.height}}px`,
      }});
      return;
    }}
    if (editor || own(event.target)) {{ hover.style.display = "none"; return; }}
    const rect = event.target.getBoundingClientRect();
    Object.assign(hover.style, {{
      display: "block",
      left: `${{rect.left - 2}}px`, top: `${{rect.top - 2}}px`,
      width: `${{rect.width + 4}}px`, height: `${{rect.height + 4}}px`,
    }});
  }};
  const swallow = event => {{ if (!own(event.target)) {{ event.preventDefault(); event.stopPropagation(); }} }};
  // Cancelling pointerdown would also suppress the mousedown and mouseup that drags rely on.
  const isolate = event => {{ if (!own(event.target)) event.stopPropagation(); }};
  const onClick = event => {{
    if (own(event.target)) return;
    swallow(event);
    if (dragged) {{ dragged = false; return; }}
    hover.style.display = "none";
    openEditor(elementTarget(event.target));
  }};
  const onKey = event => {{
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    if (editor) closeEditor();
    else {{ stop(); window.ipc.postMessage("{ANNOTATE_EXITED_MESSAGE}"); }}
  }};
  const listeners = [["mousemove", onMove], ["click", onClick], ["mousedown", onDown],
    ["mouseup", onUp], ["pointerdown", isolate], ["keydown", onKey]];
  for (const [type, listener] of listeners) window.addEventListener(type, listener, true);
  const stop = () => {{
    for (const [type, listener] of listeners) window.removeEventListener(type, listener, true);
    for (const el of ui) el.remove();
    document.documentElement.style.cursor = previousCursor;
    delete window.__warpAnnotate;
  }};
  window.__warpAnnotate = {{ stop }};
}})()"##
    )
}

#[cfg(test)]
#[path = "annotation_tests.rs"]
mod tests;
