//! Path claims use a small, repository-relative glob syntax.

use std::collections::HashMap;

use crate::{Error, Result};

/// Reject patterns that cannot name a safe relative repository path.
pub fn validate_pattern(pattern: &str) -> Result<()> {
    if pattern.is_empty()
        || pattern.trim() != pattern
        || pattern.starts_with('/')
        || pattern.contains('\\')
        || pattern.split('/').any(|part| part == "..")
    {
        return Err(Error::InvalidConfig(format!(
            "invalid claim pattern: {pattern:?}"
        )));
    }
    Ok(())
}

/// Match path components; `**` as a whole component spans zero or more directories.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern = components(pattern);
    let path = components(path);
    let mut memo = HashMap::new();
    matches_components(&pattern, &path, 0, 0, &mut memo)
}

fn components(value: &str) -> Vec<&str> {
    let value = value.trim_end_matches('/');
    if value.is_empty() {
        Vec::new()
    } else {
        value.split('/').collect()
    }
}

fn matches_components(
    pattern: &[&str],
    path: &[&str],
    p: usize,
    s: usize,
    memo: &mut HashMap<(usize, usize), bool>,
) -> bool {
    if let Some(value) = memo.get(&(p, s)) {
        return *value;
    }
    let result = if p == pattern.len() {
        s == path.len()
    } else if pattern[p] == "**" {
        matches_components(pattern, path, p + 1, s, memo)
            || (s < path.len() && matches_components(pattern, path, p, s + 1, memo))
    } else {
        s < path.len()
            && matches_component(pattern[p], path[s])
            && matches_components(pattern, path, p + 1, s + 1, memo)
    };
    memo.insert((p, s), result);
    result
}

fn matches_component(pattern: &str, path: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let path: Vec<char> = path.chars().collect();
    let mut current = vec![false; path.len() + 1];
    current[0] = true;
    for token in pattern {
        let mut next = vec![false; path.len() + 1];
        match token {
            '*' => {
                next[0] = current[0];
                for i in 1..=path.len() {
                    next[i] = current[i] || next[i - 1];
                }
            }
            '?' => {
                next[1..].copy_from_slice(&current[..path.len()]);
            }
            literal => {
                for i in 1..=path.len() {
                    next[i] = current[i - 1] && literal == path[i - 1];
                }
            }
        }
        current = next;
    }
    current[path.len()]
}

/// Return the path prefix before the first component containing `*` or `?`.
pub fn literal_prefix(pattern: &str) -> &str {
    let trimmed = pattern.trim_end_matches('/');
    let mut offset = 0;
    for component in trimmed.split('/') {
        if component.contains('*') || component.contains('?') {
            return trimmed[..offset].trim_end_matches('/');
        }
        offset += component.len() + 1;
    }
    trimmed
}

/// Advisory overlap: direct match, or shared literal path-component prefix.
pub fn overlaps(a: &str, b: &str) -> bool {
    if matches(a, b) || matches(b, a) {
        return true;
    }
    let a = literal_prefix(a);
    let b = literal_prefix(b);
    if a.is_empty() || b.is_empty() {
        return true;
    }
    let mut a = a.split('/');
    let mut b = b.split('/');
    loop {
        match (a.next(), b.next()) {
            (Some(left), Some(right)) if left != right => return false,
            (Some(_), Some(_)) => {}
            _ => return true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_respects_components_and_double_star() {
        for (pattern, path, expected) in [
            ("src/**", "src/a/b.rs", true),
            ("src/*.rs", "src/a.rs", true),
            ("src/*.rs", "src/a/b.rs", false),
            ("src/?.rs", "src/a.rs", true),
            ("**", "x", true),
            ("src/**/mod.rs", "src/mod.rs", true),
            ("src/**/mod.rs", "src/a/b/mod.rs", true),
            ("src/par", "src/parser.rs", false),
            ("docs/", "docs", true),
        ] {
            assert_eq!(matches(pattern, path), expected, "{pattern} vs {path}");
        }
    }

    #[test]
    fn literal_prefix_stops_before_wildcards() {
        assert_eq!(literal_prefix("src/parser/**"), "src/parser");
        assert_eq!(literal_prefix("*.md"), "");
        assert_eq!(literal_prefix("crates/*/Cargo.toml"), "crates");
    }

    #[test]
    fn overlap_uses_matches_and_component_prefixes() {
        for (a, b, expected) in [
            ("src/parser/**", "src/parser/lexer.rs", true),
            ("src/parser/**", "src/ui/**", false),
            ("src/**", "src/ui/**", true),
            ("src/par*", "src/parser/**", true),
            ("docs/*.md", "src/*.rs", false),
        ] {
            assert_eq!(overlaps(a, b), expected, "{a} vs {b}");
        }
    }

    #[test]
    fn validation_rejects_unsafe_or_malformed_patterns() {
        for pattern in ["", "/abs", "../x", "a/../b", "a\\b", " x"] {
            assert!(matches!(
                validate_pattern(pattern),
                Err(Error::InvalidConfig(_))
            ));
        }
        assert!(validate_pattern("src/**").is_ok());
    }
}
