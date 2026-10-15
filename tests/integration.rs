use chrono::{
  prelude::{NaiveDate, NaiveDateTime},
  DateTime, Datelike, TimeZone, Utc,
};
use tu::*;

fn tup_to_naive_date(t: (i32, u32, u32, u32, u32, u32)) -> NaiveDateTime {
  NaiveDate::from_ymd_opt(t.0, t.1, t.2)
    .unwrap()
    .and_hms_opt(t.3, t.4, t.5)
    .unwrap()
}

fn tup_to_naive_date_with_ms(
  t: (i32, u32, u32, u32, u32, u32, u32),
) -> NaiveDateTime {
  NaiveDate::from_ymd_opt(t.0, t.1, t.2)
    .unwrap()
    .and_hms_milli_opt(t.3, t.4, t.5, t.6)
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
        "\nERROR:\nFailed for input: \"{input}\"\n\n\
                        MESSAGE:\n{e}\n\n"
      )
    });
  let expected_date = DateTime::<Utc>::from_naive_utc_and_offset(
    tup_to_naive_date(expected),
    Utc,
  );
  assert_eq!(date, expected_date, "Failed for input: {input}");
  let padded_input = format!("{input:<max_test_len$}");

  println!("{}  ->  {}", padded_input, to_iso(date));
}

#[test]
fn test_parse_date_string() {
  let tests = [
    ("today", dt!(2024, 1, 1), dt!(2024, 1, 1)),
    ("tdy", dt!(2024, 1, 1), dt!(2024, 1, 1)),
    ("yesterday", dt!(2024, 1, 2), dt!(2024, 1, 1)),
    ("ytd", dt!(2024, 1, 2), dt!(2024, 1, 1)),
    ("yday", dt!(2024, 1, 2), dt!(2024, 1, 1)),
    ("tomorrow", dt!(2024, 1, 1), dt!(2024, 1, 2)),
    ("tomorrow", (2024, 1, 1, 9, 34, 52), (2024, 1, 2, 9, 34, 52)),
    ("tmr", (2024, 1, 1, 9, 34, 52), (2024, 1, 2, 9, 34, 52)),
    ("tmrw", (2024, 1, 1, 9, 34, 52), (2024, 1, 2, 9, 34, 52)),
    (
      "tomorrow 4pm",
      (2024, 1, 1, 9, 34, 52),
      (2024, 1, 2, 16, 00, 00),
    ),
    (
      "4pm tomorrow",
      (2024, 1, 1, 9, 34, 52),
      (2024, 1, 2, 16, 00, 00),
    ),
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
      // Decimal hours
      "in 1.5 hours",
      (2024, 1, 1, 10, 0, 0),
      (2024, 1, 1, 11, 30, 0),
    ),
    (
      // Decimal days
      "2.5 days",
      dt!(2024, 1, 1),
      (2024, 1, 3, 12, 0, 0),
    ),
    (
      // Decimal weeks
      "0.5 weeks",
      dt!(2024, 1, 1),
      (2024, 1, 4, 12, 0, 0),
    ),
    (
      "in an hour",
      (2024, 1, 1, 8, 34, 52),
      (2024, 1, 1, 9, 34, 52),
    ),
    (
      // RFC 2822
      "Wed, 14 Feb 2024 23:16:09 GMT",
      dt!(0, 1, 1),
      (2024, 2, 14, 23, 16, 9),
    ),
    (
      // RFC 3339
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
    (
      "2024-04-10 7:12 PM UTC",
      dt!(0, 1, 1),
      (2024, 4, 10, 19, 12, 0),
    ),
    (
      "14 december 11:20",
      dt!(2025, 1, 1),
      (2025, 12, 14, 11, 20, 0),
    ),
    (
      // Ignore irrelevant trailing text
      "14 december 11:20 at home",
      dt!(2025, 1, 1),
      (2025, 12, 14, 11, 20, 0),
    ),
    (
      // Unix timestamp - epoch
      "0",
      dt!(2024, 1, 1),
      (1970, 1, 1, 0, 0, 0),
    ),
    (
      // Unix timestamp - Y2K
      "946684800",
      dt!(2024, 1, 1),
      (2000, 1, 1, 0, 0, 0),
    ),
    (
      // Unix timestamp - Common timestamp
      "1000000000",
      dt!(2024, 1, 1),
      (2001, 9, 9, 1, 46, 40),
    ),
    (
      // Unix timestamp - Recent timestamp
      "1740599117",
      dt!(2024, 1, 1),
      (2025, 2, 26, 19, 45, 17),
    ),
    (
      // Date with dot format
      "15. Jun 2025 at 14:19:13",
      dt!(2024, 1, 1),
      (2025, 6, 15, 14, 19, 13),
    ),
    (
      // Date with dot format all lowercase
      "7. dec 1922 at 23:41:55",
      dt!(2024, 1, 1),
      (1922, 12, 7, 23, 41, 55),
    ),
    (
      "Apr 10, 2024, 7:12 PM UTC",
      dt!(2024, 1, 1),
      (2024, 4, 10, 19, 12, 0),
    ),
    (
      // New format: AM variant
      "Dec 25, 2023, 11:30 AM UTC",
      dt!(2024, 1, 1),
      (2023, 12, 25, 11, 30, 0),
    ),
    (
      // New format: Edge case - 12:00 AM (midnight)
      "Jan 1, 2025, 12:00 AM UTC",
      dt!(2024, 1, 1),
      (2025, 1, 1, 0, 0, 0),
    ),
    (
      // New format: Edge case - 12:00 PM (noon)
      "May 15, 2024, 12:00 PM UTC",
      dt!(2024, 1, 1),
      (2024, 5, 15, 12, 0, 0),
    ),
    (
      "Apr 10 2024, 7:12 PM UTC",
      dt!(2024, 1, 1),
      (2024, 4, 10, 19, 12, 0),
    ),
    (
      "Apr 10 2024 7:12 PM UTC",
      dt!(2024, 1, 1),
      (2024, 4, 10, 19, 12, 0),
    ),
    // Timezone abbreviations
    (
      "2019-01-01 20:24:45 CET",
      dt!(2024, 1, 1),
      (2019, 1, 1, 19, 24, 45), // CET = UTC+1
    ),
    (
      "2024-06-15 12:00:00 CEST",
      dt!(2024, 1, 1),
      (2024, 6, 15, 10, 0, 0), // CEST = UTC+2
    ),
    (
      "2024-01-01 00:00:00 EST",
      dt!(2024, 1, 1),
      (2024, 1, 1, 5, 0, 0), // EST = UTC-5
    ),
    (
      "2024-07-04 12:00:00 PDT",
      dt!(2024, 1, 1),
      (2024, 7, 4, 19, 0, 0), // PDT = UTC-7
    ),
    (
      "2024-03-20 09:00:00 JST",
      dt!(2024, 1, 1),
      (2024, 3, 20, 0, 0, 0), // JST = UTC+9
    ),
    (
      "2024-01-15 18:30:00 GMT",
      dt!(2024, 1, 1),
      (2024, 1, 15, 18, 30, 0), // GMT = UTC+0
    ),
    (
      "2024-05-01 15:00:00 IST",
      dt!(2024, 1, 1),
      (2024, 5, 1, 9, 30, 0), // IST = UTC+5:30 (India)
    ),
    (
      "2024-08-10 08:00:00 AEST",
      dt!(2024, 1, 1),
      (2024, 8, 9, 22, 0, 0), // AEST = UTC+10
    ),
    // "at N" is an hour and not a Unix timestamp
    ("at 12", (2024, 1, 1, 9, 0, 0), (2024, 1, 1, 12, 0, 0)),
    ("at 1230", (2024, 1, 1, 9, 0, 0), (2024, 1, 1, 12, 30, 0)),
    // Same week day as today (2024-01-03 is a Wednesday)
    ("next wed", (2024, 1, 3, 9, 0, 0), dt!(2024, 1, 10)),
    ("last wed", (2024, 1, 3, 9, 0, 0), dt!(2023, 12, 27)),
    ("wed 10:00", (2024, 1, 3, 9, 0, 0), (2024, 1, 3, 10, 0, 0)),
    ("wed 8:00", (2024, 1, 3, 9, 0, 0), (2024, 1, 10, 8, 0, 0)),
    ("wednesday", (2024, 1, 3, 9, 0, 0), dt!(2024, 1, 10)),
    // "month" is not "mon(day)"
    ("next month", (2024, 1, 3, 9, 0, 0), (2024, 2, 3, 9, 0, 0)),
    ("last month", (2024, 1, 3, 9, 0, 0), (2023, 12, 3, 9, 0, 0)),
    // Month offsets keep the time of day like day offsets
    ("1 month", (2024, 1, 3, 9, 30, 0), (2024, 2, 3, 9, 30, 0)),
    ("last 2 weeks", dt!(2024, 1, 15), dt!(2024, 1, 1)),
    ("in 1.05 hours", dt!(2024, 1, 1), (2024, 1, 1, 1, 3, 0)),
    ("12.30pm", dt!(2024, 1, 1), (2024, 1, 1, 12, 30, 0)),
    ("8:02p.m.", dt!(2024, 1, 1), (2024, 1, 1, 20, 2, 0)),
    ("5pm EST", dt!(2024, 1, 1), (2024, 1, 1, 22, 0, 0)),
    ("12:34:56-09", dt!(2024, 1, 1), (2024, 1, 1, 21, 34, 56)),
    ("Aug 9, 96", dt!(2024, 1, 1), dt!(1996, 8, 9)),
    ("22-11-14", dt!(2024, 1, 1), dt!(2022, 11, 14)),
    ("Jan 1 2025, 9:30", dt!(2024, 1, 1), (2025, 1, 1, 9, 30, 0)),
    (
      "Jul 18 06:14:49 2024",
      dt!(2024, 1, 1),
      (2024, 7, 18, 6, 14, 49),
    ),
    (
      "14:15 05/31/2024",
      dt!(2024, 1, 1),
      (2024, 5, 31, 14, 15, 0),
    ),
    ("nov14", dt!(2024, 1, 1), dt!(2024, 11, 14)),
    ("14nov2022", dt!(2024, 1, 1), dt!(2022, 11, 14)),
    ("march third", dt!(2024, 1, 1), dt!(2024, 3, 3)),
    ("oct 22nd 1987", dt!(2024, 1, 1), dt!(1987, 10, 22)),
    ("1997-01-01 00:00 +1 year", dt!(2024, 1, 1), dt!(1998, 1, 1)),
    ("+3h", dt!(2024, 1, 1), (2024, 1, 1, 3, 0, 0)),
    // Generic regional timezones depend on daylight saving time
    (
      "2024-01-04 12:00 ET",
      dt!(2024, 1, 1),
      (2024, 1, 4, 17, 0, 0),
    ),
    (
      "2024-07-04 12:00 ET",
      dt!(2024, 1, 1),
      (2024, 7, 4, 16, 0, 0),
    ),
    (
      "2024-07-04 12:00 PT",
      dt!(2024, 1, 1),
      (2024, 7, 4, 19, 0, 0),
    ),
    ("5pm CT", (2024, 7, 4, 9, 0, 0), (2024, 7, 4, 22, 0, 0)),
    // Skipped hour when DST starts: use the time after the gap
    (
      "2021-03-14 2:30 ET",
      dt!(2024, 1, 1),
      (2021, 3, 14, 7, 30, 0),
    ),
    // Repeated hour when DST ends: use the first one
    (
      "2021-11-07 1:30 ET",
      dt!(2024, 1, 1),
      (2021, 11, 7, 5, 30, 0),
    ),
    // Abbreviations which are not in the IANA timezone database anymore
    (
      "1997-01-19 08:17:48 BRT",
      dt!(2024, 1, 1),
      (1997, 1, 19, 11, 17, 48),
    ),
    (
      "2024-06-15 12:00 GST",
      dt!(2024, 1, 1),
      (2024, 6, 15, 8, 0, 0),
    ),
    (
      "2024-06-15 12:00 NPT",
      dt!(2024, 1, 1),
      (2024, 6, 15, 6, 15, 0),
    ),
    ("10:00 MON", (2024, 1, 3, 9, 0, 0), (2024, 1, 8, 10, 0, 0)),
    // Numbers written as words
    ("a week ago", (2024, 1, 10, 9, 0, 0), (2024, 1, 3, 9, 0, 0)),
    ("in an hour", (2024, 1, 1, 9, 0, 0), (2024, 1, 1, 10, 0, 0)),
    (
      "half an hour ago",
      (2024, 1, 1, 9, 0, 0),
      (2024, 1, 1, 8, 30, 0),
    ),
    ("in a half year", dt!(2024, 1, 1), dt!(2024, 7, 1)),
    ("five days ago", dt!(2024, 1, 10), dt!(2024, 1, 5)),
    (
      "twenty-five minutes ago",
      (2024, 1, 1, 9, 0, 0),
      (2024, 1, 1, 8, 35, 0),
    ),
    ("in twenty one days", dt!(2024, 1, 1), dt!(2024, 1, 22)),
    ("in a couple of days", dt!(2024, 1, 1), dt!(2024, 1, 3)),
    ("a few days ago", dt!(2024, 1, 10), dt!(2024, 1, 7)),
    ("in several hours", dt!(2024, 1, 1), (2024, 1, 1, 7, 0, 0)),
    ("next two weeks", dt!(2024, 1, 1), dt!(2024, 1, 15)),
    ("next quarter", dt!(2024, 1, 1), dt!(2024, 4, 1)),
    ("twenty-fourth of May", dt!(2024, 1, 1), dt!(2024, 5, 24)),
    ("May twenty-fourth", dt!(2024, 1, 1), dt!(2024, 5, 24)),
    ("thirtieth of december", dt!(2024, 1, 1), dt!(2024, 12, 30)),
    ("second", dt!(2024, 1, 1), (2024, 1, 1, 0, 0, 1)),
    // Month offsets crossing a year boundary
    ("one year ago", dt!(2024, 1, 6), dt!(2023, 1, 6)),
    ("12 months ago", dt!(2024, 1, 6), dt!(2023, 1, 6)),
    ("13 months ago", dt!(2024, 1, 6), dt!(2022, 12, 6)),
    ("in 24 months", dt!(2024, 1, 6), dt!(2026, 1, 6)),
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

#[test]
fn test_unix_timestamp_edge_cases() {
  let now = Utc.from_utc_datetime(&tup_to_naive_date(dt!(2024, 1, 1)));

  // Test valid Unix timestamps (seconds)
  let valid_cases = [
    ("0", (1970, 1, 1, 0, 0, 0)),
    ("1", (1970, 1, 1, 0, 0, 1)),
    ("1740599117", (2025, 2, 26, 19, 45, 17)),
    ("946684800", (2000, 1, 1, 0, 0, 0)),
    ("1000000000", (2001, 9, 9, 1, 46, 40)),
  ];

  // Test valid Unix timestamps with milliseconds
  let valid_ms_cases = [
    ("1000000000000", (2001, 9, 9, 1, 46, 40, 0)),
    ("1000000000500", (2001, 9, 9, 1, 46, 40, 500)),
    ("1740599117000", (2025, 2, 26, 19, 45, 17, 0)),
    ("1740599117123", (2025, 2, 26, 19, 45, 17, 123)),
    ("1740599117999", (2025, 2, 26, 19, 45, 17, 999)),
  ];

  for (input, expected) in valid_cases.iter() {
    let date_args = vec![input.to_string()];
    let result = parse_date_args(&date_args, now).unwrap();
    let expected_date = DateTime::<Utc>::from_naive_utc_and_offset(
      tup_to_naive_date(*expected),
      Utc,
    );
    assert_eq!(result, expected_date, "Failed for Unix timestamp: {input}");
  }

  for (input, expected) in valid_ms_cases.iter() {
    let date_args = vec![input.to_string()];
    let result = parse_date_args(&date_args, now).unwrap();
    let expected_date = DateTime::<Utc>::from_naive_utc_and_offset(
      tup_to_naive_date_with_ms(*expected),
      Utc,
    );
    assert_eq!(
      result, expected_date,
      "Failed for Unix timestamp with milliseconds: {input}"
    );
  }

  // Test that mixed alphanumeric strings don't get parsed as Unix timestamps
  let mixed_cases = ["123abc", "abc123", "12.34", "1234-56-78"];

  for input in mixed_cases.iter() {
    let date_args = vec![input.to_string()];
    // These should either parse as natural language or fail, but not as Unix timestamps
    let result = parse_date_args(&date_args, now);
    if let Ok(parsed_date) = result {
      // If it parses successfully, it should not be interpreted as a Unix timestamp
      // (i.e., it shouldn't be from 1970)
      assert_ne!(
        parsed_date.year(),
        1970,
        "Mixed string '{input}' was incorrectly parsed as Unix timestamp"
      );
    }
  }
}

#[test]
fn standalone_midnight_and_noon() {
  let now = Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
  for (input, hour) in [
    ("12am", 0),
    ("12pm", 12),
    ("12:00am", 0),
    ("12:00pm", 12),
    ("12:00 AM", 0),
    ("12:00 PM", 12),
    ("1am", 1),
    ("11pm", 23),
  ] {
    let args = vec![input.to_string()];
    let expected = Utc.with_ymd_and_hms(2026, 4, 10, hour, 0, 0).unwrap();
    assert_eq!(parse_date_args(&args, now).unwrap(), expected, "{input}");
  }
}

#[test]
fn invalid_times_return_error() {
  let now = Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
  for input in [
    "0am",
    "0pm",
    "13pm",
    "13am",
    "5:60pm",
    "25:00",
    "24:00",
    "12:60",
    "12:00:60",
    "friday 25:00",
    "tomorrow 13pm",
    "Jan 1, 2025, 13:00 PM UTC",
    "23:59:59+24:01",
    "-3",
    "- 1997-01-01",
    "1997-01-01 bogus",
    "01:02 1234",
    "06:14:49 06:14:49",
    "in 1 seconds ago",
    "0.5",
    // Unknown timezones must not be ignored
    "12:00 FOO",
    "2024-01-01 12:00 XYZ",
    "5pm ABC",
    "five",
    "a",
    "may five",
    "twenty-fourth",
    // Two-digit numbers after a month name are days
    "August 32",
    "the day",
    "for the year",
    "-1 seconds ago",
    "midnight 5pm",
    "noon 12:30",
    "13 in the morning",
    "weekday",
    "end of foo",
    "@abc",
    "TZ=\"Foo/Bar\" 2024-01-01",
  ] {
    let args: Vec<String> =
      input.split_whitespace().map(String::from).collect();
    let result = std::panic::catch_unwind(|| parse_date_args(&args, now));
    match result {
      Ok(Err(_)) => {}
      Ok(Ok(date)) => panic!("Expected error for \"{input}\", got {date}"),
      Err(_) => panic!("Panicked for \"{input}\""),
    }
  }
}

/// Reference time for the feature tests: Thursday, 2026-10-15 09:30:00
const THU: DateTimeTup = (2026, 10, 15, 9, 30, 0);

fn execute_tests(tests: &[(&str, DateTimeTup)]) {
  let max_test_len = tests.iter().map(|(input, _)| input.len()).max().unwrap();
  for (input, expected) in tests {
    execute_test(max_test_len, input, THU, *expected);
  }
}

#[test]
fn parts_of_the_day() {
  execute_tests(&[
    ("noon", (2026, 10, 15, 12, 0, 0)),
    ("midday", (2026, 10, 15, 12, 0, 0)),
    // A plain "midnight" is the end of today
    ("midnight", dt!(2026, 10, 16)),
    ("this morning", (2026, 10, 15, 6, 0, 0)),
    ("this afternoon", (2026, 10, 15, 15, 0, 0)),
    ("evening", (2026, 10, 15, 18, 0, 0)),
    ("tonight", (2026, 10, 15, 23, 0, 0)),
    ("last night", (2026, 10, 14, 23, 0, 0)),
    ("next evening", (2026, 10, 16, 18, 0, 0)),
    ("tomorrow morning", (2026, 10, 16, 6, 0, 0)),
    ("tomorrow this afternoon", (2026, 10, 16, 15, 0, 0)),
    ("Friday afternoon", (2026, 10, 16, 15, 0, 0)),
    ("on Tuesday morning", (2026, 10, 20, 6, 0, 0)),
    ("tomorrow at noon", (2026, 10, 16, 12, 0, 0)),
    ("noon tomorrow", (2026, 10, 16, 12, 0, 0)),
    ("at midnight on 12th August", dt!(2026, 8, 12)),
    ("in 6 days at midnight", dt!(2026, 10, 21)),
    ("12 noon", (2026, 10, 15, 12, 0, 0)),
    ("6 in the morning", (2026, 10, 15, 6, 0, 0)),
    ("6 in the evening", (2026, 10, 15, 18, 0, 0)),
    ("1 at night", (2026, 10, 15, 1, 0, 0)),
    ("11 at night", (2026, 10, 15, 23, 0, 0)),
    ("11 tonight", (2026, 10, 15, 23, 0, 0)),
    ("tonight at 8", (2026, 10, 15, 20, 0, 0)),
    ("tonight 8pm", (2026, 10, 15, 20, 0, 0)),
    ("tonight at 11:10 pm", (2026, 10, 15, 23, 10, 0)),
    ("this afternoon at 3", (2026, 10, 15, 15, 0, 0)),
    ("tomorrow at 7", (2026, 10, 16, 7, 0, 0)),
    ("Jan 1, 2020 Morning UTC", (2020, 1, 1, 6, 0, 0)),
  ]);
}

#[test]
fn months_with_years() {
  execute_tests(&[
    ("September 2012", dt!(2012, 9, 1)),
    ("Sep. 2012", dt!(2012, 9, 1)),
    ("Sep-2012", dt!(2012, 9, 1)),
    ("June of 2022", dt!(2022, 6, 1)),
    ("August, 2012", dt!(2012, 8, 1)),
    ("2024 Aug", dt!(2024, 8, 1)),
    ("2024-Aug", dt!(2024, 8, 1)),
    ("2024/August", dt!(2024, 8, 1)),
    ("9/2012", dt!(2012, 9, 1)),
    ("09/2012", dt!(2012, 9, 1)),
  ]);
}

#[test]
fn date_formats() {
  execute_tests(&[
    ("2015/05/25", dt!(2015, 5, 25)),
    ("2012/8/10", dt!(2012, 8, 10)),
    ("2015.05.25", dt!(2015, 5, 25)),
    ("2014 12 28", dt!(2014, 12, 28)),
    ("19970101 +1 year", dt!(1998, 1, 1)),
    ("2018 March 18", dt!(2018, 3, 18)),
    ("2012/Aug/10", dt!(2012, 8, 10)),
    ("25.05.2015", dt!(2015, 5, 25)),
    // Day and month are swapped if only that is valid
    ("05.25.2015", dt!(2015, 5, 25)),
    ("25/05/2015", dt!(2015, 5, 25)),
    ("05-25-2015", dt!(2015, 5, 25)),
    ("07-27-2022, 02:00 AM", (2022, 7, 27, 2, 0, 0)),
    ("10-August-2012", dt!(2012, 8, 10)),
    ("10/August/2012", dt!(2012, 8, 10)),
    ("10-August 2012", dt!(2012, 8, 10)),
    ("09-JAN-2017", dt!(2017, 1, 9)),
    ("14-nov", dt!(2026, 11, 14)),
    ("August-10-2012", dt!(2012, 8, 10)),
    ("August/10, 2012", dt!(2012, 8, 10)),
    ("nov-14-2022", dt!(2022, 11, 14)),
    ("06/Nov/2023:06:36:02 +0200", (2023, 11, 6, 4, 36, 2)),
    ("05/31/2024.14:15", (2024, 5, 31, 14, 15, 0)),
    ("05/31/2024:14:15", (2024, 5, 31, 14, 15, 0)),
    ("05/31/2024-14:15", (2024, 5, 31, 14, 15, 0)),
    ("Sunday, March 6, 2016", dt!(2016, 3, 6)),
    ("Sun., March 6, 2016", dt!(2016, 3, 6)),
    ("Sunday, March, 6th 2016", dt!(2016, 3, 6)),
    ("Tuesday 11/3/2015", dt!(2015, 11, 3)),
    ("wed., 2024-01-03", dt!(2024, 1, 3)),
    ("1 sept.", dt!(2026, 9, 1)),
    ("sept. 1st", dt!(2026, 9, 1)),
    ("the 22nd of october, 1987 at 7pm", (1987, 10, 22, 19, 0, 0)),
    ("9pm on the 4th of july", (2026, 7, 4, 21, 0, 0)),
    ("the 3rd", dt!(2026, 10, 3)),
    ("thirteen of february", dt!(2026, 2, 13)),
    ("August 10, 8 AD", dt!(8, 8, 10)),
    ("10 August 2555 BE", dt!(2012, 8, 10)),
    ("Thu Oct 26 11:00:09 2023", (2023, 10, 26, 11, 0, 9)),
    ("Wed Jan  1 00:00:00 1997 1 year", dt!(1998, 1, 1)),
  ]);
}

#[test]
fn durations() {
  execute_tests(&[
    ("5 days from now", (2026, 10, 20, 9, 30, 0)),
    ("12 hrs from now", (2026, 10, 15, 21, 30, 0)),
    ("half an hour from now", (2026, 10, 15, 10, 0, 0)),
    ("2 days later", (2026, 10, 17, 9, 30, 0)),
    ("3mo later", (2027, 1, 15, 9, 30, 0)),
    ("15 minute out", (2026, 10, 15, 9, 45, 0)),
    ("2 months hence", (2026, 12, 15, 9, 30, 0)),
    ("next week hence", (2026, 10, 22, 9, 30, 0)),
    ("15 minutes earlier", (2026, 10, 15, 9, 15, 0)),
    ("a min before", (2026, 10, 15, 9, 29, 0)),
    ("the min after", (2026, 10, 15, 9, 31, 0)),
    ("within 3 days", (2026, 10, 18, 9, 30, 0)),
    ("within half an hour", (2026, 10, 15, 10, 0, 0)),
    ("In about ~5 hours", (2026, 10, 15, 14, 30, 0)),
    ("after an hour", (2026, 10, 15, 10, 30, 0)),
    ("for 5 minutes 30 seconds", (2026, 10, 15, 9, 35, 30)),
    (
      "for 1 hour, 5 minutes, and 30 seconds",
      (2026, 10, 15, 10, 35, 30),
    ),
    ("1 day 21 hours ago", (2026, 10, 13, 12, 30, 0)),
    ("3m 49s ago", (2026, 10, 15, 9, 26, 11)),
    ("2 days and 6 hours ago", (2026, 10, 13, 3, 30, 0)),
    ("in 20 hours and 40 minutes", (2026, 10, 16, 6, 10, 0)),
    ("in 1d, 2hr, and 5min", (2026, 10, 16, 11, 35, 0)),
    ("next 2 weeks 3 days", (2026, 11, 1, 9, 30, 0)),
    ("+2 months, 5 days", (2026, 12, 20, 9, 30, 0)),
    ("-2 hours 5 minutes", (2026, 10, 15, 7, 25, 0)),
    ("past 2 days", (2026, 10, 13, 9, 30, 0)),
    ("fortnight", (2026, 10, 29, 9, 30, 0)),
    ("2 fortnights ago", (2026, 9, 17, 9, 30, 0)),
    ("1mth", (2026, 11, 15, 9, 30, 0)),
    ("7 hrs", (2026, 10, 15, 16, 30, 0)),
    ("-2yrs", (2024, 10, 15, 9, 30, 0)),
    ("next qtr", (2027, 1, 15, 9, 30, 0)),
    ("1 year ago 1 month", (2025, 11, 15, 9, 30, 0)),
  ]);
}

#[test]
fn relative_anchors() {
  execute_tests(&[
    ("the day before yesterday", (2026, 10, 13, 9, 30, 0)),
    ("the day after tomorrow", (2026, 10, 17, 9, 30, 0)),
    ("2 day before yesterday", (2026, 10, 12, 9, 30, 0)),
    ("a week after tomorrow", (2026, 10, 23, 9, 30, 0)),
    ("2 days after next Friday", dt!(2026, 10, 18)),
    ("2 months before 02/02", dt!(2025, 12, 2)),
    ("end of July", dt!(2026, 7, 31)),
    ("last day of February 2024", dt!(2024, 2, 29)),
    ("end of October next year", dt!(2027, 10, 31)),
    ("end of the month", dt!(2026, 10, 31)),
    ("end of month", dt!(2026, 10, 31)),
    ("end of next month", dt!(2026, 11, 30)),
    ("last day of last month", dt!(2026, 9, 30)),
    ("end of this week", dt!(2026, 10, 18)),
    ("end of next week", dt!(2026, 10, 25)),
    ("end of this year", dt!(2026, 12, 31)),
    ("end of the past year", dt!(2025, 12, 31)),
    ("this week", (2026, 10, 15, 9, 30, 0)),
    ("this month", (2026, 10, 15, 9, 30, 0)),
    ("lastmonth", (2026, 9, 15, 9, 30, 0)),
    ("past week", (2026, 10, 8, 9, 30, 0)),
    // "this" is the week day of the current week
    ("this tuesday", dt!(2026, 10, 13)),
    ("this friday", dt!(2026, 10, 16)),
    ("past Friday", dt!(2026, 10, 9)),
    ("past thursday", dt!(2026, 10, 8)),
    ("Friday of next week", dt!(2026, 10, 23)),
    ("Tuesday of last week", dt!(2026, 10, 6)),
    ("Wednesday of this week", dt!(2026, 10, 14)),
    ("on Friday next week", dt!(2026, 10, 23)),
    ("on Tuesday, next week", dt!(2026, 10, 20)),
    ("Friday of next week at 9am", (2026, 10, 23, 9, 0, 0)),
    ("Wed 2026-08-17 next week", dt!(2026, 8, 24)),
    ("this weekend", dt!(2026, 10, 17)),
    ("next weekend", dt!(2026, 10, 24)),
    ("last weekend", dt!(2026, 10, 10)),
    ("next weekday", dt!(2026, 10, 16)),
    ("last weekday", dt!(2026, 10, 14)),
  ]);
}

#[test]
fn timezones_and_timestamps() {
  execute_tests(&[
    ("@1613371067", (2021, 2, 15, 6, 37, 47)),
    ("@-1", (1969, 12, 31, 23, 59, 59)),
    ("@ - 1234567890", (1930, 11, 18, 0, 28, 30)),
    // A timezone without a time means midnight in that timezone
    ("UTC", dt!(2026, 10, 15)),
    ("cst", (2026, 10, 15, 6, 0, 0)),
    ("ist", (2026, 10, 14, 18, 30, 0)),
    ("mez", (2026, 10, 14, 23, 0, 0)),
    ("UTC+07:00", (2026, 10, 14, 17, 0, 0)),
    ("Z+0015", (2026, 10, 14, 23, 45, 0)),
    ("utc +5:30", (2026, 10, 14, 18, 30, 0)),
    ("UTC +8 years", (2034, 10, 15, 9, 30, 0)),
    (
      "september 16, 2020 at 11 am utc+02:45",
      (2020, 9, 16, 8, 15, 0),
    ),
    (
      "september 16, 2020 at 11 am GMT -08:45",
      (2020, 9, 16, 19, 45, 0),
    ),
    ("september 16, 2020 at 23.00+14", (2020, 9, 16, 9, 0, 0)),
    (
      "september 16, 2020 at 11 am GMT+0900 (JST)",
      (2020, 9, 16, 2, 0, 0),
    ),
    ("10:30 (gmt-2:30)", (2026, 10, 15, 13, 0, 0)),
    ("Thu Oct 26 11:00:09 EDT 2023", (2023, 10, 26, 15, 0, 9)),
    ("Wednesday, September 16, 2020, EST", (2020, 9, 16, 5, 0, 0)),
    ("in 1 hour GMT", (2026, 10, 15, 10, 30, 0)),
    (
      "TZ=\"Europe/Paris\" 2025-01-02 03:04:05",
      (2025, 1, 2, 2, 4, 5),
    ),
    ("TZ=\"EST5\" 1970-01-01 12:30:45", (1970, 1, 1, 17, 30, 45)),
    ("TZ=\"UTC0\" 1970-01-01 00:00", dt!(1970, 1, 1)),
    (
      "TZ=\"Australia/Perth\" 2016-08-15 07:00",
      (2016, 8, 14, 23, 0, 0),
    ),
  ]);
}

#[test]
fn two_digit_years() {
  execute_tests(&[
    ("Aug 9 96", dt!(1996, 8, 9)),
    ("03 Aug 96", dt!(1996, 8, 3)),
    ("3rd Feb 82", dt!(1982, 2, 3)),
    ("21-APR-16", dt!(2016, 4, 21)),
    ("8/10/12", dt!(2012, 8, 10)),
  ]);
}
