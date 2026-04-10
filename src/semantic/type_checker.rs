use crate::parser::ast::*;
use super::{
    errors::SemanticError,
    symbol_table::{Symbol, SymbolKind, SymbolTable},
    type_system::{FuncSignature, HulkType, TypeHierarchy, TypeInfo},
};

pub struct TypeChecker {
    pub symbols:  SymbolTable,
    pub types:    TypeHierarchy,
    pub errors:   Vec<SemanticError>,

    // Contexto de chequeo
    current_type:   Option<String>,   // tipo que se está analizando
    current_return: Option<HulkType>, // retorno esperado de la función actual
    in_initializer: bool,             // true si estamos en attr init (self prohibido)
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            symbols:        SymbolTable::new(),
            types:          TypeHierarchy::new(),
            errors:         vec![],
            current_type:   None,
            current_return: None,
            in_initializer: false,
        };
        checker.register_builtin_functions();
        checker
    }

    fn register_builtin_functions(&mut self) {
        // print, sqrt, sin, cos, range, etc.
        let builtins = [
            ("print",  vec![HulkType::Object], HulkType::Null),
            ("sqrt",   vec![HulkType::Number], HulkType::Number),
            ("sin",    vec![HulkType::Number], HulkType::Number),
            ("cos",    vec![HulkType::Number], HulkType::Number),
            ("rand",   vec![],                 HulkType::Number),
            ("range",  vec![HulkType::Number, HulkType::Number],
                                               HulkType::UserDefined("Range".into())),
        ];
        for (name, params, ret) in builtins {
            self.symbols.define(name, Symbol {
                name: name.into(),
                kind: SymbolKind::Function {
                    params,
                    return_type: ret,
                },
            });
        }
    }

    // ─── Punto de entrada ────────────────────────────────────────────────────

    pub fn check_program(&mut self, program: &Program) -> Vec<SemanticError> {
        // Paso 1: registrar todos los tipos y protocolos (forward declaration)
        self.collect_declarations(&program.declarations);

        // Paso 2: chequear cuerpos de declaraciones
        for decl in &program.declarations {
            self.check_decl(decl);
        }

        // Paso 3: chequear la expresión de entrada
        self.check_expr(&program.entry);

        std::mem::take(&mut self.errors)
    }

    // ─── Paso 1: recolectar declaraciones ────────────────────────────────────

    fn collect_declarations(&mut self, decls: &[Decl]) {
        // Primero registrar nombres (permite recursión mutua)
        for decl in decls {
            match decl {
                Decl::Function(f)  => self.register_function_signature(f),
                Decl::Type(t)      => self.register_type_signature(t),
                Decl::Protocol(p)  => self.register_protocol_signature(p),
            }
        }
    }

    fn register_function_signature(&mut self, f: &FuncDecl) {
        if self.symbols.in_current_scope(&f.name) {
            self.errors.push(SemanticError::Redefinition {
                name: f.name.clone(), span: f.span,
            });
            return;
        }
        let params: Vec<HulkType> = f.params.iter()
            .map(|p| self.resolve_type_ann(&p.type_ann, p.span))
            .collect();
        let ret = self.resolve_type_ann(&f.return_type, f.span);
        self.symbols.define(&f.name, Symbol {
            name: f.name.clone(),
            kind: SymbolKind::Function { params, return_type: ret },
        });
    }

    fn register_type_signature(&mut self, t: &TypeDecl) {
        // Verificar que no herede de primitivos
        if let Some(parent) = &t.parent {
            let pname = parent.name();
            if matches!(pname, "Number" | "String" | "Boolean") {
                self.errors.push(SemanticError::InheritFromPrimitive {
                    type_name: t.name.clone(), span: t.span,
                });
            }
        }
        self.types.types.insert(t.name.clone(), TypeInfo {
            name:       t.name.clone(),
            parent:     t.parent.as_ref().map(|p| p.name().into()),
            attributes: std::collections::HashMap::new(),
            methods:    std::collections::HashMap::new(),
            is_builtin: false,
            protocols:  vec![],
        });
        self.symbols.define(&t.name, Symbol {
            name: t.name.clone(),
            kind: SymbolKind::Type,
        });
    }

    fn register_protocol_signature(&mut self, p: &ProtocolDecl) {
        // similar a register_type_signature pero para protocolos
        self.symbols.define(&p.name, Symbol {
            name: p.name.clone(),
            kind: SymbolKind::Protocol,
        });
    }

    // ─── Paso 2: chequear declaraciones ──────────────────────────────────────

    fn check_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Function(f)  => self.check_func_decl(f),
            Decl::Type(t)      => self.check_type_decl(t),
            Decl::Protocol(p)  => self.check_protocol_decl(p),
        }
    }

    fn check_func_decl(&mut self, f: &FuncDecl) {
        self.symbols.push_scope();

        // Definir parámetros en el scope de la función
        for param in &f.params {
            let ty = self.resolve_type_ann(&param.type_ann, param.span);
            self.symbols.define(&param.name, Symbol {
                name: param.name.clone(),
                kind: SymbolKind::Variable { ty, mutable: false },
            });
        }

        let expected_ret = self.resolve_type_ann(&f.return_type, f.span);
        self.current_return = Some(expected_ret.clone());

        let actual_ret = self.check_expr(&f.body);

        // Verificar retorno si fue anotado
        if expected_ret != HulkType::Unknown {
            if !self.types.conforms(&actual_ret, &expected_ret) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: expected_ret.name(),
                    found:    actual_ret.name(),
                    span:     f.span,
                });
            }
        }

        self.current_return = None;
        self.symbols.pop_scope();
    }

    fn check_type_decl(&mut self, t: &TypeDecl) {
        self.current_type = Some(t.name.clone());
        self.symbols.push_scope();

        // Definir parámetros del constructor
        for param in &t.type_args {
            let ty = self.resolve_type_ann(&param.type_ann, param.span);
            self.symbols.define(&param.name, Symbol {
                name: param.name.clone(),
                kind: SymbolKind::Variable { ty, mutable: false },
            });
        }

        // Chequear miembros
        for member in &t.members {
            match member {
                TypeMember::Attribute(attr) => {
                    self.in_initializer = true;
                    let ty = self.check_expr(&attr.value);
                    self.in_initializer = false;

                    // Si tiene anotación, verificar conformance
                    if let Some(ann) = &attr.type_ann {
                        let ann_ty = self.resolve_type_name(ann);
                        if !self.types.conforms(&ty, &ann_ty) {
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: ann_ty.name(),
                                found:    ty.name(),
                                span:     attr.span,
                            });
                        }
                    }
                }
                TypeMember::Method(method) => {
                    self.symbols.push_scope();

                    // self disponible en métodos
                    self.symbols.define("self", Symbol {
                        name: "self".into(),
                        kind: SymbolKind::Variable {
                            ty: HulkType::UserDefined(t.name.clone()),
                            mutable: false,
                        },
                    });

                    for param in &method.params {
                        let ty = self.resolve_type_ann(&param.type_ann, param.span);
                        self.symbols.define(&param.name, Symbol {
                            name: param.name.clone(),
                            kind: SymbolKind::Variable { ty, mutable: false },
                        });
                    }

                    self.check_expr(&method.body);
                    self.symbols.pop_scope();
                }
            }
        }

        self.symbols.pop_scope();
        self.current_type = None;
    }

    fn check_protocol_decl(&mut self, p: &ProtocolDecl) {
        // Verificar que el protocolo padre existe si lo declara
        if let Some(extends) = &p.extends {
            if self.symbols.lookup(extends.name()).is_none() {
                self.errors.push(SemanticError::UndefinedType {
                    name: extends.name().into(),
                    span: extends.span(),
                });
            }
        }
        // Las firmas no tienen cuerpo — solo registrar
    }

    // ─── Paso 3: chequear expresiones ────────────────────────────────────────

    fn check_expr(&mut self, expr: &Expr) -> HulkType {
        match expr {
            Expr::Literal(lit)      => self.check_literal(lit),
            Expr::Identifier(id)    => self.check_identifier(&id.name, id.span),
            Expr::Binary(b)         => self.check_binary(b),
            Expr::Unary(u)          => self.check_unary(u),
            Expr::Block(b)          => self.check_block(b),
            Expr::Let(l)            => self.check_let(l),
            Expr::If(i)             => self.check_if(i),
            Expr::While(w)          => self.check_while(w),
            Expr::For(f)            => self.check_for(f),
            Expr::Call(c)           => self.check_call(c),
            Expr::MethodCall(m)     => self.check_method_call(m),
            Expr::Access(a)         => self.check_access(a),
            Expr::Index(i)          => self.check_index(i),
            Expr::New(n)            => self.check_new(n),
            Expr::Assign(a)         => self.check_assign(a),
            Expr::Vector(v)         => self.check_vector(v),
            Expr::Postfix(p)        => self.check_postfix(p),
            Expr::Is(i)             => self.check_is(i),
            Expr::As(a)             => self.check_as(a),
            Expr::Lambda(l)         => self.check_lambda(l),
        }
    }

    fn check_literal(&self, lit: &Literal) -> HulkType {
        match lit {
            Literal::Number { .. } => HulkType::Number,
            Literal::String { .. } => HulkType::StringT,
            Literal::Bool   { .. } => HulkType::Boolean,
            Literal::Char   { .. } => HulkType::StringT,
            Literal::Null   { .. } => HulkType::Null,
        }
    }

    fn check_identifier(&mut self, name: &str, span: Span) -> HulkType {
        // self en inicializador de atributo — error
        if name == "self" && self.in_initializer {
            self.errors.push(SemanticError::SelfInInitializer { span });
            return HulkType::Never;
        }
        match self.symbols.lookup(name) {
            Some(sym) => match &sym.kind {
                SymbolKind::Variable { ty, .. } => ty.clone(),
                SymbolKind::Function { params, return_type } =>
                    HulkType::UserDefined(format!("fn({:?})->{}", params, return_type.name())),
                _ => HulkType::Object,
            },
            None => {
                self.errors.push(SemanticError::UndefinedVariable {
                    name: name.into(), span,
                });
                HulkType::Never
            }
        }
    }

    fn check_block(&mut self, b: &BlockExpr) -> HulkType {
        self.symbols.push_scope();
        let mut ty = HulkType::Null;
        for expr in &b.body {
            ty = self.check_expr(expr);
        }
        self.symbols.pop_scope();
        ty // tipo del bloque = tipo de la última expresión
    }

    fn check_let(&mut self, l: &LetExpr) -> HulkType {
        self.symbols.push_scope();

        // Bindings de izquierda a derecha — cada uno ve los anteriores
        for binding in &l.bindings {
            let val_ty = self.check_expr(&binding.value);
            let ty = if let Some(ann) = &binding.type_ann {
                let ann_ty = self.resolve_type_name(ann);
                if !self.types.conforms(&val_ty, &ann_ty) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: ann_ty.name(),
                        found:    val_ty.name(),
                        span:     binding.span,
                    });
                }
                ann_ty
            } else {
                val_ty
            };
            if !self.symbols.define(&binding.name, Symbol {
                name: binding.name.clone(),
                kind: SymbolKind::Variable { ty, mutable: true },
            }) {
                self.errors.push(SemanticError::Redefinition {
                    name: binding.name.clone(), span: binding.span,
                });
            }
        }

        let body_ty = self.check_expr(&l.body);
        self.symbols.pop_scope();
        body_ty
    }

    fn check_if(&mut self, i: &IfExpr) -> HulkType {
        let cond_ty = self.check_expr(&i.condition);
        if cond_ty != HulkType::Boolean && cond_ty != HulkType::Never {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "Boolean".into(),
                found:    cond_ty.name(),
                span:     i.span,
            });
        }

        let mut result_ty = self.check_expr(&i.then_body);

        for elif in &i.elif_chain {
            let elif_cond = self.check_expr(&elif.condition);
            if elif_cond != HulkType::Boolean && elif_cond != HulkType::Never {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: "Boolean".into(),
                    found:    elif_cond.name(),
                    span:     elif.span,
                });
            }
            let elif_ty = self.check_expr(&elif.body);
            result_ty = self.types.lca(&result_ty, &elif_ty);
        }

        let else_ty = self.check_expr(&i.else_body);
        self.types.lca(&result_ty, &else_ty)
    }

    fn check_while(&mut self, w: &WhileExpr) -> HulkType {
        let cond_ty = self.check_expr(&w.condition);
        if cond_ty != HulkType::Boolean && cond_ty != HulkType::Never {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "Boolean".into(),
                found:    cond_ty.name(),
                span:     w.span,
            });
        }
        self.check_expr(&w.body)
    }

    fn check_for(&mut self, f: &ForExpr) -> HulkType {
        let iter_ty = self.check_expr(&f.iterable);

        // El iterable debe conformar con el protocolo Iterable
        if !self.types.conforms(&iter_ty, &HulkType::Protocol("Iterable".into())) {
            // Vector también es válido
            if !matches!(iter_ty, HulkType::Vector(_)) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: "Iterable".into(),
                    found:    iter_ty.name(),
                    span:     f.span,
                });
            }
        }

        let elem_ty = match &iter_ty {
            HulkType::Vector(t) => *t.clone(),
            _                   => HulkType::Object, // se refina con Iterable<T>
        };

        self.symbols.push_scope();
        self.symbols.define(&f.var, Symbol {
            name: f.var.clone(),
            kind: SymbolKind::Variable { ty: elem_ty, mutable: false },
        });

        let body_ty = self.check_expr(&f.body);
        self.symbols.pop_scope();
        body_ty
    }

    fn check_call(&mut self, c: &CallExpr) -> HulkType {
        match c.callee.as_ref() {
            Expr::Identifier(id) => {
                match self.symbols.lookup(&id.name).cloned() {
                    Some(Symbol { kind: SymbolKind::Function { params, return_type }, .. }) => {
                        if c.args.len() != params.len() {
                            self.errors.push(SemanticError::WrongArgCount {
                                name:     id.name.clone(),
                                expected: params.len(),
                                found:    c.args.len(),
                                span:     c.span,
                            });
                        } else {
                            for (arg, expected_ty) in c.args.iter().zip(&params) {
                                let arg_ty = self.check_expr(arg);
                                if !self.types.conforms(&arg_ty, expected_ty) {
                                    self.errors.push(SemanticError::TypeMismatch {
                                        expected: expected_ty.name(),
                                        found:    arg_ty.name(),
                                        span:     c.span,
                                    });
                                }
                            }
                        }
                        return_type
                    }
                    Some(Symbol { kind: SymbolKind::Type, .. }) => {
                        // new sin keyword — tratar como instanciación
                        HulkType::UserDefined(id.name.clone())
                    }
                    None => {
                        self.errors.push(SemanticError::UndefinedFunction {
                            name: id.name.clone(), span: c.span,
                        });
                        HulkType::Never
                    }
                    _ => {
                        self.errors.push(SemanticError::NotCallable { span: c.span });
                        HulkType::Never
                    }
                }
            }
            callee => {
                // Functor / lambda — chequear el callee y asumir retorno Object
                self.check_expr(callee);
                HulkType::Object
            }
        }
    }

    fn check_method_call(&mut self, m: &MethodCallExpr) -> HulkType {
        let obj_ty = self.check_expr(&m.object);
        let type_name = match &obj_ty {
            HulkType::UserDefined(n) => n.clone(),
            HulkType::Number   => "Number".into(),
            HulkType::StringT  => "String".into(),
            HulkType::Boolean  => "Boolean".into(),
            _                  => return HulkType::Object,
        };

        match self.types.types.get(&type_name)
            .and_then(|t| t.methods.get(&m.method))
            .cloned()
        {
            Some(sig) => {
                // Verificar args
                for (arg, (_, expected)) in m.args.iter().zip(&sig.params) {
                    let arg_ty = self.check_expr(arg);
                    if !self.types.conforms(&arg_ty, expected) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: expected.name(),
                            found:    arg_ty.name(),
                            span:     m.span,
                        });
                    }
                }
                sig.return_type
            }
            None => {
                self.errors.push(SemanticError::MethodNotFound {
                    type_name,
                    method: m.method.clone(),
                    span:   m.span,
                });
                HulkType::Never
            }
        }
    }

    fn check_access(&mut self, a: &AccessExpr) -> HulkType {
        let obj_ty = self.check_expr(&a.object);
        let type_name = match &obj_ty {
            HulkType::UserDefined(n) => n.clone(),
            _ => return HulkType::Object,
        };
        match self.types.types.get(&type_name)
            .and_then(|t| t.attributes.get(&a.field))
            .cloned()
        {
            Some(ty) => ty,
            None => {
                self.errors.push(SemanticError::AttributeNotFound {
                    type_name,
                    attr: a.field.clone(),
                    span: a.span,
                });
                HulkType::Never
            }
        }
    }

    fn check_index(&mut self, i: &IndexExpr) -> HulkType {
        let coll_ty = self.check_expr(&i.collection);
        let idx_ty  = self.check_expr(&i.index);
        if idx_ty != HulkType::Number {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "Number".into(),
                found:    idx_ty.name(),
                span:     i.span,
            });
        }
        match coll_ty {
            HulkType::Vector(t) => *t,
            _ => {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: "Vector".into(),
                    found:    coll_ty.name(),
                    span:     i.span,
                });
                HulkType::Never
            }
        }
    }

    fn check_new(&mut self, n: &NewExpr) -> HulkType {
        let ty_name = n.type_name.name();
        if self.types.types.get(ty_name).is_none() {
            self.errors.push(SemanticError::UndefinedType {
                name: ty_name.into(), span: n.span,
            });
            return HulkType::Never;
        }
        // Verificar args del constructor
        // (requiere tener la firma del constructor registrada)
        for arg in &n.args { self.check_expr(arg); }
        HulkType::UserDefined(ty_name.into())
    }

    fn check_assign(&mut self, a: &AssignExpr) -> HulkType {
        // Verificar que el target NO es self
        if let Expr::Identifier(id) = a.target.as_ref() {
            if id.name == "self" {
                self.errors.push(SemanticError::SelfAssignment { span: a.span });
                return HulkType::Never;
            }
        }

        // Verificar que el target es un lvalue válido
        let is_lvalue = matches!(
            a.target.as_ref(),
            Expr::Identifier(_) | Expr::Access(_) | Expr::Index(_)
        );
        if !is_lvalue {
            self.errors.push(SemanticError::InvalidLValue { span: a.span });
            return HulkType::Never;
        }

        let target_ty = self.check_expr(&a.target);
        let value_ty  = self.check_expr(&a.value);

        if !self.types.conforms(&value_ty, &target_ty) {
            self.errors.push(SemanticError::TypeMismatch {
                expected: target_ty.name(),
                found:    value_ty.name(),
                span:     a.span,
            });
        }
        target_ty
    }

    fn check_binary(&mut self, b: &BinaryExpr) -> HulkType {
        let lt = self.check_expr(&b.left);
        let rt = self.check_expr(&b.right);

        match &b.op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul |
            BinaryOp::Div | BinaryOp::Mod | BinaryOp::Power => {
                if lt != HulkType::Number || rt != HulkType::Number {
                    self.errors.push(SemanticError::InvalidBinaryTypes {
                        op:    format!("{:?}", b.op),
                        left:  lt.name(), right: rt.name(), span: b.span,
                    });
                }
                HulkType::Number
            }
            BinaryOp::And | BinaryOp::Or => {
                if lt != HulkType::Boolean || rt != HulkType::Boolean {
                    self.errors.push(SemanticError::InvalidBinaryTypes {
                        op: format!("{:?}", b.op),
                        left: lt.name(), right: rt.name(), span: b.span,
                    });
                }
                HulkType::Boolean
            }
            BinaryOp::Eq | BinaryOp::NotEq |
            BinaryOp::Less | BinaryOp::Greater |
            BinaryOp::LessEq | BinaryOp::GreaterEq => {
                HulkType::Boolean
            }
            BinaryOp::Concat | BinaryOp::DoubleConcat => {
                // @ y @@ permiten String @ Number, etc.
                HulkType::StringT
            }
        }
    }

    fn check_unary(&mut self, u: &UnaryExpr) -> HulkType {
        let ty = self.check_expr(&u.operand);
        match &u.op {
            UnaryOp::Neg => {
                if ty != HulkType::Number {
                    self.errors.push(SemanticError::InvalidOperandType {
                        op: "-".into(), found: ty.name(), span: u.span,
                    });
                }
                HulkType::Number
            }
            UnaryOp::Not => {
                if ty != HulkType::Boolean {
                    self.errors.push(SemanticError::InvalidOperandType {
                        op: "!".into(), found: ty.name(), span: u.span,
                    });
                }
                HulkType::Boolean
            }
        }
    }

    fn check_vector(&mut self, v: &VectorExpr) -> HulkType {
        match v {
            VectorExpr::Explicit { elements, .. } => {
                if elements.is_empty() {
                    return HulkType::Vector(Box::new(HulkType::Object));
                }
                let first_ty = self.check_expr(&elements[0]);
                let mut unified = first_ty;
                for elem in elements.iter().skip(1) {
                    let ty = self.check_expr(elem);
                    unified = self.types.lca(&unified, &ty);
                }
                HulkType::Vector(Box::new(unified))
            }
            VectorExpr::Generator { body, var, iterable, span } => {
                let iter_ty = self.check_expr(iterable);
                let elem_ty = match &iter_ty {
                    HulkType::Vector(t) => *t.clone(),
                    _ => HulkType::Object,
                };
                self.symbols.push_scope();
                self.symbols.define(var, Symbol {
                    name: var.clone(),
                    kind: SymbolKind::Variable { ty: elem_ty, mutable: false },
                });
                let body_ty = self.check_expr(body);
                self.symbols.pop_scope();
                HulkType::Vector(Box::new(body_ty))
            }
        }
    }

    fn check_postfix(&mut self, p: &PostfixExpr) -> HulkType {
        let ty = self.check_expr(&p.operand);
        if ty != HulkType::Number {
            self.errors.push(SemanticError::InvalidOperandType {
                op: format!("{:?}", p.op), found: ty.name(), span: p.span,
            });
        }
        HulkType::Number
    }

    fn check_is(&mut self, i: &IsExpr) -> HulkType {
        self.check_expr(&i.expr);
        // is siempre retorna Boolean, verificación en runtime
        HulkType::Boolean
    }

    fn check_as(&mut self, a: &AsExpr) -> HulkType {
        let expr_ty = self.check_expr(&a.expr);
        let target_ty = self.resolve_type_name(&a.target_type);

        // Solo tiene sentido si hay relación de herencia (up o downcast)
        let valid = self.types.conforms(&expr_ty, &target_ty)
                 || self.types.conforms(&target_ty, &expr_ty);
        if !valid {
            self.errors.push(SemanticError::DowncastFailed {
                from: expr_ty.name(), to: target_ty.name(), span: a.span,
            });
        }
        target_ty
    }

    fn check_lambda(&mut self, l: &LambdaExpr) -> HulkType {
        self.symbols.push_scope();
        for param in &l.params {
            let ty = self.resolve_type_ann(&param.type_ann, param.span);
            self.symbols.define(&param.name, Symbol {
                name: param.name.clone(),
                kind: SymbolKind::Variable { ty, mutable: false },
            });
        }
        let ret = self.check_expr(&l.body);
        self.symbols.pop_scope();
        // Retorna un tipo función — simplificado como Object por ahora
        HulkType::Object
    }

    // ─── Helpers ──────────────────────────────────────────────────────────────

    fn resolve_type_name(&self, tn: &TypeName) -> HulkType {
        match tn {
            TypeName::Simple { name, .. } => match name.as_str() {
                "Number"  => HulkType::Number,
                "String"  => HulkType::StringT,
                "Boolean" => HulkType::Boolean,
                "Object"  => HulkType::Object,
                other     => HulkType::UserDefined(other.into()),
            },
            TypeName::Vector { name, .. } =>
                HulkType::Vector(Box::new(HulkType::UserDefined(name.clone()))),
            TypeName::Iterable { name, .. } =>
                HulkType::Protocol("Iterable".into()),
        }
    }

    fn resolve_type_ann(&self, ann: &Option<TypeName>, _span: Span) -> HulkType {
        match ann {
            Some(tn) => self.resolve_type_name(tn),
            None     => HulkType::Unknown, // se infiere
        }
    }
}