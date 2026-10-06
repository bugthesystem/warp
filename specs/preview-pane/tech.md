# Tech Spec: Preview pane

**Product spec:** `specs/preview-pane/product.md`

## Context

The browser pane already provides most of the shell this needs: a pane type that keeps a native `NSView` over a `SavePosition` rect (`app/src/browser/view.rs`, `sync_webviews`), overlay hiding (`Scene::is_overlaid`), the local MCP endpoint with bearer token and terminal variables, per-site approval, the activity bar, the agent cursor, closed-pane handling and the Claude Code plugin. `crates/computer_use` already delivers mouse and keyboard events to one process (`Target::Window { window_id, pid }`, `CGEventPostToPid`), primes background windows (`mac/activation.rs`) and enumerates and screenshots windows.

The new work is capturing frames, presenting them, mapping input, and the simulator sources.

## Sources

| Source | Frames | Input | Discovery |
| --- | --- | --- | --- |
| Page (dev servers, any URL) | Headless Chromium started by Warp (`--headless --remote-debugging-port=0`, throwaway profile), `Page.startScreencast` JPEG frames over the DevTools protocol; console, errors and log entries from `Runtime`/`Log` events | DevTools `Input.dispatchMouseEvent` / `dispatchKeyEvent` (Playing) | Local servers from `lsof`, as the browser pane's new-tab page |
| Window (Blender, Unity, Godot, Defold, game builds, any app) | ScreenCaptureKit: `SCContentFilter(desktopIndependentWindow:)`, `SCStream`, cursor hidden; frames whose `SCFrameStatus` is not `complete` are dropped | `computer_use` with `Target::Window`; pane points mapped through the letterbox to the window's content rect (title bar and backing scale removed), bounds read per event | `SCShareableContent` windows, excluding Warp's |
| iOS Simulator | `baguette stream --udid … --format avcc` (H.264, length-prefixed) as a child process | one long-lived `baguette input --udid …` per device, JSON lines (`tap`, `swipe`, `key`, `type`, `button`) in device points | `xcrun simctl list devices booted` / `baguette list` |
| Android Emulator | emulator gRPC `EmulatorController.streamScreenshot` (launched with `-no-window -grpc <port>`) | gRPC `sendTouch`, `sendKey`, `sendMouse` | `adb devices` / emulator discovery files |

Desktop apps are launched or kept behind Warp and never activated by the pane; a covered window keeps streaming, a minimized one does not (shown as such). Engines' headless modes (`godot --headless`, Unity `-batchmode -nographics`) render nothing, so they are not used for preview.

## Presenting frames

**First release: frames drawn in WarpUI.** Every source delivers JPEG frames to `PreviewStreams` (a singleton model), which puts the newest one in the asset cache; the stage draws it as a rounded, letterboxed `Image`. Queued frames are coalesced so only the newest is decoded. Windows are captured with `SCScreenshotManager` (macOS 14+) at 20 frames a second for the front preview and 2 for cards, which works while the window is covered and never activates its app. Because WarpUI draws the picture, the capsule, tooltips, badges, cards, menus and picture-in-picture are ordinary elements on every platform, with nothing to hide under overlays. The cost is a JPEG encode and decode per frame, which is fine for watching at these sizes.

Each view showing previews saves the stage's position for a single frame; after every frame (`on_frame_drawn`) the view sets its previews' rates: Full for the front preview, Thumbnail (a few small frames a second) for cards, Paused when not drawn. So nothing is captured while no preview is visible.

**Later, for Playing at full frame rate:** a child `NSView` backed by an `AVSampleBufferDisplayLayer` and an `SCStream`, placed like the browser's web view: it takes ScreenCaptureKit's sample buffers and the simulators' H.264 directly, so Warp needs no decoder. Its cost is the browser pane's: anything Warp draws over it hides it.

Evaluated in the spike as the long-term option: draw frames inside WarpUI. ScreenCaptureKit buffers are `IOSurface`-backed, and Warp renders with Metal, so a WarpUI element could wrap the surface as a texture without copying. Tooltips, menus, rounded corners and clipping would then work normally over a live game. H.264 sources would need a `VTDecompressionSession` to get surfaces.

## Picture-in-picture and stacks

A preview is a stream owned by `PreviewStreams` and addressed by `PreviewId`, independent of where it is shown, so it moves between a pane and the picture-in-picture window without restarting. The pane and the picture-in-picture window render the same stage (`app/src/preview/stage.rs`). The picture-in-picture window is a WarpUI view positioned in the workspace's stack, one per window, dragged with `Draggable` and snapped to the nearest corner on drop. A stack is a list of preview ids with one in front: the front one renders large, the others as live cards beside it at the Thumbnail rate.

## Agent tools

Added to the local MCP server beside `browser_*`, behind the same token (`warp_browser::agent::ToolChannels` routes each kind; either can be off), and to the `warp-browser` plugin's skill. The first release ships `preview_targets`, `preview_open`, `preview_screenshot` and `preview_look` (picture plus recent log); the rest arrive with Playing:

| Tool | Does |
| --- | --- |
| `preview_targets` | Windows (app, title, size), booted simulators, running emulators, and installed engines that can be launched |
| `preview_open` | Shows a target in a preview pane, launching the app or booting the device when asked |
| `preview_screenshot` | The current frame, a small JPEG by default (full size on request), with its coordinate space |
| `preview_look` | The current frame and the recent log (page console and errors) in one call |
| `preview_click`, `preview_drag`, `preview_scroll` | Pointer input in the target's coordinates (window points or device points); each reply includes a small frame after the action |
| `preview_type`, `preview_key` | Text, and named keys with modifiers |
| `preview_describe` | Simulators: the accessibility tree (`baguette describe-ui`, `uiautomator dump`) |

Approval is per app (bundle id, stored as `app:<bundle id>`), per site for pages that are not local, or per device, reusing `ApprovedSites` in `preview-agent-sources.json`. Allow once covers the one preview. The activity bar, Pause, Stop and closed-pane rules come from `BrowserAgent`; the shared parts should move into a module both panes use rather than be copied.

Engine automation stays outside Warp: the skill tells agents which server fits (Blender MCP, MCP for Unity, GoPeak for Godot, Defold's editor commands) and to use the preview tools for what those cannot do — seeing and pressing things.

## Permissions and installs

Screen Recording for window capture (check whether Warp already holds it through `computer_use` recording) and Accessibility for input. Each is asked only when a preview first needs it (`CGRequestScreenCaptureAccess`), with a link to its Settings page; capture stops while no preview is drawn.

Chromium: `find_chromium` checks `WARP_PREVIEW_CHROMIUM`, then a copy Warp downloaded, then installed Chrome, Chromium, Edge and Brave. "Download for me" reads Chrome for Testing's last-known-good versions, downloads the stable `chrome-headless-shell` for the platform, unpacks it beside a `version.txt` in Warp's data directory and renames it into place, so a failed download leaves nothing half-installed.

Baguette: used from the path when present and version-checked. Otherwise the pane offers "Download for me" (a pinned release into Warp's data directory) and "Install with Homebrew" (puts `brew install baguette` in a terminal).

## Build order

0. **Spike (macOS, run by hand):** a debug command that streams one chosen window into an `AVSampleBufferDisplayLayer` at the pane's rect and clicks through `computer_use`. Tried on Blender, the Godot editor and a Godot game, the Unity editor and a Unity player, and the Defold editor. Settles: do engines accept posted events; does the layer take ScreenCaptureKit buffers as delivered; is a WarpUI-drawn surface worth it.
1. **Watching, first release:** `crates/warp_preview` (headless Chromium screencast and Chromium download; ScreenCaptureKit window capture; stubs elsewhere), `PreviewStreams`, `PreviewPane`/`PreviewView` with the start page and the floating capsule, picture-in-picture and cards, `FeatureFlag::PreviewPane`, palette entry, and the read-only agent tools with per-source approval.
2. **Playing:** pointer (all buttons), drag, scroll and keys; the Watching / Playing switch; focus.
3. **Agent tools for Playing:** click, drag, scroll, type, keys; Pause, Stop and the agent cursor.
4. **iOS Simulator** through Baguette (`brew install baguette`, version-checked, never bundled), headless.
5. **Android Emulator** through its gRPC API, headless.
6. **Conveniences:** restore after restart; offer to open a project's engine when a terminal's directory holds a Unity, Godot, Defold or Blender project.

## Risks

| Risk | How it is settled |
| --- | --- |
| Engines or games ignoring posted events (games reading input below the event system) | Spike, per engine; fallback is a "Bring forward" button and the engine's own automation |
| `AVSampleBufferDisplayLayer` with ScreenCaptureKit buffers | Spike; fallback sets the layer's contents to each frame's `IOSurface` |
| Baguette tracks Xcode's private frameworks | Optional external tool with a version check |
| Android gRPC needs the emulator started with `-grpc` (or its default port and token) | Launch emulators ourselves for headless use; read discovery files for running ones |
| Screen recording prompt surprises people | Ask on first use, explain why |

## Testing

Unit tests for coordinate mapping (letterbox, content rect, scale), target listing filters (Warp's own windows excluded), Baguette/gRPC message encoding, and tool parsing. Integration tests for the pane's WarpUI side, as for the browser pane. Capture and input are checked by hand on a Mac against each engine and simulator.
