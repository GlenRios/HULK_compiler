// src/parser/engine/semantic_actions.rs

use crate::parser::grammar::{Grammar, production::Production};
use crate::parser::grammar::symbol::{
    NonTerminal as NT, Terminal as Tok, Symbol,
};
use crate::parser::ast::{
    Span, Expr, ExprKind, Literal, BinaryOp, UnaryOp, PostfixOp, PostfixExpr,
    AssignOp, LetBinding, ElifBranch, VectorExpr, TypeName,
    Param, FuncDecl, TypeDecl, TypeMember, AttributeDef,
    MethodDef, ProtocolDecl, MethodSignature, Decl, Program,
    NewExpr,
};
use super::error::ParseError;

// ─────────────────────────────────────────────────────────────────────────────
//  StackValue
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum StackValue {
    Lexeme(String, Span),
    Bool(bool, Span),
    ExprList(Vec<Expr>),
    ParamList(Vec<Param>),
    BindingList(Vec<LetBinding>),
    ElifList(Vec<ElifBranch>),
    TypeMemberList(Vec<TypeMember>),
    MethodSigList(Vec<MethodSignature>),
    DeclList(Vec<Decl>),
    Expr(Expr),
    TypeNameVal(TypeName),
    ParamVal(Param),
    Binding(LetBinding),
    TypeMemberVal(TypeMember),
    MethodSigVal(MethodSignature),
    DeclVal(Decl),
    Program(Program),
    Empty,
}

impl StackValue {
    pub fn into_expr(self) -> Result<Expr, ParseError> {
        match self {
            Self::Expr(e)       => Ok(e),
            Self::Bool(b, span) => Ok(Expr::bool(b, span)),
            other => Err(ParseError::internal(
                format!("esperaba Expr, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_expr_list(self) -> Result<Vec<Expr>, ParseError> {
        match self {
            Self::ExprList(v) => Ok(v),
            Self::Expr(e)     => Ok(vec![e]),
            Self::Empty       => Ok(vec![]),
            other => Err(ParseError::internal(
                format!("esperaba ExprList, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_type_name(self) -> Result<TypeName, ParseError> {
        match self {
            Self::TypeNameVal(t) => Ok(t),
            other => Err(ParseError::internal(
                format!("esperaba TypeName, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_param_list(self) -> Result<Vec<Param>, ParseError> {
        match self {
            Self::ParamList(v) => Ok(v),
            Self::ParamVal(p)  => Ok(vec![p]),
            Self::Empty        => Ok(vec![]),
            other => Err(ParseError::internal(
                format!("esperaba ParamList, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_lexeme(self) -> Result<(String, Span), ParseError> {
        match self {
            Self::Lexeme(s, span) => Ok((s, span)),
            other => Err(ParseError::internal(
                format!("esperaba Lexeme, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_decl_list(self) -> Result<Vec<Decl>, ParseError> {
        match self {
            Self::DeclList(v) => Ok(v),
            Self::Empty       => Ok(vec![]),
            other => Err(ParseError::internal(
                format!("esperaba DeclList, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_binding_list(self) -> Result<Vec<LetBinding>, ParseError> {
        match self {
            Self::BindingList(v) => Ok(v),
            Self::Binding(b)     => Ok(vec![b]),
            other => Err(ParseError::internal(
                format!("esperaba BindingList, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_elif_list(self) -> Result<Vec<ElifBranch>, ParseError> {
        match self {
            Self::ElifList(v) => Ok(v),
            Self::Empty       => Ok(vec![]),
            other => Err(ParseError::internal(
                format!("esperaba ElifList, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_method_sig_list(self) -> Result<Vec<MethodSignature>, ParseError> {
        match self {
            Self::MethodSigList(v) => Ok(v),
            Self::Empty            => Ok(vec![]),
            other => Err(ParseError::internal(
                format!("esperaba MethodSigList, encontrado {:?}", other), Span::dummy()))
        }
    }

    pub fn into_type_member_list(self) -> Result<Vec<TypeMember>, ParseError> {
        match self {
            Self::TypeMemberList(v) => Ok(v),
            Self::Empty             => Ok(vec![]),
            other => Err(ParseError::internal(
                format!("esperaba TypeMemberList, encontrado {:?}", other), Span::dummy()))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Helpers de inspección de producción
// ─────────────────────────────────────────────────────────────────────────────

fn plen(prod: &Production) -> usize { prod.body.len() }
fn pfirst(prod: &Production) -> Option<&Symbol> { prod.body.first() }
fn psym(prod: &Production, i: usize) -> Option<&Symbol> { prod.body.get(i) }

fn is_tok(prod: &Production, i: usize, t: &Tok) -> bool {
    prod.body.get(i) == Some(&Symbol::T(t.clone()))
}
fn is_nt(prod: &Production, i: usize, nt: &NT) -> bool {
    prod.body.get(i) == Some(&Symbol::NT(nt.clone()))
}

// ─────────────────────────────────────────────────────────────────────────────
//  reduce
// ─────────────────────────────────────────────────────────────────────────────

pub fn reduce(
    prod_id: usize,
    grammar: &Grammar,
    args: Vec<StackValue>,
    span: Span,
) -> Result<StackValue, ParseError> {

    let prod = grammar.production(prod_id);
    // Macro seguro: devuelve error con contexto en lugar de panic
    macro_rules! a {
        ($i:expr) => {
            match args.get($i) {
                Some(v) => v.clone(),
                None => return Err(ParseError::internal(
                    format!(
                        "a!({}) fuera de rango: args.len()={}, prod=[{}] {:?} (body_len={})",
                        $i, args.len(), prod_id, prod.head, plen(prod)
                    ),
                    span,
                )),
            }
        }
    }

    // Passthrough: producciones de un solo símbolo NO-TERMINAL que propagan el valor.
    // Cubre: Expr→AssignExpr, AssignExpr→OrExpr, …, CallOrAccess→PrimaryExpr,
    //        Decl→FuncDecl, TypeBody→TypeMemberList, etc.
    // ⚠ Solo aplica cuando el único símbolo es un NT.
    // Si es un terminal (ej. PrimaryExpr → IDENTIFIER), el Lexeme crudo
    // necesita conversión explícita en el brazo NT::PrimaryExpr.
    if plen(prod) == 1 {
        if let Some(Symbol::NT(_)) = pfirst(prod) {
            return Ok(a!(0));
        }
    }

    let result = match &prod.head {

        NT::Start => return Err(ParseError::internal("reduce de Start", span)),

        // ── Program ──────────────────────────────────────────────────────
        NT::Program => {
            // Program → DeclList Expr        (len 2)
            // Program → DeclList Expr ;      (len 3)
            let decls = a!(0).into_decl_list()?;
            let entry = a!(1).into_expr()?;
            StackValue::Program(Program::new(decls, entry, span))
        }

        // ── DeclList ─────────────────────────────────────────────────────
        NT::DeclList => {
            // DeclList → ε  (ya cubierto por is_epsilon, pero len==0 no pasa el guard)
            if prod.is_epsilon() {
                StackValue::DeclList(vec![])
            } else {
                // DeclList → DeclList Decl   (len 2)
                let mut list = a!(0).into_decl_list()?;
                if let StackValue::DeclVal(d) = a!(1) { list.push(d); }
                StackValue::DeclList(list)
            }
        }

        // ── FuncDecl ─────────────────────────────────────────────────────
        // function id ( ParamList ) => Expr ;                len=8
        //   [function, id, (, ParamList, ), =>, Expr, ;]
        // function id ( ParamList ) BlockExpr               len=6
        //   [function, id, (, ParamList, ), Block]
        // function id ( ParamList ) : TypeName => Expr ;     len=10
        //   [function, id, (, ParamList, ), :, TypeName, =>, Expr, ;]
        // function id ( ParamList ) : TypeName BlockExpr     len=8
        //   [function, id, (, ParamList, ), :, TypeName, Block]
        NT::FuncDecl => {
            let (name, _) = a!(1).into_lexeme()?;
            let params    = a!(3).into_param_list()?;
            let (ret, body) = if is_tok(prod, 5, &Tok::Colon) {
                let ret  = a!(6).into_type_name()?;
                let body = if plen(prod) == 10 { a!(8).into_expr()? } else { a!(7).into_expr()? };
                (Some(ret), body)
            } else {
                let body = if is_tok(prod, 5, &Tok::Arrow) { a!(6).into_expr()? }
                           else                             { a!(5).into_expr()? };
                (None, body)
            };
            StackValue::DeclVal(Decl::Function(FuncDecl::new(name, params, ret, body, span)))
        }

        // ── ParamListNonEmpty ─────────────────────────────────────────────
        NT::ParamListNonEmpty => {
            // ParamListNonEmpty → ParamListNonEmpty , Param   (len 3)
            let mut list = a!(0).into_param_list()?;
            if let StackValue::ParamVal(p) = a!(2) { list.push(p); }
            StackValue::ParamList(list)
        }

        // ── Param ─────────────────────────────────────────────────────────
        // Param → IDENTIFIER          (len 1)
        // Param → IDENTIFIER : TypeName  (len 3)
        NT::Param => {
            let (name, s) = a!(0).into_lexeme()?;
            if plen(prod) == 1 {
                StackValue::ParamVal(Param::new(name, None, s))
            } else {
                let type_ann = a!(2).into_type_name()?;
                StackValue::ParamVal(Param::new(name, Some(type_ann), s))
            }
        }

        // ── TypeDecl ─────────────────────────────────────────────────────
        // type IDENTIFIER TypeArgs InheritClause { TypeBody }   len=7
        NT::TypeDecl => {
            let (name, _)  = a!(1).into_lexeme()?;
            let type_args  = a!(2).into_param_list()?;
            // InheritClause puede ser Empty, TypeNameVal, o ExprList+TypeName
            let (parent, parent_args) = extract_inherit(a!(3))?;
            let members    = a!(5).into_type_member_list()?;
            StackValue::DeclVal(Decl::Type(TypeDecl::new(
                name, type_args, parent, parent_args, members, span
            )))
        }

        // ── TypeArgs ─────────────────────────────────────────────────────
        // TypeArgs → ε              (len 0)
        // TypeArgs → ( TypeArgList )  (len 3)
        NT::TypeArgs => {
            if prod.is_epsilon() { StackValue::ParamList(vec![]) }
            else                 { a!(1) }
        }

        // ── TypeArgList ───────────────────────────────────────────────────
        // TypeArgList → TypeArgList , Param   (len 3)
        NT::TypeArgList => {
            let mut list = a!(0).into_param_list()?;
            if let StackValue::ParamVal(p) = a!(2) { list.push(p); }
            StackValue::ParamList(list)
        }

        // ── InheritClause ─────────────────────────────────────────────────
        // InheritClause → ε                       (len 0)
        // InheritClause → inherits TypeName        (len 2)
        // InheritClause → inherits TypeName(Args)  (len 5)
        NT::InheritClause => {
            if prod.is_epsilon() {
                StackValue::Empty
            } else {
                a!(1)  // TypeNameVal en ambas formas no-epsilon
            }
        }

        // ── TypeMemberList ────────────────────────────────────────────────
        // TypeMemberList → ε                           (len 0)
        // TypeMemberList → TypeMemberList TypeMember   (len 2)
        NT::TypeMemberList => {
            if prod.is_epsilon() {
                StackValue::TypeMemberList(vec![])
            } else {
                let mut list = a!(0).into_type_member_list()?;
                if let StackValue::TypeMemberVal(m) = a!(1) { list.push(m); }
                StackValue::TypeMemberList(list)
            }
        }

        // ── AttributeDef ──────────────────────────────────────────────────
        // id = Expr ;          (len 3)
        // id : TypeName = Expr ;  (len 5)
        NT::AttributeDef => {
            let (name, _) = a!(0).into_lexeme()?;
            if is_tok(prod, 1, &Tok::Colon) {
                let type_ann = a!(2).into_type_name()?;
                let value    = a!(4).into_expr()?;
                StackValue::TypeMemberVal(TypeMember::Attribute(
                    AttributeDef::new(name, Some(type_ann), value, span)
                ))
            } else {
                let value = a!(2).into_expr()?;
                StackValue::TypeMemberVal(TypeMember::Attribute(
                    AttributeDef::new(name, None, value, span)
                ))
            }
        }

        // ── MethodDef ─────────────────────────────────────────────────────
        NT::MethodDef => {
            let (name, _) = a!(0).into_lexeme()?;
            let params    = a!(2).into_param_list()?;
            let (ret, body) = if is_tok(prod, 4, &Tok::Colon) {
                let ret  = a!(5).into_type_name()?;
                let body = if plen(prod) == 9 { a!(7).into_expr()? } else { a!(6).into_expr()? };
                (Some(ret), body)
            } else {
                let body = if is_tok(prod, 4, &Tok::Arrow) { a!(5).into_expr()? }
                           else                             { a!(4).into_expr()? };
                (None, body)
            };
            StackValue::TypeMemberVal(TypeMember::Method(
                MethodDef::new(name, params, ret, body, span)
            ))
        }

        // ── ProtocolDecl ──────────────────────────────────────────────────
        // protocol id { Body }                (len 4)
        // protocol id extends TypeName { Body }  (len 6)
        NT::ProtocolDecl => {
            let (name, _) = a!(1).into_lexeme()?;
            let (extends, methods) = if is_tok(prod, 2, &Tok::Extends) {
                (Some(a!(3).into_type_name()?), a!(5).into_method_sig_list()?)
            } else {
                (None, a!(3).into_method_sig_list()?)
            };
            StackValue::DeclVal(Decl::Protocol(ProtocolDecl::new(name, extends, methods, span)))
        }

        // ── ProtocolMemberList ────────────────────────────────────────────
        // ProtocolMemberList → ProtocolMemberList ProtocolMember   (len 2)
        NT::ProtocolMemberList => {
            if prod.is_epsilon() {
                StackValue::MethodSigList(vec![])
            } else {
                let mut list = a!(0).into_method_sig_list()?;
                if let StackValue::MethodSigVal(m) = a!(1) { list.push(m); }
                StackValue::MethodSigList(list)
            }
        }

        // ── ProtocolMember ────────────────────────────────────────────────
        // ProtocolMember → MethodSignature ;   (len 2)
        NT::ProtocolMember => a!(0),

        // ── MethodSignature ───────────────────────────────────────────────
        // id ( ParamList ) : TypeName   (len 6)
        // a!(0)=id  a!(1)=(  a!(2)=ParamList  a!(3)=)  a!(4)=:  a!(5)=TypeName
        NT::MethodSignature => {
            let (name, _) = a!(0).into_lexeme()?;
            let params    = a!(2).into_param_list()?;
            let ret       = a!(5).into_type_name()?;
            StackValue::MethodSigVal(MethodSignature::new(name, params, ret, span))
        }

        // ── TypeName ──────────────────────────────────────────────────────
        // IDENTIFIER                  (len 1)  → Simple   (terminal, NO passthrough)
        // IDENTIFIER [ ]              (len 3)  → Vector
        // IDENTIFIER *                (len 2)  → Iterable
        // IDENTIFIER [ ] [ ]          (len 5)  → Vector2D (no oficial, ver gramática)
        NT::TypeName => {
            let (name, s) = a!(0).into_lexeme()?;
            let tn = match plen(prod) {
                1 => TypeName::simple(name, s),
                3 => TypeName::vector(name, s),
                5 => TypeName::vector2d(name, s),
                _ => TypeName::iterable(name, s),
            };
            StackValue::TypeNameVal(tn)
        }

        // ── ArgListNonEmpty ───────────────────────────────────────────────
        // ArgListNonEmpty → ArgListNonEmpty , Expr   (len 3)
        NT::ArgListNonEmpty => {
            let mut list = a!(0).into_expr_list()?;
            list.push(a!(2).into_expr()?);
            StackValue::ExprList(list)
        }

        // ── AssignExpr con operador ───────────────────────────────────────
        // OrExpr OP AssignExpr   (len 3)
        NT::AssignExpr => {
            let op = match psym(prod, 1) {
                Some(Symbol::T(Tok::DestructAssign)) => AssignOp::Assign,
                Some(Symbol::T(Tok::PlusAssign))     => AssignOp::PlusAssign,
                Some(Symbol::T(Tok::MinusAssign))    => AssignOp::MinusAssign,
                Some(Symbol::T(Tok::MultAssign))     => AssignOp::MulAssign,
                Some(Symbol::T(Tok::DivAssign))      => AssignOp::DivAssign,
                Some(Symbol::T(Tok::ModAssign))      => AssignOp::ModAssign,
                _ => return Err(ParseError::internal("operador := desconocido", span)),
            };
            let target = a!(0).into_expr()?;
            let value  = a!(2).into_expr()?;
            StackValue::Expr(Expr::assign(op, target, value, span))
        }

        // ── OrExpr ───────────────────────────────────────────────────────
        NT::OrExpr => {
            StackValue::Expr(Expr::binary(
                BinaryOp::Or, a!(0).into_expr()?, a!(2).into_expr()?, span
            ))
        }

        // ── AndExpr ──────────────────────────────────────────────────────
        NT::AndExpr => {
            StackValue::Expr(Expr::binary(
                BinaryOp::And, a!(0).into_expr()?, a!(2).into_expr()?, span
            ))
        }

        // ── CompareExpr ───────────────────────────────────────────────────
        NT::CompareExpr => {
            let op = match psym(prod, 1) {
                Some(Symbol::T(Tok::Equal))     => BinaryOp::Eq,
                Some(Symbol::T(Tok::NotEqual))  => BinaryOp::NotEq,
                Some(Symbol::T(Tok::Less))      => BinaryOp::Less,
                Some(Symbol::T(Tok::Greater))   => BinaryOp::Greater,
                Some(Symbol::T(Tok::LessEq))    => BinaryOp::LessEq,
                Some(Symbol::T(Tok::GreaterEq)) => BinaryOp::GreaterEq,
                _ => return Err(ParseError::internal("operador comparación desconocido", span)),
            };
            StackValue::Expr(Expr::binary(op, a!(0).into_expr()?, a!(2).into_expr()?, span))
        }

        // ── IsAsExpr ──────────────────────────────────────────────────────
        NT::IsAsExpr => {
            let expr      = a!(0).into_expr()?;
            let type_name = a!(2).into_type_name()?;
            if is_tok(prod, 1, &Tok::Is) {
                StackValue::Expr(Expr::new(ExprKind::Is { expr: Box::new(expr), type_name }, span))
            } else {
                StackValue::Expr(Expr::new(ExprKind::As { expr: Box::new(expr), type_name }, span))
            }
        }

        // ── ConcatExpr ────────────────────────────────────────────────────
        NT::ConcatExpr => {
            let op = if is_tok(prod, 1, &Tok::DoubleConcat) {
                BinaryOp::DoubleConcat } else { BinaryOp::Concat };
            StackValue::Expr(Expr::binary(op, a!(0).into_expr()?, a!(2).into_expr()?, span))
        }

        // ── AddExpr ───────────────────────────────────────────────────────
        NT::AddExpr => {
            let op = if is_tok(prod, 1, &Tok::Plus) { BinaryOp::Add } else { BinaryOp::Sub };
            StackValue::Expr(Expr::binary(op, a!(0).into_expr()?, a!(2).into_expr()?, span))
        }

        // ── MulExpr ───────────────────────────────────────────────────────
        NT::MulExpr => {
            let op = match psym(prod, 1) {
                Some(Symbol::T(Tok::Multiply)) => BinaryOp::Mul,
                Some(Symbol::T(Tok::Divide))   => BinaryOp::Div,
                Some(Symbol::T(Tok::Modulo))   => BinaryOp::Mod,
                _ => return Err(ParseError::internal("operador * desconocido", span)),
            };
            StackValue::Expr(Expr::binary(op, a!(0).into_expr()?, a!(2).into_expr()?, span))
        }

        // ── PowerExpr ─────────────────────────────────────────────────────
        // UnaryExpr ^ PowerExpr  o  UnaryExpr ** PowerExpr   (len 3)
        NT::PowerExpr => {
            StackValue::Expr(Expr::binary(
                BinaryOp::Power, a!(0).into_expr()?, a!(2).into_expr()?, span
            ))
        }

        // ── UnaryExpr prefija ─────────────────────────────────────────────
        // - UnaryExpr  o  ! UnaryExpr   (len 2)
        NT::UnaryExpr => {
            let op = if is_tok(prod, 0, &Tok::Minus) { UnaryOp::Neg } else { UnaryOp::Not };
            StackValue::Expr(Expr::unary(op, a!(1).into_expr()?, span))
        }

        // ── PostfixExpr ───────────────────────────────────────────────────
        // CallOrAccess ++  o  CallOrAccess --   (len 2)
        NT::PostfixExpr => {
            let op = if is_tok(prod, 1, &Tok::Increment) {
                PostfixOp::Increment } else { PostfixOp::Decrement };
            StackValue::Expr(Expr::new(ExprKind::Postfix(Box::new(
                PostfixExpr::new(op, a!(0).into_expr()?, span)
            )), span))
        }

        // ── CallOrAccess ──────────────────────────────────────────────────
        NT::CallOrAccess => {
            match (plen(prod), psym(prod, 1)) {
                // callee ( ArgList )           len=4, [1]=LParen
                (4, Some(Symbol::T(Tok::LParen))) => {
                    let callee    = a!(0).into_expr()?;
                    let call_args = a!(2).into_expr_list()?;
                    StackValue::Expr(Expr::call(callee, call_args, span))
                }
                // expr . id ( ArgList )        len=6  [CallOrAccess . id ( ArgList )]
                (6, Some(Symbol::T(Tok::Dot))) => {
                    let object     = a!(0).into_expr()?;
                    let (method,_) = a!(2).into_lexeme()?;
                    let margs      = a!(4).into_expr_list()?;
                    StackValue::Expr(Expr::method_call(object, method, margs, span))
                }
                // expr . id                    len=3, [1]=Dot
                (3, Some(Symbol::T(Tok::Dot))) => {
                    let object    = a!(0).into_expr()?;
                    let (field,_) = a!(2).into_lexeme()?;
                    StackValue::Expr(Expr::access(object, field, span))
                }
                // expr [ expr ]                len=4, [1]=LBracket
                (4, Some(Symbol::T(Tok::LBracket))) => {
                    let collection = a!(0).into_expr()?;
                    let index      = a!(2).into_expr()?;
                    StackValue::Expr(Expr::index(collection, index, span))
                }
                _ => return Err(ParseError::internal(
                    format!("CallOrAccess desconocido len={}", plen(prod)), span)),
            }
        }

        // ── PrimaryExpr ───────────────────────────────────────────────────
        NT::PrimaryExpr => {
            match pfirst(prod) {
                Some(Symbol::T(Tok::Number)) => {
                    let (v, s) = a!(0).into_lexeme()?;
                    StackValue::Expr(Expr::number(v, s))
                }
                Some(Symbol::T(Tok::String)) => {
                    let (v, s) = a!(0).into_lexeme()?;
                    // The lexer includes surrounding quotes in the lexeme; strip them
                    // and process escape sequences (\n, \t, \", \\).
                    let content = if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
                        let raw = &v[1..v.len() - 1];
                        let mut out = String::with_capacity(raw.len());
                        let mut chars = raw.chars().peekable();
                        while let Some(c) = chars.next() {
                            if c == '\\' {
                                match chars.next() {
                                    Some('n')  => out.push('\n'),
                                    Some('t')  => out.push('\t'),
                                    Some('"')  => out.push('"'),
                                    Some('\\') => out.push('\\'),
                                    Some(o)    => { out.push('\\'); out.push(o); }
                                    None       => out.push('\\'),
                                }
                            } else {
                                out.push(c);
                            }
                        }
                        out
                    } else {
                        v
                    };
                    StackValue::Expr(Expr::string(content, s))
                }
                Some(Symbol::T(Tok::Char)) => {
                    let (v, s) = a!(0).into_lexeme()?;
                    StackValue::Expr(Expr::new(ExprKind::Literal(Literal::Char { value: v, span: s }), s))
                }
                Some(Symbol::T(Tok::True))  => StackValue::Expr(Expr::bool(true,  span)),
                Some(Symbol::T(Tok::False)) => StackValue::Expr(Expr::bool(false, span)),
                Some(Symbol::T(Tok::Null))  => StackValue::Expr(Expr::null(span)),
                Some(Symbol::T(Tok::Base))  => StackValue::Expr(Expr::new(ExprKind::Base, span)),
                Some(Symbol::T(Tok::Identifier)) => {
                    let (name, s) = a!(0).into_lexeme()?;
                    StackValue::Expr(Expr::identifier(name, s))
                }
                // ( Expr )   len=3
                Some(Symbol::T(Tok::LParen)) => a!(1),
                // expresiones compuestas como átomos — passthrough del NT
                Some(Symbol::NT(_)) => a!(0),
                _ => return Err(ParseError::internal("PrimaryExpr desconocido", span)),
            }
        }

        // ── BlockExpr ─────────────────────────────────────────────────────
        // { ExprList }    len=3
        // { ExprList ; }  len=4
        NT::BlockExpr => {
            let body = a!(1).into_expr_list()?;
            StackValue::Expr(Expr::block(body, span))
        }

        // ── ExprList ──────────────────────────────────────────────────────
        // ExprList → ExprList ; Expr   (len 3)
        NT::ExprList => {
            let mut list = a!(0).into_expr_list()?;
            list.push(a!(2).into_expr()?);
            StackValue::ExprList(list)
        }

        // ── LetExpr ───────────────────────────────────────────────────────
        // let LetBindingList in Expr   (len 4)
        NT::LetExpr => {
            let bindings = a!(1).into_binding_list()?;
            let body     = a!(3).into_expr()?;
            StackValue::Expr(Expr::let_expr(bindings, body, span))
        }

        // ── LetBindingList ────────────────────────────────────────────────
        // LetBindingList → LetBindingList , LetBinding   (len 3)
        NT::LetBindingList => {
            let mut list = a!(0).into_binding_list()?;
            if let StackValue::Binding(b) = a!(2) { list.push(b); }
            StackValue::BindingList(list)
        }

        // ── LetBinding ────────────────────────────────────────────────────
        // id = Expr           (len 3)
        // id : TypeName = Expr  (len 5)
        NT::LetBinding => {
            let (name, s) = a!(0).into_lexeme()?;
            if is_tok(prod, 1, &Tok::Assign) {
                StackValue::Binding(LetBinding::new(name, None, a!(2).into_expr()?, s))
            } else {
                let type_ann = a!(2).into_type_name()?;
                let value    = a!(4).into_expr()?;
                StackValue::Binding(LetBinding::new(name, Some(type_ann), value, s))
            }
        }

        // ── IfExpr ────────────────────────────────────────────────────────
        // if ( Expr ) Expr ElifChain else Expr   (len 8)
        NT::IfExpr => {
            let cond  = a!(2).into_expr()?;
            let then  = a!(4).into_expr()?;
            let elifs = a!(5).into_elif_list()?;
            let else_ = a!(7).into_expr()?;
            StackValue::Expr(Expr::if_expr(cond, then, elifs, else_, span))
        }

        // ── ElifChain ─────────────────────────────────────────────────────
        // ElifChain → ε                               (len 0)
        // ElifChain → ElifChain elif ( Expr ) Expr    (len 6)
        NT::ElifChain => {
            if prod.is_epsilon() {
                StackValue::ElifList(vec![])
            } else {
                let mut list = a!(0).into_elif_list()?;
                let cond = a!(3).into_expr()?;
                let body = a!(5).into_expr()?;
                list.push(ElifBranch::new(cond, body, span));
                StackValue::ElifList(list)
            }
        }

        // ── WhileExpr ─────────────────────────────────────────────────────
        // while ( Expr ) Expr   (len 5)
        NT::WhileExpr => {
            let cond = a!(2).into_expr()?;
            let body = a!(4).into_expr()?;
            StackValue::Expr(Expr::while_expr(cond, body, span))
        }

        // ── ForExpr ───────────────────────────────────────────────────────
        // for ( id in Expr ) Expr   (len 7)
        NT::ForExpr => {
            let (var, _) = a!(2).into_lexeme()?;
            let iterable = a!(4).into_expr()?;
            let body     = a!(6).into_expr()?;
            StackValue::Expr(Expr::for_expr(var, iterable, body, span))
        }

        // ── NewExpr ───────────────────────────────────────────────────────
        // new TypeName ( ArgList )                                  (len 5, [2]=LParen)
        // new Identifier [ Expr ]                                   (len 5, [2]=LBracket) —
        // new Identifier [ Expr ] { Identifier -> Expr }            (len 10)              —
        // new Identifier [ ] [ Expr ]                               (len 7)               — 
        NT::NewExpr => {
            match (plen(prod), prod.body.get(2)) {
                (5, Some(Symbol::T(Tok::LParen))) => {
                    let type_name = a!(1).into_type_name()?;
                    let new_args  = a!(3).into_expr_list()?;
                    StackValue::Expr(Expr::new(ExprKind::New(Box::new(NewExpr::new(type_name, new_args, span))), span))
                }
                (5, Some(Symbol::T(Tok::LBracket))) => {
                    // new Identifier [ Expr ]  → vector 1D, sin generador
                    let (name, _) = a!(1).into_lexeme()?;
                    let size      = a!(3).into_expr()?;
                    let elem_type = TypeName::simple(name, span);
                    StackValue::Expr(Expr::new(ExprKind::Vector(Box::new(
                        VectorExpr::alloc(elem_type, size, None, span)
                    )), span))
                }
                (10, _) => {
                    // new Identifier [ Expr ] { Identifier -> Expr }  → vector 1D, con generador por índice
                    let (name, _) = a!(1).into_lexeme()?;
                    let size      = a!(3).into_expr()?;
                    let (var, _)  = a!(6).into_lexeme()?;
                    let body      = a!(8).into_expr()?;
                    let elem_type = TypeName::simple(name, span);
                    StackValue::Expr(Expr::new(ExprKind::Vector(Box::new(
                        VectorExpr::alloc(elem_type, size, Some((var, body)), span)
                    )), span))
                }
                (7, _) => {
                    // new Identifier [ ] [ Expr ]  → vector 2D (vector de Identifier[]), sin generador
                    let (name, _) = a!(1).into_lexeme()?;
                    let size      = a!(5).into_expr()?;
                    let elem_type = TypeName::vector(name, span);
                    StackValue::Expr(Expr::new(ExprKind::Vector(Box::new(
                        VectorExpr::alloc(elem_type, size, None, span)
                    )), span))
                }
                _ => return Err(ParseError::internal(
                    format!("NewExpr desconocido: len={}", plen(prod)), span)),
            }
        }

        // ── VectorLiteral ─────────────────────────────────────────────────
        NT::VectorLiteral => {
            match plen(prod) {
                2 => {
                    // [ ]
                    StackValue::Expr(Expr::new(ExprKind::Vector(Box::new(VectorExpr::explicit(vec![], span))), span))
                }
                3 => {
                    // [ ArgListNonEmpty ]
                    let elems = a!(1).into_expr_list()?;
                    StackValue::Expr(Expr::new(ExprKind::Vector(Box::new(VectorExpr::explicit(elems, span))), span))
                }
                7 => {
                    // [ Expr | id in Expr ]
                    let body     = a!(1).into_expr()?;
                    let (var, _) = a!(3).into_lexeme()?;
                    let iterable = a!(5).into_expr()?;
                    StackValue::Expr(Expr::new(ExprKind::Vector(Box::new(
                        VectorExpr::generator(body, var, iterable, span)
                    )), span))
                }
                5 => {
                    // { Expr , ArgListNonEmpty }   (alias de llaves, no oficial)
                    let first = a!(1).into_expr()?;
                    let rest  = a!(3).into_expr_list()?;
                    let mut elems = vec![first];
                    elems.extend(rest);
                    StackValue::Expr(Expr::new(ExprKind::Vector(Box::new(
                        VectorExpr::explicit(elems, span)
                    )), span))
                }
                _ => return Err(ParseError::internal("VectorLiteral desconocido", span)),
            }
        }

        // ── Epsilon de ParamList / ArgList ───────────────────────────────────
        // len=0 (epsilon); el caso len=1 (NT→NT) ya fue manejado por passthrough
        NT::ParamList | NT::ArgList => StackValue::Empty,

        other => {
            return Err(ParseError::internal(
                format!("NT sin acción semántica: {:?} (prod_id={})", other, prod_id),
                span,
            ));
        }
    };

    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
//  Helper para extraer herencia desde InheritClause
// ─────────────────────────────────────────────────────────────────────────────

fn extract_inherit(
    val: StackValue,
) -> Result<(Option<TypeName>, Vec<Expr>), ParseError> {
    match val {
        StackValue::Empty        => Ok((None, vec![])),
        StackValue::TypeNameVal(t) => Ok((Some(t), vec![])),
        _ => Err(ParseError::internal("InheritClause inesperado", Span::dummy())),
    }
}