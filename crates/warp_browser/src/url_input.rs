use url::form_urlencoded;

const SEARCH_URL_PREFIX: &str = "https://www.google.com/search?q=";

/// Schemes loaded as typed. Anything else that parses as a URL (such as `localhost:3000`, which
/// parses with the scheme `localhost`) is treated as a host name or a search.
const KNOWN_SCHEMES: &[&str] = &["http", "https", "file", "about", "data"];

/// Turns what the user typed into the URL field into a URL to load. URLs and host names load
/// directly, local hosts over `http` and everything else over `https`. Any other input becomes a
/// web search.
pub fn resolve_input(input: &str) -> String {
    let input = input.trim();
    if input.is_empty() {
        return "about:blank".to_owned();
    }

    if let Ok(url) = url::Url::parse(input)
        && KNOWN_SCHEMES.contains(&url.scheme())
    {
        return input.to_owned();
    }

    if !input.contains(char::is_whitespace)
        && let Some(host) = host_of(input)
    {
        let scheme = if is_local_host(host) { "http" } else { "https" };
        return format!("{scheme}://{input}");
    }

    let query: String = form_urlencoded::byte_serialize(input.as_bytes()).collect();
    format!("{SEARCH_URL_PREFIX}{query}")
}

/// A short form of `url` for an unfocused URL field: no `http(s)://` or `www.`, and no trailing
/// slash on a bare host. Other URLs, such as `about:blank`, are shown as they are.
pub fn display_url(url: &str) -> String {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return url.to_owned();
    };
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    rest.strip_suffix('/')
        .filter(|host| !host.contains('/'))
        .unwrap_or(rest)
        .to_owned()
}

/// Whether `input`, typed or clicked, points at this machine, such as a local dev server.
pub fn is_local_address(input: &str) -> bool {
    url::Url::parse(&resolve_input(input))
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|host| is_local_host(&host))
}

/// Returns the host of a scheme-less address such as `example.com/path` or `localhost:3000`, or
/// `None` if the input does not look like one.
fn host_of(input: &str) -> Option<&str> {
    let authority = input.split(['/', '?', '#']).next()?;
    if authority.starts_with('[') {
        // An IPv6 literal such as `[::1]:8080`.
        let end = authority.find(']')?;
        return Some(&authority[..=end]);
    }

    let host = match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        Some(_) => return None,
        None => authority,
    };

    let is_host_like = host == "localhost"
        || (host.contains('.')
            && !host.starts_with('.')
            && !host.ends_with('.')
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'));
    is_host_like.then_some(host)
}

fn is_local_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "0.0.0.0" | "[::1]") || host.ends_with(".localhost")
}

#[cfg(test)]
#[path = "url_input_tests.rs"]
mod tests;
