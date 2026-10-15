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

fn is_am_pm(name: &str) -> bool {
  name.eq_ignore_ascii_case("am") || name.eq_ignore_ascii_case("pm")
}

fn is_ordinal_suffix(name: &str) -> bool {
  ["st", "nd", "rd", "th"]
    .iter()
    .any(|suffix| name.eq_ignore_ascii_case(suffix))
}

const CARDINALS: [&str; 20] = [
  "zero",
  "one",
  "two",
  "three",
  "four",
  "five",
  "six",
  "seven",
  "eight",
  "nine",
  "ten",
  "eleven",
  "twelve",
  "thirteen",
  "fourteen",
  "fifteen",
  "sixteen",
  "seventeen",
  "eighteen",
  "nineteen",
];

const TENS: [&str; 8] = [
  "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

const ORDINALS: [&str; 20] = [
  "zeroth",
  "first",
  "second",
  "third",
  "fourth",
  "fifth",
  "sixth",
  "seventh",
  "eighth",
  "ninth",
  "tenth",
  "eleventh",
  "twelfth",
  "thirteenth",
  "fourteenth",
  "fifteenth",
  "sixteenth",
  "seventeenth",
  "eighteenth",
  "nineteenth",
];

const TENS_ORDINALS: [&str; 8] = [
  "twentieth",
  "thirtieth",
  "fortieth",
  "fiftieth",
  "sixtieth",
  "seventieth",
  "eightieth",
  "ninetieth",
];

/// Number for a single word like "five", "twelve", or "thirty"
fn cardinal_word(name: &str) -> Option<u32> {
  let tens = || TENS.iter().position(|w| *w == name).map(|i| 20 + 10 * i);
  CARDINALS
    .iter()
    .position(|w| *w == name)
    .or_else(tens)
    .map(|n| n as u32)
}

/// Number for a single word like "fifth", "twelfth", or "thirtieth"
fn ordinal_word(name: &str) -> Option<u32> {
  let tens = || {
    TENS_ORDINALS
      .iter()
      .position(|w| *w == name)
      .map(|i| 20 + 10 * i)
  };
  ORDINALS
    .iter()
    .position(|w| *w == name)
    .or_else(tens)
    .filter(|n| *n > 0)
    .map(|n| n as u32)
}

/// A number written as words
enum WordNumber {
  /// e.g. "a", "five", "twenty-one", "half", "a few"
  Cardinal(f64),
  /// e.g. "third", "twenty-fourth"
  Ordinal(u32),
}

/// Expand a two-digit year like POSIX `%y`: 69-99 → 1969-1999, 00-68 → 2000-2068
fn expand_year(year: u32) -> u32 {
  match year {
    0..=68 => 2000 + year,
    69..=99 => 1900 + year,
    _ => year,
  }
}

/// Split an identifier like "nov14" into "nov" and the number 14
/// (with its number of digits)
fn split_trailing_digits(name: &str) -> (&str, Option<(u32, usize)>) {
  let alpha_end = name
    .find(|c: char| c.is_ascii_digit())
    .unwrap_or(name.len());
  let (alpha, digits) = name.split_at(alpha_end);
  if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
    return (name, None);
  }
  match digits.parse::<u32>() {
    Ok(n) => (alpha, Some((n, digits.len()))),
    Err(_) => (name, None),
  }
}

pub struct DateParser<'a> {
  scanner: Scanner<'a>,
  direct: Direction,
  maybe_time: Option<(u32, TimeKind)>,
  pub american: bool, // 9/11, not 20/03
  /// Tokens which were read ahead and pushed back (with their number of digits)
  pending: Vec<(Token, usize)>,
  /// Number of digits of the last integer token
  last_len: usize,
  /// Relative offsets after the date and time (e.g. "+1 year")
  skips: Vec<Skip>,
}

impl<'a> DateParser<'a> {
  pub fn new(text: &'a str) -> DateParser<'a> {
    DateParser {
      scanner: Scanner::new(text).no_float(),
      direct: Direction::Here,
      maybe_time: None,
      american: false,
      pending: Vec::new(),
      last_len: 0,
      skips: Vec::new(),
    }
  }

  pub fn american_date(mut self) -> DateParser<'a> {
    self.american = true;
    self
  }

  /// Get the next token (pushed back tokens first)
  /// and remember the number of digits if it is an integer.
  fn get(&mut self) -> Token {
    if let Some((token, len)) = self.pending.pop() {
      self.last_len = len;
      return token;
    }
    self.last_len = 0;
    if self.scanner.skip_whitespace() && self.scanner.peek().is_ascii_digit() {
      let digits = self.scanner.grab_while(|c| c.is_ascii_digit());
      self.last_len = digits.len();
      return match digits.parse::<i64>() {
        Ok(n) => Token::Int(n),
        Err(e) => {
          Token::Error(self.scanner.scan_error("bad integer", Some(&e)))
        }
      };
    }
    self.scanner.get()
  }

  /// Push back the token which was read last
  fn unget(&mut self, token: Token) {
    let len = self.last_len;
    self.unget_len(token, len);
  }

  fn unget_len(&mut self, token: Token, len: usize) {
    if !token.finished() {
      self.pending.push((token, len));
    }
  }

  fn get_int(&mut self) -> DateResult<u32> {
    Ok(self.get().to_int_result::<u32>()?)
  }

  fn expect_char(&mut self, ch: char) -> DateResult<()> {
    match self.get() {
      Token::Char(c) if c == ch => Ok(()),
      t => date_result(&format!("expected {ch:?}, got {t:?}")),
    }
  }

  /// Consume the next token if it is the given char
  fn skip_char(&mut self, ch: char) -> bool {
    let t = self.get();
    if t.as_char() == Some(ch) {
      true
    }
    else {
      self.unget(t);
      false
    }
  }

  /// Consume the next token if it is one of the given words
  fn skip_word(&mut self, words: &[&str]) -> bool {
    let t = self.get();
    if t
      .as_iden()
      .is_some_and(|s| words.iter().any(|w| s.eq_ignore_ascii_case(w)))
    {
      true
    }
    else {
      self.unget(t);
      false
    }
  }

  /// Parse the ones after a tens word (e.g. "twenty-five", "twenty fifth").
  /// Returns the ones and whether they are an ordinal.
  fn ones_after_tens(&mut self) -> Option<(u32, bool)> {
    let t = self.get();
    let hyphen = t.as_char() == Some('-');
    let word = if hyphen { self.get() } else { t };
    let lowercase = word.as_iden().map(str::to_lowercase);
    let ones = lowercase.as_deref().and_then(|w| {
      cardinal_word(w)
        .map(|n| (n, false))
        .or_else(|| ordinal_word(w).map(|n| (n, true)))
    });
    match ones {
      Some((n, is_ordinal)) if (1..=9).contains(&n) => Some((n, is_ordinal)),
      _ => {
        self.unget(word);
        if hyphen {
          self.unget_len(Token::Char('-'), 0);
        }
        None
      }
    }
  }

  /// Parse a number written as words, starting with the given (lowercase) word
  fn word_number(&mut self, first: &str) -> Option<WordNumber> {
    use WordNumber::*;
    Some(match first {
      "a" | "an" => {
        if self.skip_word(&["few"]) {
          Cardinal(3.0)
        }
        else if self.skip_word(&["couple"]) {
          self.skip_word(&["of"]);
          Cardinal(2.0)
        }
        else if self.skip_word(&["half"]) {
          Cardinal(0.5)
        }
        else {
          Cardinal(1.0)
        }
      }
      "half" => {
        self.skip_word(&["a", "an"]);
        Cardinal(0.5)
      }
      "couple" => {
        self.skip_word(&["of"]);
        Cardinal(2.0)
      }
      "few" => Cardinal(3.0),
      "several" => Cardinal(7.0),
      _ => {
        if let Some(n) = ordinal_word(first) {
          Ordinal(n)
        }
        else {
          let n = cardinal_word(first)?;
          let ones = if n >= 20 {
            self.ones_after_tens()
          }
          else {
            None
          };
          match ones {
            Some((ones, true)) => Ordinal(n + ones),
            Some((ones, false)) => Cardinal((n + ones) as f64),
            None => Cardinal(n as f64),
          }
        }
      }
    })
  }

  fn date_shortcut_offset(name: &str) -> Option<i32> {
    match name {
      "now" => Some(0),
      "today" | "tdy" => Some(0),
      "yesterday" | "yday" | "ytd" => Some(-1),
      "tomorrow" | "tmr" | "tmrw" => Some(1),
      _ => None,
    }
  }

  /// Timezone for a name like "UTC", "CET", "BRT", or "ET"
  fn timezone(name: &str) -> Option<Zone> {
    use chrono_tz::Tz;

    let name = name.to_ascii_uppercase();
    // Generic names of regions which switch between standard and daylight
    // saving time (e.g. ET is EST in winter and EDT in summer)
    let region = match name.as_str() {
      "ET" => Some(Tz::America__New_York),
      "CT" => Some(Tz::America__Chicago),
      "MT" => Some(Tz::America__Denver),
      "PT" => Some(Tz::America__Los_Angeles),
      "AKT" => Some(Tz::America__Anchorage),
      _ => None,
    };
    region.map(Zone::Region).or_else(|| {
      Self::timezone_abbrev_offset(&name)
        .or_else(|| Self::legacy_timezone_offset(&name))
        .map(Zone::Offset)
    })
  }

  /// Offsets of common abbreviations which were removed from the
  /// IANA timezone database in favor of numeric ones (e.g. "-03" for "BRT")
  fn legacy_timezone_offset(name: &str) -> Option<i64> {
    let (hours, minutes) = match name {
      "UT" => (0, 0),
      // South America
      "BRT" => (-3, 0),
      "BRST" => (-2, 0),
      "ART" => (-3, 0),
      "UYT" => (-3, 0),
      "GFT" => (-3, 0),
      "SRT" => (-3, 0),
      "CLT" => (-4, 0),
      "CLST" => (-3, 0),
      "BOT" => (-4, 0),
      "PYT" => (-4, 0),
      "VET" => (-4, 0),
      "COT" => (-5, 0),
      "PET" => (-5, 0),
      // Europe and Asia
      "TRT" => (3, 0),
      "IRST" => (3, 30),
      "GST" => (4, 0),
      "AZT" => (4, 0),
      "GET" => (4, 0),
      "AFT" => (4, 30),
      "UZT" => (5, 0),
      "YEKT" => (5, 0),
      "NPT" => (5, 45),
      "BTT" => (6, 0),
      "OMST" => (6, 0),
      "MMT" => (6, 30),
      "ICT" => (7, 0),
      "KRAT" => (7, 0),
      "NOVT" => (7, 0),
      "SGT" => (8, 0),
      "MYT" => (8, 0),
      "PHT" => (8, 0),
      "IRKT" => (8, 0),
      "ULAT" => (8, 0),
      "YAKT" => (9, 0),
      "VLAT" => (10, 0),
      "MAGT" => (11, 0),
      "PETT" => (12, 0),
      // Pacific
      "FJT" => (12, 0),
      "CHAST" => (12, 45),
      "TOT" => (13, 0),
      _ => return None,
    };
    Some(3600 * hours + 60 * minutes)
  }

  fn timezone_abbrev_offset(name: &str) -> Option<i64> {
    use chrono::{DateTime, TimeZone as _};
    use chrono_tz::{OffsetName, TZ_VARIANTS};
    use std::collections::HashMap;
    use std::sync::OnceLock;

    // Built once from chrono-tz's full timezone database by iterating every
    // IANA zone at a NH-winter and NH-summer reference time (covering both
    // hemispheres and standard/DST pairs alike).  TZ_VARIANTS is in alphabetical
    // IANA-name order, so first-found wins for ambiguous abbreviations.
    static ABBREV_MAP: OnceLock<HashMap<String, i64>> = OnceLock::new();

    let map = ABBREV_MAP.get_or_init(|| {
      let winter = DateTime::from_timestamp(1_577_836_800, 0)
        .unwrap()
        .naive_utc(); // 2020-01-01
      let summer = DateTime::from_timestamp(1_593_561_600, 0)
        .unwrap()
        .naive_utc(); // 2020-07-01

      let mut m: HashMap<String, i64> = HashMap::new();
      for &tz in TZ_VARIANTS.iter() {
        for &dt in &[winter, summer] {
          let offset = tz.offset_from_utc_datetime(&dt);
          if let Some(abbrev) = offset.abbreviation() {
            m.entry(abbrev.to_ascii_uppercase()).or_insert_with(|| {
              chrono::Offset::fix(&offset).local_minus_utc() as i64
            });
          }
        }
      }
      m
    });

    if name.eq_ignore_ascii_case("z") {
      return Some(0);
    }
    map.get(&name.to_ascii_uppercase()).copied()
  }

  /// Parse an optional year after a day and month (e.g. "Apr 10, 2024").
  /// A number followed by a time separator is left for the time parser.
  fn optional_year(&mut self) -> DateResult<Option<u32>> {
    let had_comma = self.skip_char(',');
    let t = self.get();
    if !t.is_integer() {
      self.unget(t);
      if had_comma {
        self.unget(Token::Char(','));
      }
      return Ok(None);
    }
    let len = self.last_len;
    let next = self.get();
    let is_time = matches!(next.as_char(), Some(':') | Some('.'))
      || next.as_iden().is_some_and(is_am_pm);
    self.unget(next);
    if is_time {
      self.unget_len(t, len);
      return Ok(None);
    }
    let year = t.to_int_result::<u32>()?;
    // "Apr 10, 2024, 7:12 PM"
    self.skip_char(',');
    Ok(Some(if len == 2 { expand_year(year) } else { year }))
  }

  fn day_month_spec(&mut self, day: u32, month: u32) -> DateResult<DateSpec> {
    Ok(match self.optional_year()? {
      Some(year) => DateSpec::absolute(year, month, day),
      None => DateSpec::from_day_month(day, month, self.direct),
    })
  }

  /// Parse the day after a month name (e.g. "march 3", "march 3rd", "march third")
  fn day_of_month(&mut self) -> DateResult<Option<u32>> {
    let t = self.get();
    match t {
      Token::Int(_) => {
        let len = self.last_len;
        let next = self.get();
        if next.as_char() == Some(':') {
          // It's a time
          self.unget(next);
          self.unget_len(t, len);
          return Ok(None);
        }
        if !next.as_iden().is_some_and(is_ordinal_suffix) {
          self.unget(next);
        }
        Ok(Some(t.to_int_result::<u32>()?))
      }
      Token::Iden(ref name) => {
        let lowercase = name.to_lowercase();
        // Only ordinals, as "may five" or "may a" are no dates
        let is_ordinal = ordinal_word(&lowercase).is_some()
          || TENS.contains(&lowercase.as_str());
        match is_ordinal.then(|| self.word_number(&lowercase)).flatten() {
          Some(WordNumber::Ordinal(day)) => Ok(Some(day)),
          Some(WordNumber::Cardinal(_)) => {
            date_result("expected ordinal day after month name")
          }
          None => {
            self.unget_len(t, 0);
            Ok(None)
          }
        }
      }
      t => {
        self.unget(t);
        Ok(None)
      }
    }
  }

  fn iso_date(&mut self, year: u32) -> DateResult<DateSpec> {
    let month = self.get_int()?;
    self.expect_char('-')?;
    let day = self.get_int()?;
    // Date and time separator (e.g. "2024-04-10T13:31:46")
    if self.pending.is_empty() && matches!(self.scanner.peek(), 'T' | 't') {
      self.scanner.nextch();
    }
    Ok(DateSpec::absolute(year, month, day))
  }

  fn informal_date(&mut self, day_or_month: u32) -> DateResult<DateSpec> {
    let month_or_day = self.get_int()?;
    let (day, month) = if self.american {
      (month_or_day, day_or_month)
    }
    else {
      (day_or_month, month_or_day)
    };
    Ok(if self.skip_char('/') {
      let y = self.get_int()?;
      let y = if self.last_len == 2 {
        expand_year(y)
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

  /// A relative date like "2 days", "1.5 hours ago", or "last 2 weeks"
  fn relative(
    &mut self,
    n: f64,
    unit: Interval,
    has_sign: bool,
  ) -> DateResult<DateSpec> {
    let mut n = if self.direct == Direction::Last {
      -n
    }
    else {
      n
    };
    if !has_sign {
      let t = self.get();
      match t {
        Token::Iden(ref name) if name.eq_ignore_ascii_case("ago") => n = -n,
        Token::Int(h) => {
          self.maybe_time = Some((h as u32, TimeKind::Unknown));
        }
        t => self.unget(t),
      }
    }
    Ok(DateSpec::skip(unit, n))
  }

  fn parse_date(&mut self) -> DateResult<Option<DateSpec>> {
    let mut t = self.get();
    if t.finished() {
      return date_result("empty date string");
    }

    let sign = match t.as_char() {
      Some(ch @ ('-' | '+')) => Some(ch),
      _ => None,
    };
    if sign.is_some() {
      t = self.get();
      if t.finished() {
        return date_result("nothing after sign");
      }
    }

    if let Some(name) = t.as_iden() {
      let name = name.to_lowercase();
      if let Some(skip) = Self::date_shortcut_offset(&name) {
        if sign.is_some() {
          return date_result("unexpected sign");
        }
        return Ok(Some(DateSpec::skip(
          time_unit("day").unwrap(),
          skip as f64,
        )));
      }
      // maybe next or last?
      if let Some(d) = Direction::from_name(&name) {
        self.direct = d;
        t = self.get();
        if t.finished() {
          return date_result("nothing after last/next");
        }
      }
    }

    let date = match t {
      Token::Iden(ref name) => self.named_date(&name.to_lowercase())?,
      Token::Int(_) => {
        let len = self.last_len;
        let n = t.to_int_result::<u32>()?;
        self.numeric_date(n, len, sign)?
      }
      _ => return date_result(&format!("not expected token {t:?}")),
    };

    // A sign is only allowed before a relative date (e.g. "-2 days")
    if sign.is_some() && !matches!(date, Some(DateSpec::Relative(_))) {
      return date_result("expected time unit after sign");
    }
    Ok(date)
  }

  /// A date starting with a name (e.g. "friday", "march 3", "nov14", "week")
  fn named_date(&mut self, name: &str) -> DateResult<Option<DateSpec>> {
    // A number written as words (e.g. "a week ago", "twenty-fourth of May")
    match self.word_number(name) {
      Some(WordNumber::Cardinal(n)) => {
        let unit = self
          .get()
          .as_iden()
          .and_then(|unit| time_unit(&unit.to_lowercase()));
        return match unit {
          Some(unit) => Ok(Some(self.relative(n, unit, false)?)),
          None => date_result("expected time unit after number"),
        };
      }
      Some(WordNumber::Ordinal(day)) => {
        let has_of = self.skip_word(&["of"]);
        let t = self.get();
        let month = t
          .as_iden()
          .and_then(|month| month_name(&month.to_lowercase()));
        match month {
          Some(month) => return Ok(Some(self.day_month_spec(day, month)?)),
          // "second" is also a time unit
          None if !has_of && time_unit(name).is_some() => self.unget(t),
          None => return date_result("expected month name after ordinal"),
        }
      }
      None => {}
    }

    let (alpha, digits) = split_trailing_digits(name);
    if let Some(by_name) = ByName::from_name(alpha, self.direct) {
      // a MONTH _might_ be followed by DAY, YEAR
      if let Some(month) = by_name.as_month() {
        let day = match digits {
          Some((day, _)) => Some(day),
          None => self.day_of_month()?,
        };
        if let Some(day) = day {
          return Ok(Some(self.day_month_spec(day, month)?));
        }
      }
      else if digits.is_some() {
        return date_result("unexpected number after week day");
      }
      return Ok(Some(DateSpec::FromName(by_name)));
    }

    // A time unit without a number (e.g. "next week", "last month", "year")
    if digits.is_none() && name.len() >= 3 {
      if let Some(unit) = time_unit(name) {
        let n = if self.direct == Direction::Last {
          -1.0
        }
        else {
          1.0
        };
        return Ok(Some(DateSpec::skip(unit, n)));
      }
    }

    date_result("expected week day or month name")
  }

  /// A date starting with a number (e.g. "2024-04-10", "14 december", "2 days")
  fn numeric_date(
    &mut self,
    n: u32,
    len: usize,
    sign: Option<char>,
  ) -> DateResult<Option<DateSpec>> {
    let has_sign = sign.is_some();
    let negate = |x: f64| if sign == Some('-') { -x } else { x };

    let t = self.get();
    if t.finished() {
      // must be a year...
      return Ok(Some(DateSpec::absolute(n, 1, 1)));
    }
    Ok(match t {
      Token::Iden(ref name) => {
        let name = name.to_lowercase();
        let (alpha, digits) = split_trailing_digits(&name);
        if let Some(month) = month_name(alpha) {
          // Case: DAY MONTH (e.g. "14 december", "14nov2022")
          Some(match digits {
            Some((year, year_len)) => {
              let year = if year_len == 2 {
                expand_year(year)
              }
              else {
                year
              };
              DateSpec::absolute(year, month, n)
            }
            None => self.day_month_spec(n, month)?,
          })
        }
        else if digits.is_some() {
          return date_result("expected month or time unit");
        }
        else if is_ordinal_suffix(&name) {
          // Case: "3rd march", "1st of june"
          let mut t = self.get();
          if t.as_iden().is_some_and(|s| s.eq_ignore_ascii_case("of")) {
            t = self.get();
          }
          match t.as_iden().and_then(|s| month_name(&s.to_lowercase())) {
            Some(month) => Some(self.day_month_spec(n, month)?),
            None => return date_result("expected month name after ordinal"),
          }
        }
        else if let Some(u) = time_unit(&name) {
          // Case: NUMBER UNIT (e.g. "2 days")
          Some(self.relative(negate(n as f64), u, has_sign)?)
        }
        else if is_am_pm(&name) {
          // Case: "4pm" (a date may follow, e.g. "4pm tomorrow")
          self.maybe_time =
            Some((n, TimeKind::AmPm(name.eq_ignore_ascii_case("pm"))));
          None
        }
        else {
          return date_result("expected month or time unit");
        }
      }
      Token::Char(ch) => match ch {
        '-' => {
          let year = if len == 2 { expand_year(n) } else { n };
          Some(self.iso_date(year)?)
        }
        '/' => Some(self.informal_date(n)?),
        ':' => {
          self.maybe_time = Some((n, TimeKind::Formal));
          None
        }
        '.' => {
          // A date with dot (like "15. Jun"), decimal number (like "1.5"),
          // or time (like "11.20")
          let next = self.get();
          match next {
            Token::Iden(ref name) => match month_name(&name.to_lowercase()) {
              Some(month) => Some(self.day_month_spec(n, month)?),
              None => {
                return date_result("expected month name after day with dot")
              }
            },
            Token::Int(_) => {
              let dec_len = self.last_len;
              let decimal_part = next.to_int_result::<u32>()?;
              let unit_token = self.get();
              let unit = unit_token
                .as_iden()
                .and_then(|name| time_unit(&name.to_lowercase()));
              match unit {
                Some(u) => {
                  // This is a decimal duration like "1.5 hours"
                  let n_float = n as f64
                    + decimal_part as f64 / 10_f64.powi(dec_len as i32);
                  Some(self.relative(negate(n_float), u, has_sign)?)
                }
                None => {
                  // Treat as informal time like "11.20" or "11.20pm"
                  let is_time = dec_len == 2
                    && (unit_token.finished()
                      || unit_token.as_iden().is_some_and(is_am_pm));
                  if !is_time {
                    return date_result("expected time unit after number");
                  }
                  self.unget(unit_token);
                  self.unget_len(Token::Int(decimal_part as i64), dec_len);
                  self.maybe_time = Some((n, TimeKind::Informal));
                  None
                }
              }
            }
            _ => return date_result("unexpected token after dot"),
          }
        }
        _ => return date_result(&format!("unexpected char {ch:?}")),
      },
      _ => return date_result(&format!("unexpected token {t:?}")),
    })
  }

  /// Parse a numeric timezone offset after the sign (e.g. "05:30", "0530", "5")
  fn offset(&mut self, sign: char) -> DateResult<i64> {
    let value = self.get_int()?;
    let len = self.last_len;
    let (hour, minute) = if self.skip_char(':') {
      (value, self.get_int()?)
    }
    else if len <= 2 {
      (value, 0)
    }
    else {
      // "0230" → 02:30
      (value / 100, value % 100)
    };
    if minute >= 60 || hour * 60 + minute > 24 * 60 {
      return date_result("invalid timezone offset");
    }
    let t = self.get();
    if t.as_char() == Some(':') {
      return date_result("invalid timezone offset");
    }
    self.unget(t);
    let offset = i64::from(60 * (minute + 60 * hour));
    Ok(if sign == '-' { -offset } else { offset })
  }

  fn formal_time(&mut self, hour: u32) -> DateResult<TimeSpec> {
    let min = self.get_int()?;
    // minute may be followed by [:secs][.frac][am|pm|offset|timezone]
    let mut tnext = self.get();
    let sec = if tnext.as_char() == Some(':') {
      let sec = self.get_int()?;
      tnext = self.get();
      sec
    }
    else {
      0
    };
    let micros = if tnext.as_char() == Some('.') && self.pending.is_empty() {
      let frac = self.scanner.grab_while(|c| c.is_ascii_digit());
      if frac.is_empty() {
        return date_result("expected fractional second after '.'");
      }
      let frac = "0.".to_owned() + &frac;
      let micros_f = frac.parse::<f64>().unwrap() * 1.0e6;
      tnext = self.get();
      micros_f as u32
    }
    else {
      0
    };
    match tnext {
      Token::Char(ch @ ('+' | '-')) => {
        // A relative offset (e.g. "00:00 +1 year") and not a timezone offset
        if let Ok(skip) = self.try_relative(Token::Char(ch))? {
          self.skips.push(skip);
          return Ok(TimeSpec::new(hour, min, sec, micros));
        }
        let offset = self.offset(ch)?;
        Ok(TimeSpec::new_with_zone(
          hour,
          min,
          sec,
          Zone::Offset(offset),
          micros,
        ))
      }
      Token::Iden(ref id) if is_am_pm(id) => {
        let final_hour = DateParser::am_pm(id, hour)?;
        Ok(TimeSpec::new(final_hour, min, sec, micros))
      }
      Token::Iden(ref id) if Self::timezone(id).is_some() => {
        let zone = Self::timezone(id).unwrap();
        Ok(TimeSpec::new_with_zone(hour, min, sec, zone, micros))
      }
      t => {
        self.unget(t);
        Ok(TimeSpec::new(hour, min, sec, micros))
      }
    }
  }

  fn informal_time(&mut self, hour: u32) -> DateResult<TimeSpec> {
    let min = self.get_int()?;
    let t = self.get();
    let hour = match t.as_iden() {
      Some(name) if is_am_pm(name) => DateParser::am_pm(name, hour)?,
      _ => {
        self.unget(t);
        hour
      }
    };
    Ok(TimeSpec::new(hour, min, 0, 0))
  }

  /// Convert a 12-hour clock hour (1-12) to a 24-hour clock hour.
  /// 12 AM is midnight (0) and 12 PM is noon (12).
  fn am_pm(name: &str, hour: u32) -> DateResult<u32> {
    let is_pm = match name.to_ascii_lowercase().as_str() {
      "am" => false,
      "pm" => true,
      _ => return date_result("expected am or pm"),
    };
    if !(1..=12).contains(&hour) {
      return date_result(&format!("invalid 12-hour clock hour {hour}"));
    }
    Ok(hour % 12 + if is_pm { 12 } else { 0 })
  }

  fn parse_time(&mut self) -> DateResult<Option<TimeSpec>> {
    // here the date parser looked ahead and saw an hour followed by some separator
    if let Some((hour, mut kind)) = self.maybe_time.take() {
      // didn't see a separator, so look...
      if let TimeKind::Unknown = kind {
        kind = match self.get().as_char() {
          Some(':') => TimeKind::Formal,
          Some('.') => TimeKind::Informal,
          _ => return date_result("expected : or ."),
        };
      }
      return Ok(Some(match kind {
        TimeKind::Formal => self.formal_time(hour)?,
        TimeKind::Informal => self.informal_time(hour)?,
        TimeKind::AmPm(is_pm) => TimeSpec::new(
          DateParser::am_pm(if is_pm { "pm" } else { "am" }, hour)?,
          0,
          0,
          0,
        ),
        TimeKind::Unknown => unreachable!(),
      }));
    }

    // no lookahead...
    let mut t = self.get();
    // Skip "at" connector word or ISO 8601 separator if present
    if t.as_iden().is_some_and(|name| {
      name.eq_ignore_ascii_case("at") || name.eq_ignore_ascii_case("t")
    }) {
      t = self.get();
      if !t.is_integer() {
        return date_result("expected time after 'at'");
      }
    }
    if !t.is_integer() {
      self.unget(t);
      return Ok(None);
    }

    let hour_len = self.last_len;
    let hour = t.to_int_result::<u32>()?;
    let sep = self.get();
    Ok(Some(match sep {
      Token::Char(':') => self.formal_time(hour)?,
      Token::Char('.') => self.informal_time(hour)?,
      Token::Iden(ref name) if is_am_pm(name) => {
        TimeSpec::new(DateParser::am_pm(name, hour)?, 0, 0, 0)
      }
      sep => {
        // Not a time (e.g. a relative offset like "1 year")
        self.unget(sep);
        self.unget_len(Token::Int(hour as i64), hour_len);
        return Ok(None);
      }
    }))
  }

  /// Try to parse a relative offset like "+1 year", "- 2 hours", "3 days ago".
  /// Returns the first token if the following tokens are no relative offset.
  fn try_relative(&mut self, first: Token) -> DateResult<Result<Skip, Token>> {
    let first_len = self.last_len;
    let sign = match first.as_char() {
      Some(ch @ ('+' | '-')) => Some(ch),
      _ => None,
    };
    let (num, num_len) = if sign.is_some() {
      let num = self.get();
      if !num.is_integer() {
        self.unget(num);
        self.last_len = first_len;
        return Ok(Err(first));
      }
      (num, self.last_len)
    }
    else if first.is_integer() {
      (first, first_len)
    }
    else {
      return Ok(Err(first));
    };

    let unit_token = self.get();
    let unit = unit_token
      .as_iden()
      .map(|name| name.to_lowercase())
      .filter(|name| month_name(name).is_none())
      .and_then(|name| time_unit(&name));
    let Some(unit) = unit else {
      self.unget(unit_token);
      let first = match sign {
        Some(ch) => {
          self.unget_len(num, num_len);
          Token::Char(ch)
        }
        None => num,
      };
      self.last_len = first_len;
      return Ok(Err(first));
    };

    let mut n = num.to_int_result::<u32>()? as f64;
    if sign == Some('-') {
      n = -n;
    }
    else if sign.is_none() {
      let t = self.get();
      if t.as_iden().is_some_and(|s| s.eq_ignore_ascii_case("ago")) {
        n = -n;
      }
      else {
        self.unget(t);
      }
    }
    Ok(Ok(Skip { unit, skip: n }))
  }

  /// Parse everything after the date and time:
  /// a date following a time ("4pm tomorrow"), a year following a time
  /// ("Jul 18 06:14:49 2024"), a timezone ("5pm EST"),
  /// or relative offsets ("2024-01-31 +1 month").
  fn parse_trailing(
    &mut self,
    date: &mut Option<DateSpec>,
    time: &mut Option<TimeSpec>,
  ) -> DateResult<()> {
    loop {
      let t = self.get();
      if t.finished() {
        return Ok(());
      }

      if let Some(name) = t.as_iden() {
        // Timezone abbreviation (e.g. "5pm EST")
        if let Some(ts) = time.as_mut() {
          if ts.zone.is_none() {
            if let Some(zone) = Self::timezone(name) {
              ts.zone = Some(zone);
              continue;
            }
          }
          if is_am_pm(name) {
            return date_result(&format!("unexpected {name}"));
          }
        }
        // Redundant week day after a full date (e.g. "2026-08-17 Wed")
        if matches!(date, Some(DateSpec::Absolute(_)))
          && week_day(&name.to_lowercase()).is_some()
        {
          continue;
        }
        // Don't silently ignore unknown timezones (e.g. "12:00 FOO")
        let lowercase = name.to_lowercase();
        let is_known_word = week_day(&lowercase).is_some()
          || month_name(&lowercase).is_some()
          || (lowercase.len() >= 3 && time_unit(&lowercase).is_some())
          || Self::date_shortcut_offset(&lowercase).is_some()
          || Direction::from_name(&lowercase).is_some()
          || ["at", "on", "to", "in", "by", "and", "the", "till", "until"]
            .contains(&lowercase.as_str());
        let looks_like_zone = (2..=5).contains(&name.len())
          && name.chars().all(|c| c.is_ascii_uppercase());
        if looks_like_zone && !is_known_word {
          return date_result(&format!("unknown timezone {name}"));
        }
      }

      // Relative offset (e.g. "+1 year")
      let t = match self.try_relative(t)? {
        Ok(skip) => {
          self.skips.push(skip);
          continue;
        }
        Err(t) => t,
      };

      // Year after a time (e.g. "Jul 18 06:14:49 2024")
      if t.is_integer() && self.last_len == 4 {
        if let Some(DateSpec::FromName(ByName::DayMonth(yd))) = date {
          if yd.direct == Direction::Here {
            let year = t.to_int_result::<u32>()?;
            *date = Some(DateSpec::absolute(year, yd.month, yd.day));
            continue;
          }
        }
      }

      // Date after a time (e.g. "4pm tomorrow", "8:23 AM, Jul 9")
      // or a full date after a week day (e.g. "Wed 10:30 2026-08-17")
      let is_week_day =
        matches!(date, Some(DateSpec::FromName(ByName::WeekDay(_))));
      if (date.is_none() && time.is_some()) || is_week_day {
        let is_separator = matches!(t.as_char(), Some(',' | '∙' | '·'))
          || t.as_iden().is_some_and(|s| s.eq_ignore_ascii_case("on"));
        if is_separator {
          continue;
        }
        if t.is_integer() {
          let next = self.get();
          let is_bare_number = next.finished();
          self.unget(next);
          if is_bare_number {
            return date_result(&format!("unexpected number {t:?}"));
          }
        }
        self.unget(t);
        self.direct = Direction::Here;
        match self.parse_date() {
          Ok(Some(spec)) => {
            *date = Some(spec);
            continue;
          }
          Ok(None) => return date_result("time given twice"),
          // Ignore irrelevant trailing text after a time
          Err(_) if time.is_some() => return Ok(()),
          Err(e) => return Err(e),
        }
      }

      // Ignore irrelevant trailing text after a time,
      // but don't accept garbage after a date
      if time.is_some() {
        return Ok(());
      }
      return date_result(&format!("unexpected {t:?}"));
    }
  }

  pub fn parse(&mut self) -> DateResult<DateTimeSpec> {
    let mut date = self.parse_date()?;
    let mut time = self.parse_time()?;
    self.parse_trailing(&mut date, &mut time)?;
    let skips = std::mem::take(&mut self.skips);
    Ok(DateTimeSpec { date, time, skips })
  }
}
