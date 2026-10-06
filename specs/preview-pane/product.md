# Product Spec: Preview pane

**Issue:** none (fork feature); follows the browser pane (`specs/browser-pane/`)
**Basis:** the "App pane" plan of 6 Oct 2026, which studied joydle/Robin (window streaming) and tddworks/baguette (iOS Simulator streaming)

## Summary

A pane that shows another program live — a game engine, a 3D tool, a game build or a device simulator — and lets the user and agents drive it without leaving Warp. It ships behind `FeatureFlag::PreviewPane`, macOS first.

## Problem

People building games and apps from Warp switch to the engine, the simulator or the running game to see what a change did. Agents in Warp cannot see those programs at all, so they cannot check their own work, press Play, or tap through an app.

## Goals

- Show Blender, Unity, Godot and Defold (editors and built games) live in a pane.
- Show the iOS Simulator and the Android Emulator live in a pane, without their own windows.
- Watch what is running at a glance while working, in a pane or a floating picture-in-picture window, with several previews stacked and switchable.
- Let the user interact with what the pane shows when they want to, and let agents do the same through tools.
- Let agents open what they need themselves.

## Non-goals

- Replacing each engine's own automation. Agents connect Blender MCP, MCP for Unity, GoPeak (Godot) or Defold's editor commands themselves; the pane shows and drives pixels.
- Windows and Linux in the first release.
- Recording or sharing what the pane shows.

## Behavior

1. With `FeatureFlag::PreviewPane` off, or on a platform other than macOS, no preview pane entry point is shown and nothing is captured.
2. "Open preview pane" in the command palette splits the active pane and shows a start page listing previewable targets: open windows of other apps (app name, window title, size), booted iOS simulators and running Android emulators. Choosing one shows it in the pane. Warp's own windows are never listed.
3. The pane shows the target live at its own frame rate, letterboxed to the pane, and follows pane resizes, window resizes, zoom and tab switches. It hides under Warp's menus, palette and modals like the browser pane does.
4. Previews are for watching first: a pane opens in **Watching**, where input goes nowhere. A **Watching / Playing** switch decides whether the user's input reaches the target. In Playing, clicks, drags (left, right and middle button), scrolling and keys in the pane go to the target; the user's own pointer stays in Warp. Warp's bound shortcuts keep working.
5. A desktop target's window stays behind Warp: the pane never brings it forward, and it keeps updating while covered. A minimized window shows "This window is minimized" with a Restore button instead of a frozen frame.
6. Simulators run without their own windows: an iOS simulator is booted and streamed with Baguette, and an Android emulator runs with `-no-window` and streams over its gRPC control API.
7. Capture needs macOS Screen Recording permission and input needs Accessibility. When either is missing, the pane explains why and links to the right Settings page; nothing is asked until the first preview pane opens.
8. Capture stops while the pane is not visible, so Warp does not record in the background.
9. Agents get preview tools next to the browser tools, through the same local MCP endpoint and Claude Code plugin: list targets, open one (launching the app or booting the device when asked), screenshot, click, drag, scroll, type and press keys, and for simulators read the accessibility tree. Each action replies with a small picture of the result.
10. Agents ask before using an app, with Allow once, Always allow and Deny per app, unless the toolbar's Agent switch is on Auto. The activity bar, agent cursor, Pause and Stop work as in the browser pane, and a closed preview pane is never used.
11. Preview panes are restored after a restart by app and window title, or by device.
12. **Picture-in-picture:** any preview can pop out into a floating live window over Warp's content, snapped to a corner, draggable and resizable, and back into a pane. It follows the same Watching / Playing, approval and hiding rules as the pane.
13. **Stack:** several previews in one pane or picture-in-picture window stack as live cards; the front one is large and the rest show small live thumbnails beside it. Clicking a card brings it to the front, and agents' `preview_open` adds to the stack instead of opening another pane. A card shows its source (app icon and window title, or device name) and closes on its own.

## Success criteria

Each behavior can be checked by hand on a Mac against Blender, the Unity editor and a Unity player, the Godot editor and a running Godot game, the Defold editor, a booted iPhone simulator and a headless Android emulator.
