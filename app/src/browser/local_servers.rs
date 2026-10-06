use std::time::Duration;

use command::Stdio;
use command::r#async::Command;
use warp_browser::local_servers::{LSOF_ARGS, LocalServer, parse_lsof_listeners};
use warpui::r#async::FutureExt as _;

const LSOF_TIMEOUT: Duration = Duration::from_secs(5);

/// Servers other processes are running on this machine, or none if `lsof` is unavailable.
pub async fn detect_local_servers() -> Vec<LocalServer> {
    let mut command = Command::new("lsof");
    command
        .args(LSOF_ARGS)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    // lsof exits non-zero when nothing is listening, so its status is not checked.
    let Ok(Ok(output)) = command.output().with_timeout(LSOF_TIMEOUT).await else {
        return Vec::new();
    };
    parse_lsof_listeners(&String::from_utf8_lossy(&output.stdout), std::process::id())
}
