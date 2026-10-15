use chrono::prelude::{DateTime, Utc};

mod chrono_english {
  pub mod errors;
  pub mod lib;
  pub mod parser;
  pub mod types;
}

use chrono_english::lib::{date_error, parse_date_string, DateError, Dialect};

const DIALECT: Dialect = Dialect::Us;

// TODO: Remove after https://github.com/chronotope/chrono/issues/1228
fn append_min_if_only_hour(input: &str) -> String {
  if input.len() >= 3 {
    let last_three = &input[input.len() - 3..];

    // Check if first char is '+' and then two digits
    if last_three.starts_with('+')
      && last_three.chars().nth(1).map(|ch| ch.is_ascii_digit()) == Some(true)
      && last_three.chars().nth(2).map(|ch| ch.is_ascii_digit()) == Some(true)
    {
      return format!("{input}:00");
    }
  }

  if input.len() >= 2 {
    let last_two_char = &input[input.len() - 2..];

    if last_two_char.starts_with('+') {
      let last_char_opt = last_two_char.chars().nth(1);
      if let Some(last_char) = last_char_opt {
        if last_char.is_ascii_digit() {
          let without_last_two = &input[..input.len() - 2];
          return format!("{without_last_two}+0{last_char}:00");
        }
      }
    }
  }

  input.to_string()
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
  s.get(..prefix.len())
    .filter(|start| start.eq_ignore_ascii_case(prefix))
    .map(|_| &s[prefix.len()..])
}

/// Parse date arguments and convert to UTC timestamp
pub fn parse_date_args(
  args: &[String],
  now: DateTime<Utc>,
) -> Result<DateTime<Utc>, DateError> {
  let combined = args.join(" ");
  let combined = combined.trim();

  if let Some(rest) = strip_prefix_ignore_case(combined, "in ") {
    if rest.to_lowercase().ends_with(" ago") {
      return Err(date_error("'in' and 'ago' cannot be combined"));
    }
    if rest.trim_start().starts_with(['+', '-']) {
      return Err(date_error("'in' cannot be combined with a sign"));
    }
  }

  // Remove "in" or "at" from the beginning
  let args_combined = append_min_if_only_hour(
    if let Some(rest) = strip_prefix_ignore_case(combined, "in a ") {
      format!("1 {rest}")
    }
    else if let Some(rest) = strip_prefix_ignore_case(combined, "in an ") {
      format!("1 {rest}")
    }
    else if let Some(rest) = strip_prefix_ignore_case(combined, "in ") {
      rest.to_string()
    }
    else if let Some(rest) = strip_prefix_ignore_case(combined, "at ") {
      let stripped = rest.trim();
      // "at 5" means 5 o'clock and not a Unix timestamp
      if !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit()) {
        match stripped.len() {
          1 | 2 => format!("{stripped}:00"),
          4 => format!("{}:{}", &stripped[..2], &stripped[2..]),
          _ => return Err(date_error(format!("invalid time '{stripped}'"))),
        }
      }
      else {
        stripped.to_string()
      }
    }
    else {
      combined.to_string()
    }
    .trim(),
  );

  // Check if it's a Unix timestamp (all digits)
  if args_combined.chars().all(|c| c.is_ascii_digit()) {
    if let Ok(timestamp) = args_combined.parse::<i64>() {
      // Try as millisecond timestamp first (if it's a reasonable size)
      // Millisecond timestamps are typically 13 digits long
      // We use a heuristic: if the timestamp has exactly 13 digits and dividing by 1000
      // gives a reasonable Unix timestamp (after 2001), treat it as milliseconds
      if args_combined.len() == 13 && timestamp / 1000 >= 1_000_000_000 {
        let seconds = timestamp / 1000;
        let nanoseconds = (timestamp % 1000) * 1_000_000;
        if let Some(datetime) =
          DateTime::from_timestamp(seconds, nanoseconds as u32)
        {
          return Ok(datetime);
        }
      }

      // Fall back to regular second-based timestamp
      if let Some(datetime) = DateTime::from_timestamp(timestamp, 0) {
        return Ok(datetime);
      }
    }
  }

  DateTime::parse_from_rfc2822(&args_combined)
    .or_else(|_| DateTime::parse_from_rfc3339(&args_combined))
    .map(|datetime| datetime.with_timezone(&Utc))
    .or_else(|_| parse_date_string(&args_combined, now, DIALECT))
}

pub fn to_iso(date: DateTime<Utc>) -> String {
  date.to_rfc3339().replace("+00:00", "Z")
}

pub fn parse_print(now: DateTime<Utc>, s: &str) -> String {
  to_iso(parse_date_string(s, now, DIALECT).unwrap())
}
