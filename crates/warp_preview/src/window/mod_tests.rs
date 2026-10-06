use super::*;

fn window(pid: i32, app: &str, size: (u32, u32), layer: i64, on_screen: bool) -> WindowEntry {
    WindowEntry {
        source: WindowSource {
            window_id: pid as u32,
            pid,
            app_name: app.to_owned(),
            app_id: format!("com.example.{app}"),
            title: String::new(),
        },
        width: size.0,
        height: size.1,
        layer,
        on_screen,
    }
}

#[test]
fn keeps_other_apps_normal_windows() {
    let windows = vec![
        window(1, "Blender", (1512, 944), 0, true),
        window(2, "Warp", (1200, 800), 0, true),
        window(3, "Menu", (1200, 24), 0, true),
        window(4, "Dock", (1200, 800), 20, true),
        window(5, "", (500, 500), 0, true),
    ];
    let kept: Vec<i32> = previewable(windows, 2)
        .iter()
        .map(|window| window.source.pid)
        .collect();
    assert_eq!(kept, vec![1]);
}

#[test]
fn lists_on_screen_windows_first() {
    let windows = vec![
        window(1, "Godot", (800, 600), 0, false),
        window(3, "Unity", (800, 600), 0, true),
    ];
    let order: Vec<i32> = previewable(windows, 0)
        .iter()
        .map(|window| window.source.pid)
        .collect();
    assert_eq!(order, vec![3, 1]);
}
