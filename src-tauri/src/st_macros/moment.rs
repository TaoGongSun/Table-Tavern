//! 時間巨集用到的 moment 2.30.1（ST 06bde939 釘的版本，預設 en 語系）行為：時鐘、曆法、`format`、
//! `duration().humanize()`。網頁版直接用 moment，這裡照它的原始碼移植；字串解析在 moment_parse.rs。

use std::sync::LazyLock;

use regex::Regex;

use super::js_value::number_to_string;
use crate::world_info::js_semantics::js_round;

pub(super) const MS_PER_DAY: f64 = 864e5;
/// JS Date 的範圍（TimeClip）
pub(super) const MAX_TIME: f64 = 8.64e15;
/// JS `\s` 的字元類別內容
pub(super) const JS_SPACE: &str = r"\t\n\x0B\x0C\r \u{A0}\u{1680}\u{2000}-\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}";

/// 時鐘與本地時區（網頁版是瀏覽器的 `new Date()` 與時區）。
pub trait Clock {
    /// 現在的 epoch 毫秒
    fn now_ms(&self) -> f64;
    /// 某個時刻的本地時區相對 UTC 的分鐘數（東正西負）
    fn offset_minutes(&self, epoch_ms: f64) -> f64;
}

/// 系統時鐘與本地時區。
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> f64 {
        chrono::Utc::now().timestamp_millis() as f64
    }

    fn offset_minutes(&self, epoch_ms: f64) -> f64 {
        use chrono::{Local, Offset, TimeZone};
        match Local.timestamp_millis_opt(epoch_ms as i64) {
            chrono::LocalResult::Single(time) | chrono::LocalResult::Ambiguous(time, _) => {
                f64::from(time.offset().fix().local_minus_utc()) / 60.0
            }
            chrono::LocalResult::None => 0.0,
        }
    }
}

/// 固定時刻、固定時區（測試與對拍用）。
pub struct FixedClock {
    pub now_ms: f64,
    pub offset_minutes: f64,
}

impl Clock for FixedClock {
    fn now_ms(&self) -> f64 {
        self.now_ms
    }

    fn offset_minutes(&self, _epoch_ms: f64) -> f64 {
        self.offset_minutes
    }
}

/// 一個 moment：時刻＋顯示用的時區（本地或 `utcOffset` 指定）。
struct Moment {
    ms: f64,
    offset: f64,
    is_utc: bool,
}

/// 牆上時間拆欄
struct Wall {
    year: i64,
    /// 0–11
    month: i64,
    date: i64,
    /// 0＝星期日
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    ms: i64,
    /// 1970-01-01 起的日數
    days: i64,
}

pub(super) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    // month 1–12
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub(super) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// JS `MakeDay`＋`MakeDate`：月份可以溢出（`Date.UTC(y, 13, 1)`）、日可以超出或為負。
pub(super) fn make_date(
    year: i64,
    month0: i64,
    date: i64,
    hour: i64,
    minute: i64,
    second: i64,
    ms: i64,
) -> f64 {
    let year = year + month0.div_euclid(12);
    let month = month0.rem_euclid(12) + 1;
    let days = days_from_civil(year, month, 1) + date - 1;
    days as f64 * MS_PER_DAY + (hour * 3_600_000 + minute * 60_000 + second * 1000 + ms) as f64
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(super) fn days_in_year(year: i64) -> i64 {
    if is_leap(year) {
        366
    } else {
        365
    }
}

pub(super) fn days_in_month(year: i64, month0: i64) -> i64 {
    let month = month0.rem_euclid(12);
    let year = year + (month0 - month) / 12;
    if month == 1 {
        if is_leap(year) {
            29
        } else {
            28
        }
    } else {
        31 - ((month % 7) % 2)
    }
}

/// moment `firstWeekOffset`
fn first_week_offset(year: i64, dow: i64, doy: i64) -> i64 {
    let fwd = 7 + dow - doy;
    let weekday = (days_from_civil(year, 1, fwd) + 4).rem_euclid(7);
    let fwdlw = (7 + weekday - dow) % 7;
    -fwdlw + fwd - 1
}

pub(super) fn weeks_in_year(year: i64, dow: i64, doy: i64) -> i64 {
    let offset = first_week_offset(year, dow, doy);
    let next = first_week_offset(year + 1, dow, doy);
    (days_in_year(year) - offset + next) / 7
}

/// moment `weekOfYear` → (週, 週年)
fn week_of_year(year: i64, day_of_year: i64, dow: i64, doy: i64) -> (i64, i64) {
    let offset = first_week_offset(year, dow, doy);
    let week = (day_of_year - offset - 1).div_euclid(7) + 1;
    if week < 1 {
        (week + weeks_in_year(year - 1, dow, doy), year - 1)
    } else if week > weeks_in_year(year, dow, doy) {
        (week - weeks_in_year(year, dow, doy), year + 1)
    } else {
        (week, year)
    }
}

/// moment `dayOfYearFromWeeks` → (年, 年中第幾天)
pub(super) fn day_of_year_from_weeks(
    year: i64,
    week: i64,
    weekday: i64,
    dow: i64,
    doy: i64,
) -> (i64, i64) {
    let local_weekday = (7 + weekday - dow) % 7;
    let day_of_year = 1 + 7 * (week - 1) + local_weekday + first_week_offset(year, dow, doy);
    if day_of_year <= 0 {
        (year - 1, days_in_year(year - 1) + day_of_year)
    } else if day_of_year > days_in_year(year) {
        (year + 1, day_of_year - days_in_year(year))
    } else {
        (year, day_of_year)
    }
}

impl Moment {
    fn wall(&self) -> Wall {
        let wall = self.ms + self.offset * 60_000.0;
        let days = (wall / MS_PER_DAY).floor() as i64;
        let tod = (wall - days as f64 * MS_PER_DAY) as i64;
        let (year, month, date) = civil_from_days(days);
        Wall {
            year,
            month: month - 1,
            date,
            day: (days + 4).rem_euclid(7),
            hour: tod / 3_600_000,
            minute: tod / 60_000 % 60,
            second: tod / 1000 % 60,
            ms: tod % 1000,
            days,
        }
    }

    fn valid(&self) -> bool {
        self.ms.is_finite()
            && self.ms.abs() <= MAX_TIME
            && (self.ms + self.offset * 60_000.0).abs() <= MAX_TIME
    }
}

fn zero_fill(number: f64, length: usize, force_sign: bool) -> String {
    let digits = number_to_string(number.abs());
    let sign = if number >= 0.0 {
        if force_sign {
            "+"
        } else {
            ""
        }
    } else {
        "-"
    };
    format!(
        "{sign}{}{digits}",
        "0".repeat(length.saturating_sub(digits.len()))
    )
}

fn ordinal(number: i64) -> String {
    let b = number % 10;
    let suffix = if (number % 100) / 10 == 1 {
        "th"
    } else {
        match b {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{number}{suffix}")
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const WEEKDAYS_MIN: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

static FORMAT_TOKENS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(\[[^\[]*\])|(\\)?([Hh]mm(ss)?|Mo|MM?M?M?|Do|DDDo|DD?D?D?|ddd?d?|do?|w[o|w]?|W[o|W]?|Qo?|N{1,5}|YYYYYY|YYYYY|YYYY|YY|y{2,4}|yo?|gg(ggg?)?|GG(GGG?)?|e|E|a|A|hh?|HH?|kk?|mm?|ss?|S{1,9}|x|X|zz?|ZZ?|[^\n\r\u{2028}\u{2029}])",
    )
    .expect("moment formattingTokens")
});

static LOCAL_FORMAT_TOKENS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\[[^\[]*\])|(\\)?(LTS|LT|LL?L?L?|l{1,4})").expect("moment localFormattingTokens")
});

fn long_date_format(key: &str) -> Option<&'static str> {
    Some(match key {
        "LTS" => "h:mm:ss A",
        "LT" => "h:mm A",
        "L" => "MM/DD/YYYY",
        "LL" => "MMMM D, YYYY",
        "LLL" => "MMMM D, YYYY h:mm A",
        "LLLL" => "dddd, MMMM D, YYYY h:mm A",
        "l" => "M/D/YYYY",
        "ll" => "MMM D, YYYY",
        "lll" => "MMM D, YYYY h:mm A",
        "llll" => "ddd, MMM D, YYYY h:mm A",
        _ => return None,
    })
}

fn expand_format(format: &str) -> String {
    let mut format = format.to_owned();
    for _ in 0..6 {
        if !LOCAL_FORMAT_TOKENS.is_match(&format) {
            break;
        }
        format = LOCAL_FORMAT_TOKENS
            .replace_all(&format, |caps: &regex::Captures| {
                let whole = &caps[0];
                long_date_format(whole).unwrap_or(whole).to_owned()
            })
            .into_owned();
    }
    format
}

fn token_value(token: &str, m: &Moment, w: &Wall) -> Option<String> {
    let num = |value: i64| number_to_string(value as f64);
    let pad = |value: i64, length: usize| zero_fill(value as f64, length, false);
    let day_of_year = w.days - days_from_civil(w.year, 1, 1) + 1;
    let (week, week_year) = week_of_year(w.year, day_of_year, 0, 6);
    let (iso_week, iso_week_year) = week_of_year(w.year, day_of_year, 1, 4);
    let iso_weekday = if w.day == 0 { 7 } else { w.day };
    let h12 = if w.hour % 12 == 0 { 12 } else { w.hour % 12 };
    let quarter = (w.month + 1 + 2) / 3;
    let ad = (w.year, w.month, w.date) >= (1, 0, 1);
    let era_year = if ad { w.year } else { 1 - w.year };
    let offset = js_round(m.offset);
    let zone = |separator: &str| {
        let (sign, abs) = if offset < 0.0 {
            ("-", -offset)
        } else {
            ("+", offset)
        };
        let abs = abs as i64;
        format!("{sign}{}{separator}{}", pad(abs / 60, 2), pad(abs % 60, 2))
    };
    Some(match token {
        "M" => num(w.month + 1),
        "MM" => pad(w.month + 1, 2),
        "Mo" => ordinal(w.month + 1),
        "MMM" => MONTHS[w.month as usize][..3].to_owned(),
        "MMMM" => MONTHS[w.month as usize].to_owned(),
        "Y" => {
            if w.year <= 9999 {
                pad(w.year, 4)
            } else {
                format!("+{}", w.year)
            }
        }
        "YY" => pad(w.year % 100, 2),
        "YYYY" => pad(w.year, 4),
        "YYYYY" => pad(w.year, 5),
        "YYYYYY" => zero_fill(w.year as f64, 6, true),
        "w" => num(week),
        "ww" => pad(week, 2),
        "wo" => ordinal(week),
        "W" => num(iso_week),
        "WW" => pad(iso_week, 2),
        "Wo" => ordinal(iso_week),
        "gg" => pad(week_year % 100, 2),
        "gggg" => pad(week_year, 4),
        "ggggg" => pad(week_year, 5),
        "GG" => pad(iso_week_year % 100, 2),
        "GGGG" => pad(iso_week_year, 4),
        "GGGGG" => pad(iso_week_year, 5),
        "Q" => num(quarter),
        "Qo" => ordinal(quarter),
        "D" => num(w.date),
        "DD" => pad(w.date, 2),
        "Do" => ordinal(w.date),
        "DDD" => num(day_of_year),
        "DDDD" => pad(day_of_year, 3),
        "DDDo" => ordinal(day_of_year),
        "d" | "e" => num(w.day),
        "do" => ordinal(w.day),
        "dd" => WEEKDAYS_MIN[w.day as usize].to_owned(),
        "ddd" => WEEKDAYS[w.day as usize][..3].to_owned(),
        "dddd" => WEEKDAYS[w.day as usize].to_owned(),
        "E" => num(iso_weekday),
        "H" => num(w.hour),
        "HH" => pad(w.hour, 2),
        "h" => num(h12),
        "hh" => pad(h12, 2),
        "k" => num(if w.hour == 0 { 24 } else { w.hour }),
        "kk" => pad(if w.hour == 0 { 24 } else { w.hour }, 2),
        "hmm" => format!("{h12}{}", pad(w.minute, 2)),
        "hmmss" => format!("{h12}{}{}", pad(w.minute, 2), pad(w.second, 2)),
        "Hmm" => format!("{}{}", w.hour, pad(w.minute, 2)),
        "Hmmss" => format!("{}{}{}", w.hour, pad(w.minute, 2), pad(w.second, 2)),
        "a" => if w.hour > 11 { "pm" } else { "am" }.to_owned(),
        "A" => if w.hour > 11 { "PM" } else { "AM" }.to_owned(),
        "m" => num(w.minute),
        "mm" => pad(w.minute, 2),
        "s" => num(w.second),
        "ss" => pad(w.second, 2),
        "S" => num(w.ms / 100),
        "SS" => pad(w.ms / 10, 2),
        "SSS" => pad(w.ms, 3),
        "SSSS" | "SSSSS" | "SSSSSS" | "SSSSSSS" | "SSSSSSSS" | "SSSSSSSSS" => {
            let length = token.len();
            pad(w.ms * 10i64.pow(length as u32 - 3), length)
        }
        "z" => if m.is_utc { "UTC" } else { "" }.to_owned(),
        "zz" => if m.is_utc {
            "Coordinated Universal Time"
        } else {
            ""
        }
        .to_owned(),
        "Z" => zone(":"),
        "ZZ" => zone(""),
        "X" => number_to_string((m.ms / 1000.0).floor()),
        "x" => number_to_string(m.ms),
        "N" | "NN" | "NNN" | "NNNNN" => if ad { "AD" } else { "BC" }.to_owned(),
        "NNNN" => if ad { "Anno Domini" } else { "Before Christ" }.to_owned(),
        "y" => pad(era_year, 1),
        "yy" => pad(era_year, 2),
        "yyy" => pad(era_year, 3),
        "yyyy" => pad(era_year, 4),
        "yo" => ordinal(era_year),
        _ => return None,
    })
}

fn remove_formatting_tokens(token: &str) -> String {
    // `/\[[\s\S]/`：有 `[` 且後面還有字＝方括號跳脫，去掉頭尾的括號
    let bracket = token
        .char_indices()
        .any(|(at, ch)| ch == '[' && at + 1 < token.len());
    if bracket {
        let text = token.strip_prefix('[').unwrap_or(token);
        return text.strip_suffix(']').unwrap_or(text).to_owned();
    }
    token.replace('\\', "")
}

fn format_moment(m: &Moment, format: &str) -> String {
    if !m.valid() {
        return "Invalid date".to_owned();
    }
    let wall = m.wall();
    // moment format()：空格式用預設格式
    let format = if format.is_empty() {
        if m.is_utc && m.offset == 0.0 {
            "YYYY-MM-DDTHH:mm:ss[Z]"
        } else {
            "YYYY-MM-DDTHH:mm:ssZ"
        }
    } else {
        format
    };
    let expanded = expand_format(format);
    FORMAT_TOKENS
        .find_iter(&expanded)
        .map(|found| {
            let token = found.as_str();
            token_value(token, m, &wall).unwrap_or_else(|| remove_formatting_tokens(token))
        })
        .collect()
}

/// `moment(now).format(format)`（本地時區）。
pub fn format_local(clock: &dyn Clock, format: &str) -> String {
    let ms = clock.now_ms();
    let moment = Moment {
        ms,
        offset: clock.offset_minutes(ms),
        is_utc: false,
    };
    format_moment(&moment, format)
}

/// `moment(now).utc().utcOffset(offset).format(format)`：絕對值小於 16 視為小時。
pub fn format_utc_offset(clock: &dyn Clock, offset: f64, format: &str) -> String {
    let minutes = if offset.abs() < 16.0 {
        offset * 60.0
    } else {
        offset
    };
    let moment = Moment {
        ms: clock.now_ms(),
        offset: minutes,
        is_utc: true,
    };
    format_moment(&moment, format)
}

/// `moment.duration(ms).humanize(withSuffix)`
pub fn humanize(ms: f64, with_suffix: bool) -> String {
    if ms.is_nan() {
        return "Invalid date".to_owned();
    }
    let abs = ms.abs();
    let seconds = js_round(abs / 1000.0);
    let minutes = js_round(abs / 6e4);
    let hours = js_round(abs / 36e5);
    let days = js_round(abs / MS_PER_DAY);
    let months = js_round(abs / MS_PER_DAY * 4800.0 / 146_097.0);
    let years = js_round(abs / MS_PER_DAY * 4800.0 / 146_097.0 / 12.0);
    let n = |value: f64| number_to_string(if value == 0.0 { 1.0 } else { value });
    let text = if seconds <= 44.0 {
        "a few seconds".to_owned()
    } else if seconds < 45.0 {
        format!("{} seconds", n(seconds))
    } else if minutes <= 1.0 {
        "a minute".to_owned()
    } else if minutes < 45.0 {
        format!("{} minutes", n(minutes))
    } else if hours <= 1.0 {
        "an hour".to_owned()
    } else if hours < 22.0 {
        format!("{} hours", n(hours))
    } else if days <= 1.0 {
        "a day".to_owned()
    } else if days < 26.0 {
        format!("{} days", n(days))
    } else if months <= 1.0 {
        "a month".to_owned()
    } else if months < 11.0 {
        format!("{} months", n(months))
    } else if years <= 1.0 {
        "a year".to_owned()
    } else {
        format!("{} years", n(years))
    };
    if !with_suffix {
        return text;
    }
    if ms > 0.0 {
        format!("in {text}")
    } else {
        format!("{text} ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> FixedClock {
        // 2026-10-07 23:05:09.123 +08:00，星期三
        FixedClock {
            now_ms: make_date(2026, 9, 7, 15, 5, 9, 123),
            offset_minutes: 480.0,
        }
    }

    #[test]
    fn formats_like_moment() {
        let clock = clock();
        assert_eq!(
            format_utc_offset(&clock, 8.0, "YYYY-MM-DD HH:mm:ss.SSS Z ZZ [Q]Q X x S SS"),
            "2026-10-07 23:05:09.123 +08:00 +0800 Q4 1791385509 1791385509123 1 12"
        );
        assert_eq!(format_utc_offset(&clock, -5.0, "Z"), "-05:00");
        assert_eq!(
            format_utc_offset(&clock, 8.0, "DDD [week] WW"),
            "280 week 41"
        );
        assert_eq!(
            format_utc_offset(
                &clock,
                8.0,
                "DDDD Do E e w wo gg gggg GG GGGG k kk N Qo YYYYYY"
            ),
            "280 7th 3 3 41 41st 26 2026 26 2026 23 23 AD 4th +002026"
        );
        assert_eq!(
            format_utc_offset(&clock, 8.0, "X x Z ZZ [Do X Z]"),
            "1791385509 1791385509123 +08:00 +0800 Do X Z"
        );
        let new_year = FixedClock {
            now_ms: make_date(2027, 0, 1, 12, 0, 0, 0),
            offset_minutes: 0.0,
        };
        assert_eq!(
            format_utc_offset(&new_year, 0.0, "YYYY-MM-DD W GGGG"),
            "2027-01-01 53 2026"
        );
        assert_eq!(
            format_local(&clock, "LT|LL|llll"),
            "11:05 PM|October 7, 2026|Wed, Oct 7, 2026 11:05 PM"
        );
    }

    #[test]
    fn system_clock_reports_a_sane_local_offset() {
        let clock = SystemClock;
        let offset = clock.offset_minutes(clock.now_ms());
        assert!(offset.abs() <= 14.0 * 60.0, "{offset}");
        assert_ne!(format_local(&clock, "YYYY"), "Invalid date");
    }

    #[test]
    fn humanizes_like_moment() {
        assert_eq!(humanize(0.0, false), "a few seconds");
        assert_eq!(humanize(-2.0 * MS_PER_DAY, true), "2 days ago");
        assert_eq!(humanize(90.0 * 60_000.0, true), "in 2 hours");
        assert_eq!(humanize(f64::NAN, true), "Invalid date");
    }
}
