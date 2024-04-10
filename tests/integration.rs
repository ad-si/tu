use chrono::{
  prelude::{NaiveDate, NaiveDateTime},
  DateTime, TimeZone, Utc,
};
use tu::*;

fn tup_to_naive_date(t: (i32, u32, u32, u32, u32, u32)) -> NaiveDateTime {
  NaiveDate::from_ymd_opt(t.0, t.1, t.2)
    .unwrap()
    .and_hms_opt(t.3, t.4, t.5)
    .unwrap()
}

type DateTimeTup = (i32, u32, u32, u32, u32, u32);

// Macro to create DateTimeTup with only 3 values
macro_rules! dt {
  ($y:expr, $m:expr, $d:expr) => {
    ($y, $m, $d, 0, 0, 0)
  };
}

fn execute_test(
  max_test_len: usize,
  input: &str,
  now: DateTimeTup,
  expected: DateTimeTup,
) {
  let date_args = input //
    .split_whitespace()
    .map(String::from)
    .collect::<Vec<String>>();
  let now_utc = Utc.from_utc_datetime(&tup_to_naive_date(now));
  let date = parse_date_args(&date_args, now_utc) //
    .unwrap_or_else(|e| {
      panic!(
        "\nERROR:\nFailed for input: \"{}\"\n\n\
                        MESSAGE:\n{}\n\n",
        input, e
      )
    });
  let expected_date = DateTime::<Utc>::from_naive_utc_and_offset(
    tup_to_naive_date(expected),
    Utc,
  );
  assert_eq!(date, expected_date, "Failed for input: {}", input);
  let padded_input = format!("{:<width$}", input, width = max_test_len);

  println!("{}  ->  {}", padded_input, to_iso(date));
}

#[test]
fn test_parse_date_string() {
  let tests = [
    ("today", dt!(2024, 1, 1), dt!(2024, 1, 1)),
    ("yesterday", dt!(2024, 1, 2), dt!(2024, 1, 1)),
    ("tomorrow", dt!(2024, 1, 1), dt!(2024, 1, 2)),
    ("tomorrow", (2024, 1, 1, 9, 34, 52), (2024, 1, 2, 9, 34, 52)),
    ("2 days", dt!(2024, 1, 1), dt!(2024, 1, 3)),
    ("in 2 days", dt!(2024, 1, 1), dt!(2024, 1, 3)),
    ("1 week", dt!(2024, 1, 1), dt!(2024, 1, 8)),
    ("in a week", dt!(2024, 1, 1), dt!(2024, 1, 8)),
    ("2 weeks", dt!(2024, 1, 1), dt!(2024, 1, 15)),
    ("in 2 weeks", dt!(2024, 1, 1), dt!(2024, 1, 15)),
    ("1 month", dt!(2024, 1, 1), dt!(2024, 2, 1)),
    ("1 month", dt!(2024, 4, 30), dt!(2024, 5, 30)), // Not 31 !
    ("1 month", dt!(2024, 1, 31), dt!(2024, 2, 29)),
    (
      "in an hour",
      (2024, 1, 1, 8, 34, 52),
      (2024, 1, 1, 9, 34, 52),
    ),
    // RFC 2822
    (
      "Wed, 14 Feb 2024 23:16:09 GMT",
      dt!(0, 1, 1),
      (2024, 2, 14, 23, 16, 9),
    ),
    // RFC 3339
    (
      "2024-04-10T13:31:46+04:00",
      dt!(0, 1, 1),
      (2024, 4, 10, 9, 31, 46),
    ),
    (
      "2024-04-10T13:31:46+04",
      dt!(0, 1, 1),
      (2024, 4, 10, 9, 31, 46),
    ),
    (
      "2024-04-10T13:31:46+4",
      dt!(0, 1, 1),
      (2024, 4, 10, 9, 31, 46),
    ),
  ];
  let max_test_len = tests
    .clone()
    .iter()
    .map(|(s, _, _)| s.len())
    .max()
    .unwrap_or(0);

  for (input, now, expected) in tests.iter() {
    execute_test(max_test_len, input, *now, *expected);
  }
}
