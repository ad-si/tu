use super::errors::*;
use super::types::*;
use scanlex::{Scanner, Token};

// when we parse dates, there's often a bit of time parsed..
#[derive(Clone, Copy, Debug)]
enum TimeKind {
  Formal,
  Informal,
  AmPm(bool),
  /// Full hour on the 24-hour clock (e.g. "6 in the evening")
  Hour,
}

/// Part of the day (e.g. "morning", "tonight")
#[derive(Clone, Copy, Debug, PartialEq)]
enum DayPeriod {
  Morning,
  Noon,
  Afternoon,
  Evening,
  Night,
  Midnight,
}

impl DayPeriod {
  fn from_name(name: &str) -> Option<DayPeriod> {
    use DayPeriod::*;
    Some(match name.to_lowercase().as_str() {
      "morning" => Morning,
      "noon" | "midday" => Noon,
      "afternoon" => Afternoon,
      "evening" => Evening,
      "night" | "tonight" => Night,
      "midnight" => Midnight,
      _ => return None,
    })
  }

  /// Hour if no explicit time is given
  fn default_hour(self) -> u32 {
    use DayPeriod::*;
    match self {
      Morning => 6,
      Noon => 12,
      Afternoon => 15,
      Evening => 18,
      Night => 23,
      Midnight => 0,
    }
  }

  /// 24-hour clock hour of a 12-hour clock hour in this part of the day
  /// (e.g. 8 in the evening is 20)
  fn hour(self, hour: u32) -> u32 {
    use DayPeriod::*;
    match self {
      Morning | Midnight if hour == 12 => 0,
      Afternoon | Evening if hour < 12 => hour + 12,
      Night if hour == 12 => 0,
      Night if (6..12).contains(&hour) => hour + 12,
      _ => hour,
    }
  }
}

/// Words which can be ignored before a date (e.g. "within 3 days")
const DATE_FILLERS: [&str; 10] = [
  "on",
  "the",
  "within",
  "for",
  "after",
  "around",
  "about",
  "approximately",
  "approx",
  "roughly",
];

/// Words which can be ignored before a time (e.g. "tomorrow at 5pm")
const TIME_CONNECTORS: [&str; 7] =
  ["at", "t", "by", "before", "after", "around", "about"];

fn is_word(token: &Token, words: &[&str]) -> bool {
  token
    .as_iden()
    .is_some_and(|s| words.iter().any(|w| s.eq_ignore_ascii_case(w)))
}

fn is_am_pm(name: &str) -> bool {
  name.eq_ignore_ascii_case("am") || name.eq_ignore_ascii_case("pm")
}

/// Order a day and month given as numbers.
/// The dialect decides the order, unless only the other one is valid
/// (e.g. "25/05/2015" is the 25th of May even in the US).
fn day_month(first: u32, second: u32, american: bool) -> (u32, u32) {
  let (day, month) = if american {
    (second, first)
  }
  else {
    (first, second)
  };
  if month > 12 && day <= 12 {
    (month, day)
  }
  else {
    (day, month)
  }
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

/// Calendar era of a year
enum Era {
  /// AD or CE
  Common,
  /// Buddhist era (543 years ahead of the common era)
  Buddhist,
}

/// Number after a month name
enum DayOrYear {
  Day(u32),
  Year(u32),
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
  /// Part of the day (e.g. "tomorrow morning")
  period: Option<DayPeriod>,
  /// Timezone given without a time (e.g. "UTC+07:00")
  zone: Option<Zone>,
  /// Whether the date started with "the"
  has_article: bool,
  /// Whether the last relative date had a suffix like "ago" or "later"
  has_suffix: bool,
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
      period: None,
      zone: None,
      has_article: false,
      has_suffix: false,
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

  /// Push back tokens which were read with `take` (in reading order)
  fn restore(&mut self, taken: Vec<(Token, usize)>) {
    for (token, len) in taken.into_iter().rev() {
      self.unget_len(token, len);
    }
  }

  /// Get the next token and remember it for a later `restore`
  fn take(&mut self, taken: &mut Vec<(Token, usize)>) -> usize {
    let token = self.get();
    taken.push((token, self.last_len));
    taken.len() - 1
  }

  /// Check the next token without consuming it
  fn peek_is(&mut self, pred: impl Fn(&Token) -> bool) -> bool {
    let token = self.get();
    let len = self.last_len;
    let result = pred(&token);
    self.unget_len(token, len);
    result
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
      "MEZ" => (1, 0),
      "MESZ" | "MEST" => (2, 0),
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

  /// Parse an optional year after a day and month
  /// (e.g. "Apr 10, 2024", "10-Aug-2012", "Aug/10/2012").
  /// A number followed by a time separator is left for the time parser.
  fn optional_year(&mut self) -> DateResult<Option<u32>> {
    let mut taken = Vec::new();
    let mut t = self.get();
    if matches!(t.as_char(), Some(',' | '-' | '/')) {
      taken.push((t, self.last_len));
      t = self.get();
    }
    let len = self.last_len;
    taken.push((t, len));
    if !taken.last().unwrap().0.is_integer() {
      self.restore(taken);
      return Ok(None);
    }
    let is_time_or_offset = self.peek_is(|next| {
      (len <= 2 && matches!(next.as_char(), Some(':') | Some('.')))
        || next.as_iden().is_some_and(|name| {
          is_am_pm(name) || time_unit(&name.to_lowercase()).is_some()
        })
    });
    if is_time_or_offset {
      self.restore(taken);
      return Ok(None);
    }
    let year = taken.pop().unwrap().0.to_int_result::<u32>()?;
    let year = match self.era() {
      Some(Era::Common) => year,
      Some(Era::Buddhist) => year.saturating_sub(543),
      None if len == 2 => expand_year(year),
      None => year,
    };
    // "Apr 10, 2024, 7:12 PM"
    self.skip_char(',');
    self.skip_date_time_separator();
    Ok(Some(year))
  }

  /// Parse an optional era after a year (e.g. "8 AD", "2555 BE")
  fn era(&mut self) -> Option<Era> {
    let t = self.get();
    let era = match t.as_iden().map(str::to_ascii_uppercase).as_deref() {
      Some("AD" | "CE") => Some(Era::Common),
      Some("BE") => Some(Era::Buddhist),
      _ => None,
    };
    if era.is_none() {
      self.unget(t);
    }
    era
  }

  /// Skip a separator between a date and a time
  /// (e.g. "06/Nov/2023:06:36:02", "05/31/2024.14:15", "05/31/2024-14:15")
  fn skip_date_time_separator(&mut self) {
    let mut taken = Vec::new();
    let sep = self.take(&mut taken);
    if matches!(taken[sep].0.as_char(), Some(':' | '.' | '-')) {
      let hour = self.take(&mut taken);
      if taken[hour].0.is_integer()
        && taken[hour].1 <= 2
        && self.peek_is(|t| t.as_char() == Some(':'))
      {
        taken.remove(sep);
      }
    }
    self.restore(taken);
  }

  fn day_month_spec(&mut self, day: u32, month: u32) -> DateResult<DateSpec> {
    // "1 sept."
    self.skip_char('.');
    Ok(match self.optional_year()? {
      Some(year) => DateSpec::absolute(year, month, day),
      None => DateSpec::from_day_month(day, month, self.direct),
    })
  }

  /// Parse an optional day after a year and month (e.g. "2018 March 18")
  fn year_month_spec(&mut self, year: u32, month: u32) -> DateResult<DateSpec> {
    self.skip_char('.');
    let mut taken = Vec::new();
    let mut t = self.take(&mut taken);
    if matches!(taken[t].0.as_char(), Some('-' | '/')) {
      t = self.take(&mut taken);
    }
    let (token, len) = &taken[t];
    let is_day = token.is_integer()
      && *len <= 2
      && !self.peek_is(|next| {
        next.as_char() == Some(':')
          || next.as_iden().is_some_and(|name| {
            is_am_pm(name) || time_unit(&name.to_lowercase()).is_some()
          })
      });
    if !is_day {
      self.restore(taken);
      return Ok(DateSpec::absolute(year, month, 1));
    }
    let day = taken.swap_remove(t).0.to_int_result::<u32>()?;
    self.skip_date_time_separator();
    Ok(DateSpec::absolute(year, month, day))
  }

  /// Parse the day or year after a month name
  /// (e.g. "march 3", "march 3rd", "march third", "Sep-2012", "June of 2022")
  fn day_of_month(&mut self) -> DateResult<Option<DayOrYear>> {
    // "Sep-2012", "August/10", "June of 2022", "August, 2012"
    let mut taken = Vec::new();
    let sep = self.take(&mut taken);
    let has_sep = matches!(taken[sep].0.as_char(), Some('-' | '/' | ','))
      || is_word(&taken[sep].0, &["of"]);
    if has_sep && self.peek_is(Token::is_integer) {
      taken.clear();
    }
    else {
      self.restore(taken);
    }

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
        let n = t.to_int_result::<u32>()?;
        if len == 4 {
          // "September 2012"
          self.unget(next);
          self.skip_date_time_separator();
          return Ok(Some(DayOrYear::Year(n)));
        }
        if !next.as_iden().is_some_and(is_ordinal_suffix) {
          self.unget(next);
        }
        Ok(Some(DayOrYear::Day(n)))
      }
      Token::Iden(ref name) => {
        let lowercase = name.to_lowercase();
        // Only ordinals, as "may five" or "may a" are no dates
        let is_ordinal = ordinal_word(&lowercase).is_some()
          || TENS.contains(&lowercase.as_str());
        match is_ordinal.then(|| self.word_number(&lowercase)).flatten() {
          Some(WordNumber::Ordinal(day)) => Ok(Some(DayOrYear::Day(day))),
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

  /// A date with hyphens (e.g. "2024-04-10", "24-04-10", "04-10-2024")
  fn iso_date(&mut self, first: u32, first_len: usize) -> DateResult<DateSpec> {
    let month = self.get_int()?;
    self.expect_char('-')?;
    let day = self.get_int()?;
    if first_len <= 2 && self.last_len == 4 {
      // "05-25-2015" (US) or "25-05-2015"
      let (d, m) = day_month(first, month, self.american);
      self.skip_date_time_separator();
      return Ok(DateSpec::absolute(day, m, d));
    }
    let year = if first_len == 2 {
      expand_year(first)
    }
    else {
      first
    };
    // Date and time separator (e.g. "2024-04-10T13:31:46")
    if self.pending.is_empty() && matches!(self.scanner.peek(), 'T' | 't') {
      self.scanner.nextch();
    }
    Ok(DateSpec::absolute(year, month, day))
  }

  /// A date with slashes (e.g. "4/10", "4/10/2024", "2024/4/10")
  fn informal_date(
    &mut self,
    first: u32,
    first_len: usize,
  ) -> DateResult<DateSpec> {
    let second = self.get_int()?;
    if first_len == 4 {
      self.expect_char('/')?;
      let third = self.get_int()?;
      // Fix swapped day and month (e.g. "2024/13/1")
      let (day, month) = day_month(third, second, false);
      self.skip_date_time_separator();
      return Ok(DateSpec::absolute(first, month, day));
    }
    if self.last_len == 4 {
      // "9/2012"
      return Ok(DateSpec::absolute(second, first, 1));
    }
    let (day, month) = day_month(first, second, self.american);
    Ok(if self.skip_char('/') {
      let y = self.get_int()?;
      let y = if self.last_len == 2 {
        expand_year(y)
      }
      else {
        y
      };
      self.skip_date_time_separator();
      DateSpec::absolute(y, month, day)
    }
    else {
      DateSpec::FromName(ByName::from_day_month(day, month, self.direct))
    })
  }

  /// A date with dots (e.g. "2015.05.25", "25.05.2015").
  /// Returns `None` if the number after the second dot is missing.
  fn dotted_date(
    &mut self,
    first: u32,
    first_len: usize,
    second: u32,
  ) -> DateResult<Option<DateSpec>> {
    let mut taken = Vec::new();
    let dot = self.take(&mut taken);
    let third = self.take(&mut taken);
    if taken[dot].0.as_char() != Some('.') || !taken[third].0.is_integer() {
      self.restore(taken);
      return Ok(None);
    }
    let third_len = taken[third].1;
    let third = taken.pop().unwrap().0.to_int_result::<u32>()?;
    let spec = if first_len == 4 {
      let (day, month) = day_month(third, second, false);
      DateSpec::absolute(first, month, day)
    }
    else {
      // Dotted dates are day first, except if that's invalid
      let (day, month) = day_month(first, second, false);
      let year = if third_len == 2 {
        expand_year(third)
      }
      else {
        third
      };
      DateSpec::absolute(year, month, day)
    };
    self.skip_date_time_separator();
    Ok(Some(spec))
  }

  /// Parse a further part of a duration
  /// (e.g. the "21 hours" in "1 day, 21 hours ago")
  fn duration_part(&mut self) -> DateResult<Option<Skip>> {
    let mut taken = Vec::new();
    loop {
      let t = self.take(&mut taken);
      let token = &taken[t].0;
      if !(token.as_char() == Some(',') || is_word(token, &["and"])) {
        break;
      }
    }
    let num = taken.len() - 1;
    let sign = match taken[num].0.as_char() {
      Some('+') => Some(1.0),
      Some('-') => Some(-1.0),
      _ => None,
    };
    let num = if sign.is_some() {
      self.take(&mut taken)
    }
    else {
      num
    };
    let unit = self.take(&mut taken);
    let unit = taken[unit]
      .0
      .as_iden()
      .map(str::to_lowercase)
      .filter(|name| month_name(name).is_none())
      .and_then(|name| time_unit(&name));
    match (unit, &taken[num].0) {
      (Some(unit), Token::Int(n)) => Ok(Some(Skip {
        unit,
        skip: sign.unwrap_or(1.0) * *n as f64,
      })),
      _ => {
        self.restore(taken);
        Ok(None)
      }
    }
  }

  /// A relative date like "2 days", "1.5 hours ago", "last 2 weeks",
  /// "1 day 21 hours ago", "5 days from now", or "2 weeks after yesterday"
  fn relative(
    &mut self,
    n: f64,
    unit: Interval,
    has_sign: bool,
  ) -> DateResult<Option<DateSpec>> {
    let n = if self.direct == Direction::Last {
      -n
    }
    else {
      n
    };
    let mut skips = vec![Skip { unit, skip: n }];
    // Unsigned parts have the sign of the first one (e.g. "-2 hours 5 minutes")
    let factor = if n < 0.0 { -1.0 } else { 1.0 };
    while let Some(mut skip) = self.duration_part()? {
      if skip.skip > 0.0 {
        skip.skip *= factor;
      }
      skips.push(skip);
    }

    let t = self.get();
    let suffix = t.as_iden().map(str::to_lowercase);
    let (negate, has_anchor) = match suffix.as_deref() {
      Some("ago" | "earlier" | "back") => (true, false),
      Some("before") => (true, true),
      Some("later" | "hence" | "out") => (false, false),
      Some("after" | "from") => (false, true),
      _ => {
        self.unget(t);
        self.has_suffix = false;
        let first = skips.remove(0);
        self.skips.extend(skips);
        return Ok(Some(DateSpec::Relative(first)));
      }
    };
    self.has_suffix = true;
    if has_sign && suffix.as_deref() == Some("ago") {
      return date_result("a signed duration cannot be combined with 'ago'");
    }
    if negate {
      for skip in &mut skips {
        skip.skip = -skip.skip;
      }
    }
    // A date the duration is relative to (e.g. "2 days after tomorrow")
    if has_anchor && !self.peek_is(Token::finished) {
      self.direct = Direction::Here;
      let anchor = self.parse_date()?;
      self.skips.extend(skips);
      return Ok(anchor);
    }

    let first = skips.remove(0);
    self.skips.extend(skips);
    Ok(Some(DateSpec::Relative(first)))
  }

  fn parse_date(&mut self) -> DateResult<Option<DateSpec>> {
    // Skip filler words (e.g. "within 3 days", "on the 4th")
    loop {
      let t = self.get();
      if !(t.as_char() == Some('~') || is_word(&t, &DATE_FILLERS)) {
        self.unget(t);
        break;
      }
      self.has_article = is_word(&t, &["the"]);
    }

    if let Some(spec) = self.end_of()? {
      return Ok(Some(spec));
    }

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

    // Part of a day (e.g. "tonight", "this morning", "last night")
    if let Some(period) = t.as_iden().and_then(DayPeriod::from_name) {
      if sign.is_some() {
        return date_result("unexpected sign");
      }
      self.period = Some(period);
      let day = time_unit("day").unwrap();
      return Ok(match self.direct {
        Direction::Next => Some(DateSpec::skip(day, 1.0)),
        Direction::Last => Some(DateSpec::skip(day, -1.0)),
        Direction::Here | Direction::This => None,
      });
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

  /// Last day of a period (e.g. "end of the month", "last day of July")
  fn end_of(&mut self) -> DateResult<Option<DateSpec>> {
    let mut taken = Vec::new();
    let first = self.take(&mut taken);
    let second = self.take(&mut taken);
    let is_end_of = if is_word(&taken[first].0, &["end"]) {
      is_word(&taken[second].0, &["of"])
    }
    else {
      is_word(&taken[first].0, &["last"])
        && is_word(&taken[second].0, &["day"])
        && {
          let of = self.take(&mut taken);
          is_word(&taken[of].0, &["of"])
        }
    };
    if !is_end_of {
      self.restore(taken);
      return Ok(None);
    }

    self.skip_word(&["the"]);
    let mut t = self.get();
    let direct = match t
      .as_iden()
      .and_then(|name| Direction::from_name(&name.to_lowercase()))
    {
      Some(direct) => {
        t = self.get();
        direct
      }
      None => Direction::Here,
    };
    let Some(name) = t.as_iden().map(str::to_lowercase) else {
      return date_result("expected month or time unit after 'end of'");
    };

    if let Some(month) = month_name(&name) {
      self.skip_char('.');
      let base = match self.optional_year()? {
        Some(year) => DateSpec::absolute(year, month, 1),
        None => {
          // "end of October next year"
          if let Some(years) = self.direction_unit(Period::Year) {
            self.skips.push(Skip {
              unit: Interval::Months(12),
              skip: years as f64,
            });
          }
          DateSpec::FromName(ByName::MonthName(NamedDate::new(direct, month)))
        }
      };
      return Ok(Some(DateSpec::EndOf(Box::new(base), Period::Month)));
    }

    let unit = time_unit(&name);
    match unit.and_then(Period::from_interval) {
      Some(period) => {
        let base = DateSpec::skip(unit.unwrap(), direct.offset() as f64);
        Ok(Some(DateSpec::EndOf(Box::new(base), period)))
      }
      None => date_result("expected month or time unit after 'end of'"),
    }
  }

  /// Parse a direction with a period (e.g. "next week", "of last week")
  /// and return its offset
  fn direction_unit(&mut self, period: Period) -> Option<i64> {
    let mut taken = Vec::new();
    let mut t = self.take(&mut taken);
    if taken[t].0.as_char() == Some(',') || is_word(&taken[t].0, &["of"]) {
      t = self.take(&mut taken);
    }
    let direct = taken[t]
      .0
      .as_iden()
      .and_then(|name| Direction::from_name(&name.to_lowercase()));
    if let Some(direct) = direct {
      let unit = self.take(&mut taken);
      let is_period = taken[unit].0.as_iden().is_some_and(|name| {
        time_unit(&name.to_lowercase())
          .and_then(Period::from_interval)
          .is_some_and(|p| p == period)
      });
      if is_period {
        return Some(direct.offset());
      }
    }
    self.restore(taken);
    None
  }

  /// Timezone with an optional offset (e.g. "UTC", "UTC+07:00", "gmt-2:30")
  fn zone_with_offset(&mut self, zone: Zone) -> DateResult<Zone> {
    let t = self.get();
    let Some(sign @ ('+' | '-')) = t.as_char() else {
      self.unget(t);
      return Ok(zone);
    };
    // A relative offset (e.g. "UTC +8 years")
    match self.try_relative(t)? {
      Ok(skip) => {
        self.skips.push(skip);
        Ok(zone)
      }
      Err(_) => match zone {
        Zone::Offset(base) => Ok(Zone::Offset(base + self.offset(sign)?)),
        Zone::Region(_) => date_result("unexpected offset after timezone"),
      },
    }
  }

  /// A date starting with a name (e.g. "friday", "march 3", "nov14", "week")
  fn named_date(&mut self, name: &str) -> DateResult<Option<DateSpec>> {
    // A number written as words (e.g. "a week ago", "twenty-fourth of May")
    match self.word_number(name) {
      Some(WordNumber::Cardinal(n)) => {
        let t = self.get();
        // "thirteen of february"
        if is_word(&t, &["of"]) && n.fract() == 0.0 && (1.0..=31.0).contains(&n)
        {
          let month = self
            .get()
            .as_iden()
            .and_then(|month| month_name(&month.to_lowercase()));
          return match month {
            Some(month) => Ok(Some(self.day_month_spec(n as u32, month)?)),
            None => date_result("expected month name after 'of'"),
          };
        }
        let unit = t.as_iden().and_then(|unit| time_unit(&unit.to_lowercase()));
        return match unit {
          Some(unit) => self.relative(n, unit, false),
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

    match name {
      "weekend" | "weekends" => {
        return Ok(Some(DateSpec::FromName(ByName::WeekDayInWeek {
          week_day: 5,
          weeks: self.direct.offset(),
        })));
      }
      "weekday" | "workday" | "businessday" => {
        return match self.direct {
          Direction::Next | Direction::Last => {
            Ok(Some(DateSpec::FromName(ByName::BusinessDay(self.direct))))
          }
          _ => date_result("expected next or last before weekday"),
        };
      }
      _ => {}
    }

    // Direction and time unit without space (e.g. "lastmonth", "nextweek")
    if self.direct == Direction::Here {
      for prefix in ["last", "next", "this", "past"] {
        let unit = name.strip_prefix(prefix).filter(|unit| unit.len() >= 3);
        if unit.and_then(time_unit).is_some() {
          self.direct = Direction::from_name(prefix).unwrap();
          return self.named_date(unit.unwrap());
        }
      }
    }

    let (alpha, digits) = split_trailing_digits(name);
    if let Some(by_name) = ByName::from_name(alpha, self.direct) {
      // a MONTH _might_ be followed by DAY, YEAR
      if let Some(month) = by_name.as_month() {
        self.skip_char('.');
        let day = match digits {
          Some((year, 4)) => Some(DayOrYear::Year(year)),
          Some((day, _)) => Some(DayOrYear::Day(day)),
          None => self.day_of_month()?,
        };
        match day {
          Some(DayOrYear::Day(day)) => {
            return Ok(Some(self.day_month_spec(day, month)?))
          }
          Some(DayOrYear::Year(year)) => {
            return Ok(Some(DateSpec::absolute(year, month, 1)))
          }
          None => {}
        }
      }
      else if digits.is_some() {
        return date_result("unexpected number after week day");
      }
      else {
        // "Sun., March 6"
        self.skip_char('.');
        // "friday of next week", "Tuesday, next week"
        if let ByName::WeekDay(ref nd) = by_name {
          if nd.direct == Direction::Here {
            if let Some(weeks) = self.direction_unit(Period::Week) {
              return Ok(Some(DateSpec::FromName(ByName::WeekDayInWeek {
                week_day: nd.unit,
                weeks,
              })));
            }
          }
        }
      }
      return Ok(Some(DateSpec::FromName(by_name)));
    }

    // A time unit without a number (e.g. "next week", "last month", "year")
    if digits.is_none() && name.len() >= 2 {
      if let Some(unit) =
        time_unit(name).filter(|_| name.len() >= 3 || name == "mo")
      {
        let n = if self.direct == Direction::This {
          0.0
        }
        else {
          1.0
        };
        let has_article = std::mem::take(&mut self.has_article);
        let spec = self.relative(n, unit, false)?;
        // "the day" is no date, but "the day before yesterday" is
        if has_article && !self.has_suffix {
          return date_result(&format!("unexpected 'the' before {name}"));
        }
        return Ok(spec);
      }
    }

    // A timezone without a time means midnight in that timezone (e.g. "UTC")
    if self.zone.is_none() {
      if let Some(zone) = Self::timezone(name) {
        self.zone = Some(self.zone_with_offset(zone)?);
        return Ok(None);
      }
    }

    date_result("expected week day or month name")
  }

  /// Parse a part of the day after an hour
  /// (e.g. "at night", "in the morning", "tonight", "this evening")
  fn period_phrase(&mut self, first: &Token) -> Option<DayPeriod> {
    let name = first.as_iden()?.to_lowercase();
    if let Some(period) = DayPeriod::from_name(&name) {
      return Some(period);
    }
    if !matches!(name.as_str(), "at" | "in" | "this") {
      return None;
    }
    let mut taken = Vec::new();
    let mut t = self.take(&mut taken);
    if name == "in" && is_word(&taken[t].0, &["the"]) {
      t = self.take(&mut taken);
    }
    let period = taken[t].0.as_iden().and_then(DayPeriod::from_name);
    if period.is_none() {
      self.restore(taken);
    }
    period
  }

  /// A date starting with a number (e.g. "2024-04-10", "14 december", "2 days")
  fn numeric_date(
    &mut self,
    n: u32,
    len: usize,
    sign: Option<char>,
  ) -> DateResult<Option<DateSpec>> {
    let negate = |x: f64| if sign == Some('-') { -x } else { x };

    if len == 8 && sign.is_none() {
      // "19970101 +1 year"
      return Ok(Some(DateSpec::absolute(n / 10000, n / 100 % 100, n % 100)));
    }
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
          match digits {
            // Case: DAY MONTH YEAR (e.g. "14nov2022")
            Some((year, year_len)) => {
              let year = if year_len == 2 {
                expand_year(year)
              }
              else {
                year
              };
              Some(DateSpec::absolute(year, month, n))
            }
            // Case: YEAR MONTH (e.g. "2024 Aug", "2018 March 18")
            None if len == 4 => Some(self.year_month_spec(n, month)?),
            // Case: DAY MONTH (e.g. "14 december")
            None => Some(self.day_month_spec(n, month)?),
          }
        }
        else if digits.is_some() {
          return date_result("expected month or time unit");
        }
        else if is_ordinal_suffix(&name) {
          // Case: "3rd march", "1st of june", "the 3rd"
          let has_of = self.skip_word(&["of"]);
          let t = self.get();
          match t.as_iden().and_then(|s| month_name(&s.to_lowercase())) {
            Some(month) => Some(self.day_month_spec(n, month)?),
            None if !has_of => {
              self.unget(t);
              Some(DateSpec::FromName(ByName::DayOfMonth(n)))
            }
            None => return date_result("expected month name after ordinal"),
          }
        }
        else if let Some(u) = time_unit(&name) {
          // Case: NUMBER UNIT (e.g. "2 days")
          self.relative(negate(n as f64), u, sign.is_some())?
        }
        else if is_am_pm(&name) {
          // Case: "4pm" (a date may follow, e.g. "4pm tomorrow")
          self.maybe_time =
            Some((n, TimeKind::AmPm(name.eq_ignore_ascii_case("pm"))));
          None
        }
        else if let Some(period) = self.period_phrase(&t) {
          // Case: "6 in the morning", "11 at night", "11 tonight"
          if sign.is_some() || n > 12 {
            return date_result("invalid hour before part of day");
          }
          self.maybe_time = Some((period.hour(n), TimeKind::Hour));
          None
        }
        else {
          return date_result("expected month or time unit");
        }
      }
      Token::Char(ch) => match ch {
        '-' | '/' if self.peek_is(|t| t.as_iden().is_some()) => {
          // Case: "10-August-2012", "8/Oct/2012", "2024-Aug", "14-nov2022"
          let name = self.get();
          let name = name.as_iden().unwrap().to_lowercase();
          let (alpha, digits) = split_trailing_digits(&name);
          let Some(month) = month_name(alpha) else {
            return date_result("expected month name");
          };
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
            None if len == 4 => self.year_month_spec(n, month)?,
            None => self.day_month_spec(n, month)?,
          })
        }
        '-' => Some(self.iso_date(n, len)?),
        '/' => Some(self.informal_date(n, len)?),
        ':' => {
          self.maybe_time = Some((n, TimeKind::Formal));
          None
        }
        '.' => {
          // A date with dot (like "15. Jun" or "25.05.2015"),
          // decimal number (like "1.5"), or time (like "11.20")
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
              if let Some(spec) = self.dotted_date(n, len, decimal_part)? {
                return Ok(Some(spec));
              }
              let unit_token = self.get();
              let unit = unit_token
                .as_iden()
                .and_then(|name| time_unit(&name.to_lowercase()));
              match unit {
                Some(u) => {
                  // This is a decimal duration like "1.5 hours"
                  let n_float = n as f64
                    + decimal_part as f64 / 10_f64.powi(dec_len as i32);
                  self.relative(negate(n_float), u, sign.is_some())?
                }
                None => {
                  // Treat as informal time like "11.20" or "11.20pm"
                  let is_time = dec_len == 2
                    && (unit_token.finished()
                      || unit_token.as_iden().is_some_and(is_am_pm)
                      || matches!(unit_token.as_char(), Some('+' | '-')));
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
      Token::Int(_) if len == 4 && sign.is_none() => {
        // Case: "2014 12 28"
        let month = t.to_int_result::<u32>()?;
        let day = self.get_int()?;
        Some(DateSpec::absolute(n, month, day))
      }
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
    match t {
      Token::Iden(ref name) if is_am_pm(name) => {
        Ok(TimeSpec::new(DateParser::am_pm(name, hour)?, min, 0, 0))
      }
      // Timezone offset (e.g. "23.00+14")
      Token::Char(ch @ ('+' | '-')) => {
        if let Ok(skip) = self.try_relative(Token::Char(ch))? {
          self.skips.push(skip);
          return Ok(TimeSpec::new(hour, min, 0, 0));
        }
        let offset = self.offset(ch)?;
        Ok(TimeSpec::new_with_zone(
          hour,
          min,
          0,
          Zone::Offset(offset),
          0,
        ))
      }
      t => {
        self.unget(t);
        Ok(TimeSpec::new(hour, min, 0, 0))
      }
    }
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
    if let Some((hour, kind)) = self.maybe_time.take() {
      return Ok(Some(match kind {
        TimeKind::Formal => self.formal_time(hour)?,
        TimeKind::Informal => self.informal_time(hour)?,
        TimeKind::AmPm(is_pm) => TimeSpec::new(
          DateParser::am_pm(if is_pm { "pm" } else { "am" }, hour)?,
          0,
          0,
          0,
        ),
        TimeKind::Hour => TimeSpec::new(hour, 0, 0, 0),
      }));
    }

    // no lookahead...
    let mut t = self.get();
    // Comma between date and time (e.g. "07-27-2022, 02:00 AM")
    if t.as_char() == Some(',') {
      let next = self.get();
      let next_len = self.last_len;
      let is_time = next.is_integer()
        || next.as_iden().is_some_and(|name| {
          DayPeriod::from_name(name).is_some()
            || TIME_CONNECTORS.iter().any(|w| name.eq_ignore_ascii_case(w))
        });
      if !is_time {
        self.unget_len(next, next_len);
        self.unget_len(t, 0);
        return Ok(None);
      }
      t = next;
    }

    // Part of the day (e.g. "tomorrow morning", "at noon", "this afternoon")
    if self.period.is_none() {
      if let Some(period) = self.period_phrase(&t) {
        self.period = Some(period);
        t = self.get();
      }
    }

    // Skip connector words (e.g. "at") or ISO 8601 separator
    let mut has_connector = false;
    while is_word(&t, &TIME_CONNECTORS) {
      has_connector = true;
      t = self.get();
      if self.period.is_none() {
        if let Some(period) = self.period_phrase(&t) {
          self.period = Some(period);
          t = self.get();
          has_connector = false;
        }
      }
    }
    if !t.is_integer() {
      if has_connector {
        return date_result("expected time after 'at'");
      }
      self.unget(t);
      let period = self.period;
      return Ok(period.map(|p| TimeSpec::new(p.default_hour(), 0, 0, 0)));
    }

    let hour_len = self.last_len;
    let hour = t.to_int_result::<u32>()?;
    let sep = self.get();
    let mut time = match sep {
      Token::Char(':') => self.formal_time(hour)?,
      Token::Char('.') => self.informal_time(hour)?,
      Token::Iden(ref name) if is_am_pm(name) => {
        TimeSpec::new(DateParser::am_pm(name, hour)?, 0, 0, 0)
      }
      // A bare hour (e.g. "tomorrow at 7", "tonight at 8")
      sep
        if (has_connector || self.period.is_some())
          && hour_len <= 2
          && hour <= 24
          && sep
            .as_iden()
            .is_none_or(|name| time_unit(&name.to_lowercase()).is_none()) =>
      {
        self.unget(sep);
        TimeSpec::new(hour % 24, 0, 0, 0)
      }
      sep => {
        // Not a time (e.g. a relative offset like "1 year")
        self.unget(sep);
        self.unget_len(Token::Int(hour as i64), hour_len);
        return Ok(None);
      }
    };
    if let Some(period) = self.period {
      // Noon and midnight are already exact times
      if matches!(period, DayPeriod::Noon | DayPeriod::Midnight) {
        return date_result("time given twice");
      }
      time.hour = period.hour(time.hour);
    }
    Ok(Some(time))
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
    let t = self.get();
    if is_word(&t, &["ago"]) {
      if sign.is_some() {
        return date_result("a signed duration cannot be combined with 'ago'");
      }
      n = -n;
    }
    else if !is_word(&t, &["hence", "later"]) {
      self.unget(t);
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
    let mut in_parens = false;
    loop {
      // Time after a date (e.g. "Thu Oct 26 11:00:09 2023")
      if time.is_none() && date.is_some() {
        if let Some(ts) = self.parse_time()? {
          *time = Some(ts);
          continue;
        }
      }

      let t = self.get();
      if t.finished() {
        return Ok(());
      }

      // Parentheses around a timezone (e.g. "10:30 (GMT-2:30)")
      match t.as_char() {
        Some('(') if time.is_some() => {
          in_parens = true;
          continue;
        }
        Some(')') if in_parens => {
          in_parens = false;
          continue;
        }
        _ => {}
      }

      if let Some(name) = t.as_iden() {
        // Timezone abbreviation (e.g. "5pm EST", "11 am GMT+0900")
        if let Some(zone) = Self::timezone(name) {
          match time.as_mut() {
            Some(ts) if ts.zone.is_none() => {
              ts.zone = Some(self.zone_with_offset(zone)?);
              continue;
            }
            // Repeated timezone (e.g. "GMT+0900 (JST)")
            Some(_) if in_parens => continue,
            // Timezone after a date (e.g. "September 16, 2020, EST")
            None
              if matches!(
                date,
                Some(DateSpec::Absolute(_) | DateSpec::FromName(_))
              ) =>
            {
              let zone = self.zone_with_offset(zone)?;
              *time = Some(TimeSpec::new_with_zone(0, 0, 0, zone, 0));
              continue;
            }
            // Timezones don't change relative dates (e.g. "in 1 hour GMT")
            None if matches!(date, Some(DateSpec::Relative(_))) => {
              self.zone_with_offset(zone)?;
              continue;
            }
            _ => {}
          }
        }
        if time.is_some() {
          if is_am_pm(name) {
            return date_result(&format!("unexpected {name}"));
          }
          // "11 tonight", "8pm tonight"
          if name.eq_ignore_ascii_case("tonight") {
            continue;
          }
        }
        // Redundant week day after a full date (e.g. "2026-08-17 Wed")
        if matches!(date, Some(DateSpec::Absolute(_)))
          && week_day(&name.to_lowercase()).is_some()
        {
          continue;
        }
        let lowercase = name.to_lowercase();
        // Relative week, month, or year (e.g. "Wed 2026-08-17 next week")
        if let Some(direct) = Direction::from_name(&lowercase) {
          let unit = self.get();
          let unit_len = self.last_len;
          match unit.as_iden().and_then(|u| time_unit(&u.to_lowercase())) {
            Some(unit) => {
              self.skips.push(Skip {
                unit,
                skip: direct.offset() as f64,
              });
              continue;
            }
            None => self.unget_len(unit, unit_len),
          }
        }
        // Don't silently ignore unknown timezones (e.g. "12:00 FOO")
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
      let t_len = self.last_len;

      // Year after a time (e.g. "Jul 18 06:14:49 2024")
      if t.is_integer() && t_len == 4 {
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
        self.unget_len(t, t_len);
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

    // A timezone without a time (e.g. "UTC", "UTC +8 years")
    if let Some(zone) = self.zone {
      match time.as_mut() {
        Some(ts) if ts.zone.is_none() => ts.zone = Some(zone),
        Some(_) => return date_result("timezone given twice"),
        None if date.is_none() && !self.skips.is_empty() => {
          date = Some(DateSpec::skip(time_unit("day").unwrap(), 0.0));
        }
        None => time = Some(TimeSpec::new_with_zone(0, 0, 0, zone, 0)),
      }
    }

    // A plain "midnight" means the end of today
    if date.is_none() && self.period == Some(DayPeriod::Midnight) {
      date = Some(DateSpec::skip(time_unit("day").unwrap(), 1.0));
    }

    let skips = std::mem::take(&mut self.skips);
    Ok(DateTimeSpec { date, time, skips })
  }
}
