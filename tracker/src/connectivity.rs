//! Cheap check whether the Rolx service can be reached.

use std::{
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

/// Host and port of `server_url`; the port defaults from the scheme.
fn host_and_port(server_url: &str) -> Option<(String, u16)> {
    let (scheme, rest) = server_url.split_once("://")?;
    let default_port = match scheme {
        "https" => 443,
        "http" => 80,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;
    // `host:port`, `[v6]:port`, or a host/`[v6]` without port
    match authority.rsplit_once(':') {
        Some((host, port)) if !port.contains(']') => {
            Some((host.trim_matches(['[', ']']).to_owned(), port.parse().ok()?))
        }
        _ if authority.is_empty() => None,
        _ => Some((authority.trim_matches(['[', ']']).to_owned(), default_port)),
    }
}

/// Whether a TCP connection to the server can be opened within `timeout`.
pub fn is_reachable(server_url: &str, timeout: Duration) -> bool {
    let Some((host, port)) = host_and_port(server_url) else {
        return false;
    };
    let Ok(addresses) = (host.as_str(), port).to_socket_addrs() else {
        return false;
    };
    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, timeout).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_server_urls() {
        let hp = |u| host_and_port(u);
        assert_eq!(hp("https://rolx.cudos.ch"), Some(("rolx.cudos.ch".into(), 443)));
        assert_eq!(hp("http://example.org/x?y"), Some(("example.org".into(), 80)));
        assert_eq!(hp("https://h:8443/api"), Some(("h".into(), 8443)));
        assert_eq!(hp("http://[::1]:3000"), Some(("::1".into(), 3000)));
        assert_eq!(hp("http://[::1]"), Some(("::1".into(), 80)));
        assert_eq!(hp("https://u:p@h/"), Some(("h".into(), 443)));
        assert_eq!(hp("ftp://h"), None);
        assert_eq!(hp("h"), None);
        assert_eq!(hp("https://"), None);
    }

    #[test]
    fn reaches_local_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        assert!(is_reachable(&url, Duration::from_secs(2)));
        drop(listener);
        assert!(!is_reachable(&url, Duration::from_secs(2)));
    }
}
