use chrono::prelude::Utc;
use tu::{parse_date_args, parse_print, to_iso};

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
            \n\
            {s}{cmd} Wed, 14 Feb 2024 23:16:09 GMT -> 2024-02-14T23:16:09Z\n\
            {s}{cmd} 2024-04-10T13:31:46+04:00     -> 2024-04-10T09:31:46Z\n\
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
