// src/semantic/type_checker.rs

use crate::parser::ast::{
    // Programa
    Program,
    // Declaraciones
    Decl, FuncDecl, TypeDecl, TypeMember, ProtocolDecl, Param,
    // Expresiones
    Expr, ExprKind, Literal, BinaryOp, UnaryOp, PostfixOp, AssignOp,
    BinaryExpr, UnaryExpr, PostfixExpr, AssignExpr,
    BlockExpr, LetExpr, IfExpr, WhileExpr, ForExpr,
    CallExpr, AccessExpr, MethodCallExpr, IndexExpr,
    NewExpr, VectorExpr,
    // Tipos y spans
    TypeName, Span,
};

use super::{
    errors::SemanticError,
    symbol_table::{Symbol, SymbolTable},
    type_system::{FuncSignature, HulkType, TypeHierarchy, TypeInfo, ProtocolInfo},
};

use std::collections::{HashMap, HashSet};

// ─────────────────────────────────────────────────────────────────────────────
//  TypeChecker
// ─────────────────────────────────────────────────────────────────────────────
pub struct TypeChecker {
    pub symbols:        SymbolTable,
    pub types:          TypeHierarchy,
    pub functions:      HashMap<String, FuncSignature>,
    pub errors:         Vec<SemanticError>,
    pub expr_types:     HashMap<u32, HulkType>,

    current_type:        Option<String>,
    current_method_name: Option<String>,
    current_ret_type:    Option<HulkType>,
    in_initializer:      bool,

    // Type inference for unannotated function parameters
    inferring_params:    HashSet<String>,
    param_constraints:   HashMap<String, HulkType>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = Self {
            symbols:          SymbolTable::new(),
            types:            TypeHierarchy::new(),
            functions:        HashMap::new(),
            errors:           Vec::new(),
            expr_types:       HashMap::new(),
            current_type:        None,
            current_method_name: None,
            current_ret_type:    None,
            in_initializer:      false,
            inferring_params:    HashSet::new(),
            param_constraints:   HashMap::new(),
        };
        tc.register_builtin_functions();
        tc
    }

    // ── Built-in functions ────────────────────────────────────────────────────

    fn register_builtin_functions(&mut self) {
        let builtins: &[(&str, &[HulkType], HulkType)] = &[
            ("print",   &[HulkType::Object],                          HulkType::Null),
            ("sqrt",    &[HulkType::Number],                          HulkType::Number),
            ("sin",     &[HulkType::Number],                          HulkType::Number),
            ("cos",     &[HulkType::Number],                          HulkType::Number),
            ("exp",     &[HulkType::Number],                          HulkType::Number),
            ("log",     &[HulkType::Number, HulkType::Number],        HulkType::Number),
            ("rand",    &[],                                           HulkType::Number),
            ("range",   &[HulkType::Number, HulkType::Number],
                         HulkType::UserDefined("Range".into())),
        ];
        for (name, params, ret) in builtins {
            self.symbols.define(
                *name,
                Symbol::function(*name, params.to_vec(), ret.clone()),
            );
        }
        // Constantes
        self.symbols.define("PI",  Symbol::variable("PI",  HulkType::Number,  false));
        self.symbols.define("E",   Symbol::variable("E",   HulkType::Number,  false));
        self.symbols.define("true",  Symbol::variable("true",  HulkType::Boolean, false));
        self.symbols.define("false", Symbol::variable("false", HulkType::Boolean, false));
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  PUNTO DE ENTRADA PÚBLICO
    // ─────────────────────────────────────────────────────────────────────────

    pub fn check_program(&mut self, program: &Program) -> Vec<SemanticError> {
        // Paso 1 — forward-declare todos los tipos, protocolos y funciones
        self.collect_all_declarations(&program.declarations);

        // Paso 2 — chequear cuerpos (infiere params desde el cuerpo)
        for decl in &program.declarations {
            self.check_decl(decl);
        }

        // Paso 3 — chequear la expresión de entrada
        // Esto también dispara refine_params_from_call para params que
        // solo se pueden inferir desde el call site (e.g. id(x) => x)
        self.check_expr(&program.entry);

        // Paso 3.5 — re-inferir retornos de funciones que aún tienen Unknown
        // (ocurre cuando el param se infirió desde el call site, no del cuerpo)
        self.reinfer_unknown_returns(&program.declarations);

        std::mem::take(&mut self.errors)
    }

    fn reinfer_unknown_returns(&mut self, decls: &[Decl]) {
        for decl in decls {
            if let Decl::Function(f) = decl {
                let ret_unknown = self.functions.get(&f.name)
                    .map_or(false, |s| matches!(s.return_type, HulkType::Unknown));
                if !ret_unknown { continue; }

                // Re-chequear el cuerpo con los params ya refinados
                self.symbols.push_scope();
                let param_types: Vec<(String, HulkType)> = self.functions.get(&f.name)
                    .map(|s| s.params.clone())
                    .unwrap_or_default();
                for (pname, ptype) in &param_types {
                    self.symbols.define(pname, Symbol::variable(pname, ptype.clone(), false));
                }

                let prev_errors_len = self.errors.len();
                let inferred_ret = self.check_expr(&f.body);
                // Descartar errores duplicados del re-check
                self.errors.truncate(prev_errors_len);

                self.symbols.pop_scope();

                if !inferred_ret.is_never() && !matches!(inferred_ret, HulkType::Unknown) {
                    if let Some(sig) = self.functions.get_mut(&f.name) {
                        sig.return_type = inferred_ret;
                    }
                }
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  PASO 1: recolección de firmas
    // ─────────────────────────────────────────────────────────────────────────

    fn collect_all_declarations(&mut self, decls: &[Decl]) {
        for decl in decls {
            match decl {
                Decl::Function(f)  => self.collect_func(f),
                Decl::Type(t)      => self.collect_type(t),
                Decl::Protocol(p)  => self.collect_protocol(p),
            }
        }
        // Segunda pasada: resolver atributos y métodos de tipos
        // (ahora que todos los nombres están registrados)
        let type_decls: Vec<_> = decls.iter()
            .filter_map(|d| if let Decl::Type(t) = d { Some(t) } else { None })
            .collect();
        for t in type_decls {
            self.collect_type_members(t);
        }
    }

    fn collect_func(&mut self, f: &FuncDecl) {
        if self.symbols.in_current_scope(&f.name) {
            self.errors.push(SemanticError::Redefinition {
                name: f.name.clone(), span: f.span,
            });
            return;
        }
        let params: Vec<HulkType> = f.params.iter()
            .map(|p| self.resolve_opt_type(&p.type_ann, p.span))
            .collect();
        let ret = self.resolve_opt_type(&f.return_type, f.span);
        self.symbols.define(&f.name, Symbol::function(&f.name, params.clone(), ret.clone()));
        let params_named: Vec<(String, HulkType)> = f.params.iter()
            .zip(params.iter())
            .map(|(p, ty)| (p.name.clone(), ty.clone()))
            .collect();
        self.functions.insert(f.name.clone(), FuncSignature {
            params:      params_named,
            return_type: ret,
        });
    }

    fn collect_type(&mut self, t: &TypeDecl) {
        // Verificar herencia de primitivos
        if let Some(parent) = &t.parent {
            let pname = parent.name();
            if matches!(pname, "Number" | "String" | "Boolean") {
                self.errors.push(SemanticError::InheritFromPrimitive {
                    type_name: t.name.clone(), span: t.span,
                });
            }
            // Verificar que el padre existe
            if self.types.types.get(pname).is_none() {
                self.errors.push(SemanticError::UndefinedType {
                    name: pname.into(), span: parent.span(),
                });
            }
        }

        // ⚠️ Primero calcular params SIN borrow mutable activo
        let constructor_params: Vec<(String, HulkType)> = t.type_args.iter()
            .map(|p| (p.name.clone(), self.resolve_opt_type(&p.type_ann, p.span)))
            .collect();

        // Luego hacer el insert
        self.types.types.entry(t.name.clone()).or_insert(TypeInfo {
            name:               t.name.clone(),
            parent:             t.parent.as_ref().map(|p| p.name().into()),
            constructor_params,
            attributes:         HashMap::new(),
            methods:            HashMap::new(),
            is_builtin:         false,
        });

        // Registrar en la tabla de símbolos (para `new T(...)`)
        if !self.symbols.in_current_scope(&t.name) {
            self.symbols.define(&t.name, Symbol::type_sym(&t.name));
        }
    }

    fn collect_type_members(&mut self, t: &TypeDecl) {
        for member in &t.members {
            match member {
                TypeMember::Attribute(attr) => {
                    let ty = self.resolve_opt_type(&attr.type_ann, attr.span);
                    if let Some(info) = self.types.types.get_mut(&t.name) {
                        info.attributes.insert(attr.name.clone(), ty);
                    }
                }
                TypeMember::Method(method) => {
                    let params: Vec<(String, HulkType)> = method.params.iter()
                        .map(|p| (p.name.clone(), self.resolve_opt_type(&p.type_ann, p.span)))
                        .collect();
                    let ret = self.resolve_opt_type(&method.return_type, method.span);
                    let sig = FuncSignature { params, return_type: ret };

                    // ── Override estricto ──────────────────────────────────────
                    // Si el padre tiene este método, la firma completa debe coincidir:
                    //   • mismo número de parámetros
                    //   • mismo tipo en cada parámetro (salvo Unknown = sin anotación)
                    //   • tipo de retorno compatible (covariante)
                    if let Some(parent_name) = self.types.types.get(&t.name)
                        .and_then(|ti| ti.parent.clone())
                    {
                        if let Some(parent_sig) = self.types.types.get(&parent_name)
                            .and_then(|ti| ti.methods.get(&method.name))
                            .cloned()
                        {
                            let mut mismatch = false;

                            // Aridad
                            if sig.params.len() != parent_sig.params.len() {
                                mismatch = true;
                            } else {
                                // Tipos de parámetros
                                for ((_, child_ty), (_, parent_ty)) in
                                    sig.params.iter().zip(parent_sig.params.iter())
                                {
                                    if !matches!(child_ty,  HulkType::Unknown)
                                    && !matches!(parent_ty, HulkType::Unknown)
                                    && child_ty != parent_ty
                                    {
                                        mismatch = true;
                                        break;
                                    }
                                }
                            }

                            // Tipo de retorno: el hijo puede ser más específico (covariante)
                            if !mismatch
                            && !matches!(sig.return_type,        HulkType::Unknown)
                            && !matches!(parent_sig.return_type, HulkType::Unknown)
                            && !self.types.conforms(&sig.return_type, &parent_sig.return_type)
                            {
                                mismatch = true;
                            }

                            if mismatch {
                                self.errors.push(SemanticError::OverrideMismatch {
                                    method: method.name.clone(), span: method.span,
                                });
                            }
                        }
                    }

                    if let Some(info) = self.types.types.get_mut(&t.name) {
                        info.methods.insert(method.name.clone(), sig);
                    }
                }
            }
        }
    }

    fn collect_protocol(&mut self, p: &ProtocolDecl) {
        // Verificar que el protocolo padre existe
        if let Some(extends) = &p.extends {
            if self.types.protocols.get(extends.name()).is_none() {
                self.errors.push(SemanticError::UndefinedType {
                    name: extends.name().into(), span: extends.span(),
                });
            }
        }

        let methods: HashMap<String, FuncSignature> = p.methods.iter()
            .map(|sig| {
                let params: Vec<(String, HulkType)> = sig.params.iter()
                    .map(|par| (par.name.clone(), self.resolve_opt_type(&par.type_ann, par.span)))
                    .collect();
                let ret = self.resolve_type_name(&sig.return_type);
                (sig.name.clone(), FuncSignature { params, return_type: ret })
            })
            .collect();

        self.types.protocols.entry(p.name.clone()).or_insert(ProtocolInfo {
            name:    p.name.clone(),
            extends: p.extends.as_ref().map(|e| e.name().into()),
            methods,
        });

        if !self.symbols.in_current_scope(&p.name) {
            self.symbols.define(&p.name, Symbol::protocol_sym(&p.name));
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  PASO 2: chequeo de cuerpos de declaraciones
    // ─────────────────────────────────────────────────────────────────────────

    fn check_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Function(f)  => self.check_func_decl(f),
            Decl::Type(t)      => self.check_type_decl(t),
            Decl::Protocol(_)  => { /* firmas ya verificadas en collect */ }
        }
    }

    fn check_func_decl(&mut self, f: &FuncDecl) {
        self.symbols.push_scope();

        for param in &f.params {
            let ty = self.resolve_opt_type(&param.type_ann, param.span);
            self.symbols.define(&param.name, Symbol::variable(&param.name, ty, false));
        }

        // Inferencia bottom-up: identificar params sin anotación para rastrearlos
        let unknown_params: HashSet<String> = f.params.iter()
            .filter(|p| p.type_ann.is_none())
            .map(|p| p.name.clone())
            .collect();
        let prev_inferring   = std::mem::replace(&mut self.inferring_params,  unknown_params);
        let prev_constraints = std::mem::replace(&mut self.param_constraints, HashMap::new());

        let expected_ret = self.resolve_opt_type(&f.return_type, f.span);
        let prev_ret = self.current_ret_type.replace(expected_ret.clone());

        let actual_ret = self.check_expr(&f.body);

        // Resolver constraints: actualizar la firma con los tipos inferidos
        if !self.inferring_params.is_empty() {
            if let Some(sig) = self.functions.get_mut(&f.name) {
                for (pname, pty) in sig.params.iter_mut() {
                    if matches!(pty, HulkType::Unknown) {
                        *pty = self.param_constraints.get(pname)
                            .cloned()
                            .unwrap_or(HulkType::Object);
                    }
                }
            }
        }

        if !matches!(expected_ret, HulkType::Unknown) {
            if !self.types.conforms(&actual_ret, &expected_ret) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: expected_ret.name(),
                    found:    actual_ret.name(),
                    span:     f.span,
                });
            }
        } else if !actual_ret.is_never() && !matches!(actual_ret, HulkType::Unknown) {
            self.symbols.update_function_return(&f.name, actual_ret.clone());
            if let Some(sig) = self.functions.get_mut(&f.name) {
                sig.return_type = actual_ret;
            }
        }

        self.param_constraints = prev_constraints;
        self.inferring_params  = prev_inferring;
        self.current_ret_type  = prev_ret;
        self.symbols.pop_scope();
    }

    fn check_type_decl(&mut self, t: &TypeDecl) {
        let prev_type = self.current_type.replace(t.name.clone());
        self.symbols.push_scope();

        // Parámetros del constructor disponibles en las inicializaciones
        for param in &t.type_args {
            let ty = self.resolve_opt_type(&param.type_ann, param.span);
            self.symbols.define(&param.name, Symbol::variable(&param.name, ty, false));
        }

        // Detectar herencia circular
        if self.types.has_circular_inheritance(&t.name) {
            self.errors.push(SemanticError::CircularInheritance {
                type_name: t.name.clone(), span: t.span,
            });
            self.symbols.pop_scope();
            self.current_type = prev_type;
            return;
        }

        for member in &t.members {
            match member {
                TypeMember::Attribute(attr) => {
                    // self NO disponible en inicializadores de atributos
                    self.in_initializer = true;
                    let val_ty = self.check_expr(&attr.value);
                    self.in_initializer = false;

                    let final_ty = if let Some(ann) = &attr.type_ann {
                        let ann_ty = self.resolve_type_name(ann);
                        if !self.types.conforms(&val_ty, &ann_ty) {
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: ann_ty.name(),
                                found:    val_ty.name(),
                                span:     attr.span,
                            });
                        }
                        ann_ty
                    } else {
                        val_ty
                    };
                    // Update the attribute type with the inferred/annotated type so
                    // that codegen can load fields with the correct LLVM type.
                    if let Some(info) = self.types.types.get_mut(&t.name) {
                        info.attributes.insert(attr.name.clone(), final_ty);
                    }
                }

                TypeMember::Method(method) => {
                    self.symbols.push_scope();
                    // Guardar el nombre del método para que base() sepa a qué método del padre llamar
                    let prev_method = self.current_method_name.replace(method.name.clone());

                    // self disponible con el tipo del tipo actual
                    self.symbols.define(
                        "self",
                        Symbol::variable(
                            "self",
                            HulkType::UserDefined(t.name.clone()),
                            false,
                        ),
                    );

                    for param in &method.params {
                        let ty = self.resolve_opt_type(&param.type_ann, param.span);
                        self.symbols.define(&param.name, Symbol::variable(&param.name, ty, false));
                    }

                    let expected_ret = self.resolve_opt_type(&method.return_type, method.span);
                    let prev_ret = self.current_ret_type.replace(expected_ret.clone());

                    let actual_ret = self.check_expr(&method.body);

                    if !matches!(expected_ret, HulkType::Unknown) {
                        if !self.types.conforms(&actual_ret, &expected_ret) {
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: expected_ret.name(),
                                found:    actual_ret.name(),
                                span:     method.span,
                            });
                        }
                    } else if !actual_ret.is_never() && !matches!(actual_ret, HulkType::Unknown) {
                        // Propagate inferred return type back so codegen uses the right LLVM type.
                        if let Some(info) = self.types.types.get_mut(&t.name) {
                            if let Some(sig) = info.methods.get_mut(&method.name) {
                                sig.return_type = actual_ret;
                            }
                        }
                    }

                    self.current_ret_type = prev_ret;
                    self.current_method_name = prev_method;
                    self.symbols.pop_scope();
                }
            }
        }

        self.symbols.pop_scope();
        self.current_type = prev_type;
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  PASO 3: chequeo de expresiones
    //  Retorna el HulkType de la expresión chequeada.
    // ─────────────────────────────────────────────────────────────────────────

    pub fn check_expr(&mut self, expr: &Expr) -> HulkType {
        let ty = match &expr.kind {
            ExprKind::Literal(lit)                        => self.check_literal(lit),
            ExprKind::Identifier { name }                 => self.check_identifier(name, expr.span),
            ExprKind::Base                                => self.check_base(expr.span),
            ExprKind::Binary(b)                           => self.check_binary(b),
            ExprKind::Unary(u)                            => self.check_unary(u),
            ExprKind::Postfix(p)                          => self.check_postfix(p),
            ExprKind::Assign(a)                           => self.check_assign(a),
            ExprKind::Is { expr: inner, type_name }       => self.check_is(inner, type_name, expr.span),
            ExprKind::As { expr: inner, type_name }       => self.check_as(inner, type_name, expr.span),
            ExprKind::Call(c)                             => self.check_call(c),
            ExprKind::Access(a)                           => self.check_access(a),
            ExprKind::MethodCall(m)                       => self.check_method_call(m),
            ExprKind::Index(i)                            => self.check_index(i),
            ExprKind::Block(b)                            => self.check_block(b),
            ExprKind::Let(l)                              => self.check_let(l),
            ExprKind::If(i)                               => self.check_if(i),
            ExprKind::While(w)                            => self.check_while(w),
            ExprKind::For(f)                              => self.check_for(f),
            ExprKind::New(n)                              => self.check_new(n),
            ExprKind::Vector(v)                           => self.check_vector(v),
        };
        // Guardar el tipo de esta expresión en el side table para el codegen
        self.expr_types.insert(expr.id, ty.clone());
        ty
    }

    // ── Átomos ────────────────────────────────────────────────────────────────

    fn check_literal(&self, lit: &Literal) -> HulkType {
        match lit {
            Literal::Number { .. } => HulkType::Number,
            Literal::String { .. } => HulkType::StringT,
            Literal::Char   { .. } => HulkType::StringT,
            Literal::Bool   { .. } => HulkType::Boolean,
            Literal::Null   { .. } => HulkType::Null,
        }
    }

    fn check_identifier(&mut self, name: &str, span: Span) -> HulkType {
        // self en inicializador → error
        if name == "self" && self.in_initializer {
            self.errors.push(SemanticError::SelfInInitializer { span });
            return HulkType::Never;
        }

        match self.symbols.lookup(name) {
            Some(sym) => match &sym.kind {
                super::symbol_table::SymbolKind::Variable { ty, .. } => ty.clone(),
                super::symbol_table::SymbolKind::Function { return_type, .. } => return_type.clone(),
                super::symbol_table::SymbolKind::Type     => HulkType::UserDefined(name.into()),
                super::symbol_table::SymbolKind::Protocol => HulkType::Protocol(name.into()),
            },
            None => {
                self.errors.push(SemanticError::UndefinedVariable {
                    name: name.into(), span,
                });
                HulkType::Never
            }
        }
    }

    fn check_base(&mut self, span: Span) -> HulkType {
        // `base` solo válido dentro de un método de un tipo que tiene padre
        match &self.current_type {
            Some(type_name) => {
                let parent = self.types.types.get(type_name.as_str())
                    .and_then(|t| t.parent.clone());
                match parent {
                    Some(p) => HulkType::UserDefined(p),
                    None    => {
                        // Tipo sin padre — base() no tiene sentido, pero no es error fatal
                        HulkType::Object
                    }
                }
            }
            None => {
                self.errors.push(SemanticError::UndefinedVariable {
                    name: "base".into(), span,
                });
                HulkType::Never
            }
        }
    }

    // ── Operaciones ───────────────────────────────────────────────────────────

    fn check_binary(&mut self, b: &BinaryExpr) -> HulkType {
        let lt = self.check_expr(&b.left);
        let rt = self.check_expr(&b.right);

        if lt.is_never() || rt.is_never() { return HulkType::Never; }

        match &b.op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul
            | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Power => {
                self.constrain_number(&b.left, &lt);
                self.constrain_number(&b.right, &rt);
                if !self.types.conforms(&lt, &HulkType::Number)
                    || !self.types.conforms(&rt, &HulkType::Number)
                {
                    self.errors.push(SemanticError::InvalidBinaryTypes {
                        op:    format!("{:?}", b.op),
                        left:  lt.name(),
                        right: rt.name(),
                        span:  b.span,
                    });
                }
                HulkType::Number
            }

            BinaryOp::And | BinaryOp::Or => {
                self.constrain_bool(&b.left, &lt);
                self.constrain_bool(&b.right, &rt);
                if !self.types.conforms(&lt, &HulkType::Boolean)
                    || !self.types.conforms(&rt, &HulkType::Boolean)
                {
                    self.errors.push(SemanticError::InvalidBinaryTypes {
                        op:    format!("{:?}", b.op),
                        left:  lt.name(),
                        right: rt.name(),
                        span:  b.span,
                    });
                }
                HulkType::Boolean
            }

            BinaryOp::Eq | BinaryOp::NotEq => {
                if !self.types.conforms(&lt, &rt) && !self.types.conforms(&rt, &lt) {
                    self.errors.push(SemanticError::InvalidBinaryTypes {
                        op:    format!("{:?}", b.op),
                        left:  lt.name(),
                        right: rt.name(),
                        span:  b.span,
                    });
                }
                HulkType::Boolean
            }

            BinaryOp::Less | BinaryOp::Greater
            | BinaryOp::LessEq | BinaryOp::GreaterEq => {
                self.constrain_number(&b.left, &lt);
                self.constrain_number(&b.right, &rt);
                if !self.types.conforms(&lt, &HulkType::Number)
                    || !self.types.conforms(&rt, &HulkType::Number)
                {
                    self.errors.push(SemanticError::InvalidBinaryTypes {
                        op:    format!("{:?}", b.op),
                        left:  lt.name(),
                        right: rt.name(),
                        span:  b.span,
                    });
                }
                HulkType::Boolean
            }

            BinaryOp::Concat | BinaryOp::DoubleConcat => HulkType::StringT,
        }
    }

    // Si la expresión es una variable que estamos infiriendo, registrar su tipo esperado
    // Cuando se llama a una función con tipos concretos, refinar params Unknown/Object.
    // Esto permite inferir `id(x) => x` como Number cuando se llama con id(42).
    fn refine_params_from_call(&mut self, fn_name: &str, params: &[HulkType], args: &[Expr]) {
        let needs_refine = params.iter().any(|t| matches!(t, HulkType::Unknown | HulkType::Object));
        if !needs_refine { return; }

        let arg_types: Vec<HulkType> = args.iter().map(|a| self.check_expr(a)).collect();

        if let Some(sig) = self.functions.get_mut(fn_name) {
            for (i, ((_pname, pty), arg_ty)) in sig.params.iter_mut().zip(arg_types.iter()).enumerate() {
                let _ = i;
                if matches!(pty, HulkType::Unknown | HulkType::Object)
                    && !matches!(arg_ty, HulkType::Unknown | HulkType::Object | HulkType::Never)
                {
                    *pty = arg_ty.clone();
                }
            }
        }
    }

    fn constrain_number(&mut self, expr: &Expr, ty: &HulkType) {
        if matches!(ty, HulkType::Unknown) {
            if let ExprKind::Identifier { name } = &expr.kind {
                if self.inferring_params.contains(name.as_str()) {
                    self.param_constraints.insert(name.clone(), HulkType::Number);
                }
            }
        }
    }

    fn constrain_bool(&mut self, expr: &Expr, ty: &HulkType) {
        if matches!(ty, HulkType::Unknown) {
            if let ExprKind::Identifier { name } = &expr.kind {
                if self.inferring_params.contains(name.as_str()) {
                    self.param_constraints.insert(name.clone(), HulkType::Boolean);
                }
            }
        }
    }

    fn check_unary(&mut self, u: &UnaryExpr) -> HulkType {
        let ty = self.check_expr(&u.operand);
        if ty.is_never() { return HulkType::Never; }

        match &u.op {
            UnaryOp::Neg => {
                self.constrain_number(&u.operand, &ty);
                if !self.types.conforms(&ty, &HulkType::Number) {
                    self.errors.push(SemanticError::InvalidOperandType {
                        op: "-".into(), found: ty.name(), span: u.span,
                    });
                }
                HulkType::Number
            }
            UnaryOp::Not => {
                self.constrain_bool(&u.operand, &ty);
                if !self.types.conforms(&ty, &HulkType::Boolean) {
                    self.errors.push(SemanticError::InvalidOperandType {
                        op: "!".into(), found: ty.name(), span: u.span,
                    });
                }
                HulkType::Boolean
            }
        }
    }

    fn check_postfix(&mut self, p: &PostfixExpr) -> HulkType {
        let ty = self.check_expr(&p.operand);
        if ty.is_never() { return HulkType::Never; }

        if ty != HulkType::Number {
            self.errors.push(SemanticError::InvalidOperandType {
                op:    format!("{:?}", p.op),
                found: ty.name(),
                span:  p.span,
            });
        }
        HulkType::Number
    }

    fn check_assign(&mut self, a: &AssignExpr) -> HulkType {
        // self nunca puede ser el target
        if let ExprKind::Identifier { name } = &a.target.kind {
            if name == "self" {
                self.errors.push(SemanticError::SelfAssignment { span: a.target.span });
                return HulkType::Never;
            }
        }

        // Verificar que el target es un lvalue válido
        let is_lvalue = matches!(
            &a.target.kind,
            ExprKind::Identifier { .. } | ExprKind::Access(_) | ExprKind::Index(_)
        );
        if !is_lvalue {
            self.errors.push(SemanticError::InvalidLValue { span: a.span });
            return HulkType::Never;
        }

        let target_ty = self.check_expr(&a.target);
        let value_ty  = self.check_expr(&a.value);

        if !target_ty.is_never() && !value_ty.is_never() {
            // Para +=, -=, *=, /=, %= el target debe ser Number
            match &a.op {
                AssignOp::PlusAssign | AssignOp::MinusAssign
                | AssignOp::MulAssign | AssignOp::DivAssign | AssignOp::ModAssign => {
                    if target_ty != HulkType::Number {
                        self.errors.push(SemanticError::InvalidOperandType {
                            op:    format!("{:?}", a.op),
                            found: target_ty.name(),
                            span:  a.span,
                        });
                    }
                    if value_ty != HulkType::Number {
                        self.errors.push(SemanticError::InvalidOperandType {
                            op:    format!("{:?}", a.op),
                            found: value_ty.name(),
                            span:  a.span,
                        });
                    }
                }
                AssignOp::Assign => {
                    if !self.types.conforms(&value_ty, &target_ty) {
                        self.emit_type_or_protocol_error(&value_ty, &target_ty, a.span);
                    }
                }
            }
        }

        target_ty
    }

    // ── is / as ───────────────────────────────────────────────────────────────

    fn check_is(&mut self, expr: &Expr, type_name: &TypeName, _span: Span) -> HulkType {
        self.check_expr(expr);
        // Verificar que el tipo existe
        let tn = type_name.name();
        if self.types.types.get(tn).is_none()
            && self.types.protocols.get(tn).is_none()
        {
            self.errors.push(SemanticError::UndefinedType {
                name: tn.into(), span: type_name.span(),
            });
        }
        // `is` siempre retorna Boolean
        HulkType::Boolean
    }

    fn check_as(&mut self, expr: &Expr, type_name: &TypeName, span: Span) -> HulkType {
        let expr_ty  = self.check_expr(expr);
        let target_ty = self.resolve_type_name(type_name);

        if !expr_ty.is_never() && !target_ty.is_never() {
            // Debe haber relación de herencia en alguna dirección (up o downcast)
            let valid = self.types.conforms(&expr_ty, &target_ty)
                     || self.types.conforms(&target_ty, &expr_ty);
            if !valid {
                self.errors.push(SemanticError::InvalidCast {
                    from: expr_ty.name(),
                    to:   target_ty.name(),
                    span,
                });
            }
        }
        target_ty
    }

    // ── Llamadas y accesos ────────────────────────────────────────────────────

    fn check_call(&mut self, c: &CallExpr) -> HulkType {
        match &c.callee.kind {
            // ── base() — llamada al método del padre de mismo nombre ─────────
            // Según la spec: "base symbol refers to the implementation of the parent"
            // base() dentro de Knight.name() llama a Person.name()
            ExprKind::Base => {
                let (Some(type_name), Some(method_name)) =
                    (self.current_type.clone(), self.current_method_name.clone())
                else {
                    self.errors.push(SemanticError::UndefinedVariable {
                        name: "base".into(), span: c.callee.span,
                    });
                    return HulkType::Never;
                };

                // Padre del tipo actual
                let parent_name = match self.types.types.get(&type_name)
                    .and_then(|t| t.parent.clone())
                {
                    Some(p) => p,
                    None => {
                        self.errors.push(SemanticError::UndefinedVariable {
                            name: "base".into(), span: c.callee.span,
                        });
                        return HulkType::Never;
                    }
                };

                // Buscar el método en el padre (o el ancestro más cercano que lo implemente)
                match self.lookup_method(&parent_name, &method_name) {
                    None => {
                        // El método no existe en el padre — intentar con el constructor del padre.
                        // Permite base(args) para inicializar campos heredados.
                        let ctor_params: Vec<HulkType> = self.types.types.get(&parent_name)
                            .map(|ti| ti.constructor_params.iter().map(|(_, t)| t.clone()).collect())
                            .unwrap_or_default();
                        self.check_call_args(&parent_name, &ctor_params, &c.args, c.span);
                        HulkType::Null
                    }
                    Some(sig) => {
                        // Verificar args contra los params del método del padre
                        let param_types: Vec<HulkType> =
                            sig.params.iter().map(|(_, t)| t.clone()).collect();
                        self.check_call_args(&method_name, &param_types, &c.args, c.span);
                        // Devolver el tipo de retorno del método (no el tipo del padre)
                        sig.return_type.clone()
                    }
                }
            }

            // ── Llamada a función / constructor por nombre ────────────────────
            // ── Llamada a función / constructor por nombre ────────────────────
            ExprKind::Identifier { name } => {
                let sym = self.symbols.lookup(name).cloned();
                match sym {
                    Some(s) => match s.kind {
                        super::symbol_table::SymbolKind::Function { ref params, ref return_type } => {
                            // Refinar params Unknown/Object con los tipos concretos del call site
                            self.refine_params_from_call(name, params, &c.args);
                            // Releer la firma tras posible refinamiento
                            let (params2, ret2) = self.functions.get(name)
                                .map(|s| (s.params.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>(), s.return_type.clone()))
                                .unwrap_or_else(|| (params.clone(), return_type.clone()));
                            self.check_call_args(name, &params2, &c.args, c.span);
                            ret2
                        }
                        super::symbol_table::SymbolKind::Type => {
                            // Llamada a constructor sin `new` — verificar aridad también
                            let ctor = self.types.types.get(name)
                                .map(|t| t.constructor_params.clone())
                                .unwrap_or_default();

                            if !ctor.is_empty() || !c.args.is_empty() {
                                let param_types: Vec<HulkType> = ctor.iter()
                                    .map(|(_, t)| t.clone())
                                    .collect();
                                self.check_call_args(name, &param_types, &c.args, c.span);
                            }
                            HulkType::UserDefined(name.clone())
                        }
                        _ => {
                            self.errors.push(SemanticError::NotCallable { span: c.span });
                            HulkType::Never
                        }
                    },
                    None => {
                        self.errors.push(SemanticError::UndefinedFunction {
                            name: name.clone(), span: c.callee.span,
                        });
                        HulkType::Never
                    }
                }
            }

            // ── Functor / valor de primera clase ─────────────────────────────
            _ => {
                let callee_ty = self.check_expr(c.callee.as_ref());
                for arg in &c.args { self.check_expr(arg); }
                if callee_ty.is_never() { HulkType::Never } else { HulkType::Object }
            }
        }
    }

    fn check_call_args(
        &mut self,
        name:     &str,
        params:   &[HulkType],
        args:     &[Expr],
        span:     Span,
    ) {
        if args.len() != params.len() {
            self.errors.push(SemanticError::WrongArgCount {
                name:     name.into(),
                expected: params.len(),
                found:    args.len(),
                span,
            });
            for arg in args { self.check_expr(arg); }
        } else {
            for (arg, expected_ty) in args.iter().zip(params.iter()) {
                let arg_ty = self.check_expr(arg);
                if !self.types.conforms(&arg_ty, expected_ty) {
                    // Error más específico cuando el parámetro es un protocolo
                    self.emit_type_or_protocol_error(&arg_ty, expected_ty, span);
                }
            }
        }
    }

    /// Emite `ProtocolNotConformed` cuando el tipo esperado es un protocolo,
    /// o `TypeMismatch` en cualquier otro caso.
    /// En ambos casos solo emite si los tipos no conforman.
    fn emit_type_or_protocol_error(&mut self, found_ty: &HulkType, expected_ty: &HulkType, span: Span) {
        match expected_ty {
            HulkType::Protocol(proto_name) => {
                // Obtener el nombre concreto del tipo para el mensaje
                let type_name = match found_ty {
                    HulkType::UserDefined(n) => n.clone(),
                    HulkType::Number         => "Number".into(),
                    HulkType::StringT        => "String".into(),
                    HulkType::Boolean        => "Boolean".into(),
                    _                        => found_ty.name(),
                };
                let missing = self.types
                    .first_protocol_violation(&type_name, proto_name)
                    .unwrap_or_else(|| "<método desconocido>".into());
                self.errors.push(SemanticError::ProtocolNotConformed {
                    type_name,
                    protocol: proto_name.clone(),
                    missing,
                    span,
                });
            }
            _ => {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: expected_ty.name(),
                    found:    found_ty.name(),
                    span,
                });
            }
        }
    }

    fn check_method_call(&mut self, m: &MethodCallExpr) -> HulkType {
        let obj_ty = self.check_expr(&m.object);
        if obj_ty.is_never() { return HulkType::Never; }

        let (type_name, is_protocol) = match &obj_ty {
            HulkType::UserDefined(n) => (n.clone(), false),
            HulkType::Protocol(p)    => (p.clone(), true),
            HulkType::Number         => ("Number".into(), false),
            HulkType::StringT        => ("String".into(), false),
            HulkType::Boolean        => ("Boolean".into(), false),
            HulkType::Object         => {
                for arg in &m.args { self.check_expr(arg); }
                return HulkType::Object;
            }
            _ => {
                self.errors.push(SemanticError::MethodNotFound {
                    type_name: obj_ty.name(),
                    method:    m.method.clone(),
                    span:      m.span,
                });
                return HulkType::Never;
            }
        };

        let sig = if is_protocol {
            self.lookup_method_in_protocol(&type_name, &m.method)
        } else {
            self.lookup_method(&type_name, &m.method)
        };

        match sig {
            Some(sig) => {
                let param_types: Vec<HulkType> = sig.params.iter().map(|(_, t)| t.clone()).collect();
                self.check_call_args(&m.method, &param_types, &m.args, m.span);
                sig.return_type.clone()
            }
            None => {
                self.errors.push(SemanticError::MethodNotFound {
                    type_name: if is_protocol { format!("protocol {}", type_name) } else { type_name },
                    method: m.method.clone(),
                    span: m.span,
                });
                HulkType::Never
            }
        }
    }

    /// Busca un método subiendo la cadena de herencia
    fn lookup_method(&self, type_name: &str, method: &str) -> Option<FuncSignature> {
        let mut current = type_name.to_string();
        loop {
            if let Some(info) = self.types.types.get(&current) {
                if let Some(sig) = info.methods.get(method) {
                    return Some(sig.clone());
                }
                match &info.parent {
                    Some(parent) => current = parent.clone(),
                    None         => return None,
                }
            } else {
                return None;
            }
        }
    }

    /// Busca un método en la jerarquía de protocolos (incluyendo `extends`).
    fn lookup_method_in_protocol(&self, proto_name: &str, method: &str) -> Option<FuncSignature> {
        let mut current = proto_name.to_string();
        loop {
            if let Some(proto) = self.types.protocols.get(&current) {
                if let Some(sig) = proto.methods.get(method) {
                    return Some(sig.clone());
                }
                match &proto.extends {
                    Some(parent) => current = parent.clone(),
                    None => return None,
                }
            } else {
                return None;
            }
        }
    }

    fn check_access(&mut self, a: &AccessExpr) -> HulkType {
        let obj_ty = self.check_expr(&a.object);
        if obj_ty.is_never() { return HulkType::Never; }

        let type_name = match &obj_ty {
            HulkType::UserDefined(n) => n.clone(),
            _ => {
                // Acceso en primitivos/Object → aceptar sin error (puede ser método built-in)
                return HulkType::Object;
            }
        };

        // Buscar atributo subiendo la jerarquía
        if let Some(ty) = self.lookup_attribute(&type_name, &a.field) {
            return ty;
        }

        self.errors.push(SemanticError::AttributeNotFound {
            type_name,
            attr: a.field.clone(),
            span: a.span,
        });
        HulkType::Never
    }

    fn lookup_attribute(&self, type_name: &str, attr: &str) -> Option<HulkType> {
        let mut current = type_name.to_string();
        loop {
            if let Some(info) = self.types.types.get(&current) {
                if let Some(ty) = info.attributes.get(attr) {
                    return Some(ty.clone());
                }
                match &info.parent {
                    Some(parent) => current = parent.clone(),
                    None         => return None,
                }
            } else {
                return None;
            }
        }
    }

    fn check_index(&mut self, i: &IndexExpr) -> HulkType {
        let coll_ty = self.check_expr(&i.collection);
        let idx_ty  = self.check_expr(&i.index);

        if idx_ty != HulkType::Number && !idx_ty.is_never() {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "Number".into(),
                found:    idx_ty.name(),
                span:     i.span,
            });
        }

        match coll_ty {
            HulkType::Vector(elem_ty) => *elem_ty,
            HulkType::Never           => HulkType::Never,
            other => {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: "Vector".into(),
                    found:    other.name(),
                    span:     i.span,
                });
                HulkType::Never
            }
        }
    }

    // ── Expresiones compuestas ────────────────────────────────────────────────

    fn check_block(&mut self, b: &BlockExpr) -> HulkType {
        self.symbols.push_scope();
        let mut last_ty = HulkType::Null;
        for expr in &b.body {
            last_ty = self.check_expr(expr);
        }
        self.symbols.pop_scope();
        last_ty // tipo del bloque = tipo de la última expresión
    }

    fn check_let(&mut self, l: &LetExpr) -> HulkType {
        self.symbols.push_scope();

        // Bindings de izquierda a derecha — cada uno ve los anteriores
        for binding in &l.bindings {
            let val_ty = self.check_expr(&binding.value);

            let final_ty = if let Some(ann) = &binding.type_ann {
                let ann_ty = self.resolve_type_name(ann);
                if !self.types.conforms(&val_ty, &ann_ty) {
                    self.emit_type_or_protocol_error(&val_ty, &ann_ty, binding.span);
                }
                ann_ty
            } else {
                // Inferencia: usar el tipo del valor
                if matches!(val_ty, HulkType::Unknown) {
                    self.errors.push(SemanticError::CannotInferType {
                        name: binding.name.clone(), span: binding.span,
                    });
                }
                val_ty
            };

            if !self.symbols.define(
                &binding.name,
                Symbol::variable(&binding.name, final_ty, true),
            ) {
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
        if cond_ty != HulkType::Boolean && !cond_ty.is_never() {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "Boolean".into(),
                found:    cond_ty.name(),
                span:     i.span,
            });
        }

        let mut result_ty = self.check_expr(&i.then_body);

        for elif in &i.elif_chain {
            let elif_cond = self.check_expr(&elif.condition);
            if elif_cond != HulkType::Boolean && !elif_cond.is_never() {
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
        if cond_ty != HulkType::Boolean && !cond_ty.is_never() {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "Boolean".into(),
                found:    cond_ty.name(),
                span:     w.span,
            });
        }
        // El tipo del while es el de la última evaluación del cuerpo
        self.check_expr(&w.body)
    }

    fn check_for(&mut self, f: &ForExpr) -> HulkType {
        let iter_ty = self.check_expr(&f.iterable);

        // Determinar el tipo del elemento
        let elem_ty = match &iter_ty {
            HulkType::Vector(t) => *t.clone(),
            HulkType::UserDefined(n) => {
                // Range → Number
                if n == "Range" { HulkType::Number }
                // Verificar que implementa Iterable
                else if self.types.conforms_protocol(n, "Iterable") {
                    // Tipo del current() del iterable
                    self.types.types.get(n)
                        .and_then(|t| t.methods.get("current"))
                        .map(|s| s.return_type.clone())
                        .unwrap_or(HulkType::Object)
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "Iterable".into(),
                        found:    iter_ty.name(),
                        span:     f.span,
                    });
                    HulkType::Object
                }
            }
            HulkType::Never => HulkType::Never,
            other => {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: "Iterable".into(),
                    found:    other.name(),
                    span:     f.span,
                });
                HulkType::Object
            }
        };

        self.symbols.push_scope();
        self.symbols.define(&f.var, Symbol::variable(&f.var, elem_ty, false));
        let body_ty = self.check_expr(&f.body);
        self.symbols.pop_scope();
        body_ty
    }

    fn check_new(&mut self, n: &NewExpr) -> HulkType {
        let type_name = n.type_name.name().to_string();

        // Verificar que el tipo existe y no es primitivo
        match self.types.types.get(&type_name) {
            None => {
                self.errors.push(SemanticError::UndefinedType {
                    name: type_name.clone(), span: n.span,
                });
                return HulkType::Never;
            }
            Some(info) if info.is_builtin && matches!(
                type_name.as_str(), "Number" | "String" | "Boolean"
            ) => {
                self.errors.push(SemanticError::InheritFromPrimitive {
                    type_name: type_name.clone(), span: n.span,
                });
                return HulkType::Never;
            }
            _ => {}
        }

        // ── Verificar aridad y tipos del constructor ──────────────────────────
        let ctor_params = self.types.types.get(&type_name)
            .map(|t| t.constructor_params.clone())
            .unwrap_or_default();

        if n.args.len() != ctor_params.len() {
            self.errors.push(SemanticError::WrongArgCount {
                name:     type_name.clone(),
                expected: ctor_params.len(),
                found:    n.args.len(),
                span:     n.span,
            });
            // Chequear los args de todos modos para no perder otros errores
            for arg in &n.args { self.check_expr(arg); }
        } else {
            for (arg, (_, expected_ty)) in n.args.iter().zip(ctor_params.iter()) {
                let arg_ty = self.check_expr(arg);
                if !self.types.conforms(&arg_ty, expected_ty) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: expected_ty.name(),
                        found:    arg_ty.name(),
                        span:     n.span,
                    });
                }
            }
        }

        HulkType::UserDefined(type_name)
    }

    fn check_vector(&mut self, v: &VectorExpr) -> HulkType {
        match v {
            VectorExpr::Explicit { elements, .. } => {
                if elements.is_empty() {
                    // Vector vacío → tipo inferido como Object[]
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
            VectorExpr::Generator { body, var, iterable, .. } => {
                let iter_ty = self.check_expr(iterable);
                let elem_ty = match &iter_ty {
                    HulkType::Vector(t)          => *t.clone(),
                    HulkType::UserDefined(n) if n == "Range" => HulkType::Number,
                    HulkType::Never              => HulkType::Never,
                    _                            => HulkType::Object,
                };

                self.symbols.push_scope();
                self.symbols.define(var, Symbol::variable(var, elem_ty, false));
                let body_ty = self.check_expr(body);
                self.symbols.pop_scope();

                HulkType::Vector(Box::new(body_ty))
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  HELPERS: resolución de tipos del AST → HulkType semántico
    // ─────────────────────────────────────────────────────────────────────────

    pub fn resolve_type_name(&self, tn: &TypeName) -> HulkType {
        match tn {
            TypeName::Simple { name, .. } => self.name_to_hulk_type(name),
            TypeName::Vector { name, .. } => {
                HulkType::Vector(Box::new(self.name_to_hulk_type(name)))
            }
            TypeName::Iterable { .. } => HulkType::Protocol("Iterable".into()),
        }
    }

    fn name_to_hulk_type(&self, name: &str) -> HulkType {
        match name {
            "Number"  => HulkType::Number,
            "String"  => HulkType::StringT,
            "Boolean" => HulkType::Boolean,
            "Object"  => HulkType::Object,
            "Null"    => HulkType::Null,
            other     => {
                if self.types.protocols.contains_key(other) {
                    HulkType::Protocol(other.into())
                } else {
                    HulkType::UserDefined(other.into())
                }
            }
        }
    }

    /// Resuelve una anotación de tipo opcional.
    /// Si no hay anotación → HulkType::Unknown (para inferencia posterior).
    pub fn resolve_opt_type(&self, ann: &Option<TypeName>, _span: Span) -> HulkType {
        match ann {
            Some(tn) => self.resolve_type_name(tn),
            None     => HulkType::Unknown,
        }
    }
}