//! Decision files (`30-decision-<YYYYMMDD>-<NNN>-<slug>.md`).
//!
//! Decisions are append-only: an existing file is never rewritten. A new
//! decision replaces an old one by listing it in `Supersedes:`.

use std::fmt;
use std::str::FromStr;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::md::{MdDoc, MdSection};
use crate::{Error, Result};

pub const DECISION_PREFIX: &str = "30-decision-";

const CONTEXT: &str = "Context";
const DECISION: &str = "Decision";
const REASON: &str = "Reason";
const CONSEQUENCES: &str = "Consequences";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    Proposed,
    Accepted,
    Superseded,
    Rejected,
}

impl DecisionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionStatus::Proposed => "proposed",
            DecisionStatus::Accepted => "accepted",
            DecisionStatus::Superseded => "superseded",
            DecisionStatus::Rejected => "rejected",
        }
    }
}

impl fmt::Display for DecisionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DecisionStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "proposed" => Ok(DecisionStatus::Proposed),
            "accepted" => Ok(DecisionStatus::Accepted),
            "superseded" => Ok(DecisionStatus::Superseded),
            "rejected" => Ok(DecisionStatus::Rejected),
            _ => Err(Error::General(format!("invalid decision status: {s:?}"))),
        }
    }
}

/// `<YYYYMMDD>-<NNN>-<slug>`, e.g. `20260923-001-gist-storage`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DecisionId(String);

impl DecisionId {
    pub fn new(date: NaiveDate, seq: u32, slug: &str) -> Self {
        Self(format!("{}-{seq:03}-{slug}", date.format("%Y%m%d")))
    }

    /// Requires 8 digits, a dash, at least 3 digits, a dash and `[a-z0-9-]+`.
    pub fn parse(s: &str) -> Result<Self> {
        let invalid = || Error::General(format!("invalid decision id: {s:?}"));
        let (date, rest) = s.split_once('-').ok_or_else(invalid)?;
        let (seq, slug) = rest.split_once('-').ok_or_else(invalid)?;
        let digits = |t: &str| t.chars().all(|c| c.is_ascii_digit());
        let valid = date.len() == 8
            && digits(date)
            && seq.len() >= 3
            && digits(seq)
            && seq.parse::<u32>().is_ok()
            && !slug.is_empty()
            && slug
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if valid {
            Ok(Self(s.to_string()))
        } else {
            Err(invalid())
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `YYYYMMDD`.
    pub fn date_str(&self) -> &str {
        &self.0[..8]
    }

    pub fn seq(&self) -> u32 {
        self.0[9..]
            .split('-')
            .next()
            .and_then(|s| s.parse().ok())
            .expect("DecisionId is validated")
    }

    /// `30-decision-<id>.md`.
    pub fn file_name(&self) -> String {
        format!("{DECISION_PREFIX}{}.md", self.0)
    }

    pub fn from_file_name(name: &str) -> Option<Self> {
        let id = name.strip_prefix(DECISION_PREFIX)?.strip_suffix(".md")?;
        Self::parse(id).ok()
    }
}

impl fmt::Display for DecisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lowercase ASCII letters and digits; other runs become one `-`.
/// At most 40 characters; `decision` when nothing is left.
pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.truncate(40);
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "decision".to_string()
    } else {
        slug.to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub id: DecisionId,
    pub title: String,
    pub status: DecisionStatus,
    pub date: NaiveDate,
    pub author: Option<String>,
    pub supersedes: Vec<DecisionId>,
    pub context: String,
    pub decision: String,
    pub reason: String,
    pub consequences: String,
    /// Unknown sections, kept when the file is written back.
    pub extra_sections: Vec<MdSection>,
}

impl Decision {
    pub fn file_name(&self) -> String {
        self.id.file_name()
    }

    /// The ID comes from the file name; the `ID:` field is informational.
    pub fn parse(file_name: &str, content: &str) -> Result<Self> {
        let id = DecisionId::from_file_name(file_name)
            .ok_or_else(|| Error::General(format!("invalid decision file name: {file_name}")))?;
        let doc = MdDoc::parse(content)?;
        let title = doc
            .title
            .strip_prefix("Decision:")
            .unwrap_or(&doc.title)
            .trim()
            .to_string();
        let status = doc
            .field("Status")
            .ok_or_else(|| Error::General("missing Status".into()))?
            .parse()?;
        let date = match doc.field("Date").map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|e| Error::General(format!("invalid Date {d:?}: {e}")))?,
            None => NaiveDate::parse_from_str(id.date_str(), "%Y%m%d")
                .map_err(|e| Error::General(format!("invalid date in id {id}: {e}")))?,
        };
        let author = doc
            .field("Author")
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(str::to_string);
        let supersedes = doc
            .field("Supersedes")
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(DecisionId::parse)
            .collect::<Result<Vec<_>>>()?;

        let mut decision = Decision {
            id,
            title,
            status,
            date,
            author,
            supersedes,
            context: String::new(),
            decision: String::new(),
            reason: String::new(),
            consequences: String::new(),
            extra_sections: Vec::new(),
        };
        for section in doc.sections {
            match section.heading.as_str() {
                CONTEXT => decision.context = section.body,
                DECISION => decision.decision = section.body,
                REASON => decision.reason = section.body,
                CONSEQUENCES => decision.consequences = section.body,
                _ => decision.extra_sections.push(section),
            }
        }
        Ok(decision)
    }

    pub fn render(&self) -> String {
        let mut fields = vec![
            ("ID".to_string(), self.id.to_string()),
            ("Status".to_string(), self.status.to_string()),
            ("Date".to_string(), self.date.format("%Y-%m-%d").to_string()),
        ];
        if let Some(author) = &self.author {
            fields.push(("Author".into(), author.clone()));
        }
        if !self.supersedes.is_empty() {
            let ids: Vec<&str> = self.supersedes.iter().map(DecisionId::as_str).collect();
            fields.push(("Supersedes".into(), ids.join(", ")));
        }
        let section = |heading: &str, body: &str| MdSection {
            heading: heading.to_string(),
            body: body.to_string(),
        };
        let mut sections = vec![
            section(CONTEXT, &self.context),
            section(DECISION, &self.decision),
            section(REASON, &self.reason),
            section(CONSEQUENCES, &self.consequences),
        ];
        sections.extend(self.extra_sections.iter().cloned());
        MdDoc {
            title: format!("Decision: {}", self.title),
            fields,
            preamble: String::new(),
            sections,
        }
        .render()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn sample() -> Decision {
        Decision {
            id: DecisionId::new(date(2026, 9, 23), 1, "gist-storage"),
            title: "Use GitHub Gist".into(),
            status: DecisionStatus::Accepted,
            date: date(2026, 9, 23),
            author: Some("a1b2c3d4".into()),
            supersedes: vec![
                DecisionId::parse("20260901-002-local-files").unwrap(),
                DecisionId::parse("20260902-001-s3").unwrap(),
            ],
            context: "複数 sandbox から context を共有する必要がある。".into(),
            decision: "GitHub Gist を Git repository として利用する。".into(),
            reason: "- revision history\n- diff".into(),
            consequences: "Gist 固有の制約を受ける。".into(),
            extra_sections: vec![],
        }
    }

    #[test]
    fn renders_the_documented_format() {
        let text = sample().render();
        assert!(text.starts_with(
            "# Decision: Use GitHub Gist\n\nID: 20260923-001-gist-storage\nStatus: accepted\n\
             Date: 2026-09-23\nAuthor: a1b2c3d4\n\
             Supersedes: 20260901-002-local-files, 20260902-001-s3\n\n## Context\n\n"
        ));
        assert!(text.contains("## Consequences\n\nGist 固有の制約を受ける。\n"));
    }

    #[test]
    fn round_trips() {
        let decision = sample();
        let text = decision.render();
        let parsed = Decision::parse(&decision.file_name(), &text).unwrap();
        assert_eq!(parsed, decision);
        assert_eq!(parsed.render(), text);
    }

    #[test]
    fn round_trips_without_optional_fields_and_bodies() {
        let decision = Decision {
            author: None,
            supersedes: vec![],
            context: String::new(),
            reason: String::new(),
            ..sample()
        };
        let text = decision.render();
        assert!(!text.contains("Author:"));
        assert!(!text.contains("Supersedes:"));
        assert_eq!(
            Decision::parse(&decision.file_name(), &text).unwrap(),
            decision
        );
    }

    #[test]
    fn keeps_unknown_sections() {
        let mut decision = sample();
        decision.extra_sections.push(MdSection {
            heading: "Notes".into(),
            body: "extra".into(),
        });
        let parsed = Decision::parse(&decision.file_name(), &decision.render()).unwrap();
        assert_eq!(parsed.extra_sections, decision.extra_sections);
    }

    #[test]
    fn parses_the_documented_example() {
        let text = "# Decision: Use GitHub Gist\n\nStatus: accepted\nDate: 2026-09-23\n\n\
                    ## Context\n\n共有が必要。\n\n## Decision\n\nGist を使う。\n\n\
                    ## Reason\n\n- diff\n\n## Consequences\n\n制約を受ける。\n";
        let decision = Decision::parse("30-decision-20260923-001-gist-storage.md", text).unwrap();
        assert_eq!(decision.title, "Use GitHub Gist");
        assert_eq!(decision.status, DecisionStatus::Accepted);
        assert_eq!(decision.decision, "Gist を使う。");
        assert!(decision.author.is_none());
    }

    #[test]
    fn date_defaults_to_the_id_date() {
        let text = "# Decision: X\n\nStatus: proposed\n";
        let decision = Decision::parse("30-decision-20260923-001-x.md", text).unwrap();
        assert_eq!(decision.date, date(2026, 9, 23));
    }

    #[test]
    fn missing_or_invalid_status_is_an_error() {
        assert!(Decision::parse("30-decision-20260923-001-x.md", "# Decision: X\n").is_err());
        assert!(
            Decision::parse(
                "30-decision-20260923-001-x.md",
                "# Decision: X\n\nStatus: maybe\n"
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_file_name_or_supersedes_is_an_error() {
        let text = "# Decision: X\n\nStatus: accepted\n";
        assert!(Decision::parse("40-worker-x.md", text).is_err());
        let bad = "# Decision: X\n\nStatus: accepted\nSupersedes: nope\n";
        assert!(Decision::parse("30-decision-20260923-001-x.md", bad).is_err());
    }

    #[test]
    fn slugify_examples() {
        assert_eq!(slugify("Use GitHub Gist"), "use-github-gist");
        assert_eq!(slugify("Git  transport!!"), "git-transport");
        assert_eq!(slugify("日本語"), "decision");
        assert_eq!(slugify("--x--"), "x");
        assert_eq!(slugify(&"a".repeat(60)).len(), 40);
        assert_eq!(slugify(&format!("{} b", "a".repeat(39))), "a".repeat(39));
    }

    #[test]
    fn decision_id_parts() {
        let id = DecisionId::new(date(2026, 9, 23), 1, "a");
        assert_eq!(id.as_str(), "20260923-001-a");
        assert_eq!(id.date_str(), "20260923");
        assert_eq!(id.seq(), 1);
        assert_eq!(id.file_name(), "30-decision-20260923-001-a.md");
        assert_eq!(DecisionId::new(date(2026, 9, 23), 1234, "a").seq(), 1234);
        assert_eq!(DecisionId::parse("20260923-012-a-b-c").unwrap().seq(), 12);
    }

    #[test]
    fn decision_id_validation() {
        for bad in [
            "",
            "2026092-001-a",
            "20260923-01-a",
            "20260923-001-",
            "20260923-001-A",
            "x",
        ] {
            assert!(DecisionId::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(
            DecisionId::from_file_name("30-decision-20260923-001-a.md"),
            Some(DecisionId::parse("20260923-001-a").unwrap())
        );
        assert!(DecisionId::from_file_name("40-worker-x.md").is_none());
        assert!(DecisionId::from_file_name("30-decision-20260923-001-a.txt").is_none());
    }

    #[test]
    fn status_parsing_is_case_insensitive() {
        assert_eq!(
            "Accepted".parse::<DecisionStatus>().unwrap(),
            DecisionStatus::Accepted
        );
        assert_eq!(
            "REJECTED".parse::<DecisionStatus>().unwrap(),
            DecisionStatus::Rejected
        );
        assert_eq!(DecisionStatus::Superseded.to_string(), "superseded");
    }
}
