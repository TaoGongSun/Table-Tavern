//! 退場桌開場補齊的附帶資料（statusbar-only-fields 第 12 版）：欄位註記、時間格子同步、GM 提示快照。

use super::super::test_support::*;
use super::super::*;
use super::opening_fields::{
    fence, gm_update, leaf, north_with_template, outcome, post_opening, selection, PEOPLE,
};
use crate::data;
use crate::transport::StateBlock;
use std::collections::BTreeMap;
use std::path::Path;

/// 北境驛站原卡狀態欄條目（淘汰的 `美化状态栏`）原文。
const NORTH_TEMPLATE: &str = r#"rule:  //美化数据状态栏 美化状态栏   生成的yaml数据位于  <Status_block></Status_block> 中间
    - 状态栏数据用于显示角色当前状态与所处环境，具有全局视角。
    - 可以根据<user>的指令修改状态栏的显示字段，包括删除、修改、新增字段。
    - 只显示在场人物的数据，不在场的不显示。
    - 属性值使用双引号包裹，第一个位置是emoji字符加空格，格式为:"emoji+空格+内容"，比如：名字: "👤 林远"。
    - 行动选项生成4个，必须从<user>角度出发，风格依次为:稳妥选项/调查选项/冒险选项/交际选项，每个选项30字以内。
    - 状态栏数据是yaml格式，严格按照yaml格式生成。
formate |-
<Status_block>
状态栏:
  日期和时间: "${按照格式输出,示例:⏰ 大雍永和三年十月初七 酉时 }"
  地点: "${按照格式输出,示例:📍 北境驿站 仓房 }"
  天气: "${按照格式输出,示例:🌨️ 小雪 零下八度 }"
  驿站物资:
    粮草: "${🌾 剩余石数,示例:🌾 三百二十石 }"
    马匹: "${🐎 在册/实有,示例:🐎 在册四十 实有三十七 }"
  人物列表:
    - 人物:
        名字: "${👤 姓名 }"
        身份: "${🪪 职务 }"
        行动: "${📝 当前在做什么 }"
        装备: "${🧥 衣着与随身物品 }"
        体力: "${💪 0-100 的数值与简述 }"
        心情: "${🙂 当下情绪 }"
        内心: "${💭 只对<user>显示 }"
     {在对话现场的人物才显示，不在场的不显示}
  行动选项:
    名字: "${👤 <user> }"
    选项:
      - "1. ${稳妥选项，按规矩推进当前事务。 }"
      - "2. ${调查选项，追查账目或马匹线索。 }"
      - "3. ${冒险选项，冒风雪或冒得罪人之险行动。 }"
      - "4. ${交际选项，与在场人物谈话以换取信任或情报。 }"
</Status_block>"#;

fn time_parts(parts: &[data::TimePart]) -> std::collections::BTreeSet<data::TimePart> {
    parts.iter().copied().collect()
}

/// 套用時從模板取欄位註記（示例整段刪掉、集合用萬用路徑），時間格子記下開場值的成分、規則保留。
#[test]
fn field_notes_and_time_cells_come_from_the_template() {
    use data::TimePart::*;
    let root = TestRoot::new("opening-notes");
    let state = north_with_template(root.path(), NORTH_TEMPLATE);
    let opening = &state.mechanism.opening;
    assert!(opening.tracked);
    let notes = &opening.field_notes;
    assert!(!notes.contains_key("状态栏.天气"), "{notes:?}");
    assert!(!notes.contains_key("状态栏.日期和时间"), "{notes:?}");
    assert_eq!(notes[&format!("{PEOPLE}.*.体力")], "0-100 的数值与简述");
    assert_eq!(notes[&format!("{PEOPLE}.*.行动")], "当前在做什么");
    assert_eq!(notes["状态栏.驿站物资.粮草"], "剩余石数");
    assert!(!notes
        .keys()
        .any(|key| key.contains("名字") && key.starts_with(PEOPLE)));
    assert_eq!(
        opening.time_cells["状态栏.日期和时间"],
        time_parts(&[Year, Month, Day, Hour])
    );
    assert_eq!(opening.time_cells.len(), 1, "{:?}", opening.time_cells);
    assert!(
        state.mechanism.rules.contains_key("状态栏.日期和时间"),
        "時間格子保留規則"
    );
}

fn world_with_cells(root: &Path, cells: &str) -> data::WorldState {
    let world_id = data::create_world(root, "桌").unwrap();
    let uid = seed_entry(root, &world_id, "状态栏", "模板");
    post_opening(
        root,
        &world_id,
        &format!("正文\n<Status_block>\n状态栏:\n{cells}\n</Status_block>"),
        None,
    );
    apply(
        root,
        &world_id,
        &outcome(uid, BTreeMap::new(), Vec::new(), &[]),
        &selection(false),
    )
    .unwrap();
    data::read_state(root, &world_id).unwrap()
}

/// 倒數、期限、歷史日期、排程、人物身上的時間、數字開場值都不同步；只有白名單完整欄名收。
#[test]
fn time_cells_skip_countdowns_deadlines_and_person_fields() {
    let root = TestRoot::new("opening-time-cells");
    let state = world_with_cells(
        root.path(),
        "  剩余时间: \"三天\"\n  下次补给日期: \"十月十五\"\n  受伤日期: \"十月初三\"\n  约定时间: \"明日午时\"\n  日期: \"3\"\n  当前日期: \"十月初七\"\n  人物:\n    - 名字: \"甲\"\n      时间: \"酉时\"\n    - 名字: \"乙\"\n      时间: \"戌时\"",
    );
    let cells: Vec<&String> = state.mechanism.opening.time_cells.keys().collect();
    assert_eq!(cells, vec!["状态栏.当前日期"]);
}

fn fence_and_patch(state: &mut data::WorldState, time: Option<&str>, patch: Option<&str>) {
    let block = StateBlock {
        fields: time
            .map(|time| vec![(vec!["time".to_owned()], time.to_owned())])
            .unwrap_or_default(),
        updates: patch
            .map(|patch| vec![format!("<JSONPatch>{patch}</JSONPatch>")])
            .unwrap_or_default(),
        display: String::new(),
        lists: Vec::new(),
        maps: Vec::new(),
        fenced: None,
    };
    crate::mechanism::apply_block(state, &block, "林远");
}

/// 補寫只用本回合 GM 實際報的 time，而且成分要完全相同；GM 自己報了這一格就以 GM 為準。
#[test]
fn time_cell_sync_never_loses_information() {
    let root = TestRoot::new("opening-time-sync");
    let mut state = north_with_template(root.path(), NORTH_TEMPLATE);
    let cell = "状态栏.日期和时间";
    let opening_value = leaf(&state, cell).unwrap().to_owned();

    fence_and_patch(&mut state, Some("初七 戌时"), None);
    assert_eq!(
        leaf(&state, cell),
        Some(opening_value.as_str()),
        "完整日期時間格不被只有日時的 time 蓋掉"
    );

    fence_and_patch(&mut state, Some("大雍永和三年十月初八 子时"), None);
    assert_eq!(leaf(&state, cell), Some("大雍永和三年十月初八 子时"));

    // 本回合沒報 time：平欄殘留的舊 time 不拿來補
    state
        .state
        .table
        .insert("time".to_owned(), "大雍永和三年十月初九 午时".to_owned());
    fence_and_patch(
        &mut state,
        None,
        Some(r#"[{"op":"replace","path":"/状态栏/天气","value":"晴"}]"#),
    );
    assert_eq!(leaf(&state, cell), Some("大雍永和三年十月初八 子时"));

    // 同回合 GM 自己報了這一格：以 GM 為準
    fence_and_patch(
        &mut state,
        Some("大雍永和三年十月初九 午时"),
        Some(
            r#"[{"op":"replace","path":"/状态栏/日期和时间","value":"大雍永和三年十月初九 未时"}]"#,
        ),
    );
    assert_eq!(leaf(&state, cell), Some("大雍永和三年十月初九 未时"));
}

/// 日期格不被完整日期時間改寫；完整年月日時分的格子不被「初七 戌時」改寫。
#[test]
fn time_cell_sync_keeps_semantics_apart() {
    let root = TestRoot::new("opening-time-grain");
    let mut state = world_with_cells(
        root.path(),
        "  日期: \"大雍永和三年十月初七\"\n  日期时间: \"2026年10月28日 21点30分\"",
    );
    assert_eq!(state.mechanism.opening.time_cells.len(), 2);
    fence_and_patch(&mut state, Some("大雍永和三年十月初七 酉时"), None);
    assert_eq!(leaf(&state, "状态栏.日期"), Some("大雍永和三年十月初七"));
    fence_and_patch(&mut state, Some("初七 戌時"), None);
    assert_eq!(
        leaf(&state, "状态栏.日期时间"),
        Some("2026年10月28日 21点30分")
    );
    fence_and_patch(&mut state, Some("大雍永和三年十月初八"), None);
    assert_eq!(leaf(&state, "状态栏.日期"), Some("大雍永和三年十月初八"));
}

/// 數字日期的邊界：開場 `2026-10-28 21:30`、time `2026-10 21:30` 少了日，不寫（`2026-10` 不是月-日 `26-10`）。
#[test]
fn numeric_dates_missing_the_day_are_not_synced() {
    let root = TestRoot::new("opening-time-numeric");
    let mut state = world_with_cells(root.path(), "  日期时间: \"2026-10-28 21:30\"");
    fence_and_patch(&mut state, Some("2026-10 21:30"), None);
    assert_eq!(leaf(&state, "状态栏.日期时间"), Some("2026-10-28 21:30"));
    fence_and_patch(&mut state, Some("2026-10-29 08:15"), None);
    assert_eq!(leaf(&state, "状态栏.日期时间"), Some("2026-10-29 08:15"));
}

/// 凍結快照：開場補齊桌在人物增刪、在場與時間變動前後，GM 的 system 逐字相同（只讀套用時存下的資料）。
#[test]
fn gm_system_is_stable_across_state_changes() {
    let root = TestRoot::new("opening-snapshot");
    let mut state = north_with_template(root.path(), NORTH_TEMPLATE);
    let system = |mechanism: &data::Mechanism| {
        crate::transport::test_support::legacy::gm_lane_system(
            "",
            &[],
            None,
            &[],
            mechanism,
            "zh-TW",
        )
    };
    let before = system(&state.mechanism);
    assert!(
        before.contains("各格追蹤什麼") && before.contains("只列在場人物"),
        "{before}"
    );
    fence(
        &mut state,
        &[("time", "大雍永和三年十月初八 子时"), ("present", "林远")],
    );
    gm_update(
        &mut state,
        r#"[{"op":"insert","path":"/状态栏/人物列表/秦岚","value":{"体力":"90","身份":"斥候队长"}},{"op":"remove","path":"/状态栏/人物列表/贺铮"},{"op":"replace","path":"/状态栏/天气","value":"暴风雪"}]"#,
    );
    assert_eq!(system(&state.mechanism), before);
}
