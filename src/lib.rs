use chrono::prelude::{DateTime, Utc};

mod chrono_english {
  pub mod errors;
  pub mod lib;
  pub mod parser;
  pub mod types;
}

use chrono_english::lib::{parse_date_string, DateError, Dialect};

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

/// Parse date arguments and convert to UTC timestamp
pub fn parse_date_args(
  args: &[String],
  now: DateTime<Utc>,
) -> Result<DateTime<Utc>, DateError> {
  // Remove "in" or "at" from the beginning
  let args_combined = append_min_if_only_hour(
    match args.iter().map(String::as_ref).collect::<Vec<&str>>()[..] {
      ["in", "a", ..] => format!("1 {}", args[2..].join(" ")),
      ["in", "an", ..] => format!("1 {}", args[2..].join(" ")),
      ["in", ..] => args[1..].join(" "),
      ["at", ..] => args[1..].join(" "),
      _ => args.join(" "),
    }
    .trim(),
  );

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
