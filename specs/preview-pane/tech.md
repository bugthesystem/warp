# Tech Spec: Preview pane

**Product spec:** `specs/preview-pane/product.md`

## Context

The browser pane already provides most of the shell this needs: a pane type that keeps a native `NSView` over a `SavePosition` rect (`app/src/browser/view.rs`, `sync_webviews`), overlay hiding (`Scene::is_overlaid`), the local MCP endpoint with bearer token and terminal variables, per-site approval, the activity bar, the agent cursor, closed-pane handling and the Claude Code plugin. `crates/computer_use` already delivers mouse and keyboard events to one process (`Target::Window { window_id, pid }`, `CGEventPostToPid`), primes background windows (`mac/activation.rs`) and enumerates and screenshots windows.

The new work is capturing frames, presenting them, mapping input, and the simulator sources.

## Sources

| Source | Frames | Input | Discovery |
| --- | --- | --- | --- |
| Window (Blender, Unity, Godot, Defold, game builds, any app) | ScreenCaptureKit: `SCContentFilter(desktopIndependentWindow:)`, `SCStream`, cursor hidden; frames whose `SCFrameStatus` is not `complete` are dropped | `computer_use` with `Target::Window`; pane points mapped through the letterbox to the window's content rect (title bar and backing scale removed), bounds read per event | `SCShareableContent` windows, excluding Warp's |
| iOS Simulator | `baguette stream --udid … --format avcc` (H.264, length-prefixed) as a child process | one long-lived `baguette input --udid …` per device, JSON lines (`tap`, `swipe`, `key`, `type`, `button`) in device points | `xcrun simctl list devices booted` / `baguette list` |
| Android Emulator | emulator gRPC `EmulatorController.streamScreenshot` (launched with `-no-window -grpc <port>`) | gRPC `sendTouch`, `sendKey`, `sendMouse` | `adb devices` / emulator discovery files |

Desktop apps are launched or kept behind Warp and never activated by the pane; a covered window keeps streaming, a minimized one does not (shown as such). Engines' headless modes (`godot --headless`, Unity `-batchmode -nographics`) render nothing, so they are not used for preview.

## Presenting frames

Step 1 uses a child `NSView` backed by an `AVSampleBufferDisplayLayer`, placed like the browser's web view: it takes ScreenCaptureKit's sample buffers and the simulators' H.264 directly, so Warp needs no decoder. Its cost is the browser pane's: anything Warp draws over it hides it.

Evaluated in the spike as the long-term option: draw frames inside WarpUI. ScreenCaptureKit buffers are `IOSurface`-backed, and Warp renders with Metal, so a WarpUI element could wrap the surface as a texture without copying. Tooltips, menus, rounded corners and clipping would then work normally over a live game. H.264 sources would need a `VTDecompressionSession` to get surfaces.

## Agent tools

Added to the local MCP server beside `browser_*`, behind the same token, and to the `warp-browser` plugin as a second skill:

| Tool | Does |
| --- | --- |
| `preview_targets` | Windows (app, title, size), booted simulators, running emulators, and installed engines that can be launched |
| `preview_open` | Shows a target in a preview pane, launching the app or booting the device when asked |
| `preview_screenshot` | The current frame, a small JPEG by default (full size on request), with its coordinate space |
| `preview_click`, `preview_drag`, `preview_scroll` | Pointer input in the target's coordinates (window points or device points); each reply includes a small frame after the action |
| `preview_type`, `preview_key` | Text, and named keys with modifiers |
| `preview_describe` | Simulators: the accessibility tree (`baguette describe-ui`, `uiautomator dump`) |

Approval is per app (bundle id) or per device, reusing `ApprovedSites`' shape. The activity bar, Pause, Stop and closed-pane rules come from `BrowserAgent`; the shared parts should move into a module both panes use rather than be copied.

Engine automation stays outside Warp: the skill tells agents which server fits (Blender MCP, MCP for Unity, GoPeak for Godot, Defold's editor commands) and to use the preview tools for what those cannot do — seeing and pressing things.

## Permissions

Screen Recording for capture (check whether Warp already holds it through `computer_use` recording) and Accessibility for input. Asked only when the first preview pane opens; capture stops while the pane is hidden.

## Build order

0. **Spike (macOS, run by hand):** a debug command that streams one chosen window into an `AVSampleBufferDisplayLayer` at the pane's rect and clicks through `computer_use`. Tried on Blender, the Godot editor and a Godot game, the Unity editor and a Unity player, and the Defold editor. Settles: do engines accept posted events; does the layer take ScreenCaptureKit buffers as delivered; is a WarpUI-drawn surface worth it.
1. **Watch-only pane:** `crates/warp_preview` (capture and presenter, stub elsewhere, as `warp_browser`), `PreviewPane`/`PreviewView`, the start page, `FeatureFlag::PreviewPane`, palette entry.
2. **Playing:** pointer (all buttons), drag, scroll and keys; the Watching / Playing switch; focus.
3. **Agent tools:** the `preview_*` tools, per-app approval, activity bar, skill.
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
