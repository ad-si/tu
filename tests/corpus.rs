//! Test cases collected from the test suites of other date parsing libraries
//! (chrono-node, parse_datetime, fuzzy-time, when, timestring, …).
//!
//! `tests/corpus.tsv` columns:
//! - `source`: Library the case was taken from
//! - `status`: `pass` (supported), `todo` (not yet supported / wrong result),
//!   or `differs` (tu deliberately deviates from the source library)
//! - `input`: Input string (split on whitespace like CLI arguments)
//! - `now`: Reference time (UTC)
//! - `expected`: Expected UTC time or `ERROR` if the input must be rejected

use chrono::{DateTime, NaiveDateTime, Utc};
use std::panic::catch_unwind;
use tu::parse_date_args;

const CORPUS: &str = include_str!("corpus.tsv");

struct Case<'a> {
  source: &'a str,
  status: &'a str,
  input: &'a str,
  now: &'a str,
  expected: &'a str,
}

fn cases() -> impl Iterator<Item = Case<'static>> {
  CORPUS.lines().skip(1).map(|line| {
    let cols: Vec<&str> = line.split('\t').collect();
    assert_eq!(cols.len(), 5, "Malformed corpus line: {line}");
    Case {
      source: cols[0],
      status: cols[1],
      input: cols[2],
      now: cols[3],
      expected: cols[4],
    }
  })
}

fn parse_utc(s: &str) -> DateTime<Utc> {
  // chrono requires a sign for years with more than 4 digits
  let year_len = s.find('-').unwrap_or(0);
  let s = if year_len > 4 {
    format!("+{s}")
  }
  else {
    s.to_string()
  };
  NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f")
    .unwrap_or_else(|e| panic!("Invalid corpus date \"{s}\": {e}"))
    .and_utc()
}

/// Returns a description of the failure or `None` if the case passes
fn check(case: &Case) -> Option<String> {
  let args: Vec<String> =
    case.input.split_whitespace().map(String::from).collect();
  let now = parse_utc(case.now);
  let result = match catch_unwind(|| parse_date_args(&args, now)) {
    Ok(result) => result,
    Err(_) => return Some("panicked".into()),
  };
  match (case.expected, result) {
    ("ERROR", Err(_)) => None,
    ("ERROR", Ok(date)) => Some(format!("expected error, got {date}")),
    (expected, Ok(date)) if date == parse_utc(expected) => None,
    (expected, Ok(date)) => Some(format!("expected {expected}, got {date}")),
    (expected, Err(e)) => Some(format!("expected {expected}, got error: {e}")),
  }
}

fn format_failure(case: &Case, msg: &str) -> String {
  format!(
    "[{}] \"{}\" (now: {}): {msg}",
    case.source, case.input, case.now
  )
}

#[test]
fn corpus_supported_cases() {
  let failures: Vec<String> = cases()
    .filter(|case| case.status == "pass")
    .filter_map(|case| check(&case).map(|msg| format_failure(&case, &msg)))
    .collect();

  assert!(
    failures.is_empty(),
    "{} corpus case(s) failed:\n{}",
    failures.len(),
    failures.join("\n")
  );
}

#[test]
fn corpus_todo_cases_do_not_panic() {
  let panics: Vec<String> = cases()
    .filter(|case| case.status != "pass")
    .filter_map(|case| {
      let args: Vec<String> =
        case.input.split_whitespace().map(String::from).collect();
      let now = parse_utc(case.now);
      catch_unwind(|| parse_date_args(&args, now))
        .is_err()
        .then(|| format_failure(&case, "panicked"))
    })
    .collect();

  assert!(panics.is_empty(), "Panics:\n{}", panics.join("\n"));
}

/// Run with `cargo test -- --ignored` to see which features are still missing.
/// Cases that pass should be marked as `pass` in `corpus.tsv`.
#[test]
#[ignore]
fn corpus_todo_cases() {
  let mut newly_passing = Vec::new();
  let mut failures = Vec::new();

  for case in cases().filter(|case| case.status == "todo") {
    match check(&case) {
      Some(msg) => failures.push(format_failure(&case, &msg)),
      None => newly_passing.push(format_failure(&case, "passes")),
    }
  }

  assert!(
    failures.is_empty() && newly_passing.is_empty(),
    "{} todo case(s) fail:\n{}\n\n\
      {} todo case(s) pass and should be marked as `pass`:\n{}",
    failures.len(),
    failures.join("\n"),
    newly_passing.len(),
    newly_passing.join("\n"),
  );
}
