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
                let dotted_path =
                    chars[i] == '.' && chars.get(i + 1).copied().is_some_and(is_path_start);
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
fn eval_call(
    name: &str,
    args: &[Expr],
    lookup: &dyn Fn(&str) -> Option<f64>,
) -> Result<f64, String> {
    match name {
        "min" | "max" => {
            if args.len() < 2 {
                return Err(format!("{name} 至少要 2 個參數"));
            }
            let mut values = args.iter().map(|arg| eval_expr(arg, lookup));
            let mut acc = values.next().expect("已檢查長度 >= 2")?;
            for value in values {
                let value = value?;
                acc = if name == "min" {
                    acc.min(value)
                } else {
                    acc.max(value)
                };
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
mod tests;
