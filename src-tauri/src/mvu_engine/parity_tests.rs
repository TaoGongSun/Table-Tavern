//! parity json 的 zod 分支（mvu-zod-semantics 規格 6.2）：同一份 patch 經桌面 QuickJS 走 zod 迴圈，要和 Node 核心
//! （source-zod.test.ts）讀到同一份預期值；預期值在本機跑上游 mvu_zod.ts 比對過。
use super::*;
use serde_json::{Map, Value};
use std::sync::Arc;

const MVU_ZOD: &str =
    "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js";

fn run(
    engine: &MvuEngine,
    name: &str,
    schema: &str,
    stat: &Value,
    message: &str,
) -> (Value, Vec<CommandStatus>) {
    let script = Arc::new(SchemaScript::new(format!(
        "import {{ registerMvuSchema }} from '{MVU_ZOD}';\nregisterMvuSchema({schema});"
    )));
    let mut old = Map::new();
    old.insert("stat_data".into(), stat.clone());
    let output = engine
        .run_batch(
            name,
            &script,
            message,
            &old,
            &Macros {
                user: "玩家".into(),
                char: None,
            },
            engine.deadline(),
        )
        .unwrap_or_else(|failure| panic!("{name}: {failure}"));
    let stat = output
        .detailed
        .table
        .get("stat_data")
        .cloned()
        .unwrap_or(Value::Null);
    let statuses = output
        .detailed
        .results
        .iter()
        .map(|result| result.status)
        .collect();
    (stat, statuses)
}

#[test]
fn zod_branch_of_the_replace_parity_contract_matches_on_quickjs() {
    let parity: Value = serde_json::from_str(include_str!(
        "../../../src/shared/contracts/mvu-replace-parity.json"
    ))
    .unwrap();
    let engine = MvuEngine::new(Limits::default());
    let statuses =
        |value: &Value| -> Vec<CommandStatus> { serde_json::from_value(value.clone()).unwrap() };
    for sample in parity["cases"].as_array().unwrap() {
        let name = sample["name"].as_str().unwrap();
        let message = format!(
            "<UpdateVariable>\n<JSONPatch>\n{}\n</JSONPatch>\n</UpdateVariable>",
            sample["patch"]
        );
        let (stat, got) = run(
            &engine,
            name,
            "z.object({})",
            &sample["stat_data"],
            &message,
        );
        assert_eq!(stat, sample["zod"]["expected"], "{name}");
        assert_eq!(got, statuses(&sample["zod"]["statuses"]), "{name}");
    }
    let zod_cases = parity["zod_cases"].as_array().unwrap();
    assert!(!zod_cases.is_empty());
    for sample in zod_cases {
        let name = sample["name"].as_str().unwrap();
        let (stat, got) = run(
            &engine,
            name,
            sample["schema"].as_str().unwrap(),
            &sample["stat_data"],
            sample["message"].as_str().unwrap(),
        );
        assert_eq!(stat, sample["expected"], "{name}");
        assert_eq!(got, statuses(&sample["statuses"]), "{name}");
    }
}
