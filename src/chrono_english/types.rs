#![allow(deprecated)]
use chrono::prelude::*;
use chrono::Duration;
use chrono::LocalResult;

// implements next/last direction in expressions like 'next friday' and 'last 4 july'
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
  Next,
  Last,
  Here,
  /// The current week, month, or year (e.g. "this friday", "this month")
  This,
}

impl Direction {
  pub fn from_name(s: &str) -> Option<Direction> {
    use Direction::*;
    match s {
      "next" | "coming" | "upcoming" => Some(Next),
      "last" | "past" | "previous" | "prev" => Some(Last),
      "this" => Some(This),
      _ => None,
    }
  }

  /// Offset relative to the current week, month, or year
  pub fn offset(self) -> i64 {
    match self {
      Direction::Next => 1,
      Direction::Last => -1,
      Direction::Here | Direction::This => 0,
    }
  }
}

// this is a day-month with direction, like 'next 10 Dec'
#[derive(Debug)]
pub struct YearDate {
  pub direct: Direction,
  pub month: u32,
  pub day: u32,
}

// for expressions like 'friday' and 'July' modifiable with next/last
#[derive(Debug)]
pub struct NamedDate {
  pub direct: Direction,
  pub unit: u32,
}

impl NamedDate {
  pub fn new(direct: Direction, unit: u32) -> NamedDate {
    NamedDate { direct, unit }
  }
}

// all expressions modifiable with next/last; 'fri', 'jul', '5 may'.
#[derive(Debug)]
pub enum ByName {
  WeekDay(NamedDate),
  MonthName(NamedDate),
  DayMonth(YearDate),
  /// Week day in a week relative to the current one
  /// (e.g. "friday of next week", "this tuesday")
  WeekDayInWeek {
    week_day: u32,
    weeks: i64,
  },
  /// Next or previous Monday to Friday (e.g. "next weekday")
  BusinessDay(Direction),
  /// Day of the current month (e.g. "the 3rd")
  DayOfMonth(u32),
}

fn add_days<Tz: TimeZone>(
  base: DateTime<Tz>,
  days: i64,
) -> Option<DateTime<Tz>> {
  base.checked_add_signed(Duration::days(days))
}

//fn next_last_direction<Tz: TimeZone>(date: Date<Tz>, base: Date<Tz>, direct: Direction) -> Option<i32> {

fn next_last_direction<T: PartialOrd>(
  date: &T,
  base: &T,
  direct: Direction,
) -> Option<i32> {
  let mut res = None;
  if date > base {
    if direct == Direction::Last {
      res = Some(-1);
    }
  }
  else if date < base && direct == Direction::Next {
    res = Some(1)
  }
  res
}

impl ByName {
  pub fn from_name(s: &str, direct: Direction) -> Option<ByName> {
    Some(if let Some(wd) = week_day(s) {
      if direct == Direction::This {
        ByName::WeekDayInWeek {
          week_day: wd,
          weeks: 0,
        }
      }
      else {
        ByName::WeekDay(NamedDate::new(direct, wd))
      }
    }
    else {
      let mn = month_name(s)?;
      ByName::MonthName(NamedDate::new(direct, mn))
    })
  }

  pub fn as_month(&self) -> Option<u32> {
    match *self {
      ByName::MonthName(ref nd) => Some(nd.unit),
      _ => None,
    }
  }

  pub fn from_day_month(day: u32, month: u32, direct: Direction) -> ByName {
    ByName::DayMonth(YearDate { direct, day, month })
  }

  pub fn to_date_time<Tz: TimeZone>(
    &self,
    base: DateTime<Tz>,
    ts: TimeSpec,
    american: bool,
  ) -> Option<DateTime<Tz>>
  where
    <Tz as TimeZone>::Offset: Copy,
  {
    let this_year = base.year();
    match self {
      ByName::WeekDay(ref nd) => {
        let this_day = base.weekday().num_days_from_monday() as i64;
        let that_day = nd.unit as i64;
        let diff_days = that_day - this_day;
        let shift_weeks = match nd.direct {
          // A plain 'Friday' means the upcoming Friday.
          // On a Friday, it's today unless the time has already passed.
          Direction::Here => {
            let has_passed =
              NaiveTime::from_hms_opt(ts.hour, ts.min, ts.sec)? < base.time();
            if diff_days < 0 || (diff_days == 0 && has_passed) {
              1
            }
            else {
              0
            }
          }
          // An explicit 'next Friday' has dialect-dependent meaning!
          // In US English, it's just the next Friday,
          // but in UK English, it means 'Friday of next week'.
          Direction::Next => {
            if american {
              if diff_days <= 0 {
                1
              }
              else {
                0
              }
            }
            else {
              1
            }
          }
          Direction::Last => {
            if diff_days >= 0 {
              -1
            }
            else {
              0
            }
          }
          Direction::This => 0,
        };
        let date = add_days(base, diff_days + 7 * shift_weeks)?;
        ts.to_date_time(date.date())
      }
      ByName::WeekDayInWeek { week_day, weeks } => {
        let this_day = base.weekday().num_days_from_monday() as i64;
        let diff_days = *week_day as i64 - this_day + 7 * weeks;
        ts.to_date_time(add_days(base, diff_days)?.date())
      }
      ByName::BusinessDay(direct) => {
        let step = if *direct == Direction::Last { -1 } else { 1 };
        let mut date = add_days(base, step)?;
        while date.weekday().num_days_from_monday() >= 5 {
          date = add_days(date, step)?;
        }
        ts.to_date_time(date.date())
      }
      ByName::DayOfMonth(day) => ts.to_date_time(
        base
          .timezone()
          .ymd_opt(this_year, base.month(), *day)
          .single()?,
      ),
      ByName::MonthName(ref nd) => {
        let mut date =
          base.timezone().ymd_opt(this_year, nd.unit, 1).single()?;
        if let Some(correct) =
          next_last_direction(&date, &base.date(), nd.direct)
        {
          date = base
            .timezone()
            .ymd_opt(this_year + correct, nd.unit, 1)
            .single()?;
        }
        ts.to_date_time(date)
      }
      ByName::DayMonth(ref yd) => {
        let mut date = base
          .timezone()
          .ymd_opt(this_year, yd.month, yd.day)
          .single()?;
        if let Some(correct) =
          next_last_direction(&date, &base.date(), yd.direct)
        {
          date = base
            .timezone()
            .ymd_opt(this_year + correct, yd.month, yd.day)
            .single()?;
        }
        ts.to_date_time(date)
      }
    }
  }
}

#[derive(Debug)]
pub struct AbsDate {
  pub year: i32,
  pub month: u32,
  pub day: u32,
}

impl AbsDate {
  pub fn to_date<Tz: TimeZone>(&self, base: DateTime<Tz>) -> Option<Date<Tz>> {
    base
      .timezone()
      .ymd_opt(self.year, self.month, self.day)
      .single()
  }
}

/// A generic amount of time, in either seconds, days, or months.
///
/// This way, a user can decide how they want to treat days (which do
/// not always have the same number of seconds) or months (which do not always
/// have the same number of days).
//
// Skipping a given number of time units.
// The subtlety is that we treat duration as seconds until we get
// to months, where we want to preserve dates. So adding a month to
// '5 May' gives '5 June'. Adding a month to '30 Jan' gives 'Feb 28' or 'Feb 29'
// depending on whether this is a leap year.
#[derive(Debug, Hash, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum Interval {
  Seconds(i32),
  Days(i32),
  Months(i32),
}

#[derive(Debug)]
pub struct Skip {
  pub unit: Interval,
  pub skip: f64,
}

impl Skip {
  pub fn to_date_time<Tz: TimeZone>(
    &self,
    base: DateTime<Tz>,
    ts: TimeSpec,
  ) -> Option<DateTime<Tz>> {
    Some(match self.unit {
      Interval::Seconds(secs) => base.checked_add_signed(Duration::seconds(
        ((secs as f64) * self.skip) as i64,
      ))?,
      Interval::Days(days) => {
        let secs = 60 * 60 * 24 * days;
        let date = base.checked_add_signed(Duration::seconds(
          ((secs as f64) * self.skip) as i64,
        ))?;
        if !ts.empty() {
          ts.to_date_time(date.date())?
        }
        else {
          date
        }
      }
      Interval::Months(mm) => {
        let (year, month, mut day) =
          (base.year(), (base.month() - 1) as i32, base.day());
        let delta = ((mm as f64) * self.skip) as i32;
        // Our new month number, which may run over to other years
        let month = month + delta;
        let (year, month) =
          (year + month.div_euclid(12), month.rem_euclid(12) + 1);
        // let chrono work out if the result makes sense
        let mut date =
          base.timezone().ymd_opt(year, month as u32, day).single();
        // dud dates like Feb 30 may result, so we back off...
        while date.is_none() {
          day -= 1;
          if day == 0 || day < 28 {
            // sanity check...
            eprintln!("fkd date");
            return None;
          }
          date = base.timezone().ymd_opt(year, month as u32, day).single();
        }
        let date = date.unwrap();
        if ts.empty() {
          // keep the time of day like for seconds and days
          date.and_time(base.time())?
        }
        else {
          ts.to_date_time(date)?
        }
      }
    })
  }

  pub fn to_interval(&self) -> Interval {
    use Interval::*;

    match self.unit {
      Seconds(s) => Seconds(((s as f64) * self.skip) as i32),
      Days(d) => Days(((d as f64) * self.skip) as i32),
      Months(m) => Months(((m as f64) * self.skip) as i32),
    }
  }
}

/// Calendar period whose last day can be requested (e.g. "end of the month")
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Period {
  Week,
  Month,
  Year,
}

impl Period {
  pub fn from_interval(unit: Interval) -> Option<Period> {
    match unit {
      Interval::Days(7) => Some(Period::Week),
      Interval::Months(1) => Some(Period::Month),
      Interval::Months(12) => Some(Period::Year),
      _ => None,
    }
  }

  /// Last day of the period containing the given date
  fn last_day<Tz: TimeZone>(self, date: Date<Tz>) -> Option<Date<Tz>> {
    let tz = date.timezone();
    match self {
      Period::Week => {
        let days_left = 6 - date.weekday().num_days_from_monday() as i64;
        date.checked_add_signed(Duration::days(days_left))
      }
      Period::Month => {
        let (year, month) = if date.month() == 12 {
          (date.year() + 1, 1)
        }
        else {
          (date.year(), date.month() + 1)
        };
        tz.ymd_opt(year, month, 1).single()?.pred_opt()
      }
      Period::Year => tz.ymd_opt(date.year(), 12, 31).single(),
    }
  }
}

#[derive(Debug)]
pub enum DateSpec {
  Absolute(AbsDate), // Y M D (e.g. 2018-06-02, 4 July 2017)
  Relative(Skip),    // n U (e.g. 2min, 3 years ago, -2d)
  FromName(ByName),  // (e.g. 'next fri', 'jul')
  /// Last day of the period containing a date (e.g. "end of next month")
  EndOf(Box<DateSpec>, Period),
}

impl DateSpec {
  pub fn absolute(year: u32, month: u32, day: u32) -> DateSpec {
    DateSpec::Absolute(AbsDate {
      year: year as i32,
      month,
      day,
    })
  }

  pub fn from_day_month(day: u32, month: u32, direct: Direction) -> DateSpec {
    DateSpec::FromName(ByName::from_day_month(day, month, direct))
  }

  pub fn skip(unit: Interval, skip: f64) -> DateSpec {
    DateSpec::Relative(Skip { unit, skip })
  }

  pub fn to_date_time<Tz: TimeZone>(
    &self,
    base: DateTime<Tz>,
    ts: TimeSpec,
    american: bool,
  ) -> Option<DateTime<Tz>>
  where
    Tz::Offset: Copy,
  {
    use DateSpec::*;
    match self {
      Absolute(ref ad) => ts.to_date_time(ad.to_date(base)?),
      Relative(ref skip) => skip.to_date_time(base, ts), // might need time
      FromName(ref byname) => byname.to_date_time(base, ts, american),
      EndOf(ref spec, period) => {
        let date = spec.to_date_time(base, TimeSpec::new_empty(), american)?;
        ts.to_date_time(period.last_day(date.date())?)
      }
    }
  }
}

/// Timezone of a time
#[derive(Debug, Clone, Copy)]
pub enum Zone {
  /// Fixed offset from UTC in seconds (e.g. "+02:00", "CET")
  Offset(i64),
  /// Region whose offset depends on the date (e.g. "ET" for US Eastern Time)
  Region(chrono_tz::Tz),
}

#[derive(Debug)]
pub struct TimeSpec {
  pub hour: u32,
  pub min: u32,
  pub sec: u32,
  pub empty: bool,
  pub zone: Option<Zone>,
  pub microsec: u32,
}

impl TimeSpec {
  pub fn new(hour: u32, min: u32, sec: u32, microsec: u32) -> TimeSpec {
    TimeSpec {
      hour,
      min,
      sec,
      empty: false,
      zone: None,
      microsec,
    }
  }

  pub fn new_with_zone(
    hour: u32,
    min: u32,
    sec: u32,
    zone: Zone,
    microsec: u32,
  ) -> TimeSpec {
    TimeSpec {
      hour,
      min,
      sec,
      empty: false,
      zone: Some(zone),
      microsec,
    }
  }

  pub fn new_empty() -> TimeSpec {
    TimeSpec {
      hour: 0,
      min: 0,
      sec: 0,
      empty: true,
      zone: None,
      microsec: 0,
    }
  }

  pub fn empty(&self) -> bool {
    self.empty
  }

  pub fn to_date_time<Tz: TimeZone>(
    &self,
    date: Date<Tz>,
  ) -> Option<DateTime<Tz>> {
    match self.zone {
      None => {
        date.and_hms_micro_opt(self.hour, self.min, self.sec, self.microsec)
      }
      Some(Zone::Offset(offs)) => {
        let dt = date.and_hms_micro_opt(
          self.hour,
          self.min,
          self.sec,
          self.microsec,
        )?;
        let zoffset = dt.offset().clone();
        let tstamp =
          dt.timestamp() - offs + zoffset.fix().local_minus_utc() as i64;
        let nd =
          NaiveDateTime::from_timestamp_opt(tstamp, 1000 * self.microsec)?;
        Some(DateTime::from_utc(nd, zoffset))
      }
      Some(Zone::Region(region)) => {
        let naive = date.naive_local().and_hms_micro_opt(
          self.hour,
          self.min,
          self.sec,
          self.microsec,
        )?;
        let local = match region.from_local_datetime(&naive) {
          LocalResult::Single(dt) => dt,
          // Repeated hour when DST ends: use the first one
          LocalResult::Ambiguous(earliest, _) => earliest,
          // Skipped hour when DST starts: use the time after the gap
          LocalResult::None => region
            .from_local_datetime(&(naive + Duration::hours(1)))
            .earliest()?,
        };
        Some(local.with_timezone(&date.timezone()))
      }
    }
  }
}

#[derive(Debug)]
pub struct DateTimeSpec {
  pub date: Option<DateSpec>,
  pub time: Option<TimeSpec>,
  /// Relative offsets applied after the date and time (e.g. "+1 year")
  pub skips: Vec<Skip>,
}

/// Index of the name which `s` is an abbreviation of (at least 3 chars)
fn name_prefix_index(names: &[&str], s: &str) -> Option<u32> {
  if s.len() < 3 {
    return None;
  }
  names
    .iter()
    .position(|name| name.starts_with(s))
    .map(|pos| pos as u32)
}

// same as chrono's 'count days from monday' convention
pub fn week_day(s: &str) -> Option<u32> {
  const WEEK_DAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
  ];
  name_prefix_index(&WEEK_DAYS, s)
}

pub fn month_name(s: &str) -> Option<u32> {
  const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
  ];
  name_prefix_index(&MONTHS, s).map(|i| i + 1)
}

pub fn time_unit(s: &str) -> Option<Interval> {
  use Interval::*;
  // Abbreviations which aren't prefixes of the full name
  match s {
    "mo" | "mos" | "mth" | "mths" => return Some(Months(1)),
    "hr" | "hrs" => return Some(Seconds(60 * 60)),
    "wk" | "wks" => return Some(Days(7)),
    "yr" | "yrs" => return Some(Months(12)),
    "q" | "qr" | "qtr" | "qtrs" => return Some(Months(3)),
    "fortnight" | "fortnights" => return Some(Days(14)),
    _ => {}
  }
  let name = if s.len() < 3 {
    match &s[0..1] {
      "s" => "sec",
      "m" => "min",
      "h" => "hou",
      "w" => "wee",
      "d" => "day",
      "y" => "yea",
      _ => return None,
    }
  }
  else {
    &s[0..3]
  };
  Some(match name {
    "sec" => Seconds(1),
    "min" => Seconds(60),
    "hou" => Seconds(60 * 60),
    "day" => Days(1),
    "wee" => Days(7),
    "mon" => Months(1),
    "qua" => Months(3),
    "yea" => Months(12),
    _ => return None,
  })
}
