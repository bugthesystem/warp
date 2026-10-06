use warp_preview::simulator::SimulatorEntry;
use warp_preview::window::WindowEntry;
use warp_preview::{SimulatorSource, WindowSource};

use super::{PreviewStatus, format_targets, status_note};

fn window(window_id: u32, app_name: &str, title: &str, on_screen: bool) -> WindowEntry {
    WindowEntry {
        source: WindowSource {
            window_id,
            pid: 7,
            app_name: app_name.to_owned(),
            app_id: format!("com.example.{app_name}"),
            title: title.to_owned(),
        },
        width: 1280,
        height: 720,
        layer: 0,
        on_screen,
    }
}

#[test]
fn lists_previews_windows_and_how_to_open_a_url() {
    let text = format_targets(
        &["- preview 1: http://localhost:3000/".to_owned()],
        Ok(vec![
            window(42, "Godot", "Main.tscn", true),
            window(43, "Simulator", "", false),
        ]),
        Ok(vec![SimulatorEntry {
            source: SimulatorSource {
                udid: "ABC".to_owned(),
                name: "iPhone 17 Pro".to_owned(),
            },
            runtime: "iOS 26.5".to_owned(),
            booted: true,
        }]),
    );
    assert!(text.starts_with("Open previews:\n- preview 1: http://localhost:3000/\n"));
    assert!(text.contains("- window 42: Godot \"Main.tscn\" (1280x720)\n"));
    assert!(text.contains("- window 43: Simulator (1280x720, minimized)\n"));
    assert!(text.contains("- iPhone 17 Pro (iOS 26.5, booted), udid ABC\n"));
    assert!(text.ends_with("preview_open url=<url>."));
}

#[test]
fn leaves_windows_out_where_they_are_unsupported() {
    let text = format_targets(
        &[],
        Err(warp_preview::Error::Unsupported),
        Err(warp_preview::Error::Unsupported),
    );
    assert!(text.starts_with("No previews are open.\n"));
    assert!(!text.contains("App windows"));
    assert!(!text.contains("simulators"));
}

#[test]
fn explains_pictures_that_may_be_stale() {
    assert_eq!(status_note(&PreviewStatus::Live), None);
    assert_eq!(
        status_note(&PreviewStatus::Ended("The page closed".to_owned())).as_deref(),
        Some("ended: The page closed")
    );
}
