//! Detection of obvious credentials in text written to the shared context.
//!
//! A secret Gist is not a secret store: anyone with the URL can read it.
//! ctx-sync only warns (v0.1); it never blocks the write. The detected value
//! itself is never included in the warning.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFinding {
    pub kind: &'static str,
    /// Where the text came from, e.g. `summary` or `attention[0]`.
    pub field: String,
}

struct TokenRule {
    kind: &'static str,
    prefixes: &'static [&'static str],
    body: fn(char) -> bool,
    min_len: usize,
    /// When set, the body must be exactly this long.
    exact_len: Option<usize>,
}

fn alnum(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

fn alnum_dash_underscore(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn alnum_underscore(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn upper_digit(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit()
}

fn slack_body(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

const TOKEN_RULES: &[TokenRule] = &[
    TokenRule {
        kind: "openai-api-key",
        prefixes: &["sk-"],
        body: alnum_dash_underscore,
        min_len: 20,
        exact_len: None,
    },
    TokenRule {
        kind: "github-token",
        prefixes: &["ghp_", "gho_", "ghu_", "ghs_", "ghr_"],
        body: alnum,
        min_len: 20,
        exact_len: None,
    },
    TokenRule {
        kind: "github-pat",
        prefixes: &["github_pat_"],
        body: alnum_underscore,
        min_len: 20,
        exact_len: None,
    },
    TokenRule {
        kind: "aws-access-key",
        prefixes: &["AKIA"],
        body: upper_digit,
        min_len: 16,
        exact_len: Some(16),
    },
    TokenRule {
        kind: "slack-token",
        prefixes: &["xoxb-", "xoxp-", "xoxa-", "xoxr-"],
        body: slack_body,
        min_len: 1,
        exact_len: None,
    },
];

/// Kinds of credentials found in `text`, at most one finding per kind.
pub fn scan(field: &str, text: &str) -> Vec<SecretFinding> {
    let mut kinds: Vec<&'static str> = Vec::new();
    for rule in TOKEN_RULES {
        if rule.prefixes.iter().any(|p| has_token(text, p, rule)) {
            kinds.push(rule.kind);
        }
    }
    if text.contains("-----BEGIN ") && text.contains("PRIVATE KEY-----") {
        kinds.push("private-key");
    }
    kinds
        .into_iter()
        .map(|kind| SecretFinding {
            kind,
            field: field.to_string(),
        })
        .collect()
}

/// Whether `prefix` occurs at a word boundary followed by a matching body.
fn has_token(text: &str, prefix: &str, rule: &TokenRule) -> bool {
    text.match_indices(prefix).any(|(start, _)| {
        let preceded_by_word = text[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric());
        if preceded_by_word {
            return false;
        }
        let rest = &text[start + prefix.len()..];
        let len = rest.chars().take_while(|&c| (rule.body)(c)).count();
        match rule.exact_len {
            Some(exact) => {
                len == exact
                    && !rest[exact..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphanumeric())
            }
            None => len >= rule.min_len,
        }
    })
}

pub fn warning_message(finding: &SecretFinding) -> String {
    format!(
        "possible credential ({}) in {}; do not put secrets into ctx-sync context",
        finding.kind, finding.field
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<&'static str> {
        scan("f", text).into_iter().map(|f| f.kind).collect()
    }

    /// Fake credentials are assembled at runtime so that the source itself
    /// does not look like it contains secrets (and does not trip secret
    /// scanners such as pre-commit hooks).
    fn fake(parts: &[&str]) -> String {
        parts.concat()
    }

    #[test]
    fn detects_each_pattern() {
        let cases = [
            (
                fake(&["key sk-", "abcdefghijklmnopqrstuvwx"]),
                "openai-api-key",
            ),
            (
                fake(&["sk-", "proj-abc_def-ghi_jkl-mno_pqr"]),
                "openai-api-key",
            ),
            (
                fake(&["see ghp_", "0123456789abcdefghij0123"]),
                "github-token",
            ),
            (fake(&["gho_", "0123456789ABCDEFGHIJ"]), "github-token"),
            (
                fake(&["github_pat_", "11ABCDEFG0123456789_abcdefghij"]),
                "github-pat",
            ),
            (fake(&["AKIA", "IOSFODNN7EXAMPLE"]), "aws-access-key"),
            (
                fake(&["id=", "AKIA", "IOSFODNN7EXAMPLE", "."]),
                "aws-access-key",
            ),
            (fake(&["xoxb", "-123-456-abc"]), "slack-token"),
            (
                fake(&["-----BEGIN OPENSSH ", "PRIVATE KEY", "-----\nabc"]),
                "private-key",
            ),
            (
                fake(&["-----BEGIN RSA ", "PRIVATE KEY", "-----"]),
                "private-key",
            ),
        ];
        for (text, kind) in cases {
            assert_eq!(kinds(&text), [kind], "{text}");
        }
    }

    #[test]
    fn ignores_look_alikes() {
        for text in [
            fake(&["task-skills are important"]),
            fake(&["sk-", "short"]),
            fake(&["AKIA", "123"]),
            fake(&["AKIA", "IOSFODNN7EXAMPLEXYZ"]),
            fake(&["askghp_", "0123456789abcdefghij0123"]),
            fake(&["ghp_", "short"]),
            fake(&["xoxb"]),
            fake(&["-----BEGIN ", "CERTIFICATE", "-----"]),
            fake(&["plain text"]),
        ] {
            assert!(kinds(&text).is_empty(), "{text}");
        }
    }

    #[test]
    fn reports_each_kind_once_with_the_field() {
        let text = fake(&[
            "ghp_",
            "0123456789abcdefghij0123 and ghp_",
            "abcdefghij0123456789abcd",
        ]);
        let findings = scan("attention[0]", &text);
        assert_eq!(
            findings,
            [SecretFinding {
                kind: "github-token",
                field: "attention[0]".into()
            }]
        );
    }

    #[test]
    fn warning_does_not_contain_the_value() {
        let text = fake(&["ghp_", "0123456789abcdefghij0123"]);
        let message = warning_message(&scan("summary", &text)[0]);
        assert_eq!(
            message,
            "possible credential (github-token) in summary; do not put secrets into ctx-sync context"
        );
        assert!(!message.contains("ghp_0123"));
    }
}
