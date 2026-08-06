//! 安全的算術表達式引擎：`derived` 衍生值公式與骰值欄位共用同一個核心。純自寫的
//! tokenize → 遞迴下降 parse → eval 三段式，不呼叫任何外部執行環境、不掛任何直譯器
//! ——同 `ejs.rs` 那條紅線：卡片／規則裡的文字永遠當資料，永不執行任意程式。
//!
//! 語法：四則 `+ - * / %`、括號、一元負號 `-`；比較 `> >= < <= == !=`；邏輯 `&& || !`
//! （`&&`／`||` 短路求值，只算用得到的那一半）；函式 `min` `max`（可變參數，至少 2 個）、
//! `floor` `ceil` `round`（各 1 個參數）、`if(cond, then, else)`（3 個參數，只算命中的那支）。
//! 欄位路徑是點分字串（如 `World.威脅度`），一律以 `f64` 運算；比較／邏輯的真假用
//! 非 0.0／0.0 表示，不另立布林型別。
//!
//! token 白名單以外的字元、壞語法（含輸入截斷）、未知函式、路徑取不到值、除以零
//! 一律回 `Err(String)` 說哪裡壞，絕不 panic；巢狀深度另有上限，見 `MAX_DEPTH`。

/// 欄位取值來源：點分路徑 → 數值；查不到回 `None`。路徑怎麼在狀態樹上找、
/// `"500/500"` 這種現值/上限對怎麼取現值，都由呼叫端（`mechanism.rs`）決定。
pub fn eval(source: &str, lookup: &dyn Fn(&str) -> Option<f64>) -> Result<f64, String> {
    let tokens = tokenize(source)?;
    let mut parser = Parser::new(&tokens);
    let expr = parser.parse_expr()?;
    parser.expect_eof()?;
    eval_expr(&expr, lookup)
}

// ---------------------------------------------------------------------
// 詞法：字元 → token；白名單以外的字元一律報錯
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Path(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Gt,
    Ge,
    Lt,
    Le,
    EqEq,
    Ne,
    AndAnd,
    OrOr,
    Not,
    LParen,
    RParen,
    Comma,
    Eof,
}

fn is_path_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

fn is_path_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// 數字只收無號的整數／小數，負號一律交給 parser 的一元運算子處理。
/// 路徑吃連續的字母數字底線，`.` 只在後面緊接著另一個路徑起始字元時才併入同一個
/// token（`World.威脅度` 是一個 Path，但結尾的 `.` 不會被誤吃進去）。
fn tokenize(source: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch.is_ascii_digit() {
            let start = i;
            i += 1;
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
                i += 1;
                while chars.get(i).is_some_and(char::is_ascii_digit) {
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            let value = text
                .parse::<f64>()
                .map_err(|_| format!("不是合法的數字：{text}"))?;
            tokens.push(Token::Num(value));
            continue;
        }
        if is_path_start(ch) {
            let start = i;
            i += 1;
            while i < chars.len() {
                let dotted_path = chars[i] == '.' && chars.get(i + 1).copied().is_some_and(is_path_start);
                if is_path_char(chars[i]) || dotted_path {
                    i += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token::Path(chars[start..i].iter().collect()));
            continue;
        }
        let (token, width) = match (ch, chars.get(i + 1).copied()) {
            ('>', Some('=')) => (Token::Ge, 2),
            ('>', _) => (Token::Gt, 1),
            ('<', Some('=')) => (Token::Le, 2),
            ('<', _) => (Token::Lt, 1),
            ('=', Some('=')) => (Token::EqEq, 2),
            ('!', Some('=')) => (Token::Ne, 2),
            ('!', _) => (Token::Not, 1),
            ('&', Some('&')) => (Token::AndAnd, 2),
            ('|', Some('|')) => (Token::OrOr, 2),
            ('+', _) => (Token::Plus, 1),
            ('-', _) => (Token::Minus, 1),
            ('*', _) => (Token::Star, 1),
            ('/', _) => (Token::Slash, 1),
            ('%', _) => (Token::Percent, 1),
            ('(', _) => (Token::LParen, 1),
            (')', _) => (Token::RParen, 1),
            (',', _) => (Token::Comma, 1),
            (other, _) => return Err(format!("不認得的字元：{other}")),
        };
        tokens.push(token);
        i += width;
    }
    tokens.push(Token::Eof);
    Ok(tokens)
}

// ---------------------------------------------------------------------
// 語法樹
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Path(String),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy)]
enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
    And,
    Or,
}

// ---------------------------------------------------------------------
// 遞迴下降 parser：由弱到強分層——|| → && → ==/!= → 比較 → +- → */%  → 一元 → 主項
// ---------------------------------------------------------------------

/// 巢狀深度上限。括號、函式參數、連續一元運算子最終都會回到 `parse_unary`，
/// 這裡是唯一的守門——上千層括號這種病態輸入在這裡報錯收場，不會把呼叫堆疊撐爆。
/// 正常公式的巢狀深度個位數，128 留了充裕的餘裕。
const MAX_DEPTH: u32 = 128;

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    depth: u32,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    /// token 序列固定以 `Eof` 收尾，卡在 `Eof` 上不再前進——輸入截斷時後續的
    /// `expect` 只會不斷比對到 `Eof` 而報錯，不會索引越界。
    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        if *self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            Err(format!("預期 {expected:?}，卻遇到 {:?}", self.peek()))
        }
    }

    fn expect_eof(&mut self) -> Result<(), String> {
        if *self.peek() == Token::Eof {
            Ok(())
        } else {
            Err(format!("算式後面還有多餘的內容：{:?}", self.peek()))
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while *self.peek() == Token::OrOr {
            self.advance();
            left = Expr::Binary(BinOp::Or, Box::new(left), Box::new(self.parse_and()?));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;
        while *self.peek() == Token::AndAnd {
            self.advance();
            left = Expr::Binary(BinOp::And, Box::new(left), Box::new(self.parse_equality()?));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Token::EqEq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                _ => return Ok(left),
            };
            self.advance();
            left = Expr::Binary(op, Box::new(left), Box::new(self.parse_comparison()?));
        }
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                _ => return Ok(left),
            };
            self.advance();
            left = Expr::Binary(op, Box::new(left), Box::new(self.parse_additive()?));
        }
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => return Ok(left),
            };
            self.advance();
            left = Expr::Binary(op, Box::new(left), Box::new(self.parse_multiplicative()?));
        }
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => return Ok(left),
            };
            self.advance();
            left = Expr::Binary(op, Box::new(left), Box::new(self.parse_unary()?));
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        self.depth += 1;
        let result = if self.depth > MAX_DEPTH {
            Err("算式巢狀太深".to_owned())
        } else {
            match self.peek() {
                Token::Minus => {
                    self.advance();
                    self.parse_unary()
                        .map(|inner| Expr::Unary(UnaryOp::Neg, Box::new(inner)))
                }
                Token::Not => {
                    self.advance();
                    self.parse_unary()
                        .map(|inner| Expr::Unary(UnaryOp::Not, Box::new(inner)))
                }
                _ => self.parse_primary(),
            }
        };
        self.depth -= 1;
        result
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.advance() {
            Token::Num(value) => Ok(Expr::Num(value)),
            Token::Path(name) => {
                if *self.peek() == Token::LParen {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Path(name))
                }
            }
            Token::LParen => {
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(inner)
            }
            other => Err(format!("這裡該是數值、欄位路徑或括號，卻遇到 {other:?}")),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if *self.peek() == Token::RParen {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while *self.peek() == Token::Comma {
            self.advance();
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }
}

// ---------------------------------------------------------------------
// eval：語法樹＋欄位取值來源 → 數值
// ---------------------------------------------------------------------

fn from_bool(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

/// 左結合鏈（`1+1+1+…`）在 parser 裡是用迴圈拼出來的，但拼出來的樹是左深的巢狀
/// `Binary`——按樹的形狀遞迴求值，算式長度會直接變成呼叫堆疊深度，長公式一樣會把
/// 堆疊撐爆。這裡先沿著左枝把鏈攤平成一條 `Vec` 再迭代摺算：鏈多長都只吃堆積體，
/// 不吃呼叫堆疊。真正的巢狀（括號展開後的子式、一元運算子連寫、函式參數）才會
/// 遞迴下去，而那些在 parse 階段已經被 `MAX_DEPTH` 頂住，遞迴進去是安全的。
fn eval_expr(expr: &Expr, lookup: &dyn Fn(&str) -> Option<f64>) -> Result<f64, String> {
    let mut chain = Vec::new();
    let mut head = expr;
    while let Expr::Binary(op, left, right) = head {
        chain.push((*op, right.as_ref()));
        head = left.as_ref();
    }
    let mut acc = match head {
        Expr::Num(value) => *value,
        Expr::Path(path) => lookup(path).ok_or_else(|| format!("找不到欄位：{path}"))?,
        Expr::Unary(UnaryOp::Neg, operand) => -eval_expr(operand, lookup)?,
        Expr::Unary(UnaryOp::Not, operand) => from_bool(eval_expr(operand, lookup)? == 0.0),
        Expr::Call(name, args) => eval_call(name, args, lookup)?,
        Expr::Binary(..) => unreachable!("while 迴圈已經把 Binary 節點沿左枝攤平掉了"),
    };
    // && 與 || 短路：只算用得到的那一半，「威脅低 && 危險算式」這種寫法在威脅低時
    // 不會因為危險算式報錯（例如除以零）而白白讓整條公式失敗。
    for (op, operand) in chain.into_iter().rev() {
        acc = match op {
            BinOp::And if acc == 0.0 => 0.0,
            BinOp::And => from_bool(eval_expr(operand, lookup)? != 0.0),
            BinOp::Or if acc != 0.0 => 1.0,
            BinOp::Or => from_bool(eval_expr(operand, lookup)? != 0.0),
            _ => {
                let value = eval_expr(operand, lookup)?;
                eval_binary(op, acc, value)?
            }
        };
    }
    Ok(acc)
}

fn eval_binary(op: BinOp, left: f64, right: f64) -> Result<f64, String> {
    match op {
        BinOp::Add => Ok(left + right),
        BinOp::Sub => Ok(left - right),
        BinOp::Mul => Ok(left * right),
        BinOp::Div if right == 0.0 => Err("除以零".to_owned()),
        BinOp::Div => Ok(left / right),
        BinOp::Mod if right == 0.0 => Err("除以零".to_owned()),
        BinOp::Mod => Ok(left % right),
        BinOp::Gt => Ok(from_bool(left > right)),
        BinOp::Ge => Ok(from_bool(left >= right)),
        BinOp::Lt => Ok(from_bool(left < right)),
        BinOp::Le => Ok(from_bool(left <= right)),
        BinOp::Eq => Ok(from_bool(left == right)),
        BinOp::Ne => Ok(from_bool(left != right)),
        BinOp::And | BinOp::Or => unreachable!("&& 和 || 在 eval_expr 就短路處理掉了"),
    }
}

/// 函式白名單就這六個；`min`／`max` 可變參數（至少 2 個），其餘固定元數。
/// 元數不對、名字不認得都回 `Err`，不猜測、不容錯。
fn eval_call(name: &str, args: &[Expr], lookup: &dyn Fn(&str) -> Option<f64>) -> Result<f64, String> {
    match name {
        "min" | "max" => {
            if args.len() < 2 {
                return Err(format!("{name} 至少要 2 個參數"));
            }
            let mut values = args.iter().map(|arg| eval_expr(arg, lookup));
            let mut acc = values.next().expect("已檢查長度 >= 2")?;
            for value in values {
                let value = value?;
                acc = if name == "min" { acc.min(value) } else { acc.max(value) };
            }
            Ok(acc)
        }
        "floor" | "ceil" | "round" => {
            let [arg] = args else {
                return Err(format!("{name} 要剛好 1 個參數"));
            };
            let value = eval_expr(arg, lookup)?;
            Ok(match name {
                "floor" => value.floor(),
                "ceil" => value.ceil(),
                _ => value.round(),
            })
        }
        "if" => {
            let [cond, then_branch, else_branch] = args else {
                return Err("if 要剛好 3 個參數".to_owned());
            };
            if eval_expr(cond, lookup)? != 0.0 {
                eval_expr(then_branch, lookup)
            } else {
                eval_expr(else_branch, lookup)
            }
        }
        other => Err(format!("不認得的函式：{other}")),
    }
}

#[cfg(test)]
mod tests {
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
}
