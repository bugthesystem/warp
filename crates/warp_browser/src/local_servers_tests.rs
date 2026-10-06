use super::{LocalServer, parse_lsof_listeners};

fn server(port: u16, process: &str) -> LocalServer {
    LocalServer {
        port,
        process: process.to_owned(),
    }
}

#[test]
fn lists_loopback_and_wildcard_listeners_by_port() {
    let output = "p100\ncnode\nf23\nn*:5173\np200\ncpython3\nf4\nn127.0.0.1:8000\np300\ncruby\nf9\nn[::1]:3000\n";

    let servers = parse_lsof_listeners(output, 1);

    assert_eq!(
        servers,
        [
            server(3000, "ruby"),
            server(5173, "node"),
            server(8000, "python3")
        ]
    );
}

#[test]
fn lists_a_port_once_when_bound_on_several_addresses() {
    let output = "p100\ncnode\nf23\nn127.0.0.1:3000\nf24\nn[::1]:3000\n";

    let servers = parse_lsof_listeners(output, 1);

    assert_eq!(servers, [server(3000, "node")]);
}

#[test]
fn skips_the_excluded_process() {
    let output = "p42\ncwarp\nf10\nn127.0.0.1:9282\np100\ncnode\nf23\nn*:3000\n";

    let servers = parse_lsof_listeners(output, 42);

    assert_eq!(servers, [server(3000, "node")]);
}

#[test]
fn skips_system_services_databases_and_privileged_ports() {
    let output = "p1\ncControlCenter\nf5\nn*:5000\np2\ncpostgres\nf6\nn127.0.0.1:5432\n\
                  p3\ncnginx\nf7\nn*:80\np4\ncnode\nf8\nn192.168.1.20:4000\n";

    let servers = parse_lsof_listeners(output, 0);

    assert_eq!(servers, []);
}

#[test]
fn builds_a_localhost_url() {
    assert_eq!(server(5173, "node").url(), "http://localhost:5173/");
}
