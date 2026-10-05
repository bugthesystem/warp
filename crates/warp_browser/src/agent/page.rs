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

/// A script that clicks the element `element` numbered by the last read.
pub fn click_script(element: u64) -> String {
    format!(
        r#"(() => {{
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  el.scrollIntoView({{ block: "center" }});
  if (el.focus) el.focus();
  el.click();
  return {{ ok: true }};
}})()"#
    )
}

/// A script that replaces the value of the element `element` numbered by the last read with
/// `text`, then optionally submits it.
pub fn type_script(element: u64, text: &str, submit: bool) -> String {
    let text = serde_json::to_string(text).expect("strings always serialize to JSON");
    format!(
        r#"(() => {{
  const el = document.querySelector('[{ELEMENT_ID_ATTRIBUTE}="{element}"]');
  if (!el) return {{ ok: false, error: "No element {element}; call browser_read to refresh the element numbers." }};
  const text = {text};
  el.scrollIntoView({{ block: "center" }});
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
