#![allow(deprecated)]
#![allow(dead_code)]

use chrono::prelude::*;

pub use super::errors::*;
use super::parser;
use super::types::*;

// pub use errors::{date_error, date_result};
// pub use errors::{DateError, DateResult};
// pub use types::Interval;

#[derive(Debug, Hash, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]

pub enum Dialect {
  Uk,
  Us,
}

pub fn parse_date_string<Tz: TimeZone>(
  s: &str,
  now: DateTime<Tz>,
  dialect: Dialect,
) -> DateResult<DateTime<Tz>>
where
  Tz::Offset: Copy,
{
  let s = normalize_meridiem(s);
  let mut dp = parser::DateParser::new(&s);
  if let Dialect::Us = dialect {
    dp = dp.american_date();
  }
  let d = dp.parse()?;

  // we may have explicit hour:minute:sec
  let tspec = match d.time {
    Some(tspec) => tspec,
    None => TimeSpec::new_empty(),
  };
  let mut date_time = if let Some(dspec) = d.date {
    dspec
      .to_date_time(now, tspec, dp.american)
      .or_err("bad date")?
  }
  else {
    // no date, time set for today's date
    tspec.to_date_time(now.date()).or_err("bad time")?
  };
  for skip in d.skips {
    date_time = skip
      .to_date_time(date_time, TimeSpec::new_empty())
      .or_err("bad relative offset")?;
  }
  Ok(date_time)
}

/// Replace dotted meridiems with plain ones ("8 p.m." → "8 pm")
fn normalize_meridiem(s: &str) -> String {
  let chars: Vec<char> = s.chars().collect();
  let mut result = String::with_capacity(s.len());
  let mut i = 0;
  while i < chars.len() {
    let ch = chars[i];
    let at_word_start =
      i == 0 || chars[i - 1].is_ascii_digit() || chars[i - 1].is_whitespace();
    let is_dotted = matches!(ch, 'a' | 'A' | 'p' | 'P')
      && chars.get(i + 1) == Some(&'.')
      && matches!(chars.get(i + 2), Some('m' | 'M'));
    if at_word_start && is_dotted {
      let mut end = i + 3;
      if chars.get(end) == Some(&'.') {
        end += 1;
      }
      if !chars.get(end).is_some_and(|c| c.is_alphanumeric()) {
        result.push(ch);
        result.push('m');
        i = end;
        continue;
      }
    }
    result.push(ch);
    i += 1;
  }
  result
}

pub fn parse_duration(s: &str) -> DateResult<Interval> {
  let mut dp = parser::DateParser::new(s);
  let d = dp.parse()?;

  if d.time.is_some() {
    return date_result("unexpected time component");
  }

  // shouldn't happen, but.
  if d.date.is_none() {
    return date_result("could not parse date");
  }

  match d.date.unwrap() {
    DateSpec::Absolute(_) => date_result("unexpected absolute date"),
    DateSpec::FromName(_) => date_result("unexpected date component"),
    DateSpec::Relative(skip) => Ok(skip.to_interval()),
  }
}
