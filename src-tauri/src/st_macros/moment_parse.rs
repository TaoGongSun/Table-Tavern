//! `{{timeDiff}}` 的 moment(字串) 解析，照 moment 2.30.1 原始碼：ASP.NET `/Date(…)`、ISO 8601（非嚴格逐 token 解析＋
//! 溢位檢查）、RFC 2822。不支援：moment 解析不了時退回 JS `new Date(字串)` 的那條路（瀏覽器各自實作），
//! 桌面版一律當無效日期。

use std::sync::LazyLock;

use regex::Regex;

use super::moment::{
    civil_from_days, day_of_year_from_weeks, days_from_civil, days_in_month, days_in_year,
    humanize, make_date, weeks_in_year, Clock, JS_SPACE, MAX_TIME, MS_PER_DAY,
};

/// `{{timeDiff}}`：`moment.duration(moment(left).diff(moment(right))).humanize(true)`。
pub fn time_diff(clock: &dyn Clock, left: &str, right: &str) -> String {
    match (parse_moment(clock, left), parse_moment(clock, right)) {
        (Some(left), Some(right)) => humanize(left - right, true),
        _ => "Invalid date".to_owned(),
    }
}

/// 本地牆上時間 → epoch 毫秒（JS `new Date(y, m, d, …)`）。
/// 時區切換附近照 ECMAScript `UTC(t)`：落在跳過的那段用切換前的偏移（往後推，紐約 3/8 02:30 → 03:30），
/// 落在重複的那段取較早的時刻。
pub(super) fn local_to_epoch(clock: &dyn Clock, wall: f64) -> f64 {
    let before = clock.offset_minutes(wall - MS_PER_DAY);
    let after = clock.offset_minutes(wall + MS_PER_DAY);
    let consistent = |offset: f64| {
        let epoch = wall - offset * 60_000.0;
        (clock.offset_minutes(epoch) == offset).then_some(epoch)
    };
    match (consistent(before), consistent(after)) {
        (Some(a), Some(b)) => a.min(b),
        (Some(epoch), None) | (None, Some(epoch)) => epoch,
        (None, None) => wall - before * 60_000.0,
    }
}

fn time_clip(ms: f64) -> Option<f64> {
    (ms.is_finite() && ms.abs() <= MAX_TIME).then_some(ms.trunc())
}

static ASP_NET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?i-u:/?Date\()(-?[0-9]+)").expect("aspNetJsonRegex"));

/// moment(字串) 的時刻；無效回 `None`。
fn parse_moment(clock: &dyn Clock, input: &str) -> Option<f64> {
    if input.is_empty() {
        return None;
    }
    if let Some(caps) = ASP_NET.captures(input) {
        return time_clip(caps[1].parse::<f64>().ok()?);
    }
    match parse_iso(clock, input) {
        Iso::Parsed(result) => return result,
        Iso::NotIso => {}
    }
    parse_rfc2822(input)
}

enum Iso {
    NotIso,
    Parsed(Option<f64>),
}

static EXTENDED_ISO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^[{JS_SPACE}]*((?:[+-][0-9]{{6}}|[0-9]{{4}})-(?:[0-9]{{2}}-[0-9]{{2}}|W[0-9]{{2}}-[0-9]|W[0-9]{{2}}|[0-9]{{3}}|[0-9]{{2}}))(?:(T| )([0-9]{{2}}(?::[0-9]{{2}}(?::[0-9]{{2}}(?:[.,][0-9]+)?)?)?)([+-][0-9]{{2}}(?::?[0-9]{{2}})?|[{JS_SPACE}]*Z)?)?$"
    ))
    .expect("extendedIsoRegex")
});

static BASIC_ISO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^[{JS_SPACE}]*((?:[+-][0-9]{{6}}|[0-9]{{4}})(?:[0-9]{{4}}|W[0-9]{{3}}|W[0-9]{{2}}|[0-9]{{3}}|[0-9]{{2}}|))(?:(T| )([0-9]{{2}}(?:[0-9]{{2}}(?:[0-9]{{2}}(?:[.,][0-9]+)?)?)?)([+-][0-9]{{2}}(?::?[0-9]{{2}})?|[{JS_SPACE}]*Z)?)?$"
    ))
    .expect("basicIsoRegex")
});

static TZ: LazyLock<Regex> = LazyLock::new(|| re(r"Z|[+-][0-9]{2}(?::?[0-9]{2})?"));

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("moment regex")
}

/// moment `isoDates`：（格式的 token 序列、偵測用的不錨定正則、可不可以帶時間）
static ISO_DATES: LazyLock<Vec<(&'static [&'static str], Regex, bool)>> = LazyLock::new(|| {
    vec![
        (
            &["YYYYYY", "-", "MM", "-", "DD"][..],
            re(r"[+-][0-9]{6}-[0-9]{2}-[0-9]{2}"),
            true,
        ),
        (
            &["YYYY", "-", "MM", "-", "DD"][..],
            re(r"[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            true,
        ),
        (
            &["GGGG", "-", "W", "WW", "-", "E"][..],
            re(r"[0-9]{4}-W[0-9]{2}-[0-9]"),
            true,
        ),
        (
            &["GGGG", "-", "W", "WW"][..],
            re(r"[0-9]{4}-W[0-9]{2}"),
            false,
        ),
        (&["YYYY", "-", "DDD"][..], re(r"[0-9]{4}-[0-9]{3}"), true),
        (&["YYYY", "-", "MM"][..], re(r"[0-9]{4}-[0-9]{2}"), false),
        (&["YYYYYY", "MM", "DD"][..], re(r"[+-][0-9]{10}"), true),
        (&["YYYY", "MM", "DD"][..], re(r"[0-9]{8}"), true),
        (
            &["GGGG", "W", "WW", "E"][..],
            re(r"[0-9]{4}W[0-9]{3}"),
            true,
        ),
        (&["GGGG", "W", "WW"][..], re(r"[0-9]{4}W[0-9]{2}"), false),
        (&["YYYY", "DDD"][..], re(r"[0-9]{7}"), true),
        (&["YYYY", "MM"][..], re(r"[0-9]{6}"), false),
        (&["YYYY"][..], re(r"[0-9]{4}"), false),
    ]
});

static ISO_TIMES: LazyLock<Vec<(&'static [&'static str], Regex)>> = LazyLock::new(|| {
    vec![
        (
            &["HH", ":", "mm", ":", "ss", ".", "SSSS"][..],
            re(r"[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+"),
        ),
        (
            &["HH", ":", "mm", ":", "ss", ",", "SSSS"][..],
            re(r"[0-9]{2}:[0-9]{2}:[0-9]{2},[0-9]+"),
        ),
        (
            &["HH", ":", "mm", ":", "ss"][..],
            re(r"[0-9]{2}:[0-9]{2}:[0-9]{2}"),
        ),
        (&["HH", ":", "mm"][..], re(r"[0-9]{2}:[0-9]{2}")),
        (
            &["HH", "mm", "ss", ".", "SSSS"][..],
            re(r"[0-9]{6}\.[0-9]+"),
        ),
        (&["HH", "mm", "ss", ",", "SSSS"][..], re(r"[0-9]{6},[0-9]+")),
        (&["HH", "mm", "ss"][..], re(r"[0-9]{6}")),
        (&["HH", "mm"][..], re(r"[0-9]{4}")),
        (&["HH"][..], re(r"[0-9]{2}")),
    ]
});

/// 非嚴格解析時各 token 的正則（moment `addRegexToken` 的第一個）；其餘是字面字元。
static TOKEN_REGEXES: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        ("YYYYYY", re(r"[+-]?[0-9]{1,6}")),
        ("YYYY", re(r"[0-9]{1,4}")),
        ("GGGG", re(r"[0-9]{1,4}")),
        ("MM", re(r"[0-9][0-9]?")),
        ("DD", re(r"[0-9][0-9]?")),
        ("WW", re(r"[0-9][0-9]?")),
        ("E", re(r"[0-9][0-9]?")),
        ("DDD", re(r"[0-9]{1,3}")),
        ("HH", re(r"[0-9][0-9]?")),
        ("mm", re(r"[0-9][0-9]?")),
        ("ss", re(r"[0-9][0-9]?")),
        ("SSSS", re(r"[0-9]+")),
        ("Z", re(r"(?i-u:Z)|[+-][0-9]{2}(?::?[0-9]{2})?")),
    ]
});

/// moment `toInt`：轉數字後往零截斷，非有限數或 0 回 0。
fn to_int(text: &str) -> i64 {
    let number = super::js_value::JsValue::str(text).to_number();
    if number != 0.0 && number.is_finite() {
        number.trunc() as i64
    } else {
        0
    }
}

/// moment `offsetFromString`：取正負號與兩位數字段（ISO 解析時只會拿到一段）。
fn offset_from_string(input: &str) -> i64 {
    let bytes = input.as_bytes();
    let mut parts: Vec<&str> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'+' || bytes[at] == b'-' {
            parts.push(&input[at..at + 1]);
            at += 1;
        } else if bytes[at].is_ascii_digit() && bytes.get(at + 1).is_some_and(u8::is_ascii_digit) {
            parts.push(&input[at..at + 2]);
            at += 2;
        } else {
            at += 1;
        }
    }
    let Some(sign) = parts.first() else {
        return 0;
    };
    let minutes =
        parts.get(1).map_or(0, |h| to_int(h)) * 60 + parts.get(2).map_or(0, |m| to_int(m));
    if minutes == 0 {
        0
    } else if *sign == "+" {
        minutes
    } else {
        -minutes
    }
}

#[derive(Default)]
struct Parsed {
    year: Option<i64>,
    month: Option<i64>,
    date: Option<i64>,
    hour: Option<i64>,
    minute: Option<i64>,
    second: Option<i64>,
    ms: Option<i64>,
    day_of_year: Option<i64>,
    week_year: Option<i64>,
    week: Option<i64>,
    weekday: Option<i64>,
    tzm: Option<i64>,
}

fn parse_iso(clock: &dyn Clock, input: &str) -> Iso {
    let Some(caps) = EXTENDED_ISO
        .captures(input)
        .or_else(|| BASIC_ISO.captures(input))
    else {
        return Iso::NotIso;
    };
    let date_part = &caps[1];
    let Some((date_tokens, _, allow_time)) = ISO_DATES
        .iter()
        .find(|(_, detect, _)| detect.is_match(date_part))
    else {
        return Iso::NotIso;
    };
    let mut tokens: Vec<&str> = date_tokens.to_vec();
    if let Some(time) = caps.get(3).filter(|time| !time.as_str().is_empty()) {
        let Some((time_tokens, _)) = ISO_TIMES
            .iter()
            .find(|(_, detect)| detect.is_match(time.as_str()))
        else {
            return Iso::NotIso;
        };
        if !allow_time {
            return Iso::NotIso;
        }
        tokens.push(caps.get(2).map_or(" ", |sep| sep.as_str()));
        tokens.extend_from_slice(time_tokens);
    }
    if let Some(zone) = caps.get(4).filter(|zone| !zone.as_str().is_empty()) {
        if !TZ.is_match(zone.as_str()) {
            return Iso::NotIso;
        }
        tokens.push("Z");
    }
    Iso::Parsed(parse_with_tokens(clock, input, &tokens))
}

fn parse_with_tokens(clock: &dyn Clock, input: &str, tokens: &[&str]) -> Option<f64> {
    let mut rest = input;
    let mut parsed = Parsed::default();
    for token in tokens {
        let regex = TOKEN_REGEXES.iter().find(|(name, _)| name == token);
        let found = match regex {
            Some((_, regex)) => regex.find(rest).map(|m| (m.start(), m.as_str())),
            None => rest.find(token).map(|at| (at, &rest[at..at + token.len()])),
        };
        let Some((at, text)) = found.filter(|(_, text)| !text.is_empty()) else {
            continue;
        };
        rest = &rest[at + text.len()..];
        match *token {
            "YYYYYY" => parsed.year = Some(to_int(text)),
            "YYYY" => {
                parsed.year = Some(if text.len() == 2 {
                    let year = to_int(text);
                    year + if year > 68 { 1900 } else { 2000 }
                } else {
                    to_int(text)
                })
            }
            "MM" => parsed.month = Some(to_int(text) - 1),
            "DD" => parsed.date = Some(to_int(text)),
            "DDD" => parsed.day_of_year = Some(to_int(text)),
            "GGGG" => parsed.week_year = Some(to_int(text)),
            "WW" => parsed.week = Some(to_int(text)),
            "E" => parsed.weekday = Some(to_int(text)),
            "HH" => parsed.hour = Some(to_int(text)),
            "mm" => parsed.minute = Some(to_int(text)),
            "ss" => parsed.second = Some(to_int(text)),
            "SSSS" => {
                let fraction: f64 = format!("0.{text}").parse().unwrap_or(0.0);
                let ms = fraction * 1000.0;
                parsed.ms = Some(if ms.is_finite() { ms.trunc() as i64 } else { 0 });
            }
            "Z" => parsed.tzm = Some(offset_from_string(text)),
            _ => {}
        }
    }
    build_date(clock, parsed)
}

/// moment `configFromArray`＋`checkOverflow`（只走 ISO 用得到的分支）。
fn build_date(clock: &dyn Clock, mut p: Parsed) -> Option<f64> {
    let mut overflow_day_of_year = false;
    let mut overflow_weeks = false;
    let mut overflow_weekday = false;
    if (p.week_year.is_some() || p.week.is_some() || p.weekday.is_some())
        && p.date.is_none()
        && p.month.is_none()
    {
        let week_year = p.week_year.or(p.year).unwrap_or(0);
        let week = p.week.unwrap_or(1);
        let weekday = p.weekday.unwrap_or(1);
        let weekday_bad = !(1..=7).contains(&weekday);
        if week < 1 || week > weeks_in_year(week_year, 1, 4) {
            overflow_weeks = true;
        } else if weekday_bad {
            overflow_weekday = true;
        } else {
            let (year, day_of_year) = day_of_year_from_weeks(week_year, week, weekday, 1, 4);
            p.year = Some(year);
            p.day_of_year = Some(day_of_year);
        }
    }
    let year = p.year?;
    if let Some(day_of_year) = p.day_of_year {
        if day_of_year > days_in_year(year) || day_of_year == 0 {
            overflow_day_of_year = true;
        }
        let days = days_from_civil(year, 1, 1) + day_of_year - 1;
        let (_, month, date) = civil_from_days(days);
        p.month = Some(month - 1);
        p.date = Some(date);
    }
    let month = p.month.unwrap_or(0);
    let date = p.date.unwrap_or(1);
    let mut hour = p.hour.unwrap_or(0);
    let minute = p.minute.unwrap_or(0);
    let second = p.second.unwrap_or(0);
    let ms = p.ms.unwrap_or(0);
    let next_day = hour == 24 && minute == 0 && second == 0 && ms == 0;
    if next_day {
        hour = 0;
    }
    let wall = make_date(year, month, date, hour, minute, second, ms);
    let mut epoch = match p.tzm {
        Some(tzm) => wall - tzm as f64 * 60_000.0,
        None => local_to_epoch(clock, wall),
    };
    if next_day {
        // moment `add(1, 'd')`（本地模式）：本地日曆日加一、牆上時刻不變
        hour = 24;
        let local_wall = epoch + clock.offset_minutes(epoch) * 60_000.0;
        epoch = local_to_epoch(clock, local_wall + MS_PER_DAY);
    }
    let overflow = !(0..=11).contains(&month)
        || date < 1
        || date > days_in_month(year, month)
        || !(0..=24).contains(&hour)
        || (hour == 24 && (minute != 0 || second != 0 || ms != 0))
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
        || !(0..=999).contains(&ms);
    if overflow || overflow_day_of_year || overflow_weeks || overflow_weekday {
        return None;
    }
    time_clip(epoch)
}

static RFC2822: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:(Mon|Tue|Wed|Thu|Fri|Sat|Sun),?[{JS_SPACE}])?([0-9]{{1,2}})[{JS_SPACE}](Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[{JS_SPACE}]([0-9]{{2,4}})[{JS_SPACE}]([0-9]{{2}}):([0-9]{{2}})(?::([0-9]{{2}}))?[{JS_SPACE}](?:(UT|GMT|[ECMP][SD]T)|([Zz])|([+-][0-9]{{4}}))$"
    ))
    .expect("rfc2822")
});

static RFC_COMMENTS: LazyLock<Regex> = LazyLock::new(|| re(r"\([^()]*\)|[\n\t]"));
static RFC_SPACES: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"[{JS_SPACE}][{JS_SPACE}]+")));

/// moment `preprocessRFC2822`：註解與折行換空白、連續空白併成一個、去頭尾空白。
fn preprocess_rfc2822(text: &str) -> String {
    let text = RFC_COMMENTS.replace_all(text, " ");
    let text = RFC_SPACES.replace_all(&text, " ");
    crate::world_info::js_semantics::js_trim(&text).to_owned()
}

fn parse_rfc2822(input: &str) -> Option<f64> {
    let text = preprocess_rfc2822(input);
    let caps = RFC2822.captures(&text)?;
    let year_raw: i64 = caps[4].parse().ok()?;
    let year = if year_raw <= 49 {
        2000 + year_raw
    } else if year_raw <= 999 {
        1900 + year_raw
    } else {
        year_raw
    };
    const SHORT: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = SHORT.iter().position(|name| *name == &caps[3])? as i64;
    let date: i64 = caps[2].parse().ok()?;
    let hour: i64 = caps[5].parse().ok()?;
    let minute: i64 = caps[6].parse().ok()?;
    let second: i64 = caps.get(7).map_or(Some(0), |s| s.as_str().parse().ok())?;
    if let Some(weekday) = caps.get(1) {
        let provided = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
            .iter()
            .position(|name| *name == weekday.as_str())? as i64;
        let days = (make_date(year, month, date, 0, 0, 0, 0) / MS_PER_DAY).floor() as i64;
        if provided != (days + 4).rem_euclid(7) {
            // moment 判無效後退回 JS Date 解析——桌面版不做那條路
            return None;
        }
    }
    // moment checkOverflow：RFC 2822 的陣列沒有毫秒（缺秒時也沒有秒），所以 24 時一律溢位
    let overflow = date < 1
        || date > days_in_month(year, month)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second);
    if overflow {
        return None;
    }
    let tzm = if let Some(obs) = caps.get(8) {
        match obs.as_str() {
            "UT" | "GMT" => 0,
            "EDT" => -4 * 60,
            "EST" | "CDT" => -5 * 60,
            "CST" | "MDT" => -6 * 60,
            "MST" | "PDT" => -7 * 60,
            "PST" => -8 * 60,
            _ => return None,
        }
    } else if caps.get(9).is_some() {
        0
    } else {
        let hm: i64 = caps[10].parse().ok()?;
        let m = hm % 100;
        let h = (hm - m) / 100;
        h * 60 + m
    };
    time_clip(make_date(year, month, date, hour, minute, second, 0) - tzm as f64 * 60_000.0)
}

#[cfg(test)]
mod tests {
    use super::super::moment::FixedClock;
    use super::*;

    fn clock() -> FixedClock {
        FixedClock {
            now_ms: make_date(2026, 9, 7, 15, 5, 9, 123),
            offset_minutes: 480.0,
        }
    }

    #[test]
    fn parses_iso_and_rfc2822() {
        let clock = clock();
        assert_eq!(time_diff(&clock, "2026-01-01", "2026-01-03"), "2 days ago");
        assert_eq!(
            parse_moment(&clock, "2026-10-07T15:05:09Z"),
            Some(make_date(2026, 9, 7, 15, 5, 9, 0))
        );
        assert_eq!(
            parse_moment(&clock, "2026-W41-3"),
            parse_moment(&clock, "2026-10-07")
        );
        assert_eq!(parse_moment(&clock, "2026-02-30"), None);
        assert_eq!(
            parse_moment(&clock, "Wed, 07 Oct 2026 15:05:09 GMT"),
            Some(make_date(2026, 9, 7, 15, 5, 9, 0))
        );
        assert_eq!(parse_moment(&clock, "not a date"), None);
    }

    /// 2026 年的 America/New_York：3/8 07:00Z 起 EDT（-240），11/1 06:00Z 起 EST（-300）。
    struct NewYork2026;

    impl Clock for NewYork2026 {
        fn now_ms(&self) -> f64 {
            0.0
        }

        fn offset_minutes(&self, epoch_ms: f64) -> f64 {
            let start = make_date(2026, 2, 8, 7, 0, 0, 0);
            let end = make_date(2026, 10, 1, 6, 0, 0, 0);
            if (start..end).contains(&epoch_ms) {
                -240.0
            } else {
                -300.0
            }
        }
    }

    #[test]
    fn local_times_follow_js_around_dst() {
        // 預期值：TZ=America/New_York 下 moment(字串).valueOf()
        let ny = NewYork2026;
        let cases = [
            ("2026-03-08T02:30", 1_772_955_000_000.0),
            ("2026-11-01T01:30", 1_793_511_000_000.0),
            ("2026-03-07T24:00", 1_772_946_000_000.0),
            // 3/9 00:00 EDT：比 3/8 00:00 EST 加 24 小時少 1 小時
            ("2026-03-08T24:00", 1_773_028_800_000.0),
            ("2026-10-31T24:00", 1_793_505_600_000.0),
            ("2026-03-08T00:00", 1_772_946_000_000.0),
            ("2026-11-01T00:00", 1_793_505_600_000.0),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_moment(&ny, input), Some(expected), "{input}");
        }
    }
}
