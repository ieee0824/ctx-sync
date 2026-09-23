//! Minimal interactive prompts. Questions go to stderr so that stdout stays
//! machine-readable. Every command also works without prompts.

use std::io::{self, BufRead, IsTerminal, Write};

/// Whether both stdin and stderr are terminals.
pub fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

/// Asks a question; an empty answer (or end of input) gives `default`.
pub fn ask(question: &str, default: Option<&str>) -> io::Result<String> {
    ask_with(
        &mut io::stdin().lock(),
        &mut io::stderr(),
        question,
        default,
    )
}

/// `[Y/n]` / `[y/N]` question.
pub fn confirm(question: &str, default: bool) -> io::Result<bool> {
    confirm_with(
        &mut io::stdin().lock(),
        &mut io::stderr(),
        question,
        default,
    )
}

/// Numbered choice; returns the index of the chosen option.
pub fn choose(question: &str, options: &[&str], default: usize) -> io::Result<usize> {
    choose_with(
        &mut io::stdin().lock(),
        &mut io::stderr(),
        question,
        options,
        default,
    )
}

/// Reads one trimmed line; `None` at the end of input.
fn read_answer(input: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_string()))
}

fn ask_with(
    input: &mut impl BufRead,
    output: &mut impl Write,
    question: &str,
    default: Option<&str>,
) -> io::Result<String> {
    match default {
        Some(d) => write!(output, "{question} [{d}]: ")?,
        None => write!(output, "{question}: ")?,
    }
    output.flush()?;
    let answer = read_answer(input)?.unwrap_or_default();
    Ok(if answer.is_empty() {
        default.unwrap_or_default().to_string()
    } else {
        answer
    })
}

fn confirm_with(
    input: &mut impl BufRead,
    output: &mut impl Write,
    question: &str,
    default: bool,
) -> io::Result<bool> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    loop {
        write!(output, "{question} {hint} ")?;
        output.flush()?;
        let Some(answer) = read_answer(input)? else {
            return Ok(default);
        };
        match answer.to_ascii_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => writeln!(output, "Please answer y or n.")?,
        }
    }
}

fn choose_with(
    input: &mut impl BufRead,
    output: &mut impl Write,
    question: &str,
    options: &[&str],
    default: usize,
) -> io::Result<usize> {
    for (i, option) in options.iter().enumerate() {
        writeln!(output, "  {}) {option}", i + 1)?;
    }
    loop {
        write!(output, "{question} [{}]: ", options[default])?;
        output.flush()?;
        let Some(answer) = read_answer(input)? else {
            return Ok(default);
        };
        if answer.is_empty() {
            return Ok(default);
        }
        if let Ok(n) = answer.parse::<usize>()
            && (1..=options.len()).contains(&n)
        {
            return Ok(n - 1);
        }
        if let Some(i) = options.iter().position(|o| o.eq_ignore_ascii_case(&answer)) {
            return Ok(i);
        }
        writeln!(output, "Please choose 1-{}.", options.len())?;
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn run<T>(input: &str, f: impl FnOnce(&mut Cursor<&[u8]>, &mut Vec<u8>) -> T) -> (T, String) {
        let mut cursor = Cursor::new(input.as_bytes());
        let mut out = Vec::new();
        let result = f(&mut cursor, &mut out);
        (result, String::from_utf8(out).unwrap())
    }

    #[test]
    fn ask_returns_the_answer_or_the_default() {
        let (answer, out) = run("parser\n", |i, o| {
            ask_with(i, o, "Name", Some("x")).unwrap()
        });
        assert_eq!(answer, "parser");
        assert_eq!(out, "Name [x]: ");
        let (answer, _) = run("\n", |i, o| ask_with(i, o, "Name", Some("x")).unwrap());
        assert_eq!(answer, "x");
        let (answer, _) = run("", |i, o| ask_with(i, o, "Name", None).unwrap());
        assert_eq!(answer, "");
    }

    #[test]
    fn confirm_accepts_yes_no_and_default() {
        let (yes, _) = run("Y\n", |i, o| confirm_with(i, o, "Create?", false).unwrap());
        assert!(yes);
        let (no, _) = run("no\n", |i, o| confirm_with(i, o, "Create?", true).unwrap());
        assert!(!no);
        let (default, out) = run("\n", |i, o| confirm_with(i, o, "Create?", true).unwrap());
        assert!(default);
        assert_eq!(out, "Create? [Y/n] ");
        let (retried, out) = run("maybe\nn\n", |i, o| {
            confirm_with(i, o, "Create?", true).unwrap()
        });
        assert!(!retried);
        assert!(out.contains("Please answer y or n."));
        let (eof, _) = run("", |i, o| confirm_with(i, o, "Create?", false).unwrap());
        assert!(!eof);
    }

    #[test]
    fn choose_accepts_numbers_names_and_default() {
        let options = ["secret", "public"];
        let (i, out) = run("2\n", |i, o| {
            choose_with(i, o, "Visibility", &options, 0).unwrap()
        });
        assert_eq!(i, 1);
        assert!(out.starts_with("  1) secret\n  2) public\nVisibility [secret]: "));
        let (i, _) = run("PUBLIC\n", |i, o| {
            choose_with(i, o, "Visibility", &options, 0).unwrap()
        });
        assert_eq!(i, 1);
        let (i, _) = run("\n", |i, o| {
            choose_with(i, o, "Visibility", &options, 0).unwrap()
        });
        assert_eq!(i, 0);
        let (i, out) = run("9\n1\n", |i, o| {
            choose_with(i, o, "Visibility", &options, 1).unwrap()
        });
        assert_eq!(i, 0);
        assert!(out.contains("Please choose 1-2."));
    }
}
