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
  let mut dp = parser::DateParser::new(s);
  if let Dialect::Us = dialect {
    dp = dp.american_date();
  }
  let d = dp.parse()?;

  // we may have explicit hour:minute:sec
  let tspec = match d.time {
    Some(tspec) => tspec,
    None => TimeSpec::new_empty(),
  };
  if tspec.offset.is_some() {
    //   return DateTime::fix()::parse_from_rfc3339(s);
  }
  let date_time = if let Some(dspec) = d.date {
    dspec
      .to_date_time(now, tspec, dp.american)
      .or_err("bad date")?
  }
  else {
    // no date, time set for today's date
    tspec.to_date_time(now.date()).or_err("bad time")?
  };
  Ok(date_time)
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
