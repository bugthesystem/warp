//! iOS simulators, booted, streamed and driven headlessly with Baguette
//! (<https://github.com/tddworks/baguette>), so the Simulator app's window is never needed and the
//! device can be played from any desktop. macOS only: elsewhere nothing is listed.

#[cfg(target_os = "macos")]
mod run;
mod wire;

use std::path::PathBuf;

use crate::SimulatorSource;

pub use wire::parse_simctl_devices;

/// A simulator device that can be previewed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulatorEntry {
    pub source: SimulatorSource,
    /// The OS it runs, such as "iOS 26.5".
    pub runtime: String,
    pub booted: bool,
}

/// Whether simulators can be previewed on this platform.
pub const fn is_supported() -> bool {
    cfg!(target_os = "macos")
}

/// Where Baguette is installed, if it is.
pub fn find_baguette() -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    // Apps started from the Dock get a minimal PATH without Homebrew's directories.
    let homebrew = [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    from_path
        .into_iter()
        .chain(homebrew)
        .map(|dir| dir.join("baguette"))
        .find(|path| path.is_file())
}

/// The iOS simulators installed with Xcode, booted ones first. Blocks briefly, so call it off the
/// main thread.
pub fn list_simulators() -> Result<Vec<SimulatorEntry>, crate::Error> {
    #[cfg(target_os = "macos")]
    {
        run::list_simulators()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(crate::Error::Unsupported)
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn start(
    source: SimulatorSource,
    rate: crate::Rate,
    events: async_channel::Sender<crate::StreamEvent>,
) -> Result<crate::Stream, crate::Error> {
    #[cfg(target_os = "macos")]
    {
        let baguette = find_baguette().ok_or(crate::Error::NoBaguette)?;
        run::start(source, baguette, rate, events)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (source, rate, events);
        Err(crate::Error::Unsupported)
    }
}
