use cyfs_base::*;

#[derive(Debug, Eq, PartialEq)]
enum ExpLexItem {
    Op(ExpOp),
    LeftParen,
    RightParen,
    Token(String),
}

// 操作符的元数，目前只支持一元和二元操作符
pub enum ExpOpArity {
    Unary,
    Binary,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ExpOp {
    // ==
    EQ,

    // !=
    NE,

    // <
    LT,

    // <=
    LE,

    // >
    GT,

    // >=
    GE,

    // !
    NOT,

    // &&
    AND,

    // 支持三个位操作，和整型数操作结果转化为bool(0 -> false, 1 -> true)
    // &
    BAND,

    // |
    BOR,

    // ^
    BXOR,

    // ||
    OR,
}

impl ExpOp {
    fn arity(&self) -> ExpOpArity {
        match &self {
            ExpOp::NOT => ExpOpArity::Unary,
            _ => ExpOpArity::Binary,
        }
    }

    // 获取运算符优先级
    fn priority(&self) -> u8 {
        match &self {
            ExpOp::NOT => 17,
            ExpOp::LT | ExpOp::LE | ExpOp::GT | ExpOp::GE => 12,
            ExpOp::NE | ExpOp::EQ => 11,
            ExpOp::BAND => 10,
            ExpOp::BXOR => 9,
            ExpOp::BOR => 8,
            ExpOp::AND => 7,
            ExpOp::OR => 6,
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        let ret = match s {
            "==" => Self::EQ,
            "!=" => Self::NE,

            "<" => Self::LT,
            "<=" => Self::LE,

            ">" => Self::GT,
            ">=" => Self::GE,

            "!" => Self::NOT,

            "&&" => Self::AND,
            "||" => Self::OR,

            "&" => Self::BAND,
            "^" => Self::BXOR,
            "|" => Self::BOR,

            _ => {
                return None;
            }
        };

        Some(ret)
    }

    fn to_str(&self) -> &str {
        match *self {
            Self::EQ => "==",
            Self::NE => "!=",

            Self::LT => "<",
            Self::LE => "<=",

            Self::GT => ">",
            Self::GE => ">=",

            Self::NOT => "!",

            Self::AND => "&&",
            Self::OR => "||",

            Self::BAND => "&",
            Self::BXOR => "^",
            Self::BOR => "|",
        }
    }

    pub fn parse(ch: char, it: &mut std::str::Chars<'_>) -> Option<Self> {
        let result = match ch {
            '!' => match it.clone().next() {
                Some('=') => {
                    it.next();
                    Self::NE
                }
                _ => Self::NOT,
            },
            '=' => match it.next() {
                Some('=') => Self::EQ,
                _ => return None,
            },
            '>' => match it.clone().next() {
                Some('=') => {
                    it.next();
                    Self::GE
                }
                _ => Self::GT,
            },
            '<' => match it.clone().next() {
                Some('=') => {
                    it.next();
                    Self::LE
                }
                _ => Self::LT,
            },
            '&' => match it.clone().next() {
                Some('&') => {
                    it.next();
                    Self::AND
                }
                _ => Self::BAND,
            },
            '|' => match it.clone().next() {
                Some('|') => {
                    it.next();
                    Self::OR
                }
                _ => Self::BOR,
            },
            '^' => Self::BXOR,
            _ => return None,
        };

        Some(result)
    }
}

// 解析引号之间的token，支持单引号和双引号，但开始和结束必须匹配
#[derive(Eq, PartialEq)]
enum ExpTokenQuote {
    Single,
    Double,
    None,
}

enum ExpTokenQuoteParserResult {
    Continue,
    Begin,
    Token,
    End,
}

impl ExpTokenQuote {
    fn from_char(c: char) -> Self {
        match c {
            '"' => Self::Double,
            '\'' => Self::Single,
            _ => Self::None,
        }
    }
}

struct ExpTokenQuoteParser {
    state: ExpTokenQuote,
}

impl ExpTokenQuoteParser {
    pub fn new() -> Self {
        Self {
            state: ExpTokenQuote::None,
        }
    }

    // 返回值
    fn next(&mut self, exp: &str, c: char) -> BuckyResult<ExpTokenQuoteParserResult> {
        match ExpTokenQuote::from_char(c) {
            ExpTokenQuote::None => match self.state {
                ExpTokenQuote::None => return Ok(ExpTokenQuoteParserResult::Continue),
                _ => Ok(ExpTokenQuoteParserResult::Token),
            },
            state @ _ => match self.state {
                ExpTokenQuote::None => {
                    self.state = state;
                    Ok(ExpTokenQuoteParserResult::Begin)
                }
                _ => {
                    if self.state != state {
                        let msg = format!("filter exp quote unmatch! exp={}", exp);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }

                    self.state = ExpTokenQuote::None;
                    Ok(ExpTokenQuoteParserResult::End)
                }
            },
        }
    }
}

//name op value
//((obj_type >= 0 && obj_type <= 10 ) || obj_type == 100 && !first_time)
struct ExpParser;

const EXP_TOKEN_QUOTES: [char; 2] = ['\'', '"'];

impl ExpParser {
    // 判断一个字符是不是操作数
    fn is_operand_char(ch: char) -> bool {
        ch.is_ascii_alphabetic()
            || ch.is_numeric()
            || ch == '-'
            || ch == '_'
            || ch == '.'
            || ch == '$'
            || ch == '*'
            || ch == '/'
            || ch == '\\'
    }

    fn parse_token(token: &[char]) -> BuckyResult<ExpLexItem> {
        let s: String = token.iter().collect();

        match ExpOp::from_str(&s) {
            Some(op) => Ok(ExpLexItem::Op(op)),
            None => {
                // 检查是不是有效token
                for ch in token {
                    if !Self::is_operand_char(*ch) {
                        let msg = format!("invalid exp token: {}", s);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                }

                // 尝试去掉引号
                let s = s
                    .trim_start_matches(&EXP_TOKEN_QUOTES[..])
                    .trim_end_matches(&EXP_TOKEN_QUOTES[..]);

                Ok(ExpLexItem::Token(s.to_owned()))
            }
        }
    }

    fn direct_parse_token(token: &[char]) -> BuckyResult<ExpLexItem> {
        let s: String = token.iter().collect();

        // 尝试去掉引号
        let s = s
            .trim_start_matches(&EXP_TOKEN_QUOTES[..])
            .trim_end_matches(&EXP_TOKEN_QUOTES[..]);

        Ok(ExpLexItem::Token(s.to_owned()))
    }

    // 转为逆波兰表达式
    fn convert_to_rpn(exp: &str, list: Vec<ExpLexItem>) -> BuckyResult<Vec<ExpLexItem>> {
        let mut operands = vec![];
        let mut operators = vec![];

        for item in list {
            match item {
                ExpLexItem::Token(ref _v) => {
                    operands.push(item);
                }
                ExpLexItem::LeftParen => {
                    operators.push(item);
                }
                ExpLexItem::RightParen => {
                    // 弹出所有操作符，直到左括号
                    loop {
                        match operators.pop() {
                            Some(v) => match v {
                                ExpLexItem::LeftParen => {
                                    break;
                                }
                                _ => {
                                    operands.push(v);
                                }
                            },
                            None => {
                                let msg = format!("unmatch exp paren: {}", exp);
                                error!("{}", msg);

                                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }
                        }
                    }
                }

                ExpLexItem::Op(v) => {
                    loop {
                        let last_op = operators.last();

                        // 如果运算符堆栈为空，那么直接放入运算符堆栈
                        if last_op.is_none() {
                            operators.push(ExpLexItem::Op(v));
                            break;
                        }

                        let last_op = last_op.unwrap();

                        // 处理!!情况
                        if v == ExpOp::NOT {
                            if let ExpLexItem::Op(prev) = last_op {
                                if *prev == v {
                                    operators.pop();
                                    break;
                                }
                            }
                        }

                        // 取出栈顶比当前运算符优先级低的运算符，放入操作数堆栈
                        match last_op {
                            ExpLexItem::Op(lv) => {
                                // 若比运算符堆栈栈顶的运算符优先级高，则直接存入运算符堆栈
                                if v.priority() > lv.priority() {
                                    operators.push(ExpLexItem::Op(v));
                                    break;
                                } else {
                                    let last_op = operators.pop().unwrap();
                                    operands.push(last_op);
                                }
                            }
                            // 若运算符堆栈栈顶的运算符为括号（只可能是左括号），则直接存入运算符堆栈。
                            ExpLexItem::LeftParen => {
                                operators.push(ExpLexItem::Op(v));
                                break;
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }

        // 表达式读取完毕，剩余的运算符依次放入操作数堆栈
        while !operators.is_empty() {
            let op = operators.pop().unwrap();
            operands.push(op);
        }

        /*
        if !operators.is_empty() {
            let msg = format!("invalid exp: {}, {:?}", exp, operators);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }
        */

        Ok(operands)
    }

    pub fn parse_lex(exp: &str) -> BuckyResult<Vec<ExpLexItem>> {
        let mut it = exp.chars();

        let mut token_list = vec![];
        let mut token = vec![];
        let mut quote_parse = ExpTokenQuoteParser::new();
        loop {
            match it.next() {
                Some(c) => {
                    // 首先需要解析是不是引号括起来的内容
                    match quote_parse.next(exp, c)? {
                        ExpTokenQuoteParserResult::Continue => {
                            // 无关，继续下面的解析
                        }
                        ExpTokenQuoteParserResult::Token => {
                            token.push(c);
                            continue;
                        }
                        ExpTokenQuoteParserResult::Begin => {
                            continue;
                        }
                        ExpTokenQuoteParserResult::End => {
                            token_list.push(Self::direct_parse_token(&token)?);
                            token.clear();
                            continue;
                        }
                    }
                    if c.is_whitespace() {
                        if !token.is_empty() {
                            token_list.push(Self::parse_token(&token)?);
                            token.clear();
                        }
                        continue;
                    }

                    match c {
                        '(' => {
                            if !token.is_empty() {
                                token_list.push(Self::parse_token(&token)?);
                                token.clear();
                            }
                            token_list.push(ExpLexItem::LeftParen);
                        }
                        ')' => {
                            if !token.is_empty() {
                                token_list.push(Self::parse_token(&token)?);
                                token.clear();
                            }
                            token_list.push(ExpLexItem::RightParen);
                        }
                        _ => {
                            if Self::is_operand_char(c) {
                                token.push(c);
                            } else {
                                match ExpOp::parse(c, &mut it) {
                                    Some(op) => {
                                        if !token.is_empty() {
                                            token_list.push(Self::parse_token(&token)?);
                                            token.clear();
                                        }

                                        token_list.push(ExpLexItem::Op(op));
                                    }
                                    None => {
                                        let msg = format!(
                                            "invalid operand or operator: exp={}, char={}, token={}",
                                            exp,
                                            c,
                                            it.as_str()
                                        );
                                        error!("{}", msg);
                                        return Err(BuckyError::new(
                                            BuckyErrorCode::InvalidFormat,
                                            msg,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                None => {
                    break;
                }
            }
        }
        if !token.is_empty() {
            token_list.push(Self::parse_token(&token)?);
        }

        debug!("exp to lex list: exp={}, list={:?}", exp, token_list);

        // 转为逆波兰表达式
        Self::convert_to_rpn(exp, token_list)
    }
}

pub struct ExpReservedTokenList {
    list: Vec<(String, ExpTokenEvalValue)>,
}

impl ExpReservedTokenList {
    pub fn new() -> Self {
        Self { list: Vec::new() }
    }

    // 添加前缀
    pub fn translate(&mut self, prefix: &str) {
        self.list
            .iter_mut()
            .for_each(|v| v.0 = format!("{}.{}", prefix, v.0));
    }

    // 为所有条目添加"resp."前缀
    pub fn translate_resp(&mut self) {
        self.translate("resp");
    }

    pub fn append(&mut self, other: Self) {
        for item in other.list {
            assert!(!self.is_reserved_token(&item.0));

            self.list.push(item);
        }
    }

    fn add_token(&mut self, token: &str, default_value: ExpTokenEvalValue) {
        assert!(!self.is_reserved_token(token));

        self.list.push((token.to_owned(), default_value));
    }

    pub fn add_string(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::String(String::default()))
    }

    pub fn add_glob(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::Glob(ExpGlobToken::default()))
    }

    pub fn add_bool(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::Bool(false))
    }

    pub fn add_i8(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::I8(0))
    }
    pub fn add_i16(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::I16(0))
    }
    pub fn add_i32(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::I32(0))
    }
    pub fn add_i64(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::I64(0))
    }

    pub fn add_u8(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::U8(0))
    }
    pub fn add_u16(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::U16(0))
    }
    pub fn add_u32(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::U32(0))
    }
    pub fn add_u64(&mut self, token: &str) {
        self.add_token(token, ExpTokenEvalValue::U64(0))
    }

    pub fn is_reserved_token(&self, token: &str) -> bool {
        self.list.iter().find(|v| v.0.as_str() == token).is_some()
    }

    pub fn default_value(&self, token: &str) -> Option<ExpTokenEvalValue> {
        self.list
            .iter()
            .find(|v| v.0.as_str() == token)
            .map(|v| v.1.clone())
    }
}

impl ExpReservedTokenTranslator for ExpReservedTokenList {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        self.default_value(token).unwrap()
    }
}

impl ExpReservedTokenTranslator for &ExpReservedTokenList {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        self.default_value(token).unwrap()
    }
}

#[derive(Clone, Debug)]
pub enum ExpGlobToken {
    Glob(globset::GlobMatcher),
    StringList(Vec<String>),
    String(String),
}

impl ExpGlobToken {
    pub fn new_glob(token: &str) -> BuckyResult<Self> {
        let glob = globset::GlobBuilder::new(token)
            .case_insensitive(true)
            .literal_separator(true)
            .build()
            .map_err(|e| {
                let msg = format!("parse filter glob as glob error! token={}, {}", token, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

        Ok(Self::Glob(glob.compile_matcher()))
    }

    // 对于一个req_path，确保req_path和req_path/都在匹配列表里面
    fn append_string(list: &mut Vec<String>, token: String) {
        let s = token.trim_end_matches('/');
        if s.len() == token.len() {
            list.push(format!("{}/", s));
            drop(s);
            list.push(token);
        } else {
            list.push(s.to_owned());
            if s.len() + 1 == token.len() {
                drop(s);
                list.push(token);
            } else {
                list.push(format!("{}/", s));
            }
        }
    }

    pub fn new_string(token: String) -> Self {
        let mut list = Vec::with_capacity(2);
        Self::append_string(&mut list, token);
        Self::StringList(list)
    }

    pub fn new_string_list(tokens: Vec<String>) -> Self {
        let mut list = Vec::with_capacity(tokens.len() * 2);
        for token in tokens {
            Self::append_string(&mut list, token);
        }
        Self::StringList(list)
    }

    pub fn is_glob(&self) -> bool {
        match *self {
            Self::Glob(_) => true,
            _ => false,
        }
    }

    pub fn as_glob(&self) -> &globset::GlobMatcher {
        match self {
            Self::Glob(v) => v,
            _ => unreachable!(),
        }
    }

    // 左右必须一个glob，一个string
    pub fn eq(left: &Self, right: &Self) -> bool {
        match left {
            Self::Glob(_) => Self::eq(right, left),
            Self::String(s) => right.as_glob().is_match(s),
            Self::StringList(list) => {
                for s in list {
                    if right.as_glob().is_match(&s) {
                        return true;
                    }
                }

                false
            }
        }
    }
}

impl Default for ExpGlobToken {
    fn default() -> Self {
        Self::new_string("".to_owned())
    }
}

impl PartialEq for ExpGlobToken {
    fn eq(&self, other: &Self) -> bool {
        Self::eq(&self, other)
    }
}
impl Eq for ExpGlobToken {}

impl PartialOrd for ExpGlobToken {
    fn partial_cmp(&self, _other: &Self) -> Option<std::cmp::Ordering> {
        unreachable!();
    }
}

impl Ord for ExpGlobToken {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        unreachable!();
    }
}

// token用以计算的目标类型
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum ExpTokenEvalValue {
    // 空值
    None,

    String(String),
    Glob(ExpGlobToken),
    Bool(bool),

    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
}

impl Into<i8> for ExpTokenEvalValue {
    fn into(self) -> i8 {
        match self {
            Self::I8(v) => v,
            _ => unreachable!(),
        }
    }
}
impl Into<i16> for ExpTokenEvalValue {
    fn into(self) -> i16 {
        match self {
            Self::I16(v) => v,
            _ => unreachable!(),
        }
    }
}
impl Into<i32> for ExpTokenEvalValue {
    fn into(self) -> i32 {
        match self {
            Self::I32(v) => v,
            _ => unreachable!(),
        }
    }
}
impl Into<i64> for ExpTokenEvalValue {
    fn into(self) -> i64 {
        match self {
            Self::I64(v) => v,
            _ => unreachable!(),
        }
    }
}

impl Into<u8> for ExpTokenEvalValue {
    fn into(self) -> u8 {
        match self {
            Self::U8(v) => v,
            _ => unreachable!(),
        }
    }
}
impl Into<u16> for ExpTokenEvalValue {
    fn into(self) -> u16 {
        match self {
            Self::U16(v) => v,
            _ => unreachable!(),
        }
    }
}
impl Into<u32> for ExpTokenEvalValue {
    fn into(self) -> u32 {
        match self {
            Self::U32(v) => v,
            _ => unreachable!(),
        }
    }
}
impl Into<u64> for ExpTokenEvalValue {
    fn into(self) -> u64 {
        match self {
            Self::U64(v) => v,
            _ => unreachable!(),
        }
    }
}

// 对from_str_radix封装一级trait
trait FromStrRadix<T> {
    fn from_str_radix(src: &str, radix: u32) -> Result<T, std::num::ParseIntError>;
}

macro_rules! from_str_radix_trait_impl {
    ($T:ty) => {
        impl FromStrRadix<$T> for $T {
            fn from_str_radix(src: &str, radix: u32) -> Result<$T, std::num::ParseIntError> {
                <$T>::from_str_radix(src, radix)
            }
        }
    };
}
from_str_radix_trait_impl!(i8);
from_str_radix_trait_impl!(i16);
from_str_radix_trait_impl!(i32);
from_str_radix_trait_impl!(i64);
from_str_radix_trait_impl!(u8);
from_str_radix_trait_impl!(u16);
from_str_radix_trait_impl!(u32);
from_str_radix_trait_impl!(u64);

impl ExpTokenEvalValue {
    pub fn from_string<T>(v: &T) -> Self
    where
        T: ToString,
    {
        Self::String(v.to_string())
    }

    pub fn from_opt_string<T>(v: &Option<T>) -> Self
    where
        T: ToString,
    {
        match v {
            Some(v) => Self::String(v.to_string()),
            None => Self::None,
        }
    }

    pub fn from_glob_list<T>(v: &Vec<T>) -> Self
    where
        T: ToString,
    {
        let list: Vec<String> = v.iter().map(|v| v.to_string()).collect();
        Self::Glob(ExpGlobToken::new_string_list(list))
    }

    pub fn from_glob<T>(v: &T) -> Self
    where
        T: ToString,
    {
        Self::Glob(ExpGlobToken::new_string(v.to_string()))
    }

    pub fn from_opt_glob<T>(v: &Option<T>) -> Self
    where
        T: ToString,
    {
        match v {
            Some(v) => Self::Glob(ExpGlobToken::new_string(v.to_string())),
            None => Self::None,
        }
    }

    pub fn from_opt_u64(v: Option<u64>) -> Self
    {
        match v {
            Some(v) => Self::U64(v),
            None => Self::None,
        }
    }

    pub fn is_none(&self) -> bool {
        match *self {
            Self::None => true,
            _ => false,
        }
    }

    // 判断是不是独立的token，比如*
    pub fn try_from_single_const_token(token: &str) -> Option<Self> {
        if token == "*" {
            Some(Self::Bool(true))
        } else {
            None
        }
    }

    pub fn new_from_const_token(target: &ExpTokenEvalValue, token: &str) -> BuckyResult<Self> {
        // info!("token={}", token);
        if token == "$none" {
            return Ok(Self::None);
        }

        let ret = match target {
            Self::None => unreachable!(),
            Self::String(_) => Self::String(token.to_owned()),
            Self::Glob(_) => Self::Glob(ExpGlobToken::new_glob(token)?),
            Self::Bool(_) => {
                let v;
                if token == "true" || token == "1" {
                    v = true;
                } else if token == "false" || token == "0" {
                    v = false;
                } else {
                    let msg = format!("invalid const value, bool expected: {}", token);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }
                Self::Bool(v)
            }
            Self::I8(_) => {
                let v = Self::parse_number(token, "i8")?;
                Self::I8(v)
            }
            Self::I16(_) => {
                let v = Self::parse_number(token, "i16")?;
                Self::I16(v)
            }
            Self::I32(_) => {
                let v = Self::parse_number(token, "i32")?;
                Self::I32(v)
            }
            Self::I64(_) => {
                let v = Self::parse_number(token, "i64")?;
                Self::I64(v)
            }

            Self::U8(_) => {
                let v = Self::parse_number(token, "u8")?;
                Self::U8(v)
            }
            Self::U16(_) => {
                let v = Self::parse_number(token, "u16")?;
                Self::U16(v)
            }
            Self::U32(_) => {
                let v = Self::parse_number(token, "u32")?;
                Self::U32(v)
            }
            Self::U64(_) => {
                let v = Self::parse_number(token, "u64")?;
                Self::U64(v)
            }
        };

        Ok(ret)
    }

    /*
    fn parse_number<I>(token: &str, type_name: &str) -> BuckyResult<I>
    where
        I: FromStr,
        <I as FromStr>::Err: std::fmt::Display,
    {
        I::from_str(token).map_err(|e| {
            let msg = format!(
                "invalid number value, {} expected: {}, {}",
                type_name, token, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })
    }
    */

    fn parse_number<I>(token: &str, type_name: &str) -> BuckyResult<I>
    where
        I: FromStrRadix<I>,
    {
        let radix = if token.starts_with("0x") || token.starts_with("0X") {
            16
        } else if token.starts_with("0o") || token.starts_with("0O") {
            8
        } else if token.starts_with("0b") || token.starts_with("0B") {
            2
        } else {
            10
        };

        let token = if radix != 10 {
            token.split_at(2).1
        } else {
            token
        };

        I::from_str_radix(token, radix).map_err(|e| {
            let msg = format!(
                "invalid number value, {} expected: {}, {}",
                type_name, token, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })
    }

    pub fn support_ops(&self) -> Vec<ExpOp> {
        match *self {
            Self::String(_) | Self::Glob(_) | Self::None => vec![ExpOp::EQ, ExpOp::NE],
            Self::Bool(_) => vec![
                ExpOp::EQ,
                ExpOp::NE,
                ExpOp::LT,
                ExpOp::LE,
                ExpOp::GT,
                ExpOp::GE,
                ExpOp::NOT,
                ExpOp::AND,
                ExpOp::OR,
            ],

            // 剩余所有的整型数
            _ => vec![
                ExpOp::EQ,
                ExpOp::NE,
                ExpOp::LT,
                ExpOp::LE,
                ExpOp::GT,
                ExpOp::GE,
                ExpOp::NOT,
                ExpOp::BAND,
                ExpOp::BOR,
                ExpOp::BXOR,
            ],
        }
    }

    pub fn is_support_op(&self, op: &ExpOp) -> bool {
        self.support_ops().contains(op)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    // 如果是整型数，那么尝试进行|位运算，结果为bool类型
    fn bitor(&self, rhs: &Self) -> bool {
        let rhs = rhs.to_owned();
        match *self {
            Self::I8(v) => v | Into::<i8>::into(rhs) != 0,
            Self::I16(v) => v | Into::<i16>::into(rhs) != 0,
            Self::I32(v) => v | Into::<i32>::into(rhs) != 0,
            Self::I64(v) => v | Into::<i64>::into(rhs) != 0,

            Self::U8(v) => v | Into::<u8>::into(rhs) != 0,
            Self::U16(v) => v | Into::<u16>::into(rhs) != 0,
            Self::U32(v) => v | Into::<u32>::into(rhs) != 0,
            Self::U64(v) => v | Into::<u64>::into(rhs) != 0,
            _ => {
                unreachable!("bitor only for int numbers");
            }
        }
    }

    fn bitand(&self, rhs: &Self) -> bool {
        let rhs = rhs.to_owned();
        match *self {
            Self::I8(v) => v & Into::<i8>::into(rhs) != 0,
            Self::I16(v) => v & Into::<i16>::into(rhs) != 0,
            Self::I32(v) => v & Into::<i32>::into(rhs) != 0,
            Self::I64(v) => v & Into::<i64>::into(rhs) != 0,

            Self::U8(v) => v & Into::<u8>::into(rhs) != 0,
            Self::U16(v) => v & Into::<u16>::into(rhs) != 0,
            Self::U32(v) => v & Into::<u32>::into(rhs) != 0,
            Self::U64(v) => v & Into::<u64>::into(rhs) != 0,
            _ => {
                unreachable!("bitand only for int numbers");
            }
        }
    }

    fn bitxor(&self, rhs: &Self) -> bool {
        let rhs = rhs.to_owned();
        match *self {
            Self::I8(v) => v ^ Into::<i8>::into(rhs) != 0,
            Self::I16(v) => v ^ Into::<i16>::into(rhs) != 0,
            Self::I32(v) => v ^ Into::<i32>::into(rhs) != 0,
            Self::I64(v) => v ^ Into::<i64>::into(rhs) != 0,

            Self::U8(v) => v ^ Into::<u8>::into(rhs) != 0,
            Self::U16(v) => v ^ Into::<u16>::into(rhs) != 0,
            Self::U32(v) => v ^ Into::<u32>::into(rhs) != 0,
            Self::U64(v) => v ^ Into::<u64>::into(rhs) != 0,
            _ => {
                unreachable!("bitxor only for int numbers");
            }
        }
    }
}

// 关键字计算为具体的值
pub trait ExpReservedTokenTranslator {
    fn trans(&self, token: &str) -> ExpTokenEvalValue;
}

#[derive(Debug, Clone)]
enum ExpEvalItem {
    Op(ExpOp),

    ReservedToken(String),

    // 常量，目前只支持string
    ConstToken(String),

    // 经过目标类型转换的token
    EvalToken(ExpTokenEvalValue),
}

impl ExpEvalItem {
    pub fn trans(self, translator: &impl ExpReservedTokenTranslator) -> Self {
        match self {
            Self::ReservedToken(v) => ExpEvalItem::EvalToken(translator.trans(&v)),
            _ => self,
        }
    }

    pub fn is_token(&self) -> bool {
        match self {
            Self::Op(_) => false,
            _ => true,
        }
    }

    pub fn is_const_token(&self) -> bool {
        match self {
            Self::ConstToken(_) => true,
            _ => false,
        }
    }

    pub fn is_reserved_token(&self) -> bool {
        match self {
            Self::ReservedToken(_) => true,
            _ => false,
        }
    }

    pub fn is_eval_token(&self) -> bool {
        match self {
            Self::EvalToken(_) => true,
            _ => false,
        }
    }

    pub fn into_const_token(self) -> String {
        match self {
            Self::ConstToken(v) => v,
            _ => unreachable!(),
        }
    }

    pub fn into_eval_value(self) -> ExpTokenEvalValue {
        match self {
            Self::EvalToken(v) => v,
            _ => unreachable!(),
        }
    }
}

pub struct ExpEvaluator {
    exp: String,
    rpn: Vec<ExpEvalItem>,
}

impl std::fmt::Debug for ExpEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.exp)?;

        Ok(())
    }
}

impl std::fmt::Display for ExpEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.exp)?;

        Ok(())
    }
}

impl PartialEq for ExpEvaluator {
    fn eq(&self, other: &Self) -> bool {
        self.exp() == other.exp()
    }
}
impl Eq for ExpEvaluator {}

use std::cmp::Ordering;
impl PartialOrd for ExpEvaluator {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.exp().partial_cmp(other.exp())
    }
}

impl Ord for ExpEvaluator {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}


impl ExpEvaluator {
    pub fn new(exp: impl Into<String>, reserved_token_list: &ExpReservedTokenList) -> BuckyResult<Self> {
        let exp = exp.into();
        let lex_list = ExpParser::parse_lex(&exp)?;
        // 不支持空表达式
        if lex_list.is_empty() {
            let msg = format!("empty exp filter not supported! exp={}", exp);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        debug!("exp to rpn lex list: exp={}, list={:?}", exp, lex_list);

        let rpn = Self::convert(&exp, lex_list, reserved_token_list)?;

        Ok(Self {
            exp,
            rpn,
        })
    }

    pub fn new_uninit(exp: impl Into<String>) -> Self {
        let exp = exp.into();
        
        Self {
            exp,
            rpn: vec![],
        }
    }

    pub fn exp(&self) -> &str {
        &self.exp
    }

    pub fn into_exp(self) -> String {
        self.exp
    }
    
    fn convert(
        exp: &str,
        lex_list: Vec<ExpLexItem>,
        reserved_token_list: &ExpReservedTokenList,
    ) -> BuckyResult<Vec<ExpEvalItem>> {
        // 分离token，解析为保留关键字和常量
        let mut result = vec![];
        for item in lex_list.into_iter() {
            match item {
                ExpLexItem::Op(v) => {
                    result.push(ExpEvalItem::Op(v));
                }
                ExpLexItem::Token(v) => {
                    if reserved_token_list.is_reserved_token(&v) {
                        result.push(ExpEvalItem::ReservedToken(v));
                    } else {
                        result.push(ExpEvalItem::ConstToken(v));
                    }
                }
                _ => unreachable!(),
            }
        }

        debug!("exp lex to eval: {:?}", result);

        // 表达式类型检测和常量解析
        Self::check_and_convert(exp, result, reserved_token_list)
    }

    fn check_and_convert(
        exp: &str,
        mut rpn: Vec<ExpEvalItem>,
        translator: &impl ExpReservedTokenTranslator,
    ) -> BuckyResult<Vec<ExpEvalItem>> {
        // 目前所有表达式的中间结果都是bool值
        let mid_result = ExpEvalItem::EvalToken(ExpTokenEvalValue::Bool(false));

        let mut operands: Vec<(ExpEvalItem, usize)> = vec![];
        let mut result = vec![];

        for (i, item) in rpn.iter().enumerate() {
            match item {
                ExpEvalItem::Op(op) => {
                    match op.arity() {
                        ExpOpArity::Unary => {
                            if operands.len() < 1 {
                                let msg = format!(
                                    "exp operator need one operands, got none: exp={}, op='{}'",
                                    exp,
                                    op.to_str()
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }

                            // 单操作符，操作数必须是关键字or中间结果，不能是常量
                            let (operand, _) = operands.pop().unwrap();
                            if operand.is_const_token() {
                                let msg = format!(
                                    "unary operator not support const token: exp={}, op='{}'",
                                    exp,
                                    op.to_str()
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }

                            let value = operand.into_eval_value();

                            // 判断是否支持操作符
                            if !value.is_support_op(op) {
                                let msg = format!("operand not support operator: exp={}, operator={}, operand={:?}", exp, op.to_str(), value);
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }

                            // 结果直接压入操作数堆栈
                            operands.push((mid_result.clone(), usize::MAX));
                        }
                        ExpOpArity::Binary => {
                            if operands.len() < 2 {
                                let msg = format!(
                                    "exp operator need two operands, got one: {}",
                                    op.to_str()
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }

                            let (right, right_index) = operands.pop().unwrap();
                            let (left, left_index) = operands.pop().unwrap();

                            // 二元运算，至少要有一个关键字或者一个中间结果，不能两个都是常量
                            if left.is_const_token() && right.is_const_token() {
                                let msg = format!(
                                    "binary operator not support two const token: exp={}, op='{}'",
                                    exp,
                                    op.to_str()
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }

                            // 确保左操作数不是常量
                            let (left, _left_index, right, right_index) = if left.is_eval_token() {
                                (left, left_index, right, right_index)
                            } else {
                                (right, right_index, left, left_index)
                            };

                            assert!(!left.is_const_token());

                            let left_value = left.into_eval_value();
                            // 检查是否支持运算符
                            if !left_value.is_support_op(op) {
                                let msg = format!("operand not support operator: exp={}, operator={}, operand={:?}", 
                                    exp, op.to_str(), left_value);
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }

                            // 如果右侧也是中间结果，那么需要检查类型是否相同
                            if right.is_eval_token() {
                                let right_value = right.into_eval_value();

                                // 现在要求类型必须严格匹配
                                if !left_value.is_none()
                                    && !right_value.is_none()
                                    && left_value != right_value
                                {
                                    let msg = format!("binary operator left type and right type not equal: exp={}, op='{}', left={:?}, right={:?}", 
                                        exp, op.to_str(), left_value, right_value);
                                    error!("{}", msg);
                                    return Err(BuckyError::new(
                                        BuckyErrorCode::InvalidFormat,
                                        msg,
                                    ));
                                }
                            } else {
                                assert!(right.is_const_token());

                                // 右侧是常量，那么尝试转换成左侧相同类型并更新表达式
                                let right_token = right.into_const_token();
                                let right_value = ExpTokenEvalValue::new_from_const_token(
                                    &left_value,
                                    &right_token,
                                )?;

                                // 更新rpn序列
                                assert!(right_index != usize::MAX);
                                result.push((ExpEvalItem::EvalToken(right_value), right_index));
                            }

                            // 结果直接压入操作数堆栈
                            operands.push((mid_result.clone(), usize::MAX));
                        }
                    }
                }
                ExpEvalItem::ReservedToken(v) => {
                    // 如果是关键字，这里需要求值
                    let value = translator.trans(&v);
                    operands.push((ExpEvalItem::EvalToken(value), i));
                }
                ExpEvalItem::ConstToken(v) => {
                    if let Some(value) = ExpTokenEvalValue::try_from_single_const_token(&v) {
                        trace!("got const token={}, value={:?}", v, value);

                        // 独立常量，直接入栈
                        result.push((ExpEvalItem::EvalToken(value), i))
                    } else {
                        // 常量，需要依赖类型推导
                        operands.push((item.clone(), i));
                    }
                }
                ExpEvalItem::EvalToken(_) => {
                    // 类型推导过程中，不应该出现中间值
                    unreachable!();
                }
            }
        }

        // 更新const token的计算结果
        for (item, i) in result.into_iter() {
            assert!(rpn[i].is_const_token());
            rpn[i] = item;
        }

        Ok(rpn)
    }

    // 进行一次求值运算
    pub fn eval(&self, translator: &impl ExpReservedTokenTranslator) -> BuckyResult<bool> {
        let mut operands: Vec<ExpTokenEvalValue> = vec![];

        for item in self.rpn.iter() {
            match item {
                ExpEvalItem::Op(op) => match op.arity() {
                    ExpOpArity::Unary => {
                        assert!(operands.len() >= 1);
                        let operand = operands.pop().unwrap();
                        assert!(operand.is_support_op(op));

                        // 结果直接压入操作数堆栈
                        let result = Self::unary_eval(op, operand);
                        operands.push(result);
                    }
                    ExpOpArity::Binary => {
                        assert!(operands.len() >= 2);

                        // 右操作数在栈顶
                        let right = operands.pop().unwrap();
                        let left = operands.pop().unwrap();

                        assert!(left.is_support_op(op) && right.is_support_op(op));

                        let result = Self::binary_eval(op, left, right);

                        // 结果直接压入操作数堆栈
                        operands.push(result);
                    }
                },
                ExpEvalItem::ReservedToken(v) => {
                    // 如果是关键字，这里需要求值
                    let value = translator.trans(&v);
                    operands.push(value);
                }
                ExpEvalItem::EvalToken(v) => {
                    operands.push(v.clone());
                }
                /*
                ExpEvalItem::ConstToken(v) => {
                    if v == "*" {
                        operands.push(ExpTokenEvalValue::Bool(true));
                    } else {
                        let msg = format!("unknown const token! exp={}, {}", self.exp, v);
                        error!("{}", msg);
                    }
                }
                */
                _ => unreachable!(),
            }
        }

        assert!(operands.len() == 1);
        let result = operands.pop().unwrap();
        match result {
            ExpTokenEvalValue::Bool(v) => Ok(v),
            _ => unreachable!(),
        }
    }

    fn unary_eval(op: &ExpOp, operand: ExpTokenEvalValue) -> ExpTokenEvalValue {
        let value = match *op {
            ExpOp::NOT => match operand {
                ExpTokenEvalValue::Bool(v) => ExpTokenEvalValue::Bool(!v),
                ExpTokenEvalValue::I8(v) => ExpTokenEvalValue::Bool(v == 0),
                ExpTokenEvalValue::I16(v) => ExpTokenEvalValue::Bool(v == 0),
                ExpTokenEvalValue::I32(v) => ExpTokenEvalValue::Bool(v == 0),
                ExpTokenEvalValue::I64(v) => ExpTokenEvalValue::Bool(v == 0),
                ExpTokenEvalValue::U8(v) => ExpTokenEvalValue::Bool(v == 0),
                ExpTokenEvalValue::U16(v) => ExpTokenEvalValue::Bool(v == 0),
                ExpTokenEvalValue::U32(v) => ExpTokenEvalValue::Bool(v == 0),
                ExpTokenEvalValue::U64(v) => ExpTokenEvalValue::Bool(v == 0),
                _ => unreachable!(),
            },
            _ => {
                unreachable!();
            }
        };

        value
    }

    fn binary_eval(
        op: &ExpOp,
        left_operand: ExpTokenEvalValue,
        right_operand: ExpTokenEvalValue,
    ) -> ExpTokenEvalValue {
        let value = match *op {
            ExpOp::EQ => ExpTokenEvalValue::Bool(left_operand == right_operand),
            ExpOp::NE => ExpTokenEvalValue::Bool(left_operand != right_operand),
            ExpOp::LT => ExpTokenEvalValue::Bool(left_operand < right_operand),
            ExpOp::LE => ExpTokenEvalValue::Bool(left_operand <= right_operand),
            ExpOp::GT => ExpTokenEvalValue::Bool(left_operand > right_operand),
            ExpOp::GE => ExpTokenEvalValue::Bool(left_operand >= right_operand),

            ExpOp::AND => {
                let left = left_operand.as_bool().unwrap();
                let right = right_operand.as_bool().unwrap();
                ExpTokenEvalValue::Bool(left && right)
            }
            ExpOp::OR => {
                let left = left_operand.as_bool().unwrap();
                let right = right_operand.as_bool().unwrap();
                ExpTokenEvalValue::Bool(left || right)
            }

            ExpOp::BAND => ExpTokenEvalValue::Bool(left_operand.bitand(&right_operand)),
            ExpOp::BOR => ExpTokenEvalValue::Bool(left_operand.bitor(&right_operand)),
            ExpOp::BXOR => ExpTokenEvalValue::Bool(left_operand.bitxor(&right_operand)),

            _ => unreachable!(),
        };

        /*
        trace!(
            "binary eval: {:?} {} {:?} = {:?}",
            left_operand,
            op.to_str(),
            right_operand,
            value
        );
        */
        value
    }
}


#[cfg(test)]
mod exp_tests {
    use super::*;

    struct TestTranslator {}

    impl ExpReservedTokenTranslator for TestTranslator {
        fn trans(&self, token: &str) -> ExpTokenEvalValue {
            match token {
                "a" => ExpTokenEvalValue::I8(10),
                "b" => ExpTokenEvalValue::I32(1),
                "b1" => ExpTokenEvalValue::I32(1),
                "c" => ExpTokenEvalValue::Bool(true),
                "d" => ExpTokenEvalValue::I16(100),
                "x" => ExpTokenEvalValue::None,
                "req_path" => {
                    ExpTokenEvalValue::Glob(ExpGlobToken::new_string("/hello/world.js".to_owned()))
                }
                "req_path2" => {
                    ExpTokenEvalValue::Glob(ExpGlobToken::new_string("/test/some/globs".to_owned()))
                }
                _ => {
                    unreachable!("unknown token {}", token);
                }
            }
        }
    }

    // 使用 cargo test -- --nocapture 运行测试用例
    #[test]
    fn test() {
        cyfs_base::init_simple_log("test-exp-filter", Some("trace"));

        let mut token_list = ExpReservedTokenList::new();
        token_list.add_string("object-id");
        token_list.add_string("source");
        token_list.add_string("target");
        token_list.add_glob("req_path");
        token_list.add_glob("req_path2");

        token_list.add_i8("a");
        token_list.add_i32("b");
        token_list.add_i32("b1");
        token_list.add_bool("c");
        token_list.add_u32("d");
        token_list.add_u32("x");

        let translator = TestTranslator {};

        // 空字符串
        let ret = ExpEvaluator::new(" ", &token_list);
        assert!(ret.is_err());

        // *
        let exp = ExpEvaluator::new("(* )", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        // glob
        let exp = ExpEvaluator::new("req_path == '/**/*.js'", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        let exp = ExpEvaluator::new("req_path2 == '/**/*.js'", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);

        // x=none
        let exp = ExpEvaluator::new("(x != $none)", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);

        let exp = ExpEvaluator::new("(a != $none)", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        let exp = ExpEvaluator::new("(x == $none)", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        // a=10, b=b1=1, c=true, d=100
        let exp = ExpEvaluator::new("(a != 0 && (b == 0 || c))", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        let exp = ExpEvaluator::new("(a != 0 && (b != 1 || !c))", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);

        let exp = ExpEvaluator::new(
            "(a != 0 && (a!=1) && a == a && (b != b1 || (!c) || d == 1))",
            &token_list,
        )
        .unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);

        let exp = ExpEvaluator::new("(a != 0 && (a!=1) && a <= 10) && c", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        let exp = ExpEvaluator::new("((a < 11)) && a >= 10", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);
        let exp = ExpEvaluator::new("a < 11 && a > 10", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);
        let exp = ExpEvaluator::new("a >=10 && a == 10 && a <= 10", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);
        let exp = ExpEvaluator::new("a < 11 || (a!=10) && a > 10", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        let exp = ExpEvaluator::new("a >= 11 || (a==10) && a < 10", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);
        let exp = ExpEvaluator::new("(a >= 11 || (a==10)) && a <= 10", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        let exp = ExpEvaluator::new("!c", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);

        let exp = ExpEvaluator::new("!!c", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        let exp = ExpEvaluator::new("!!!c", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);

        let exp = ExpEvaluator::new("!!(a == 10)", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);

        let exp = ExpEvaluator::new("c && !!c", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);
        let exp = ExpEvaluator::new("!(c && !!c)", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);

        let exp = ExpEvaluator::new("(a != 0 && (a!=1) && a <10)", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);

        let exp = ExpEvaluator::new("a & 0x10", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, false);
        let exp = ExpEvaluator::new("a & 10", &token_list).unwrap();
        let result = exp.eval(&translator).unwrap();
        assert_eq!(result, true);
    }
}
