//! Servers listening on this machine, suggested as pages to open.

/// A process accepting TCP connections on a loopback or wildcard address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalServer {
    pub port: u16,
    /// Name of the listening process, such as `node` or `python3`.
    pub process: String,
}

impl LocalServer {
    pub fn url(&self) -> String {
        format!("http://localhost:{}/", self.port)
    }
}

/// `lsof` arguments listing listening TCP sockets in the format [`parse_lsof_listeners`] reads.
/// `+c 0` keeps process names untruncated so [`IGNORED_PROCESSES`] can match them.
pub const LSOF_ARGS: &[&str] = &["-nP", "+c", "0", "-iTCP", "-sTCP:LISTEN", "-Fpcn"];

/// Processes that listen locally but do not serve web pages: macOS system services (AirPlay
/// takes ports 5000 and 7000) and common databases.
const IGNORED_PROCESSES: &[&str] = &[
    "AirPlayXPCHelper",
    "ControlCenter",
    "mongod",
    "mysqld",
    "postgres",
    "rapportd",
    "redis-server",
];

/// Parses `lsof -F pcn` output into servers sorted by port, one per port. Skips `excluded_pid`,
/// privileged ports, sockets bound only to other interfaces and [`IGNORED_PROCESSES`].
pub fn parse_lsof_listeners(output: &str, excluded_pid: u32) -> Vec<LocalServer> {
    let mut servers: Vec<LocalServer> = Vec::new();
    let mut pid = None;
    let mut process = "";
    for line in output.lines() {
        let Some(field) = line.chars().next() else {
            continue;
        };
        let value = &line[field.len_utf8()..];
        match field {
            'p' => pid = value.parse::<u32>().ok(),
            'c' => process = value,
            'n' => {
                if pid == Some(excluded_pid) || IGNORED_PROCESSES.contains(&process) {
                    continue;
                }
                let Some(port) = listening_port(value) else {
                    continue;
                };
                if servers.iter().all(|server| server.port != port) {
                    servers.push(LocalServer {
                        port,
                        process: process.to_owned(),
                    });
                }
            }
            _ => {}
        }
    }
    servers.sort_by_key(|server| server.port);
    servers
}

/// The port of a socket address such as `*:5173` or `[::1]:3000`, if `localhost` reaches it.
fn listening_port(address: &str) -> Option<u16> {
    let (host, port) = address.rsplit_once(':')?;
    let reachable = matches!(host, "*" | "0.0.0.0" | "[::]" | "[::1]") || host.starts_with("127.");
    let port = port.parse::<u16>().ok()?;
    (reachable && port >= 1024).then_some(port)
}

#[cfg(test)]
#[path = "local_servers_tests.rs"]
mod tests;
