//! Parser and renderer for the Markdown files stored in the Gist.
//!
//! A document is a `# Title`, optional `Key: value` fields, and `## Section`s:
//!
//! ```text
//! # Worker
//!
//! ID: 6f1c2b7e-...
//! Name: parser
//!
//! ## Task
//!
//! Implement the parser.
//! ```
//!
//! Headings inside fenced code blocks are not treated as headings.

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MdDoc {
    /// Text after `# `.
    pub title: String,
    /// `Key: value` lines between the title and the first `## `.
    pub fields: Vec<(String, String)>,
    /// Other non-empty lines in the same area, joined with `\n`.
    pub preamble: String,
    pub sections: Vec<MdSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MdSection {
    /// Text after `## `, trimmed.
    pub heading: String,
    /// Body without leading and trailing blank lines.
    pub body: String,
}

impl MdDoc {
    pub fn parse(input: &str) -> Result<Self> {
        let text = input.replace("\r\n", "\n");
        let mut lines = text.split('\n').skip_while(|l| l.trim().is_empty());
        let title = lines
            .next()
            .and_then(parse_title)
            .ok_or_else(|| Error::General("invalid markdown: missing '# ' title".into()))?;

        let mut doc = MdDoc {
            title,
            ..Default::default()
        };
        let mut preamble = Vec::new();
        let mut current: Option<(String, Vec<&str>)> = None;
        let mut fence = Fence::default();

        for line in lines {
            if !fence.is_open()
                && let Some(heading) = parse_h2(line)
            {
                if let Some((heading, body)) = current.take() {
                    doc.sections.push(MdSection::new(heading, &body));
                }
                current = Some((heading, Vec::new()));
                continue;
            }
            let in_code = fence.update(line);
            match &mut current {
                Some((_, body)) => body.push(line),
                None => match parse_field(line).filter(|_| !in_code) {
                    Some(field) => doc.fields.push(field),
                    None if !line.trim().is_empty() => preamble.push(line),
                    None => {}
                },
            }
        }
        if let Some((heading, body)) = current {
            doc.sections.push(MdSection::new(heading, &body));
        }
        doc.preamble = preamble.join("\n");
        Ok(doc)
    }

    pub fn render(&self) -> String {
        let mut blocks = vec![if self.title.is_empty() {
            "#".to_string()
        } else {
            format!("# {}", self.title)
        }];
        if !self.fields.is_empty() {
            blocks.push(render_fields(&self.fields));
        }
        if !self.preamble.is_empty() {
            blocks.push(self.preamble.clone());
        }
        for section in &self.sections {
            blocks.push(if section.body.is_empty() {
                format!("## {}", section.heading)
            } else {
                format!("## {}\n\n{}", section.heading, section.body)
            });
        }
        let mut out = blocks.join("\n\n");
        out.push('\n');
        out
    }

    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Replaces the value of `key`, or appends the field.
    pub fn set_field(&mut self, key: &str, value: &str) {
        match self.fields.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value.to_string(),
            None => self.fields.push((key.to_string(), value.to_string())),
        }
    }

    pub fn section(&self, heading: &str) -> Option<&MdSection> {
        self.sections.iter().find(|s| s.heading == heading)
    }

    pub fn section_body(&self, heading: &str) -> Option<&str> {
        self.section(heading).map(|s| s.body.as_str())
    }

    /// Replaces the body of `heading` in place, or appends the section.
    pub fn set_section(&mut self, heading: &str, body: &str) {
        let body = trim_blank_lines(&body.split('\n').collect::<Vec<_>>());
        match self.sections.iter_mut().find(|s| s.heading == heading) {
            Some(section) => section.body = body,
            None => self.sections.push(MdSection {
                heading: heading.to_string(),
                body,
            }),
        }
    }
}

impl MdSection {
    fn new(heading: String, lines: &[&str]) -> Self {
        Self {
            heading,
            body: trim_blank_lines(lines),
        }
    }
}

/// Collects `Key: value` lines (e.g. `Branch:` / `Commit:` inside a section).
pub fn parse_fields(body: &str) -> Vec<(String, String)> {
    body.lines().filter_map(parse_field).collect()
}

/// `Key: value` per line; an empty value renders as `Key:`.
pub fn render_fields(fields: &[(String, String)]) -> String {
    fields
        .iter()
        .map(|(k, v)| {
            if v.is_empty() {
                format!("{k}:")
            } else {
                format!("{k}: {v}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Items of `- ` / `* ` lines. Without any bullet, a non-empty body is one item.
pub fn parse_list(body: &str) -> Vec<String> {
    let mut has_bullet = false;
    let items: Vec<String> = body
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let item = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))?;
            has_bullet = true;
            Some(item.trim().to_string())
        })
        .filter(|item| !item.is_empty())
        .collect();
    if has_bullet {
        items
    } else if body.trim().is_empty() {
        Vec::new()
    } else {
        vec![body.trim().to_string()]
    }
}

/// `- a\n- b` (empty string for no items).
pub fn render_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Makes every ATX heading outside fenced code `levels` deeper (at most `######`).
pub fn demote_headings(markdown: &str, levels: usize) -> String {
    let mut fence = Fence::default();
    markdown
        .split('\n')
        .map(|line| {
            let was_open = fence.is_open();
            fence.update(line);
            if was_open || fence.is_open() {
                return line.to_string();
            }
            let hashes = line.chars().take_while(|&c| c == '#').count();
            let rest = &line[hashes..];
            if (1..=6).contains(&hashes) && (rest.is_empty() || rest.starts_with(' ')) {
                format!("{}{rest}", "#".repeat((hashes + levels).min(6)))
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Tracks whether we are inside a fenced code block.
#[derive(Default)]
struct Fence {
    marker: Option<&'static str>,
}

impl Fence {
    fn is_open(&self) -> bool {
        self.marker.is_some()
    }

    /// Feeds one line. Returns true when the line belongs to a code block
    /// (including the fence lines themselves).
    fn update(&mut self, line: &str) -> bool {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (self.marker, marker) {
            (None, Some(m)) => {
                self.marker = Some(m);
                true
            }
            (Some(open), Some(m)) if open == m => {
                self.marker = None;
                true
            }
            (open, _) => open.is_some(),
        }
    }
}

fn parse_title(line: &str) -> Option<String> {
    if line.trim_end() == "#" {
        return Some(String::new());
    }
    line.strip_prefix("# ").map(|t| t.trim().to_string())
}

fn parse_h2(line: &str) -> Option<String> {
    if line.trim_end() == "##" {
        return Some(String::new());
    }
    line.strip_prefix("## ").map(|h| h.trim().to_string())
}

fn parse_field(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once(':')?;
    let valid_key = key.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'));
    if !valid_key {
        return None;
    }
    let value = value
        .strip_prefix(' ')
        .or_else(|| value.strip_prefix('\t'))
        .unwrap_or(value);
    Some((key.to_string(), value.trim_end().to_string()))
}

fn trim_blank_lines(lines: &[&str]) -> String {
    let start = lines.iter().position(|l| !l.trim().is_empty());
    let end = lines.iter().rposition(|l| !l.trim().is_empty());
    match (start, end) {
        (Some(start), Some(end)) => lines[start..=end].join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKER: &str = "# Worker

ID: 123
Name: parser

## Task

Parser を実装する。

## Changed

- src/a.rs
- src/b.rs
";

    #[test]
    fn parses_title_fields_and_sections() {
        let doc = MdDoc::parse(WORKER).unwrap();
        assert_eq!(doc.title, "Worker");
        assert_eq!(doc.field("ID"), Some("123"));
        assert_eq!(doc.field("Name"), Some("parser"));
        assert_eq!(doc.preamble, "");
        assert_eq!(doc.section_body("Task"), Some("Parser を実装する。"));
        assert_eq!(doc.section_body("Changed"), Some("- src/a.rs\n- src/b.rs"));
    }

    #[test]
    fn round_trips() {
        assert_eq!(MdDoc::parse(WORKER).unwrap().render(), WORKER);
    }

    #[test]
    fn headings_inside_fences_are_not_sections() {
        let doc = MdDoc::parse("# T\n\n## A\n\n```\n## not a heading\n```\n\n## B\n").unwrap();
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.section_body("A"), Some("```\n## not a heading\n```"));
        let doc = MdDoc::parse("# T\n\n## A\n\n~~~\n## still code\n~~~\n").unwrap();
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn deeper_headings_stay_in_the_body() {
        let doc = MdDoc::parse("# T\n\n## A\n\n### Sub\n\ntext\n").unwrap();
        assert_eq!(doc.section_body("A"), Some("### Sub\n\ntext"));
    }

    #[test]
    fn extra_blank_lines_are_normalized() {
        let messy = "\n\n# T\r\n\r\nK: v\n\n\n\n## A\n\n\n\nbody\n\n\n\n## B\n\n\n";
        let doc = MdDoc::parse(messy).unwrap();
        assert_eq!(doc.render(), "# T\n\nK: v\n\n## A\n\nbody\n\n## B\n");
        assert_eq!(MdDoc::parse(&doc.render()).unwrap(), doc);
    }

    #[test]
    fn keeps_preamble_lines() {
        let doc = MdDoc::parse("# T\n\nKey: v\nSome free text.\n\n## A\n").unwrap();
        assert_eq!(doc.fields, vec![("Key".into(), "v".into())]);
        assert_eq!(doc.preamble, "Some free text.");
        assert_eq!(MdDoc::parse(&doc.render()).unwrap(), doc);
    }

    #[test]
    fn field_value_keeps_colons() {
        let doc = MdDoc::parse("# T\n\nLast Updated: 2026-09-23T18:20:00+09:00\n").unwrap();
        assert_eq!(doc.field("Last Updated"), Some("2026-09-23T18:20:00+09:00"));
    }

    #[test]
    fn missing_title_is_an_error() {
        assert!(MdDoc::parse("no title\n").is_err());
        assert!(MdDoc::parse("").is_err());
        assert!(MdDoc::parse("## Section only\n").is_err());
    }

    #[test]
    fn set_section_replaces_or_appends() {
        let mut doc = MdDoc::parse(WORKER).unwrap();
        doc.set_section("Task", "\nNew task\n\n");
        assert_eq!(doc.section_body("Task"), Some("New task"));
        assert_eq!(doc.sections[0].heading, "Task");
        doc.set_section("Summary", "done");
        assert_eq!(doc.sections.last().unwrap().heading, "Summary");
        assert_eq!(doc.sections.len(), 3);
    }

    #[test]
    fn set_field_replaces_or_appends() {
        let mut doc = MdDoc::parse(WORKER).unwrap();
        doc.set_field("Name", "gist");
        doc.set_field("Status", "working");
        assert_eq!(
            doc.fields,
            vec![
                ("ID".into(), "123".into()),
                ("Name".into(), "gist".into()),
                ("Status".into(), "working".into()),
            ]
        );
    }

    #[test]
    fn parse_list_handles_bullets_text_and_empty() {
        assert_eq!(parse_list("- a\n* b\n-   c  "), vec!["a", "b", "c"]);
        assert_eq!(
            parse_list("ParserResult に warnings を追加。"),
            vec!["ParserResult に warnings を追加。"]
        );
        assert!(parse_list("  \n").is_empty());
        let items = vec!["a".to_string(), "b".to_string()];
        assert_eq!(parse_list(&render_list(&items)), items);
        assert_eq!(render_list(&[]), "");
    }

    #[test]
    fn fields_round_trip_with_empty_values() {
        let fields = vec![
            ("Branch".to_string(), "feat/x".to_string()),
            ("Commit".to_string(), String::new()),
        ];
        let text = render_fields(&fields);
        assert_eq!(text, "Branch: feat/x\nCommit:");
        assert_eq!(parse_fields(&text), fields);
    }

    #[test]
    fn demote_headings_skips_code() {
        assert_eq!(
            demote_headings("## A\n```\n# x\n```\n", 1),
            "### A\n```\n# x\n```\n"
        );
        assert_eq!(
            demote_headings("###### deep\n#tag\n", 2),
            "###### deep\n#tag\n"
        );
        assert_eq!(demote_headings("# A\ntext", 1), "## A\ntext");
    }
}
