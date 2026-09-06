use super::*;

fn no_fields(_: &str) -> Option<f64> {
    None
}

fn lookup_from(pairs: &'static [(&'static str, f64)]) -> impl Fn(&str) -> Option<f64> {
    move |path| {
        pairs
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, value)| *value)
    }
}

// ---- 四則與優先序 ----

#[test]
fn arithmetic_follows_standard_precedence() {
    assert_eq!(eval("1+2*3", &no_fields), Ok(7.0));
    assert_eq!(eval("2*3+1", &no_fields), Ok(7.0));
    assert_eq!(eval("10-4/2", &no_fields), Ok(8.0));
    assert_eq!(eval("7%3", &no_fields), Ok(1.0));
}

/// eval 把左結合鏈攤平成迭代摺算（見 eval_expr 的註解）；這裡確認攤平沒有
/// 弄丟不同優先層級交錯時的分組——`2*3` 要先算完再跟外層的 `+`／`>`／`&&` 結合。
#[test]
fn chains_spanning_multiple_precedence_tiers_still_group_correctly() {
    assert_eq!(eval("1*2+3*4", &no_fields), Ok(14.0));
    assert_eq!(eval("1+2>3", &no_fields), Ok(0.0));
    assert_eq!(eval("1==1 && 2==2", &no_fields), Ok(1.0));
    assert_eq!(eval("1+2*3>5 && 10-3==7", &no_fields), Ok(1.0));
}

// ---- 括號與一元負號 ----

#[test]
fn parens_override_precedence_and_unary_minus_negates() {
    assert_eq!(eval("(1+2)*3", &no_fields), Ok(9.0));
    assert_eq!(eval("-(3+2)", &no_fields), Ok(-5.0));
    assert_eq!(eval("- -5", &no_fields), Ok(5.0));
    assert_eq!(eval("3 - -2", &no_fields), Ok(5.0));
}

// ---- min/max/floor/ceil/round ----

#[test]
fn min_and_max_take_two_or_more_args() {
    assert_eq!(eval("min(3,1,2)", &no_fields), Ok(1.0));
    assert_eq!(eval("max(3,1,2)", &no_fields), Ok(3.0));
    assert!(eval("min(1)", &no_fields).is_err());
}

#[test]
fn floor_ceil_round_take_exactly_one_arg() {
    assert_eq!(eval("floor(3.7)", &no_fields), Ok(3.0));
    assert_eq!(eval("ceil(3.2)", &no_fields), Ok(4.0));
    assert_eq!(eval("round(3.5)", &no_fields), Ok(4.0));
    assert!(eval("floor(1,2)", &no_fields).is_err());
}

// ---- if＋比較＋邏輯 ----

#[test]
fn if_picks_a_branch_by_comparison_and_logic() {
    assert_eq!(eval("if(3>2, 10, 20)", &no_fields), Ok(10.0));
    assert_eq!(eval("if(3<2, 10, 20)", &no_fields), Ok(20.0));
    assert_eq!(eval("if(3>2 && 1==1, 1, 0)", &no_fields), Ok(1.0));
    assert_eq!(eval("if(3<2 || 1!=1, 1, 0)", &no_fields), Ok(0.0));
    assert_eq!(eval("!(1==2)", &no_fields), Ok(1.0));
}

#[test]
fn logic_operators_short_circuit_the_untaken_side() {
    // 左半就決定結果時，右半即使會出錯（除以零）也不該被算到。
    assert_eq!(eval("0 && 1/0", &no_fields), Ok(0.0));
    assert_eq!(eval("1 || 1/0", &no_fields), Ok(1.0));
}

// ---- 路徑取值 ----

#[test]
fn field_paths_resolve_through_the_lookup_closure() {
    let lookup = lookup_from(&[("HP", 10.0), ("World.威脅度", 60.0)]);
    assert_eq!(eval("HP+1", &lookup), Ok(11.0));
    assert_eq!(eval("World.威脅度/2", &lookup), Ok(30.0));
}

#[test]
fn missing_field_path_is_an_error() {
    assert!(eval("Missing+1", &no_fields).is_err());
}

// ---- 除以零 ----

#[test]
fn division_and_modulo_by_zero_are_errors() {
    assert!(eval("1/0", &no_fields).is_err());
    assert!(eval("1%0", &no_fields).is_err());
}

// ---- 壞語法（含輸入截斷）----

#[test]
fn malformed_and_truncated_input_is_an_error_not_a_panic() {
    assert!(eval("", &no_fields).is_err());
    assert!(eval("1+", &no_fields).is_err());
    assert!(eval("min(1,", &no_fields).is_err());
    assert!(eval("(1+2", &no_fields).is_err());
    assert!(eval("1 2", &no_fields).is_err());
    assert!(eval("1 @ 2", &no_fields).is_err());
}

#[test]
fn unknown_function_name_is_an_error() {
    assert!(eval("foo(1)", &no_fields).is_err());
}

// ---- 深巢狀不 stack overflow ----

#[test]
fn deeply_nested_parens_error_out_instead_of_overflowing_the_stack() {
    let source = format!("{}1{}", "(".repeat(300), ")".repeat(300));
    assert!(eval(&source, &no_fields).is_err());
}

#[test]
fn long_flat_chains_are_iterative_and_do_not_overflow() {
    let source = std::iter::repeat_n("1", 5000).collect::<Vec<_>>().join("+");
    assert_eq!(eval(&source, &no_fields), Ok(5000.0));
}
