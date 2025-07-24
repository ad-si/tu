use super::errors::*;
use super::types::*;
use scanlex::{Scanner, Token};

// when we parse dates, there's often a bit of time parsed..
#[derive(Clone, Copy, Debug)]
enum TimeKind {
  Formal,
  Informal,
  AmPm(bool),
  Unknown,
}

pub struct DateParser<'a> {
  scanner: Scanner<'a>,
  direct: Direction,
  maybe_time: Option<(u32, TimeKind)>,
  pub american: bool, // 9/11, not 20/03
}

impl<'a> DateParser<'a> {
  pub fn new(text: &'a str) -> DateParser<'a> {
    DateParser {
      scanner: Scanner::new(text).no_float(),
      direct: Direction::Here,
      maybe_time: None,
      american: false,
    }
  }

  pub fn american_date(mut self) -> DateParser<'a> {
    self.american = true;
    self
  }

  fn iso_date(&mut self, year: u32) -> DateResult<DateSpec> {
    let month = self.scanner.get_int::<u32>()?;
    self.scanner.get_ch_matching(&['-'])?;
    let day = self.scanner.get_int::<u32>()?;
    Ok(DateSpec::absolute(year, month, day))
  }

  fn informal_date(&mut self, day_or_month: u32) -> DateResult<DateSpec> {
    let month_or_day = self.scanner.get_int::<u32>()?;
    let (day, month) = if self.american {
      (month_or_day, day_or_month)
    }
    else {
      (day_or_month, month_or_day)
    };
    Ok(if self.scanner.peek() == '/' {
      self.scanner.get();
      let y = self.scanner.get_int::<u32>()?;
      let y = if y < 100 {
        // pivot (1940, 2040)
        if y > 40 {
          1900 + y
        }
        else {
          2000 + y
        }
      }
      else {
        y
      };
      DateSpec::absolute(y, month, day)
    }
    else {
      DateSpec::FromName(ByName::from_day_month(day, month, self.direct))
    })
  }

  fn parse_date(&mut self) -> DateResult<Option<DateSpec>> {
    let mut t = self.scanner.next().or_err("empty date string")?;

    let sign = t.is_char() && t.as_char().unwrap() == '-';
    if sign {
      t = self.scanner.next().or_err("nothing after '-'")?;
    }
    if let Some(name) = t.as_iden() {
      let shortcut = match name {
        "now" => Some(0),
        "today" => Some(0),
        "yesterday" => Some(-1),
        "tomorrow" => Some(1),
        _ => None,
      };
      if let Some(skip) = shortcut {
        return Ok(Some(DateSpec::skip(time_unit("day").unwrap(), skip)));
      }
      else
      // maybe next or last?
      if let Some(d) = Direction::from_name(name) {
        self.direct = d;
      }
    }
    if self.direct != Direction::Here {
      t = self.scanner.next().or_err("nothing after last/next")?;
    }
    Ok(match t {
      Token::Iden(ref name) => {
        let name = name.to_lowercase();
        // maybe weekday or month name?
        if let Some(by_name) = ByName::from_name(&name, self.direct) {
          // however, MONTH _might_ be followed by DAY, YEAR
          if let Some(month) = by_name.as_month() {
            let t = self.scanner.get();
            if t.is_integer() {
              let day = t.to_int_result::<u32>()?;
              return Ok(Some(if self.scanner.peek() == ',' {
                self.scanner.get_char()?; // eat ','
                let year = self.scanner.get_int::<u32>()?;
                DateSpec::absolute(year, month, day)
              }
              else {
                // MONTH DAY is like DAY MONTH (tho no time!)
                DateSpec::from_day_month(day, month, self.direct)
              }));
            }
          }
          Some(DateSpec::FromName(by_name))
        }
        else {
          return date_result("expected week day or month name");
        }
      }
      Token::Int(_) => {
        let n = t.to_int_result::<u32>()?;
        let t = self.scanner.get();
        if t.finished() {
          // must be a year...
          return Ok(Some(DateSpec::absolute(n, 1, 1)));
        }
        match t {
          Token::Iden(ref name) => {
            let day = n;
            let name = name.to_lowercase();
            // Case: NUMBER IDEN (e.g., "14 december", "2 days")
            if let Some(month) = month_name(&name) {
              // Parsed DAY MONTH (e.g., "14 december").
              // Stop parsing the date part here. Let the main loop
              // handle subsequent tokens (like time "11:20" or year).
              Some(DateSpec::from_day_month(day, month, self.direct))
            } else if let Some(u) = time_unit(&name) {
              // Parsed NUMBER UNIT (e.g., "2 days")
              let mut n = n as i32;
              if sign {
                n = -n;
              }
              else {
                let t = self.scanner.get();
                let got_ago = if let Some(name) = t.as_iden() {
                  if name == "ago" {
                    n = -n;
                    true
                  }
                  else {
                    return date_result("only expected 'ago'");
                  }
                }
                else {
                  false
                };
                if !got_ago {
                  if let Some(h) = t.to_integer() {
                    self.maybe_time = Some((h as u32, TimeKind::Unknown));
                  }
                }
              }
              Some(DateSpec::skip(u, n))
            }
            else if name == "am" || name == "pm" {
              self.maybe_time = Some((n, TimeKind::AmPm(name == "pm")));
              None
            }
            else {
              return date_result("expected month or time unit");
            }
          }
          Token::Char(ch) => match ch {
            '-' => Some(self.iso_date(n)?),
            '/' => Some(self.informal_date(n)?),
            ':' => {
              self.maybe_time = Some((n, TimeKind::Formal));
              None
            }
            '.' => {
              // Check if this is a date with dot (like "15. Jun") or time (like "11.20")
              // Look ahead to see if next token is an identifier (month name)
              let next_token = self.scanner.get();
              if let Token::Iden(ref name) = next_token {
                // This looks like "15. Jun" format
                let name = name.to_lowercase();
                if let Some(month) = month_name(&name) {
                  let day = n;
                  // Check if there's a year following
                  let next_token = self.scanner.get();
                  if next_token.is_integer() {
                    let year = next_token.to_int_result::<u32>()?;
                    Some(DateSpec::absolute(year, month, day))
                  } else {
                    // No year found, just use day/month
                    Some(DateSpec::from_day_month(day, month, self.direct))
                  }
                } else {
                  return date_result("expected month name after day with dot");
                }
              } else if next_token.is_integer() {
                // This is time format like "11.20"
                self.maybe_time = Some((n, TimeKind::Informal));
                None
              } else {
                return date_result("unexpected token after dot");
              }
            }
            _ => return date_result(&format!("unexpected char {ch:?}")),
          },
          _ => return date_result(&format!("unexpected token {t:?}")),
        }
      }
      _ => return date_result(&format!("not expected token {t:?}")),
    })
  }

  fn formal_time(&mut self, hour: u32) -> DateResult<TimeSpec> {
    let min = self.scanner.get_int::<u32>()?;
    // minute may be followed by [:secs][am|pm]
    let mut tnext = None;
    let sec = if let Some(t) = self.scanner.next() {
      if let Some(ch) = t.as_char() {
        if ch != ':' {
          return date_result("expecting ':'");
        }
        self.scanner.get_int::<u32>()?
      }
      else {
        tnext = Some(t);
        0
      }
    }
    else {
      0
    };
    // we found seconds, look ahead
    if tnext.is_none() {
      tnext = self.scanner.next();
    }
    let micros = if let Some(Some('.')) = tnext.as_ref().map(|t| t.as_char()) {
      let frac = self.scanner.grab_while(char::is_numeric);
      if frac.is_empty() {
        return date_result("expected fractional second after '.'");
      }
      let frac = "0.".to_owned() + &frac;
      let micros_f = frac.parse::<f64>().unwrap() * 1.0e6;
      tnext = self.scanner.next();
      micros_f as u32
    }
    else {
      0
    };
    if tnext.is_none() {
      Ok(TimeSpec::new(hour, min, sec, micros))
    }
    else {
      let tok = tnext.as_ref().unwrap();
      if let Some(ch) = tok.as_char() {
        let expecting_offset = match ch {
          '+' | '-' => true,
          _ => return date_result("expected +/- before timezone"),
        };

        let offset = if expecting_offset {
          let hour_and_minute = self.scanner.get_int::<u32>()?;
          let (hour, minute) = if self.scanner.peek() == ':' {
            // 02:00
            self.scanner.nextch();
            (hour_and_minute, self.scanner.get_int::<u32>()?)
          }
          else {
            // Parse 0230 statements.
            // -> 0230 / 100 -> 02
            // -> 0230 % 100 -> 30
            let hour = hour_and_minute / 100;
            let minute = hour_and_minute % 100;
            (hour, minute)
          };

          // Convert to i64, as we might deal with signed times.
          let res: i64 = (60 * (minute + 60 * hour)).into();

          // Apply sign.
          if ch == '-' {
            -res
          }
          else {
            res
          }
        }
        else {
          0
        };
        Ok(TimeSpec::new_with_offset(hour, min, sec, offset, micros))
      }
      else if let Some(id) = tok.as_iden() {
        if id == "Z" {
          Ok(TimeSpec::new_with_offset(hour, min, sec, 0, micros))
        }
        else { // id is not "Z"
          // check if it's am or pm
          let final_hour = if id == "am" || id == "pm" {
              DateParser::am_pm(id, hour)?
          } else {
              // It's some other identifier (like "at").
              // Ignore it and use the original hour.
              // The token `id` was already consumed by scanner.next() earlier.
              hour
          };
          Ok(TimeSpec::new(final_hour, min, sec, micros))
        }
      }
      else {
        Ok(TimeSpec::new(hour, min, sec, micros))
      }
    }
  }

  fn informal_time(&mut self, hour: u32) -> DateResult<TimeSpec> {
    let min = self.scanner.get_int::<u32>()?;
    let hour = if let Some(t) = self.scanner.next() {
      let name = t.to_iden_result()?;
      DateParser::am_pm(&name, hour)?
    }
    else {
      hour
    };
    Ok(TimeSpec::new(hour, min, 0, 0))
  }

  fn am_pm(name: &str, mut hour: u32) -> DateResult<u32> {
    if name == "pm" {
      hour += 12;
    }
    else if name != "am" {
      return date_result("expected am or pm");
    }
    Ok(hour)
  }

  fn hour_time(name: &str, hour: u32) -> DateResult<TimeSpec> {
    Ok(TimeSpec::new(DateParser::am_pm(name, hour)?, 0, 0, 0))
  }

  fn parse_time(&mut self) -> DateResult<Option<TimeSpec>> {
    // here the date parser looked ahead and saw an hour followed by some separator
    if let Some(hour_sep) = self.maybe_time {
      // didn't see a separator, so look...
      let (hour, mut kind) = hour_sep;
      if let TimeKind::Unknown = kind {
        kind = match self.scanner.get_char()? {
          ':' => TimeKind::Formal,
          '.' => TimeKind::Informal,
          ch => return date_result(&format!("expected : or ., not {ch}")),
        };
      }
      Ok(Some(match kind {
        TimeKind::Formal => self.formal_time(hour)?,
        TimeKind::Informal => self.informal_time(hour)?,
        TimeKind::AmPm(is_pm) => {
          DateParser::hour_time(if is_pm { "pm" } else { "am" }, hour)?
        }
        TimeKind::Unknown => unreachable!(),
      }))
    }
    else {
      // no lookahead...
      if self.scanner.peek() == 'T' {
        self.scanner.nextch();
      }
      let mut t = self.scanner.get();
      if t.finished() {
        return Ok(None);
      }

      // Skip "at" connector word if present
      if let Some(name) = t.as_iden() {
        if name == "at" {
          t = self.scanner.get();
          if t.finished() {
            return Ok(None);
          }
        }
      }

      let hour = t.to_int_result::<u32>()?;
      Ok(Some(match self.scanner.get() {
        Token::Char(ch) => match ch {
          ':' => self.formal_time(hour)?,
          '.' => self.informal_time(hour)?,
          ch => return date_result(&format!("unexpected char {ch:?}")),
        },
        Token::Iden(name) => DateParser::hour_time(&name, hour)?,
        t => return date_result(&format!("unexpected token {t:?}")),
      }))
    }
  }

  pub fn parse(&mut self) -> DateResult<DateTimeSpec> {
    let date = self.parse_date()?;
    let time = self.parse_time()?;
    Ok(DateTimeSpec { date, time })
  }
}
