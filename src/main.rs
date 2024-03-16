use chrono::prelude::{DateTime, Utc};
use chrono_english::{parse_date_string, DateError, Dialect};

const DIALECT: Dialect = Dialect::Us;

fn to_iso(date: DateTime<Utc>) -> String {
    date.to_rfc3339().replace("+00:00", "Z")
}

/// Parse date arguments and convert to UTC timestamp
fn parse_date_args(
    args: &[String],
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, DateError> {
    // Remove "in" or "at" from the beginning
    let args_combined =
        match args.iter().map(String::as_ref).collect::<Vec<&str>>()[..] {
            ["in", "a", ..] => format!("1 {}", args[2..].join(" ")),
            ["in", ..] => args[1..].join(" "),
            ["at", ..] => args[1..].join(" "),
            _ => args.join(" "),
        };

    parse_date_string(&args_combined, now, DIALECT)
}

fn parse_print(now: DateTime<Utc>, s: &str) -> String {
    to_iso(parse_date_string(s, now, DIALECT).unwrap())
}

/// Parse arguments and convert to UTC timestamp
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let now = Utc::now();

    if args.len() < 2 {
        let s = "  ";
        let cmd = &args[0];
        let today = parse_print(now, "today");
        let tomorrow = parse_print(now, "tomorrow");
        let day2 = parse_print(now, "2 days");
        let week9 = parse_print(now, "9 weeks");
        let month1 = parse_print(now, "1 month");

        eprintln!(
            "Usage: {cmd} <natural time/duration> \n\
            \n\
            Examples:\n\
            {s}{cmd} today      -> {today}\n\
            {s}{cmd} tomorrow   -> {tomorrow}\n\
            {s}{cmd} 2 day      -> {day2}\n\
            {s}{cmd} 9 week     -> {week9}\n\
            {s}{cmd} 1 month    -> {month1}\n\
            "
        );
        std::process::exit(1);
    }

    let date = parse_date_args(&args[1..], now);
    match date {
        Ok(date) => print!("{}", to_iso(date)),
        Err(e) => eprintln!("ERROR:\n{}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{
        prelude::{NaiveDate, NaiveDateTime},
        TimeZone,
    };

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
}
