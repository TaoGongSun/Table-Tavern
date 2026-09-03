use crate::data::{
    self, Condition, FieldKind, FieldRule, Mechanism, StateNode, TriggerMode,
};
use std::collections::BTreeMap;

pub(super) fn tree_from(pairs: &[(&str, &str)]) -> BTreeMap<String, StateNode> {
    let mut tree = BTreeMap::new();
    for (path, value) in pairs {
        let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
        data::set_tree_value(&mut tree, &segments, value);
    }
    tree
}

pub(super) fn rule(kind: FieldKind, min: Option<f64>, max: Option<f64>) -> FieldRule {
    let mut rule = FieldRule::for_kind(kind);
    rule.min = min;
    rule.max = max;
    rule
}

pub(super) fn mechanism_with(rules: &[(&str, FieldRule)]) -> Mechanism {
    Mechanism {
        version: 1,
        rules: rules
            .iter()
            .map(|(path, rule)| ((*path).to_owned(), rule.clone()))
            .collect(),
        triggers: Vec::new(),
        incremental: false,
        guide: String::new(),
    }
}

pub(super) fn derived_rule(formula: &str) -> FieldRule {
    let mut rule = FieldRule::for_kind(FieldKind::Derived);
    rule.formula = Some(formula.to_owned());
    rule
}

pub(super) fn world_with(pairs: &[(&str, &str)], mechanism: Mechanism) -> data::WorldState {
    data::WorldState {
        id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
        name: "測試桌".to_owned(),
        model_bindings: BTreeMap::new(),
        player_card_id: None,
        current_scene: 0,
        catchup_summaries: BTreeMap::new(),
        scene_titles: BTreeMap::new(),
        scene_labels: BTreeMap::new(),
        state: data::TableState {
            table: BTreeMap::new(),
            tree: tree_from(pairs),
            notes: Vec::new(),
            changes: BTreeMap::new(),
            triggers: BTreeMap::new(),
            jumps: BTreeMap::new(),
        },
        mechanism,
        aligned_scene: None,
        branch_bindings: BTreeMap::new(),
        refactor_mode: None,
    }
}

pub(super) fn range_case(min: f64, text: &str) -> data::TriggerCase {
    data::TriggerCase {
        when: vec![Condition::Range {
            path: "World.Invasion".to_owned(),
            min: Some(min),
            max: None,
            min_exclusive: false,
            max_exclusive: false,
            default: Some(0.0),
        }],
        text: text.to_owned(),
    }
}

pub(super) fn else_case(text: &str) -> data::TriggerCase {
    data::TriggerCase {
        when: Vec::new(),
        text: text.to_owned(),
    }
}

pub(super) fn once_mechanism() -> Mechanism {
    Mechanism {
        version: 1,
        rules: BTreeMap::new(),
        triggers: vec![data::Trigger {
            id: "國變".to_owned(),
            title: "國變".to_owned(),
            mode: TriggerMode::Once,
            cases: vec![data::TriggerCase {
                when: vec![
                    Condition::Range {
                        path: "World.Invasion".to_owned(),
                        min: Some(50.0),
                        max: None,
                        min_exclusive: false,
                        max_exclusive: false,
                        default: Some(0.0),
                    },
                    Condition::Flag {
                        path: "Events.國變".to_owned(),
                        expect: false,
                    },
                ],
                text: "國都淪陷。".to_owned(),
            }],
            preamble: String::new(),
            scope: Vec::new(),
            flag: Some("Events.國變".to_owned()),
        }],
        incremental: true,
        guide: String::new(),
    }
}
