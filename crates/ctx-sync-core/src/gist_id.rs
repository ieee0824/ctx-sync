//! Parsing of Gist IDs and URLs.

use crate::{Error, Result};

const GIST_HOST: &str = "gist.github.com";
const SSH_PREFIX: &str = "git@gist.github.com:";

/// Returns the Gist ID from an ID or a Gist URL.
///
/// Accepted forms:
/// - `0123abcd`
/// - `https://gist.github.com/<user>/0123abcd` and `https://gist.github.com/0123abcd`,
///   optionally followed by `/`, `.git`, `#fragment` or `?query`
/// - `git@gist.github.com:0123abcd.git`
pub fn parse_gist_id(input: &str) -> Result<String> {
    let trimmed = input.trim();
    let invalid = || Error::InvalidConfig(format!("invalid gist id or url: {input:?}"));

    let candidate = if let Some(rest) = trimmed.strip_prefix(SSH_PREFIX) {
        rest
    } else if let Some((scheme, rest)) = trimmed.split_once("://") {
        if scheme != "https" && scheme != "http" {
            return Err(invalid());
        }
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if !host.eq_ignore_ascii_case(GIST_HOST) {
            return Err(invalid());
        }
        let path = path.split(['#', '?']).next().unwrap_or("");
        path.trim_end_matches('/').rsplit('/').next().unwrap_or("")
    } else {
        trimmed
    };

    let id = candidate.strip_suffix(".git").unwrap_or(candidate);
    if is_valid_id(id) {
        Ok(id.to_string())
    } else {
        Err(invalid())
    }
}

fn is_valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ids_and_urls() {
        let inputs = [
            "0123abcd",
            "  0123abcd\n",
            "https://gist.github.com/user/0123abcd",
            "https://gist.github.com/0123abcd",
            "https://gist.github.com/user/0123abcd/",
            "https://gist.github.com/user/0123abcd.git",
            "https://gist.github.com/user/0123abcd#file-x",
            "https://gist.github.com/user/0123abcd?a=b",
            "https://gist.github.com/0123abcd/",
            "https://gist.github.com/0123abcd.git",
            "https://gist.github.com/0123abcd#file-x",
            "https://gist.github.com/0123abcd?a=b",
            "git@gist.github.com:0123abcd.git",
        ];
        for input in inputs {
            assert_eq!(parse_gist_id(input).unwrap(), "0123abcd", "{input:?}");
        }
    }

    #[test]
    fn accepts_full_length_id() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(parse_gist_id(id).unwrap(), id);
    }

    #[test]
    fn rejects_invalid_input() {
        let inputs = [
            "",
            "   ",
            "https://github.com/user/repo",
            "abc/def",
            "0123-abcd",
            "https://gist.github.com/",
            "ftp://gist.github.com/0123abcd",
        ];
        for input in inputs {
            let err = parse_gist_id(input).unwrap_err();
            assert_eq!(err.exit_code(), 4, "{input:?}: {err}");
        }
    }
}
