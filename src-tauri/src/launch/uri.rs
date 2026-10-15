//! `rhyme://` → [`LaunchRequest`].
//!
//! Windows hands a registered scheme to the app as a single argument, so this
//! is reached from the same argv the command line uses — one more spelling of
//! a request, not a second entry point. Parsed by hand: the grammar is one
//! action plus a query string, and a URL crate would be a dependency for it.

use super::{LaunchRequest, LaunchSource};

pub const SCHEME: &str = "rhyme";

/// Whether this argument is a `rhyme://` URL rather than a path or a flag.
pub fn is_uri(argument: &str) -> bool {
    argument
        .get(..SCHEME.len() + 3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&format!("{SCHEME}://")))
}

/// Parse `rhyme://open?cwd=…`, `rhyme://ssh?host=…&user=…`, or `rhyme://open`.
///
/// Unknown actions and unknown parameters are ignored rather than rejected: a
/// URL comes from outside the app, and the worst it should be able to do is
/// open a plain terminal.
pub fn parse(uri: &str) -> LaunchRequest {
    let rest = uri
        .get(SCHEME.len() + 3..)
        .unwrap_or_default()
        .trim_start_matches('/');
    let (action, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut request = LaunchRequest {
        source: LaunchSource::Uri,
        ..Default::default()
    };
    let mut host = None;
    let mut user = None;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = decode(value);
        if value.is_empty() {
            continue;
        }
        match key.to_ascii_lowercase().as_str() {
            "cwd" | "dir" | "path" => request.cwd = Some(value),
            "file" => request.files.push(value),
            "command" | "cmd" => request.command = Some(value),
            "ssh" | "target" => request.ssh = Some(value),
            "host" => host = Some(value),
            "user" => user = Some(value),
            "newwindow" => request.new_window = value != "0" && value != "false",
            _ => {}
        }
    }
    if action.eq_ignore_ascii_case("ssh") && request.ssh.is_none() {
        request.ssh = match (user, host) {
            (Some(user), Some(host)) => Some(format!("{user}@{host}")),
            (None, Some(host)) => Some(host),
            _ => None,
        };
    }
    request
}

/// Percent-decoding with `+` for space, over bytes so a multi-byte character
/// split across escapes survives.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                match u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_carries_a_working_directory() {
        let request = parse("rhyme://open?cwd=D%3A%5Cworkspace");
        assert_eq!(request.cwd.as_deref(), Some(r"D:\workspace"));
        assert_eq!(request.source, LaunchSource::Uri);
    }

    #[test]
    fn ssh_builds_its_target_from_user_and_host() {
        let request = parse("rhyme://ssh?host=example.com&user=test");
        assert_eq!(request.ssh.as_deref(), Some("test@example.com"));
    }

    #[test]
    fn a_percent_encoded_hangul_path_round_trips() {
        let request = parse(
            "rhyme://open?cwd=D%3A%5C%EA%B0%9C%EB%B0%9C%20%ED%94%84%EB%A1%9C%EC%A0%9D%ED%8A%B8",
        );
        assert_eq!(request.cwd.as_deref(), Some(r"D:\개발 프로젝트"));
    }

    #[test]
    fn an_unknown_action_still_opens_a_terminal() {
        assert!(parse("rhyme://whatever?nope=1").is_empty());
        assert!(parse("rhyme://").is_empty());
    }

    #[test]
    fn a_uri_is_told_apart_from_a_path_or_a_flag() {
        assert!(is_uri("rhyme://open"));
        assert!(is_uri("RHYME://open"));
        assert!(!is_uri(r"D:\rhyme\project"));
        assert!(!is_uri("--cwd"));
    }
}
