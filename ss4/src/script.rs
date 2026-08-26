//! Движок скриптов: парсинг, компиляция и выполнение PRE/POST выражений и операторов.

use std::collections::HashMap;

#[derive(Clone, Debug)]
/// Событие `emit(ts, reg_id, value)`, сформированное скриптом.
pub struct EmitEvent {
    pub ts: f64,
    pub reg_id: i32,
    pub value: f64,
}

#[derive(Clone, Debug)]
/// Результат выполнения скрипта: значения регистров и список `emit`-событий.
pub struct EvalResult {
    pub regs: HashMap<i32, f64>,
    pub emits: Vec<EmitEvent>,
}

#[derive(Clone, Debug)]
/// Скомпилированный скрипт (инструкции VM + список зависимостей `rv`).
pub struct Script {
    instrs: Vec<Instr>,
    used_rv_keys: Vec<i32>,
}

impl Script {
    /// Парсит исходный текст и компилирует его во внутренний набор инструкций.
    ///
    /// # Parameters
    /// - `src`: текст скрипта DSL.
    ///
    /// # Returns
    /// - `Ok(Script)`: скомпилированный скрипт.
    /// - `Err(String)`: синтаксическая/семантическая ошибка разбора.
    pub fn parse(src: &str) -> Result<Script, String> {
        let mut t = Tokenizer::new(src);
        let mut stmts = Vec::new();

        while t.peek().kind != TokKind::Eof {
            if t.peek_text(";") {
                t.next();
                continue;
            }
            stmts.push(parse_stmt(&mut t, true)?);
        }

        let instrs = compile_stmts(&stmts)?;
        let mut keys = Vec::new();
        for st in &stmts {
            collect_used_rv_stmt(st, &mut keys);
        }
        keys.sort_unstable();
        keys.dedup();
        Ok(Script {
            instrs,
            used_rv_keys: keys,
        })
    }

    /// Возвращает уникальный список константных ключей, используемых в `rv(...)`.
    ///
    /// # Returns
    /// - Срез ключей `rv`, отсортированный по возрастанию.
    pub fn used_rv_keys(&self) -> &[i32] {
        &self.used_rv_keys
    }

    /// Выполняет скомпилированный скрипт и возвращает рассчитанные `reg(...)` и `emit(...)`.
    ///
    /// # Parameters
    /// - `words`: считанные слова Modbus для функций `u16/i16/u32/i32/f32/bit`.
    /// - `hi_lo`: порядок слов для 32-битных преобразований.
    /// - `reg_value`: колбэк доступа к текущим `rv(...)` значениям.
    /// - `arx_value`: колбэк доступа к `av(kpz, reg)` значениям архива.
    /// - `on_print`: необязательный колбэк для `print`.
    /// - `on_emit`: необязательный колбэк для `emit`.
    /// - `max_steps`: ограничение числа VM-шагов.
    ///
    /// # Returns
    /// - `Ok(EvalResult)`: результат вычисления.
    /// - `Err(String)`: ошибка выполнения (например, превышен `max_steps`).
    pub fn eval_result(
        &self,
        words: &[u16],
        hi_lo: bool,
        reg_value: &dyn Fn(i32) -> f64,
        arx_value: &dyn Fn(i32, i32) -> f64,
        on_print: Option<&dyn Fn(&str)>,
        on_emit: Option<&dyn Fn(f64, i32, f64)>,
        max_steps: usize,
    ) -> Result<EvalResult, String> {
        let mut vars: HashMap<String, f64> = HashMap::new();
        let mut regs: HashMap<i32, f64> = HashMap::new();
        let mut emits: Vec<EmitEvent> = Vec::new();

        let mut stack: Vec<f64> = Vec::new();
        let mut ip: usize = 0;
        let mut steps: usize = 0;

        let mut step = || -> Result<(), String> {
            steps += 1;
            if steps > max_steps {
                return Err(format!(
                    "SCRIPT: step limit exceeded (maxSteps={})",
                    max_steps
                ));
            }
            Ok(())
        };

        while ip < self.instrs.len() {
            match &self.instrs[ip] {
                Instr::Step => {
                    step()?;
                    ip += 1;
                }
                Instr::PushNum(v) => {
                    stack.push(*v);
                    ip += 1;
                }
                Instr::LoadVar(name) => {
                    let v = vars.get(name).copied().unwrap_or(0.0);
                    stack.push(v);
                    ip += 1;
                }
                Instr::StoreVar(name) => {
                    let v = stack.pop().unwrap_or(0.0);
                    vars.insert(name.clone(), v);
                    ip += 1;
                }
                Instr::Unary(op) => {
                    let a = stack.pop().unwrap_or(0.0);
                    let v = match op {
                        UnaryOp::Neg => -a,
                    };
                    stack.push(v);
                    ip += 1;
                }
                Instr::Bin(op) => {
                    let b = stack.pop().unwrap_or(0.0);
                    let a = stack.pop().unwrap_or(0.0);
                    let v = eval_bin(*op, a, b);
                    stack.push(v);
                    ip += 1;
                }
                Instr::Call1(fn1) => {
                    let a = stack.pop().unwrap_or(0.0);
                    let v = eval_call1(*fn1, a, words, hi_lo, reg_value, on_print);
                    stack.push(v);
                    ip += 1;
                }
                Instr::Call2(fn2) => {
                    let b = stack.pop().unwrap_or(0.0);
                    let a = stack.pop().unwrap_or(0.0);
                    let v = eval_call2(*fn2, a, b, words, hi_lo, arx_value, on_print);
                    stack.push(v);
                    ip += 1;
                }
                Instr::Call3(fn3) => {
                    let c = stack.pop().unwrap_or(0.0);
                    let b = stack.pop().unwrap_or(0.0);
                    let a = stack.pop().unwrap_or(0.0);
                    let v = eval_call3(*fn3, a, b, c);
                    stack.push(v);
                    ip += 1;
                }
                Instr::Reg => {
                    let value = stack.pop().unwrap_or(0.0);
                    let reg_id = stack.pop().unwrap_or(0.0) as i32;
                    if reg_id > 0 {
                        regs.insert(reg_id, value);
                    }
                    ip += 1;
                }
                Instr::Emit => {
                    let value = stack.pop().unwrap_or(0.0);
                    let reg_id = stack.pop().unwrap_or(0.0) as i32;
                    let ts = stack.pop().unwrap_or(0.0);
                    if reg_id > 0 {
                        emits.push(EmitEvent { ts, reg_id, value });
                        if let Some(cb) = on_emit {
                            cb(ts, reg_id, value);
                        }
                    }
                    ip += 1;
                }
                Instr::Jump(target) => {
                    ip = *target;
                }
                Instr::JumpIfZero(target) => {
                    let v = stack.pop().unwrap_or(0.0);
                    if v == 0.0 {
                        ip = *target;
                    } else {
                        ip += 1;
                    }
                }
            }
        }

        Ok(EvalResult { regs, emits })
    }
}

fn collect_used_rv_stmt(st: &Stmt, out: &mut Vec<i32>) {
    match st {
        Stmt::Let { expr, .. } | Stmt::Assign { expr, .. } => collect_used_rv_expr(expr, out),
        Stmt::Reg {
            reg_id_expr,
            value_expr,
        } => {
            collect_used_rv_expr(reg_id_expr, out);
            collect_used_rv_expr(value_expr, out);
        }
        Stmt::Emit {
            ts_expr,
            reg_id_expr,
            value_expr,
        } => {
            collect_used_rv_expr(ts_expr, out);
            collect_used_rv_expr(reg_id_expr, out);
            collect_used_rv_expr(value_expr, out);
        }
        Stmt::Block(v) => {
            for s in v {
                collect_used_rv_stmt(s, out);
            }
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_used_rv_expr(cond, out);
            collect_used_rv_stmt(then_branch, out);
            if let Some(e) = else_branch.as_deref() {
                collect_used_rv_stmt(e, out);
            }
        }
        Stmt::While { cond, body } => {
            collect_used_rv_expr(cond, out);
            collect_used_rv_stmt(body, out);
        }
        Stmt::For {
            init,
            cond,
            step,
            body,
        } => {
            if let Some(i) = init.as_deref() {
                collect_used_rv_stmt(i, out);
            }
            if let Some(c) = cond {
                collect_used_rv_expr(c, out);
            }
            if let Some(s) = step.as_deref() {
                collect_used_rv_stmt(s, out);
            }
            collect_used_rv_stmt(body, out);
        }
    }
}

fn collect_used_rv_expr(e: &Expr, out: &mut Vec<i32>) {
    match e {
        Expr::Num(_) | Expr::Var(_) => {}
        Expr::Unary { a, .. } => collect_used_rv_expr(a, out),
        Expr::Bin { l, r, .. } => {
            collect_used_rv_expr(l, out);
            collect_used_rv_expr(r, out);
        }
        Expr::Call1 { func, a } => {
            if matches!(func, Call1::Rv) {
                if let Expr::Num(v) = a.as_ref() {
                    out.push(*v as i32);
                }
            }
            collect_used_rv_expr(a, out);
        }
        Expr::Call2 { a, b, .. } => {
            collect_used_rv_expr(a, out);
            collect_used_rv_expr(b, out);
        }
        Expr::Call3 { a, b, c, .. } => {
            collect_used_rv_expr(a, out);
            collect_used_rv_expr(b, out);
            collect_used_rv_expr(c, out);
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum UnaryOp {
    Neg,
}

#[derive(Clone, Copy, Debug)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Band,
    Bor,
    Bxor,
    Shl,
    Shr,
}

#[derive(Clone, Copy, Debug)]
enum Call1 {
    U16,
    I16,
    U32,
    I32,
    F32,
    Dt2Unix,
    Rv,
    Print,
    Abs,
    Sqrt,
    Floor,
    Ceil,
    Round,
}

#[derive(Clone, Copy, Debug)]
enum Call2 {
    Bit,
    Av,
    Print2,
    Min,
    Max,
    Pow,
}

#[derive(Clone, Copy, Debug)]
enum Call3 {
    Clamp,
}

#[derive(Clone, Debug)]
enum Instr {
    Step,
    PushNum(f64),
    LoadVar(String),
    StoreVar(String),
    Unary(UnaryOp),
    Bin(BinOp),
    Call1(Call1),
    Call2(Call2),
    Call3(Call3),
    Reg,
    Emit,
    Jump(usize),
    JumpIfZero(usize),
}

// ===== Parser =====

#[derive(Clone, Debug)]
enum Stmt {
    Let {
        name: String,
        expr: Expr,
    },
    Assign {
        name: String,
        expr: Expr,
    },
    Reg {
        reg_id_expr: Expr,
        value_expr: Expr,
    },
    Emit {
        ts_expr: Expr,
        reg_id_expr: Expr,
        value_expr: Expr,
    },
    Block(Vec<Stmt>),
    If {
        cond: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Box<Stmt>,
    },
}

#[derive(Clone, Debug)]
enum Expr {
    Num(f64),
    Var(String),
    Unary {
        op: UnaryOp,
        a: Box<Expr>,
    },
    Bin {
        op: BinOp,
        l: Box<Expr>,
        r: Box<Expr>,
    },
    Call1 {
        func: Call1,
        a: Box<Expr>,
    },
    Call2 {
        func: Call2,
        a: Box<Expr>,
        b: Box<Expr>,
    },
    Call3 {
        func: Call3,
        a: Box<Expr>,
        b: Box<Expr>,
        c: Box<Expr>,
    },
}
fn parse_stmt(t: &mut Tokenizer, require_semicolon: bool) -> Result<Stmt, String> {
    if t.peek_text("{") {
        t.next();
        let mut body = Vec::new();
        while !t.peek_text("}") {
            if t.peek().kind == TokKind::Eof {
                return Err("SCRIPT: missing \"}\"".to_string());
            }
            body.push(parse_stmt(t, true)?);
        }
        t.next();
        return Ok(Stmt::Block(body));
    }

    if t.is_kw("while") {
        t.next();
        t.expect("(")?;
        let cond = parse_expr_until(t, ")")?;
        t.expect(")")?;
        let body = parse_stmt(t, true)?;
        return Ok(Stmt::While {
            cond,
            body: Box::new(body),
        });
    }

    if t.is_kw("for") {
        t.next();
        t.expect("(")?;

        let init = if !t.peek_text(";") {
            Some(Box::new(parse_for_header_stmt(t, "for-init")?))
        } else {
            None
        };
        t.expect(";")?;

        let cond = if !t.peek_text(";") {
            Some(parse_expr_until(t, ";")?)
        } else {
            None
        };
        t.expect(";")?;

        let step = if !t.peek_text(")") {
            Some(Box::new(parse_for_header_stmt(t, "for-step")?))
        } else {
            None
        };
        t.expect(")")?;

        let body = parse_stmt(t, true)?;
        return Ok(Stmt::For {
            init,
            cond,
            step,
            body: Box::new(body),
        });
    }

    if t.is_kw("if") {
        t.next();
        t.expect("(")?;
        let cond = parse_expr_until(t, ")")?;
        t.expect(")")?;
        let then_branch = parse_stmt(t, true)?;
        let else_branch = if t.is_kw("else") {
            t.next();
            Some(Box::new(parse_stmt(t, true)?))
        } else {
            None
        };
        return Ok(Stmt::If {
            cond,
            then_branch: Box::new(then_branch),
            else_branch,
        });
    }

    if t.is_kw("let") {
        t.next();
        let name_tok = t.next();
        if name_tok.kind != TokKind::Ident || !is_ident(&name_tok.text) {
            return Err(format!("SCRIPT: bad var name \"{}\"", name_tok.text));
        }
        t.expect("=")?;
        let expr = parse_expr_until_semicolon_or_stop(t, require_semicolon)?;
        if require_semicolon {
            t.expect(";")?;
        }
        return Ok(Stmt::Let {
            name: name_tok.text,
            expr,
        });
    }

    if t.is_kw("reg") {
        t.next();
        t.expect("(")?;
        let reg_id_expr = parse_expr_until(t, ")")?;
        t.expect(")")?;
        t.expect("=")?;
        let value_expr = parse_expr_until_semicolon_or_stop(t, require_semicolon)?;
        if require_semicolon {
            t.expect(";")?;
        }
        return Ok(Stmt::Reg {
            reg_id_expr,
            value_expr,
        });
    }

    if t.is_kw("emit") {
        t.next();
        t.expect("(")?;
        let ts_expr = parse_expr_until(t, ",")?;
        t.expect(",")?;
        let reg_id_expr = parse_expr_until(t, ",")?;
        t.expect(",")?;
        let value_expr = parse_expr_until(t, ")")?;
        t.expect(")")?;
        if require_semicolon {
            t.expect(";")?;
        }
        return Ok(Stmt::Emit {
            ts_expr,
            reg_id_expr,
            value_expr,
        });
    }

    if t.is_kw("print") && t.peek_ahead_text(1, "(") {
        return Err(
            "SCRIPT: \"print(...)\" is not a statement. Use: let _ = print(...);".to_string(),
        );
    }

    if t.peek().kind == TokKind::Ident && is_ident(&t.peek().text) && t.peek_ahead_text(1, "=") {
        let name_tok = t.next();
        t.expect("=")?;
        let expr = parse_expr_until_semicolon_or_stop(t, require_semicolon)?;
        if require_semicolon {
            t.expect(";")?;
        }
        return Ok(Stmt::Assign {
            name: name_tok.text,
            expr,
        });
    }

    Err(format!(
        "SCRIPT: unknown statement near \"{}\"",
        t.peek().text
    ))
}

fn parse_expr_until(t: &mut Tokenizer, stop_text: &str) -> Result<Expr, String> {
    let start = t.peek().start;
    let mut depth = 0i32;
    let mut end = start;

    loop {
        let tok = t.peek();
        if tok.kind == TokKind::Eof {
            return Err(format!(
                "SCRIPT: unexpected EOF while reading expression (expected \"{}\")",
                stop_text
            ));
        }
        if depth == 0 && tok.text == stop_text {
            break;
        }
        if tok.text == "(" {
            depth += 1;
        } else if tok.text == ")" {
            depth -= 1;
        }
        let consumed = t.next();
        end = consumed.end;
    }

    let expr_text = t.s[start..end].trim();
    if expr_text.is_empty() {
        return Err(format!("SCRIPT: empty expression before \"{}\"", stop_text));
    }
    ExprParser::new(expr_text).parse()
}

fn parse_expr_until_semicolon_or_stop(
    t: &mut Tokenizer,
    require_semicolon: bool,
) -> Result<Expr, String> {
    let start = t.peek().start;
    let mut depth = 0i32;
    let mut end = start;

    loop {
        let tok = t.peek();
        if tok.kind == TokKind::Eof {
            if require_semicolon {
                return Err(
                    "SCRIPT: unexpected EOF while reading expression (expected \";\")".to_string(),
                );
            }
            break;
        }
        if depth == 0 && tok.text == ";" {
            break;
        }
        if tok.text == "(" {
            depth += 1;
        } else if tok.text == ")" {
            depth -= 1;
        }
        let consumed = t.next();
        end = consumed.end;
    }

    let expr_text = t.s[start..end].trim();
    if expr_text.is_empty() {
        return Err("SCRIPT: empty expression".to_string());
    }
    ExprParser::new(expr_text).parse()
}

fn parse_for_header_stmt(t: &mut Tokenizer, part_name: &str) -> Result<Stmt, String> {
    if t.peek().kind == TokKind::Ident && t.peek().text == "let" {
        t.next();
        let name_tok = t.next();
        if name_tok.kind != TokKind::Ident || !is_ident(&name_tok.text) {
            return Err(format!(
                "SCRIPT: bad var name \"{}\" in {}",
                name_tok.text, part_name
            ));
        }
        t.expect("=")?;
        let expr = parse_for_header_expr(t, part_name)?;
        return Ok(Stmt::Let {
            name: name_tok.text,
            expr,
        });
    }

    if t.peek().kind == TokKind::Ident && is_ident(&t.peek().text) && t.peek_ahead_text(1, "=") {
        let name_tok = t.next();
        t.expect("=")?;
        let expr = parse_for_header_expr(t, part_name)?;
        return Ok(Stmt::Assign {
            name: name_tok.text,
            expr,
        });
    }

    Err(format!(
        "SCRIPT: only \"let x = expr\" or \"x = expr\" is allowed in {}",
        part_name
    ))
}

fn parse_for_header_expr(t: &mut Tokenizer, part_name: &str) -> Result<Expr, String> {
    let start = t.peek().start;
    let mut depth = 0i32;
    let mut end = start;

    loop {
        let tok = t.peek();
        if tok.kind == TokKind::Eof {
            return Err(format!("SCRIPT: unexpected EOF in {}", part_name));
        }
        if depth == 0 && (tok.text == ";" || tok.text == ")") {
            break;
        }
        if tok.text == "(" {
            depth += 1;
        } else if tok.text == ")" {
            depth -= 1;
        }
        let consumed = t.next();
        end = consumed.end;
    }

    let expr_text = t.s[start..end].trim();
    if expr_text.is_empty() {
        return Err(format!("SCRIPT: empty expression in {}", part_name));
    }
    ExprParser::new(expr_text).parse()
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    for c in chars {
        if !(c == '_' || c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}

// ===== Expression parser =====

struct ExprParser<'a> {
    s: &'a str,
    t: ExprTokenizer<'a>,
}

impl<'a> ExprParser<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s,
            t: ExprTokenizer::new(s),
        }
    }

    fn parse(mut self) -> Result<Expr, String> {
        let e = self.parse_lor()?;
        if self.t.peek().kind != TokKind::Eof {
            return Err(format!(
                "SCRIPT expr: unexpected token \"{}\" in \"{}\"",
                self.t.peek().text,
                self.s
            ));
        }
        Ok(e)
    }

    fn parse_lor(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_land()?;
        while self.t.peek_text("||") {
            self.t.next();
            e = Expr::Bin {
                op: BinOp::Or,
                l: Box::new(e),
                r: Box::new(self.parse_land()?),
            };
        }
        Ok(e)
    }

    fn parse_land(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_cmp()?;
        while self.t.peek_text("&&") {
            self.t.next();
            e = Expr::Bin {
                op: BinOp::And,
                l: Box::new(e),
                r: Box::new(self.parse_cmp()?),
            };
        }
        Ok(e)
    }

    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_bor()?;
        loop {
            if self.t.peek_text("<=") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Le,
                    l: Box::new(e),
                    r: Box::new(self.parse_bor()?),
                };
            } else if self.t.peek_text("<") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Lt,
                    l: Box::new(e),
                    r: Box::new(self.parse_bor()?),
                };
            } else if self.t.peek_text(">=") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Ge,
                    l: Box::new(e),
                    r: Box::new(self.parse_bor()?),
                };
            } else if self.t.peek_text(">") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Gt,
                    l: Box::new(e),
                    r: Box::new(self.parse_bor()?),
                };
            } else if self.t.peek_text("==") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Eq,
                    l: Box::new(e),
                    r: Box::new(self.parse_bor()?),
                };
            } else if self.t.peek_text("!=") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Ne,
                    l: Box::new(e),
                    r: Box::new(self.parse_bor()?),
                };
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_bor(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_bxor()?;
        while self.t.peek_text("|") {
            self.t.next();
            e = Expr::Bin {
                op: BinOp::Bor,
                l: Box::new(e),
                r: Box::new(self.parse_bxor()?),
            };
        }
        Ok(e)
    }

    fn parse_bxor(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_band()?;
        while self.t.peek_text("^") {
            self.t.next();
            e = Expr::Bin {
                op: BinOp::Bxor,
                l: Box::new(e),
                r: Box::new(self.parse_band()?),
            };
        }
        Ok(e)
    }

    fn parse_band(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_shift()?;
        while self.t.peek_text("&") {
            self.t.next();
            e = Expr::Bin {
                op: BinOp::Band,
                l: Box::new(e),
                r: Box::new(self.parse_shift()?),
            };
        }
        Ok(e)
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_add()?;
        loop {
            if self.t.peek_text("<<") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Shl,
                    l: Box::new(e),
                    r: Box::new(self.parse_add()?),
                };
            } else if self.t.peek_text(">>") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Shr,
                    l: Box::new(e),
                    r: Box::new(self.parse_add()?),
                };
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_mul()?;
        loop {
            if self.t.peek_text("+") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Add,
                    l: Box::new(e),
                    r: Box::new(self.parse_mul()?),
                };
            } else if self.t.peek_text("-") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Sub,
                    l: Box::new(e),
                    r: Box::new(self.parse_mul()?),
                };
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_unary()?;
        loop {
            if self.t.peek_text("*") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Mul,
                    l: Box::new(e),
                    r: Box::new(self.parse_unary()?),
                };
            } else if self.t.peek_text("/") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Div,
                    l: Box::new(e),
                    r: Box::new(self.parse_unary()?),
                };
            } else if self.t.peek_text("%") {
                self.t.next();
                e = Expr::Bin {
                    op: BinOp::Mod,
                    l: Box::new(e),
                    r: Box::new(self.parse_unary()?),
                };
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.t.peek_text("-") {
            self.t.next();
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                a: Box::new(self.parse_unary()?),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let tok = self.t.peek().clone();
        if tok.kind == TokKind::Num {
            self.t.next();
            let v = if tok.text.contains('.') || tok.text.contains('e') || tok.text.contains('E') {
                tok.text
                    .parse::<f64>()
                    .map_err(|e| format!("SCRIPT expr: bad number {}: {}", tok.text, e))?
            } else {
                tok.text
                    .parse::<i64>()
                    .map_err(|e| format!("SCRIPT expr: bad int {}: {}", tok.text, e))?
                    as f64
            };
            return Ok(Expr::Num(v));
        }

        if tok.kind == TokKind::Ident {
            self.t.next();
            let name = tok.text;
            if self.t.peek_text("(") {
                self.t.next();
                let a = self.parse_lor()?;
                if self.t.peek_text(",") {
                    self.t.next();
                    let b = self.parse_lor()?;
                    if self.t.peek_text(",") {
                        self.t.next();
                        let c = self.parse_lor()?;
                        self.t.expect(")")?;
                        return Ok(Expr::Call3 {
                            func: map_call3(&name)?,
                            a: Box::new(a),
                            b: Box::new(b),
                            c: Box::new(c),
                        });
                    }
                    self.t.expect(")")?;
                    return Ok(Expr::Call2 {
                        func: map_call2(&name)?,
                        a: Box::new(a),
                        b: Box::new(b),
                    });
                }
                self.t.expect(")")?;
                return Ok(Expr::Call1 {
                    func: map_call1(&name)?,
                    a: Box::new(a),
                });
            }
            return Ok(Expr::Var(name));
        }

        if self.t.peek_text("(") {
            self.t.next();
            let e = self.parse_lor()?;
            self.t.expect(")")?;
            return Ok(e);
        }

        Err(format!(
            "SCRIPT expr: bad token \"{}\" in \"{}\"",
            tok.text, self.s
        ))
    }
}

fn map_call1(name: &str) -> Result<Call1, String> {
    match name {
        "u16" => Ok(Call1::U16),
        "i16" => Ok(Call1::I16),
        "u32" => Ok(Call1::U32),
        "i32" => Ok(Call1::I32),
        "f32" => Ok(Call1::F32),
        "dt2unix" => Ok(Call1::Dt2Unix),
        "rv" => Ok(Call1::Rv),
        "print" => Ok(Call1::Print),
        "abs" => Ok(Call1::Abs),
        "sqrt" => Ok(Call1::Sqrt),
        "floor" => Ok(Call1::Floor),
        "ceil" => Ok(Call1::Ceil),
        "round" => Ok(Call1::Round),
        _ => Err(format!("SCRIPT expr: unknown function \"{}\"", name)),
    }
}

fn map_call2(name: &str) -> Result<Call2, String> {
    match name {
        "bit" => Ok(Call2::Bit),
        "av" => Ok(Call2::Av),
        "print2" => Ok(Call2::Print2),
        "min" => Ok(Call2::Min),
        "max" => Ok(Call2::Max),
        "pow" => Ok(Call2::Pow),
        _ => Err(format!("SCRIPT expr: unknown function \"{}\"", name)),
    }
}

fn map_call3(name: &str) -> Result<Call3, String> {
    match name {
        "clamp" => Ok(Call3::Clamp),
        _ => Err(format!("SCRIPT expr: unknown function \"{}\"", name)),
    }
}
// ===== Bytecode compiler =====

fn compile_stmts(stmts: &[Stmt]) -> Result<Vec<Instr>, String> {
    let mut out = Vec::new();
    for st in stmts {
        compile_stmt(st, &mut out)?;
    }
    Ok(out)
}

fn compile_stmt(st: &Stmt, out: &mut Vec<Instr>) -> Result<(), String> {
    out.push(Instr::Step);
    match st {
        Stmt::Let { name, expr } => {
            compile_expr(expr, out)?;
            out.push(Instr::StoreVar(name.clone()));
        }
        Stmt::Assign { name, expr } => {
            compile_expr(expr, out)?;
            out.push(Instr::StoreVar(name.clone()));
        }
        Stmt::Reg {
            reg_id_expr,
            value_expr,
        } => {
            compile_expr(reg_id_expr, out)?;
            compile_expr(value_expr, out)?;
            out.push(Instr::Reg);
        }
        Stmt::Emit {
            ts_expr,
            reg_id_expr,
            value_expr,
        } => {
            compile_expr(ts_expr, out)?;
            compile_expr(reg_id_expr, out)?;
            compile_expr(value_expr, out)?;
            out.push(Instr::Emit);
        }
        Stmt::Block(body) => {
            for s in body {
                compile_stmt(s, out)?;
            }
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            compile_expr(cond, out)?;
            let jmp_if_zero = out.len();
            out.push(Instr::JumpIfZero(0));
            compile_stmt(then_branch, out)?;
            let jmp_end = out.len();
            out.push(Instr::Jump(0));
            let else_target = out.len();
            if let Some(eb) = else_branch {
                compile_stmt(eb, out)?;
            }
            let end_target = out.len();
            if let Instr::JumpIfZero(ref mut t) = out[jmp_if_zero] {
                *t = else_target;
            }
            if let Instr::Jump(ref mut t) = out[jmp_end] {
                *t = end_target;
            }
        }
        Stmt::While { cond, body } => {
            let loop_start = out.len();
            out.push(Instr::Step);
            compile_expr(cond, out)?;
            let jmp_if_zero = out.len();
            out.push(Instr::JumpIfZero(0));
            compile_stmt(body, out)?;
            out.push(Instr::Jump(loop_start));
            let end_target = out.len();
            if let Instr::JumpIfZero(ref mut t) = out[jmp_if_zero] {
                *t = end_target;
            }
        }
        Stmt::For {
            init,
            cond,
            step,
            body,
        } => {
            if let Some(init_stmt) = init {
                compile_stmt(init_stmt, out)?;
            }
            let loop_start = out.len();
            out.push(Instr::Step);
            if let Some(cond_expr) = cond {
                compile_expr(cond_expr, out)?;
            } else {
                out.push(Instr::PushNum(1.0));
            }
            let jmp_if_zero = out.len();
            out.push(Instr::JumpIfZero(0));
            compile_stmt(body, out)?;
            if let Some(step_stmt) = step {
                compile_stmt(step_stmt, out)?;
            }
            out.push(Instr::Jump(loop_start));
            let end_target = out.len();
            if let Instr::JumpIfZero(ref mut t) = out[jmp_if_zero] {
                *t = end_target;
            }
        }
    }
    Ok(())
}

fn compile_expr(e: &Expr, out: &mut Vec<Instr>) -> Result<(), String> {
    match e {
        Expr::Num(v) => out.push(Instr::PushNum(*v)),
        Expr::Var(name) => out.push(Instr::LoadVar(name.clone())),
        Expr::Unary { op, a } => {
            compile_expr(a, out)?;
            out.push(Instr::Unary(*op));
        }
        Expr::Bin { op, l, r } => {
            compile_expr(l, out)?;
            compile_expr(r, out)?;
            out.push(Instr::Bin(*op));
        }
        Expr::Call1 { func, a } => {
            compile_expr(a, out)?;
            out.push(Instr::Call1(*func));
        }
        Expr::Call2 { func, a, b } => {
            compile_expr(a, out)?;
            compile_expr(b, out)?;
            out.push(Instr::Call2(*func));
        }
        Expr::Call3 { func, a, b, c } => {
            compile_expr(a, out)?;
            compile_expr(b, out)?;
            compile_expr(c, out)?;
            out.push(Instr::Call3(*func));
        }
    }
    Ok(())
}

// ===== Runtime helpers =====

fn eval_bin(op: BinOp, a: f64, b: f64) -> f64 {
    match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => {
            if b == 0.0 {
                0.0
            } else {
                a / b
            }
        }
        BinOp::Mod => {
            let bi = b as i64;
            if bi == 0 {
                0.0
            } else {
                (a as i64 % bi) as f64
            }
        }
        BinOp::Lt => {
            if a < b {
                1.0
            } else {
                0.0
            }
        }
        BinOp::Le => {
            if a <= b {
                1.0
            } else {
                0.0
            }
        }
        BinOp::Gt => {
            if a > b {
                1.0
            } else {
                0.0
            }
        }
        BinOp::Ge => {
            if a >= b {
                1.0
            } else {
                0.0
            }
        }
        BinOp::Eq => {
            if a == b {
                1.0
            } else {
                0.0
            }
        }
        BinOp::Ne => {
            if a != b {
                1.0
            } else {
                0.0
            }
        }
        BinOp::And => {
            if a != 0.0 && b != 0.0 {
                1.0
            } else {
                0.0
            }
        }
        BinOp::Or => {
            if a != 0.0 || b != 0.0 {
                1.0
            } else {
                0.0
            }
        }
        BinOp::Band => ((a as i64) & (b as i64)) as f64,
        BinOp::Bor => ((a as i64) | (b as i64)) as f64,
        BinOp::Bxor => ((a as i64) ^ (b as i64)) as f64,
        BinOp::Shl => ((a as i64) << (b as i64)) as f64,
        BinOp::Shr => ((a as i64) >> (b as i64)) as f64,
    }
}

fn eval_call1(
    fn1: Call1,
    a: f64,
    words: &[u16],
    hi_lo: bool,
    reg_value: &dyn Fn(i32) -> f64,
    on_print: Option<&dyn Fn(&str)>,
) -> f64 {
    match fn1 {
        Call1::U16 => u16(words, a as i32) as f64,
        Call1::I16 => i16(words, a as i32) as f64,
        Call1::U32 => u32(words, a as i32, hi_lo) as f64,
        Call1::I32 => i32v(words, a as i32, hi_lo) as f64,
        Call1::F32 => f32v(words, a as i32, hi_lo) as f64,
        Call1::Dt2Unix => dt2unix(a),
        Call1::Rv => reg_value(a as i32),
        Call1::Print => {
            if let Some(p) = on_print {
                p(&a.to_string());
            }
            a
        }
        Call1::Abs => a.abs(),
        Call1::Sqrt => {
            if a <= 0.0 {
                0.0
            } else {
                a.sqrt()
            }
        }
        Call1::Floor => a.floor(),
        Call1::Ceil => a.ceil(),
        Call1::Round => a.round(),
    }
}

fn eval_call2(
    fn2: Call2,
    a: f64,
    b: f64,
    words: &[u16],
    _hi_lo: bool,
    arx_value: &dyn Fn(i32, i32) -> f64,
    on_print: Option<&dyn Fn(&str)>,
) -> f64 {
    match fn2 {
        Call2::Bit => bit(words, a as i32, b as i32) as f64,
        Call2::Av => arx_value(a as i32, b as i32),
        Call2::Print2 => {
            if let Some(p) = on_print {
                p(&format!("{}: {}", a as i32, b));
            }
            b
        }
        Call2::Min => {
            if a < b {
                a
            } else {
                b
            }
        }
        Call2::Max => {
            if a > b {
                a
            } else {
                b
            }
        }
        Call2::Pow => a.powf(b),
    }
}

fn eval_call3(fn3: Call3, a: f64, b: f64, c: f64) -> f64 {
    match fn3 {
        Call3::Clamp => {
            if a < b {
                b
            } else if a > c {
                c
            } else {
                a
            }
        }
    }
}

fn u16(words: &[u16], i: i32) -> i64 {
    if i < 0 || i as usize >= words.len() {
        return 0;
    }
    words[i as usize] as i64
}

fn i16(words: &[u16], i: i32) -> i64 {
    let v = u16(words, i);
    if (v & 0x8000) != 0 {
        v - 0x10000
    } else {
        v
    }
}

fn u32(words: &[u16], i: i32, hi_lo: bool) -> i64 {
    if i < 0 || (i + 1) as usize >= words.len() {
        return 0;
    }
    let w0 = words[i as usize] as i64;
    let w1 = words[(i + 1) as usize] as i64;
    if hi_lo {
        (w0 << 16) | w1
    } else {
        (w1 << 16) | w0
    }
}

fn dt2unix(raw_in: f64) -> f64 {
    if !raw_in.is_finite() || raw_in <= 0.0 {
        return 0.0;
    }

    let raw = raw_in.round() as i64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // 2000-01-01 .. 2100-01-01 in unix seconds.
    const MIN_SEC: i64 = 946_684_800;
    const MAX_SEC: i64 = 4_102_444_800;

    let swap16_words = |v: i64| -> i64 {
        let u = v as u64 as u32;
        let hi = (u >> 16) & 0xFFFF;
        let lo = u & 0xFFFF;
        ((lo << 16) | hi) as i64
    };

    let to_sec = |v: i64| -> [i64; 4] { [v, v / 1_000, v / 1_000_000, v / 1_000_000_000] };

    let mut best: Option<i64> = None;
    for base in [raw, swap16_words(raw)] {
        for cand in to_sec(base) {
            if !(MIN_SEC..=MAX_SEC).contains(&cand) {
                continue;
            }
            match best {
                None => best = Some(cand),
                Some(prev) => {
                    if (cand - now).abs() < (prev - now).abs() {
                        best = Some(cand);
                    }
                }
            }
        }
    }

    best.unwrap_or(raw) as f64
}

fn i32v(words: &[u16], i: i32, hi_lo: bool) -> i64 {
    let v = u32(words, i, hi_lo);
    if (v & 0x8000_0000) != 0 {
        v - 0x1_0000_0000
    } else {
        v
    }
}

fn f32v(words: &[u16], i: i32, hi_lo: bool) -> f32 {
    if i < 0 || (i + 1) as usize >= words.len() {
        return 0.0;
    }
    let w0 = words[i as usize];
    let w1 = words[(i + 1) as usize];
    let (hi, lo) = if hi_lo { (w0, w1) } else { (w1, w0) };
    let bytes = [
        ((hi >> 8) & 0xFF) as u8,
        (hi & 0xFF) as u8,
        ((lo >> 8) & 0xFF) as u8,
        (lo & 0xFF) as u8,
    ];
    f32::from_be_bytes(bytes)
}

fn bit(words: &[u16], i: i32, b: i32) -> i64 {
    if i < 0 || i as usize >= words.len() {
        return 0;
    }
    if !(0..=15).contains(&b) {
        return 0;
    }
    let w = words[i as usize];
    ((w >> b) & 1) as i64
}
// ===== Tokenizers =====

#[derive(Clone, Debug, PartialEq)]
enum TokKind {
    Num,
    Ident,
    Sym,
    Eof,
}

#[derive(Clone, Debug)]
struct Tok {
    kind: TokKind,
    text: String,
    start: usize,
    end: usize,
}

struct Tokenizer<'a> {
    s: &'a str,
    i: usize,
    next_tok: Tok,
}

impl<'a> Tokenizer<'a> {
    fn new(s: &'a str) -> Self {
        let mut t = Self {
            s,
            i: 0,
            next_tok: Tok {
                kind: TokKind::Eof,
                text: "".to_string(),
                start: 0,
                end: 0,
            },
        };
        let tok = t.read();
        t.next_tok = tok;
        t
    }

    fn peek(&self) -> &Tok {
        &self.next_tok
    }

    fn peek_text(&self, t: &str) -> bool {
        self.next_tok.text == t
    }

    fn is_kw(&self, k: &str) -> bool {
        self.next_tok.kind == TokKind::Ident && self.next_tok.text == k
    }

    fn peek_ahead_text(&mut self, n: usize, text: &str) -> bool {
        let save_i = self.i;
        let save_tok = self.next_tok.clone();
        let mut tok = self.next_tok.clone();
        for _ in 0..n {
            tok = self.read();
        }
        let ok = tok.text == text;
        self.i = save_i;
        self.next_tok = save_tok;
        ok
    }

    fn next(&mut self) -> Tok {
        let cur = self.next_tok.clone();
        self.next_tok = self.read();
        cur
    }

    fn expect(&mut self, t: &str) -> Result<(), String> {
        if self.next_tok.text != t {
            return Err(format!(
                "SCRIPT: expected \"{}\", got \"{}\"",
                t, self.next_tok.text
            ));
        }
        self.next();
        Ok(())
    }

    fn read(&mut self) -> Tok {
        self.skip_ws_and_comments();
        if self.i >= self.s.len() {
            return Tok {
                kind: TokKind::Eof,
                text: "".to_string(),
                start: self.i,
                end: self.i,
            };
        }

        let start = self.i;
        let bytes = self.s.as_bytes();

        if self.i + 1 < self.s.len() {
            let two = &self.s[self.i..self.i + 2];
            if matches!(two, "<<" | ">>" | "<=" | ">=" | "==" | "!=" | "&&" | "||") {
                self.i += 2;
                return Tok {
                    kind: TokKind::Sym,
                    text: two.to_string(),
                    start,
                    end: self.i,
                };
            }
        }

        let ch = bytes[self.i] as char;
        let syms = "(){};=+-*/%,&|^<>!";
        if syms.contains(ch) {
            self.i += 1;
            return Tok {
                kind: TokKind::Sym,
                text: ch.to_string(),
                start,
                end: self.i,
            };
        }

        if is_digit(ch) || ch == '.' {
            if ch == '.'
                && (self.i + 1 >= self.s.len() || !is_digit(self.s.as_bytes()[self.i + 1] as char))
            {
                return Tok {
                    kind: TokKind::Sym,
                    text: ".".to_string(),
                    start,
                    end: self.i + 1,
                };
            }

            if is_digit(ch) {
                while self.i < self.s.len() && is_digit(self.s.as_bytes()[self.i] as char) {
                    self.i += 1;
                }
            }

            if self.i < self.s.len() && self.s.as_bytes()[self.i] as char == '.' {
                self.i += 1;
                while self.i < self.s.len() && is_digit(self.s.as_bytes()[self.i] as char) {
                    self.i += 1;
                }
            }

            if self.i < self.s.len() {
                let c = self.s.as_bytes()[self.i] as char;
                if c == 'e' || c == 'E' {
                    let exp_pos = self.i;
                    self.i += 1;
                    if self.i < self.s.len() {
                        let c2 = self.s.as_bytes()[self.i] as char;
                        if c2 == '+' || c2 == '-' {
                            self.i += 1;
                        }
                    }
                    let exp_digits_start = self.i;
                    while self.i < self.s.len() && is_digit(self.s.as_bytes()[self.i] as char) {
                        self.i += 1;
                    }
                    if self.i == exp_digits_start {
                        self.i = exp_pos;
                    }
                }
            }

            return Tok {
                kind: TokKind::Num,
                text: self.s[start..self.i].to_string(),
                start,
                end: self.i,
            };
        }

        if is_ident_start(ch) {
            self.i += 1;
            while self.i < self.s.len() && is_ident_continue(self.s.as_bytes()[self.i] as char) {
                self.i += 1;
            }
            return Tok {
                kind: TokKind::Ident,
                text: self.s[start..self.i].to_string(),
                start,
                end: self.i,
            };
        }

        Tok {
            kind: TokKind::Sym,
            text: ch.to_string(),
            start,
            end: self.i + 1,
        }
    }

    fn skip_ws_and_comments(&mut self) {
        while self.i < self.s.len() {
            let b = self.s.as_bytes()[self.i];
            if b <= 32 {
                self.i += 1;
                continue;
            }
            if self.s.as_bytes()[self.i] as char == '#' {
                while self.i < self.s.len() && self.s.as_bytes()[self.i] as char != '\n' {
                    self.i += 1;
                }
                continue;
            }
            if self.i + 1 < self.s.len()
                && self.s.as_bytes()[self.i] as char == '/'
                && self.s.as_bytes()[self.i + 1] as char == '/'
            {
                self.i += 2;
                while self.i < self.s.len() && self.s.as_bytes()[self.i] as char != '\n' {
                    self.i += 1;
                }
                continue;
            }
            break;
        }
    }
}

struct ExprTokenizer<'a> {
    s: &'a str,
    i: usize,
    next_tok: Tok,
}

impl<'a> ExprTokenizer<'a> {
    fn new(s: &'a str) -> Self {
        let mut t = Self {
            s,
            i: 0,
            next_tok: Tok {
                kind: TokKind::Eof,
                text: "".to_string(),
                start: 0,
                end: 0,
            },
        };
        let tok = t.read();
        t.next_tok = tok;
        t
    }

    fn peek(&self) -> &Tok {
        &self.next_tok
    }

    fn peek_text(&self, t: &str) -> bool {
        self.next_tok.text == t
    }

    fn next(&mut self) -> Tok {
        let cur = self.next_tok.clone();
        self.next_tok = self.read();
        cur
    }

    fn expect(&mut self, t: &str) -> Result<(), String> {
        if self.next_tok.text != t {
            return Err(format!(
                "SCRIPT expr: expected \"{}\", got \"{}\"",
                t, self.next_tok.text
            ));
        }
        self.next();
        Ok(())
    }

    fn read(&mut self) -> Tok {
        while self.i < self.s.len() && self.s.as_bytes()[self.i] <= 32 {
            self.i += 1;
        }
        if self.i >= self.s.len() {
            return Tok {
                kind: TokKind::Eof,
                text: "".to_string(),
                start: self.i,
                end: self.i,
            };
        }

        let start = self.i;

        if self.i + 1 < self.s.len() {
            let two = &self.s[self.i..self.i + 2];
            if matches!(two, "<<" | ">>" | "<=" | ">=" | "==" | "!=" | "&&" | "||") {
                self.i += 2;
                return Tok {
                    kind: TokKind::Sym,
                    text: two.to_string(),
                    start,
                    end: self.i,
                };
            }
        }

        let ch = self.s.as_bytes()[self.i] as char;
        let syms = "(){};=+-*/%,&|^<>!";
        if syms.contains(ch) {
            self.i += 1;
            return Tok {
                kind: TokKind::Sym,
                text: ch.to_string(),
                start,
                end: self.i,
            };
        }

        if is_digit(ch) || ch == '.' {
            if ch == '.'
                && (self.i + 1 >= self.s.len() || !is_digit(self.s.as_bytes()[self.i + 1] as char))
            {
                return Tok {
                    kind: TokKind::Sym,
                    text: ".".to_string(),
                    start,
                    end: self.i + 1,
                };
            }

            if is_digit(ch) {
                while self.i < self.s.len() && is_digit(self.s.as_bytes()[self.i] as char) {
                    self.i += 1;
                }
            }
            if self.i < self.s.len() && self.s.as_bytes()[self.i] as char == '.' {
                self.i += 1;
                while self.i < self.s.len() && is_digit(self.s.as_bytes()[self.i] as char) {
                    self.i += 1;
                }
            }
            if self.i < self.s.len() {
                let c = self.s.as_bytes()[self.i] as char;
                if c == 'e' || c == 'E' {
                    let exp_pos = self.i;
                    self.i += 1;
                    if self.i < self.s.len() {
                        let c2 = self.s.as_bytes()[self.i] as char;
                        if c2 == '+' || c2 == '-' {
                            self.i += 1;
                        }
                    }
                    let exp_digits_start = self.i;
                    while self.i < self.s.len() && is_digit(self.s.as_bytes()[self.i] as char) {
                        self.i += 1;
                    }
                    if self.i == exp_digits_start {
                        self.i = exp_pos;
                    }
                }
            }
            return Tok {
                kind: TokKind::Num,
                text: self.s[start..self.i].to_string(),
                start,
                end: self.i,
            };
        }

        if is_ident_start(ch) {
            self.i += 1;
            while self.i < self.s.len() && is_ident_continue(self.s.as_bytes()[self.i] as char) {
                self.i += 1;
            }
            return Tok {
                kind: TokKind::Ident,
                text: self.s[start..self.i].to_string(),
                start,
                end: self.i,
            };
        }

        Tok {
            kind: TokKind::Sym,
            text: ch.to_string(),
            start,
            end: self.i + 1,
        }
    }
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dt2unix_rejects_non_positive() {
        assert_eq!(dt2unix(0.0), 0.0);
        assert_eq!(dt2unix(-1.0), 0.0);
    }

    #[test]
    fn dt2unix_accepts_milliseconds() {
        let ms = 1_760_000_000_000f64;
        let sec = dt2unix(ms);
        assert!((sec - 1_760_000_000f64).abs() < 1.0);
    }

    #[test]
    fn parse_and_eval_reg_emit() {
        let src = "let a = 1 + 2; reg(1001) = a; emit(1700000000, 1002, 5);";
        let s = Script::parse(src).expect("parse");
        let out = s
            .eval_result(&[], true, &|_| 0.0, &|_, _| 0.0, None, None, 1000)
            .expect("eval");

        assert_eq!(out.regs.get(&1001).copied().unwrap_or_default(), 3.0);
        assert_eq!(out.emits.len(), 1);
        assert_eq!(out.emits[0].reg_id, 1002);
        assert_eq!(out.emits[0].value, 5.0);
        assert_eq!(out.emits[0].ts, 1_700_000_000f64);
    }

    #[test]
    fn eval_fails_on_step_limit_in_while_loop() {
        let src = "while(1){ let a = 1; }";
        let s = Script::parse(src).expect("parse");
        let err = s
            .eval_result(&[], true, &|_| 0.0, &|_, _| 0.0, None, None, 64)
            .err()
            .unwrap_or_else(|| "no error".to_string());
        assert!(
            err.contains("step limit exceeded"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn parse_for_loop_and_emit_multiple_values() {
        let src = "for(let i=0; i<3; i=i+1){ emit(1700000000+i, 1000+i, i*10); }";
        let s = Script::parse(src).expect("parse");
        let out = s
            .eval_result(&[], true, &|_| 0.0, &|_, _| 0.0, None, None, 10_000)
            .expect("eval");

        assert_eq!(out.emits.len(), 3);
        assert_eq!(out.emits[0].reg_id, 1000);
        assert_eq!(out.emits[0].value, 0.0);
        assert_eq!(out.emits[1].reg_id, 1001);
        assert_eq!(out.emits[1].value, 10.0);
        assert_eq!(out.emits[2].reg_id, 1002);
        assert_eq!(out.emits[2].value, 20.0);
    }

    #[test]
    fn used_rv_keys_extracts_unique_constants() {
        let src = r#"
            let a = rv(100) + rv(200);
            let b = rv(100);
            if (rv(300) > 0) { reg(1) = a + b; }
        "#;
        let s = Script::parse(src).expect("parse");
        assert_eq!(s.used_rv_keys(), &[100, 200, 300]);
    }

    #[test]
    fn used_rv_keys_ignores_non_constant_rv_argument() {
        let src = r#"
            let x = 123;
            let a = rv(x);
            let b = rv(42);
            reg(1) = a + b;
        "#;
        let s = Script::parse(src).expect("parse");
        assert_eq!(s.used_rv_keys(), &[42]);
    }
}
