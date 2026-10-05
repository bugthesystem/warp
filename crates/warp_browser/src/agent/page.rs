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
/// Tools wait this long, plus a margin, before reporting back.
pub const ACTION_DELAY: Duration = Duration::from_millis(400);

/// Defines `warpPointAt(el)`, which moves a visible agent cursor to an element and outlines the
/// element, so the user can follow what the agent does.
const AGENT_CURSOR_SCRIPT: &str = r##"const warpPointAt = el => {
  const id = "__warp_agent_cursor";
  let cursor = document.getElementById(id);
  if (!cursor) {
    cursor = document.createElement("div");
    cursor.id = id;
    cursor.setAttribute("aria-hidden", "true");
    cursor.style.cssText = "position:fixed;left:0;top:0;z-index:2147483647;pointer-events:none;"
      + "transition:transform 350ms ease-out;transform:translate(-40px,-40px)";
    cursor.innerHTML = '<svg width="22" height="22" viewBox="0 0 22 22">'
      + '<path d="M2 2 L2 18 L7 13 L10.5 20 L13 19 L9.5 12 L16 12 Z" fill="#111" stroke="#fff" '
      + 'stroke-width="1.5" stroke-linejoin="round"/></svg>';
    document.documentElement.appendChild(cursor);
    cursor.getBoundingClientRect();
  }
  const rect = el.getBoundingClientRect();
  cursor.style.transform = `translate(${rect.left + rect.width / 2}px, ${rect.top + rect.height / 2}px)`;
  const ring = document.createElement("div");
  ring.setAttribute("aria-hidden", "true");
  ring.style.cssText = `position:fixed;z-index:2147483646;pointer-events:none;`
    + `left:${rect.left - 3}px;top:${rect.top - 3}px;width:${rect.width + 6}px;height:${rect.height + 6}px;`
    + `border:2px solid #3b82f6;border-radius:6px;transition:opacity 600ms ease-out 350ms`;
  document.documentElement.appendChild(ring);
  requestAnimationFrame(() => { ring.style.opacity = "0"; });
  setTimeout(() => ring.remove(), 1200);
};"##;

/// A script that moves the agent cursor to the element `element` numbered by the last read, then
/// clicks it once the cursor arrives.
pub fn click_script(element: u64) -> String {
    let delay_ms = ACTION_DELAY.as_millis();
    format!(
        r#"(() => {{
  {AGENT_CURSOR_SCRIPT}
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  el.scrollIntoView({{ block: "center" }});
  warpPointAt(el);
  setTimeout(() => {{
    if (el.focus) el.focus();
    el.click();
  }}, {delay_ms});
  return {{ ok: true }};
}})()"#
    )
}

/// A script that moves the agent cursor to the element `element` numbered by the last read, then
/// replaces its value with `text` and optionally submits it.
pub fn type_script(element: u64, text: &str, submit: bool) -> String {
    let text = serde_json::to_string(text).expect("strings always serialize to JSON");
    let delay_ms = ACTION_DELAY.as_millis();
    format!(
        r#"(() => {{
  {AGENT_CURSOR_SCRIPT}
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  const text = {text};
  el.scrollIntoView({{ block: "center" }});
  warpPointAt(el);
  setTimeout(() => {{
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
  }}, {delay_ms});
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
