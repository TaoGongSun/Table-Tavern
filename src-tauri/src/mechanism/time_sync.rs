//! 時間格子同步（statusbar-only-fields 第 11 版）：退場桌卡片自己的時間格子（「日期和时间」這類）保留規則、GM 照常可報；
//! GM 本回合在圍欄報了 time、沒有自己報這一格，而且 time 的成分（年、月、日、時辰／點鐘、分、刻）跟這格開場值的
//! 成分完全相同時，App 才把 time 原字補寫進去。成分不同（「初七 戌时」對完整年月日時分、完整日期時間對日期格）
//! 一律不寫，寧可停在 GM 最後回報的值，也不蓋掉資訊。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::data::{self, StateNode, TimePart, WorldState};

fn part_regexes() -> &'static [(TimePart, regex::Regex)] {
    static RE: OnceLock<Vec<(TimePart, regex::Regex)>> = OnceLock::new();
    RE.get_or_init(|| {
        let numeral = "[〇零一二三四五六七八九十百千两兩廿卅0-9]";
        // 數字日期要整段比、前後不接數字（也不接分隔符）：年-月-日、年-月（四位年）、月-日；
        // 不然 `2026-10` 會被看成月-日 `26-10`
        let ymd = r"(?:^|[^0-9])\d{4}[-/.]\d{1,2}[-/.]\d{1,2}(?:[^0-9]|$)";
        let ym = r"(?:^|[^0-9])\d{4}[-/.]\d{1,2}(?:[^0-9]|$)";
        let md = r"(?:^|[^0-9\-/.])\d{1,2}[-/.]\d{1,2}(?:[^0-9\-/.]|$)";
        let clock = r"(?:^|[^0-9])\d{1,2}[:：]\d{2}(?:[^0-9]|$)";
        [
            (TimePart::Year, format!(r"{numeral}+年|元年|{ymd}|{ym}")),
            (
                TimePart::Month,
                format!(r"{numeral}+月|[正冬腊臘]月|{ymd}|{ym}|{md}"),
            ),
            (
                TimePart::Day,
                format!(r"初[一二三四五六七八九十]|{numeral}+[日号號]|{ymd}|{md}"),
            ),
            (
                TimePart::Hour,
                format!(r"[子丑寅卯辰巳午未申酉戌亥][时時正初]|{numeral}+[点點]|{clock}"),
            ),
            (TimePart::Minute, format!(r"{numeral}+分|{clock}")),
            (TimePart::Quarter, r"[一二三四1-4]刻".to_owned()),
        ]
        .into_iter()
        .map(|(part, pattern)| (part, regex::Regex::new(&pattern).expect("time part regex")))
        .collect()
    })
}

/// 時間文字拆出的成分（先折成簡體字形）；一個都沒有＝不是時間。
pub fn time_parts(text: &str) -> BTreeSet<TimePart> {
    let folded = data::fold_glyphs(text);
    part_regexes()
        .iter()
        .filter(|(_, re)| re.is_match(&folded))
        .map(|(part, _)| *part)
        .collect()
}

/// 本回合補寫：`reported_time` 是 GM 本回合圍欄實際報的 time（沒報就是 None，不拿平欄殘留的舊值），
/// `reported_paths` 是 GM 本回合自己寫到的樹路徑（點分），那些格子以 GM 為準。回傳補寫了的路徑。
pub(super) fn sync(
    world: &mut WorldState,
    reported_time: Option<&str>,
    reported_paths: &BTreeSet<String>,
) -> Vec<String> {
    let Some(time) = reported_time.map(str::trim).filter(|time| !time.is_empty()) else {
        return Vec::new();
    };
    if world.mechanism.opening.time_cells.is_empty() {
        return Vec::new();
    }
    let parts = time_parts(time);
    let mut written = Vec::new();
    let cells: Vec<String> = world
        .mechanism
        .opening
        .time_cells
        .iter()
        .filter(|(path, cell_parts)| **cell_parts == parts && !reported_paths.contains(*path))
        .map(|(path, _)| path.clone())
        .collect();
    for path in cells {
        let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
        if !matches!(
            data::node_at(&world.state.tree, &segments),
            Some(StateNode::Leaf(_))
        ) {
            continue;
        }
        let value =
            super::prefix::strip(&world.mechanism.display_prefixes, &segments, time).to_owned();
        if data::set_tree_value(&mut world.state.tree, &segments, &value) {
            written.push(path);
        }
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use TimePart::*;

    fn set(parts: &[TimePart]) -> BTreeSet<TimePart> {
        parts.iter().copied().collect()
    }

    #[test]
    fn parts_are_split_finely() {
        assert_eq!(
            time_parts("大雍永和三年十月初七 酉时"),
            set(&[Year, Month, Day, Hour])
        );
        assert_eq!(time_parts("初七 戌時"), set(&[Day, Hour]));
        assert_eq!(time_parts("戌时"), set(&[Hour]));
        assert_eq!(time_parts("大雍永和三年十月初七"), set(&[Year, Month, Day]));
        assert_eq!(
            time_parts("永和三年十月初七 戌时三刻"),
            set(&[Year, Month, Day, Hour, Quarter])
        );
        assert_eq!(
            time_parts("2026-10-28 21:30"),
            set(&[Year, Month, Day, Hour, Minute])
        );
        assert_eq!(
            time_parts("永和三年十月初七 戌时 二十分"),
            set(&[Year, Month, Day, Hour, Minute])
        );
        // 數字日期的邊界：`2026-10` 只有年月，不會被看成月-日 `26-10`
        assert_eq!(
            time_parts("2026-10 21:30"),
            set(&[Year, Month, Hour, Minute])
        );
        assert_eq!(time_parts("2026-10-28"), set(&[Year, Month, Day]));
        assert_eq!(time_parts("10-28 戌时"), set(&[Month, Day, Hour]));
        assert_eq!(time_parts("12026-10"), set(&[]));
        assert!(time_parts("北风小雪").is_empty());
        assert!(time_parts("未必如此").is_empty());
        assert!(time_parts("冬日暖阳").is_empty());
        assert_eq!(time_parts("元年正月"), set(&[Year, Month]));
    }
}
