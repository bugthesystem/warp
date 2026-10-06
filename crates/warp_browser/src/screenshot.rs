//! A preview of a screenshot the user took, shown in the corner of the page like macOS shows one.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::PAGE_ACTION_PREFIX;

/// Page actions the preview's buttons post.
pub const COPY_ACTION: &str = "screenshot-copy";
pub const SEND_ACTION: &str = "screenshot-send";
pub const REVEAL_ACTION: &str = "screenshot-reveal";

/// How long the preview stays while the pointer is not over it.
const PREVIEW_MS: u32 = 8000;

/// A script that removes the preview, so it is not captured in the next screenshot.
pub const REMOVE_PREVIEW_SCRIPT: &str = r#"(() => { const old = document.getElementById("__warp_screenshot"); if (old) old.remove(); })()"#;

/// A script that slides a thumbnail of `png` into the page's bottom-right corner, with buttons to
/// copy it, send it to the agent and show the saved file. `file_name` is shown under it.
pub fn preview_script(png: &[u8], file_name: &str) -> String {
    let data_url = format!("data:image/png;base64,{}", STANDARD.encode(png));
    let file_name = serde_json::to_string(file_name).expect("strings always serialize to JSON");
    PREVIEW_SCRIPT
        .replace("__DATA_URL__", &data_url)
        .replace("__FILE_NAME__", &file_name)
        .replace("__PREVIEW_MS__", &PREVIEW_MS.to_string())
        .replace("__ACTION__", PAGE_ACTION_PREFIX)
        .replace("__COPY__", COPY_ACTION)
        .replace("__SEND__", SEND_ACTION)
        .replace("__REVEAL__", REVEAL_ACTION)
}

const PREVIEW_SCRIPT: &str = r##"(() => {
  const old = document.getElementById("__warp_screenshot");
  if (old) old.remove();
  const font = "-apple-system,BlinkMacSystemFont,system-ui,sans-serif";
  const card = document.createElement("div");
  card.id = "__warp_screenshot";
  card.style.cssText = "position:fixed;right:16px;bottom:16px;z-index:2147483647;width:280px;padding:8px;"
    + "box-sizing:border-box;border-radius:14px;background:rgba(22,20,30,.96);border:1px solid rgba(124,92,255,.55);"
    + `box-shadow:0 14px 40px rgba(0,0,0,.45);color:#f4f3f8;font:12px/16px ${font}`;
  const image = document.createElement("img");
  image.src = "__DATA_URL__";
  image.style.cssText = "display:block;width:100%;border-radius:8px;border:1px solid rgba(255,255,255,.12)";
  const name = document.createElement("div");
  name.textContent = __FILE_NAME__;
  name.style.cssText = "margin:6px 2px;color:#a9a6b8;font-size:11px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap";
  const row = document.createElement("div");
  row.style.cssText = "display:flex;gap:6px";
  const button = (label, action, primary) => {
    const el = document.createElement("button");
    el.textContent = label;
    el.style.cssText = "all:unset;cursor:pointer;flex:1;text-align:center;padding:4px 6px;border-radius:8px;white-space:nowrap;"
      + `font:600 11px/16px ${font};` + (primary ? "background:#7c5cff;color:#fff" : "background:rgba(255,255,255,.08);color:#e8e6f0");
    el.addEventListener("click", () => window.ipc.postMessage("__ACTION__" + action));
    row.appendChild(el);
    return el;
  };
  button("Copy", "__COPY__", false);
  button("Send to agent", "__SEND__", true);
  button("Show file", "__REVEAL__", false);
  const close = document.createElement("button");
  close.textContent = "×";
  close.style.cssText = "all:unset;cursor:pointer;position:absolute;top:-8px;left:-8px;width:20px;height:20px;"
    + "border-radius:50%;background:#2a2735;color:#e8e6f0;text-align:center;font:14px/20px sans-serif;"
    + "box-shadow:0 2px 6px rgba(0,0,0,.4)";
  card.append(image, name, row, close);
  document.documentElement.appendChild(card);
  card.animate([{ opacity: 0, transform: "translateX(40px) scale(.96)" }, { opacity: 1, transform: "none" }],
    { duration: 260, easing: "cubic-bezier(.2,.8,.2,1)" });
  const dismiss = () => {
    card.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: "translateX(40px)" }],
      { duration: 200, easing: "ease-in" }).onfinish = () => card.remove();
  };
  close.addEventListener("click", dismiss);
  let timer = setTimeout(dismiss, __PREVIEW_MS__);
  card.addEventListener("mouseenter", () => clearTimeout(timer));
  card.addEventListener("mouseleave", () => { timer = setTimeout(dismiss, __PREVIEW_MS__ / 2); });
})()"##;

#[cfg(test)]
#[path = "screenshot_tests.rs"]
mod tests;
