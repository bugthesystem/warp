use std::time::Duration;

use serde::Deserialize;

/// Attribute the read script stamps on each interactive element, so later clicks and typing can
/// find an element by the number the agent saw.
const ELEMENT_ID_ATTRIBUTE: &str = "data-warp-agent-id";

/// Upper bound on page text returned to the agent, in characters.
const MAX_PAGE_TEXT_CHARS: usize = 20_000;

/// A script that numbers the page's visible interactive elements and returns them with the page
/// text, as the JSON that [`format_page_snapshot`] reads.
pub fn read_page_script() -> String {
    format!(
        r#"(() => {{
  const attr = "{ELEMENT_ID_ATTRIBUTE}";
  const selector = 'a[href], button, input:not([type=hidden]), textarea, select, summary, '
    + '[role=button], [role=link], [role=checkbox], [role=radio], [role=tab], [role=menuitem], '
    + '[role=option], [role=switch], [contenteditable=""], [contenteditable=true]';
  const isVisible = el => {{
    const rect = el.getBoundingClientRect();
    const style = getComputedStyle(el);
    return rect.width > 0 && rect.height > 0 && style.visibility !== "hidden" && style.display !== "none";
  }};
  const clean = value => (value || "").trim().replace(/\s+/g, " ").slice(0, 100);
  document.querySelectorAll("[" + attr + "]").forEach(el => el.removeAttribute(attr));
  const elements = [];
  for (const el of document.querySelectorAll(selector)) {{
    if (!isVisible(el) || el.disabled) continue;
    const id = elements.length + 1;
    el.setAttribute(attr, String(id));
    const kind = el.getAttribute("role") || el.tagName.toLowerCase() + (el.type ? "[" + el.type + "]" : "");
    const name = clean(el.getAttribute("aria-label") || el.innerText || el.value || el.placeholder || el.title || el.getAttribute("alt"));
    elements.push({{ id, kind, name, href: el.href ? String(el.href) : null }});
  }}
  const text = (document.body ? document.body.innerText : "").replace(/\n{{3,}}/g, "\n\n");
  return {{ url: location.href, title: document.title, ready: document.readyState, text, elements }};
}})()"#
    )
}

/// How long the agent cursor takes to reach an element before the click or typing happens.
pub const ACTION_DELAY: Duration = Duration::from_millis(650);

/// Defines `warpPointAt(el)`, which glides a visible agent cursor to an element, outlines it and
/// returns the point it reached, and `warpPress(point)`, which shows a click there. The cursor
/// starts where it last stopped on this site, kept in session storage across page loads.
const AGENT_CURSOR_SCRIPT: &str = r##"const warpCursorKey = "__warpAgentCursor";
const warpCursor = () => {
  const id = "__warp_agent_cursor";
  let cursor = document.getElementById(id);
  if (cursor) return cursor;
  let start = null;
  try { start = JSON.parse(sessionStorage.getItem(warpCursorKey)); } catch (_) {}
  if (!start) start = { x: innerWidth / 2, y: innerHeight - 48 };
  cursor = document.createElement("div");
  cursor.id = id;
  cursor.setAttribute("aria-hidden", "true");
  cursor.style.cssText = "position:fixed;left:0;top:0;z-index:2147483647;pointer-events:none;"
    + "will-change:transform;transition:transform 550ms cubic-bezier(.22,.61,.36,1);"
    + `transform:translate(${start.x}px,${start.y}px)`;
  cursor.innerHTML = '<svg width="28" height="28" viewBox="0 0 28 28" style="display:block;'
    + 'filter:drop-shadow(0 2px 4px rgba(0,0,0,.35));transition:transform 120ms ease-out">'
    + '<path d="M3 2 L3 22 L9 16.5 L13 25 L16.5 23.5 L12.5 15.5 L20.5 15.5 Z" fill="#7c5cff" '
    + 'stroke="#fff" stroke-width="2" stroke-linejoin="round"/></svg>'
    + '<span style="position:absolute;left:20px;top:22px;padding:2px 8px;border-radius:999px;'
    + 'background:#7c5cff;color:#fff;font:600 11px/16px -apple-system,system-ui,sans-serif;'
    + 'white-space:nowrap;box-shadow:0 2px 6px rgba(0,0,0,.3)">Agent</span>';
  document.documentElement.appendChild(cursor);
  cursor.getBoundingClientRect();
  return cursor;
};
const warpPointAt = el => {
  const cursor = warpCursor();
  const rect = el.getBoundingClientRect();
  const point = { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  cursor.style.transform = `translate(${point.x}px, ${point.y}px)`;
  try { sessionStorage.setItem(warpCursorKey, JSON.stringify(point)); } catch (_) {}
  const ring = document.createElement("div");
  ring.setAttribute("aria-hidden", "true");
  ring.style.cssText = `position:fixed;z-index:2147483646;pointer-events:none;`
    + `left:${rect.left - 4}px;top:${rect.top - 4}px;width:${rect.width + 8}px;height:${rect.height + 8}px;`
    + `border:2px solid #7c5cff;border-radius:8px;box-shadow:0 0 0 4px rgba(124,92,255,.18);`
    + `opacity:0;transition:opacity 250ms ease-out`;
  document.documentElement.appendChild(ring);
  setTimeout(() => { ring.style.opacity = "1"; }, 350);
  setTimeout(() => { ring.style.opacity = "0"; }, 1300);
  setTimeout(() => ring.remove(), 1700);
  return point;
};
const warpPress = point => {
  const arrow = warpCursor().firstElementChild;
  arrow.style.transform = "scale(.82)";
  setTimeout(() => { arrow.style.transform = ""; }, 140);
  const ripple = document.createElement("div");
  ripple.setAttribute("aria-hidden", "true");
  ripple.style.cssText = `position:fixed;z-index:2147483646;pointer-events:none;`
    + `left:${point.x - 18}px;top:${point.y - 18}px;width:36px;height:36px;border-radius:50%;`
    + `background:rgba(124,92,255,.35);transform:scale(.2);opacity:1;`
    + `transition:transform 450ms ease-out,opacity 450ms ease-out`;
  document.documentElement.appendChild(ripple);
  requestAnimationFrame(() => { ripple.style.transform = "scale(1.6)"; ripple.style.opacity = "0"; });
  setTimeout(() => ripple.remove(), 600);
};"##;

/// How long after pointing at an element the page treats mouse input as the agent's, which keeps
/// the agent's native click from moving Warp's focus to the page.
const AGENT_INPUT_GUARD_MS: u128 = 1500;

/// How long to let a smooth scroll to an element run before pointing at it.
pub const SCROLL_DURATION: Duration = Duration::from_millis(500);

/// A script that smoothly scrolls the element `element` numbered by the last read to the middle
/// of the view when it is not fully visible, so the user can follow where the agent is going. It
/// jumps instead when the user prefers reduced motion. Returns `{ok, scrolling}`, where
/// `scrolling` says whether a smooth scroll started.
pub fn scroll_to_script(element: u64) -> String {
    format!(
        r#"(() => {{
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  const rect = el.getBoundingClientRect();
  if (rect.top >= 0 && rect.left >= 0 && rect.bottom <= innerHeight && rect.right <= innerWidth) {{
    return {{ ok: true, scrolling: false }};
  }}
  const smooth = !matchMedia("(prefers-reduced-motion: reduce)").matches;
  el.scrollIntoView({{ block: "center", inline: "nearest", behavior: smooth ? "smooth" : "auto" }});
  return {{ ok: true, scrolling: smooth }};
}})()"#
    )
}

#[derive(Deserialize)]
struct ScrollOutcome {
    ok: bool,
    error: Option<String>,
    scrolling: Option<bool>,
}

/// Reads whether [`scroll_to_script`] started a smooth scroll. The error is a message for the
/// agent.
pub fn scroll_started(json: &str) -> Result<bool, String> {
    let outcome: ScrollOutcome = serde_json::from_str(json)
        .map_err(|err| format!("Unexpected result from the page: {err}"))?;
    match outcome {
        ScrollOutcome {
            ok: true,
            scrolling,
            ..
        } => Ok(scrolling.unwrap_or(false)),
        ScrollOutcome { error, .. } => {
            Err(error.unwrap_or_else(|| "The page did not report the element.".to_owned()))
        }
    }
}

/// A script that makes sure the element `element` numbered by the last read is in view, glides
/// the agent cursor to it, and returns `{ok, x, y}`: the point reached, in logical pixels from the
/// viewport's top-left corner, for a native click there once the cursor arrives. A smooth scroll
/// still running is stopped where it is first, so the point stays valid.
pub fn point_script(element: u64) -> String {
    let guard_ms = ACTION_DELAY.as_millis() + AGENT_INPUT_GUARD_MS;
    format!(
        r#"(() => {{
  {AGENT_CURSOR_SCRIPT}
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  window.scrollTo({{ left: scrollX, top: scrollY, behavior: "instant" }});
  el.scrollIntoView({{ block: "nearest", inline: "nearest" }});
  const point = warpPointAt(el);
  window.__warpAgentInputUntil = Date.now() + {guard_ms};
  return {{ ok: true, x: point.x, y: point.y }};
}})()"#
    )
}

/// A point in a page, in logical pixels from the viewport's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct PagePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize)]
struct PointOutcome {
    ok: bool,
    error: Option<String>,
    x: Option<f32>,
    y: Option<f32>,
}

/// Reads the point [`point_script`] returns. The error is a message for the agent.
pub fn page_point(json: &str) -> Result<PagePoint, String> {
    let outcome: PointOutcome = serde_json::from_str(json)
        .map_err(|err| format!("Unexpected result from the page: {err}"))?;
    match outcome {
        PointOutcome {
            ok: true,
            x: Some(x),
            y: Some(y),
            ..
        } => Ok(PagePoint { x, y }),
        PointOutcome { error, .. } => {
            Err(error.unwrap_or_else(|| "The page did not report where the element is.".to_owned()))
        }
    }
}

/// A script that shows the agent cursor pressing at `x`, `y`.
pub fn press_script(x: f32, y: f32) -> String {
    format!(
        r#"(() => {{
  {AGENT_CURSOR_SCRIPT}
  warpPress({{ x: {x}, y: {y} }});
}})()"#
    )
}

/// A script that clicks the element `element` from inside the page, for when native input is not
/// available. Some pages ignore such clicks.
pub fn click_now_script(element: u64) -> String {
    format!(
        r#"(() => {{
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  if (el.focus) el.focus();
  el.click();
  return {{ ok: true }};
}})()"#
    )
}

/// A script that focuses the element `element` and selects its contents, so native typing
/// replaces them.
pub fn prepare_type_script(element: u64) -> String {
    let guard_ms = AGENT_INPUT_GUARD_MS;
    format!(
        r#"(() => {{
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  window.__warpAgentInputUntil = Date.now() + {guard_ms};
  el.focus();
  if (el.isContentEditable) document.execCommand("selectAll");
  else if (el.select) el.select();
  return {{ ok: true }};
}})()"#
    )
}

/// A script that replaces the value of the element `element` with `text` from inside the page,
/// and optionally submits it, for when native input is not available.
pub fn type_now_script(element: u64, text: &str, submit: bool) -> String {
    let text = serde_json::to_string(text).expect("strings always serialize to JSON");
    format!(
        r#"(() => {{
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  const text = {text};
  el.focus();
  if (el.isContentEditable) {{
    document.execCommand("selectAll");
    document.execCommand("insertText", false, text);
  }} else {{
    // Frameworks such as React track the value through the prototype setter, so set it there.
    const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, "value").set.call(el, text);
    el.dispatchEvent(new Event("input", {{ bubbles: true }}));
    el.dispatchEvent(new Event("change", {{ bubbles: true }}));
  }}
  if ({submit}) {{
    const enter = {{ key: "Enter", code: "Enter", keyCode: 13, bubbles: true }};
    const proceed = el.dispatchEvent(new KeyboardEvent("keydown", enter));
    el.dispatchEvent(new KeyboardEvent("keyup", enter));
    if (proceed && el.form) el.form.requestSubmit ? el.form.requestSubmit() : el.form.submit();
  }}
  return {{ ok: true }};
}})()"#
    )
}

/// Installed in every page at document start. Records console messages, uncaught errors and failed
/// network requests in `window.__warpConsole` for [`console_script`] to read.
pub const CONSOLE_CAPTURE_SCRIPT: &str = r##"(() => {
  if (window.__warpConsole) return;
  const entries = [];
  window.__warpConsole = entries;
  const describe = value => {
    if (value instanceof Error) return value.stack || String(value);
    if (typeof value === "object" && value !== null) {
      try { return JSON.stringify(value); } catch (_) { return String(value); }
    }
    return String(value);
  };
  const push = (level, parts) => {
    entries.push({ level, message: parts.map(describe).join(" ").slice(0, 2000) });
    if (entries.length > 500) entries.shift();
  };
  for (const level of ["log", "info", "warn", "error", "debug"]) {
    const original = console[level];
    console[level] = function (...args) {
      push(level, args);
      return original.apply(this, args);
    };
  }
  window.addEventListener("error", event => {
    const where = event.filename ? ` (${event.filename}:${event.lineno})` : "";
    push("error", [event.message + where]);
  });
  window.addEventListener("unhandledrejection", event => push("error", ["Unhandled rejection:", event.reason]));
  const originalFetch = window.fetch;
  if (originalFetch) {
    window.fetch = function (...args) {
      const target = args[0] && args[0].url ? args[0].url : String(args[0]);
      return originalFetch.apply(this, args).then(
        response => {
          if (!response.ok) push("network", [`${response.status} ${response.url || target}`]);
          return response;
        },
        error => {
          push("network", [`Failed ${target}: ${error}`]);
          throw error;
        });
    };
  }
  const open = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url, ...rest) {
    this.addEventListener("loadend", () => {
      if (this.status === 0 || this.status >= 400) push("network", [`${this.status} ${method} ${url}`]);
    });
    return open.call(this, method, url, ...rest);
  };
})();"##;

/// A script that returns the messages [`CONSOLE_CAPTURE_SCRIPT`] recorded, as the JSON that
/// [`format_console_messages`] reads, and clears them when `clear` is set.
pub fn console_script(clear: bool) -> String {
    format!(
        r#"(() => {{
  const entries = window.__warpConsole;
  if (!entries) return null;
  const copy = entries.slice();
  if ({clear}) entries.length = 0;
  return copy;
}})()"#
    )
}

#[derive(Deserialize)]
struct ConsoleMessage {
    level: String,
    message: String,
}

/// Formats the JSON returned by [`console_script`] as text for the agent.
pub fn format_console_messages(json: &str) -> Result<String, String> {
    let messages: Option<Vec<ConsoleMessage>> =
        serde_json::from_str(json).map_err(|err| format!("Unexpected console messages: {err}"))?;
    let Some(messages) = messages else {
        return Ok("Console capture is not available on this page.".to_owned());
    };
    if messages.is_empty() {
        return Ok("No console messages, page errors or failed requests.".to_owned());
    }
    Ok(messages
        .iter()
        .map(|entry| format!("[{}] {}", entry.level, entry.message))
        .collect::<Vec<_>>()
        .join("\n"))
}

#[derive(Deserialize)]
struct PageSnapshot {
    url: String,
    title: String,
    ready: String,
    text: String,
    elements: Vec<PageElement>,
}

#[derive(Deserialize)]
struct PageElement {
    id: u64,
    kind: String,
    name: String,
    href: Option<String>,
}

/// Formats the JSON returned by [`read_page_script`] as text for the agent.
pub fn format_page_snapshot(json: &str) -> Result<String, String> {
    let page: PageSnapshot =
        serde_json::from_str(json).map_err(|err| format!("Unexpected page snapshot: {err}"))?;

    let mut output = format!("Title: {}\nURL: {}\n", page.title, page.url);
    if page.ready != "complete" {
        output.push_str(&format!(
            "Note: the page is still loading ({}).\n",
            page.ready
        ));
    }

    output
        .push_str("\nInteractive elements (use the number with browser_click or browser_type):\n");
    if page.elements.is_empty() {
        output.push_str("(none)\n");
    }
    for element in &page.elements {
        output.push_str(&format!(
            "[{}] {} \"{}\"",
            element.id, element.kind, element.name
        ));
        if let Some(href) = &element.href {
            output.push_str(&format!(" -> {href}"));
        }
        output.push('\n');
    }

    output.push_str("\nPage text:\n");
    let text: String = page.text.chars().take(MAX_PAGE_TEXT_CHARS).collect();
    output.push_str(&text);
    if page.text.chars().count() > MAX_PAGE_TEXT_CHARS {
        output.push_str("\n(page text truncated)");
    }
    Ok(output)
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
