// src/semantic/type_system.rs

use std::collections::{HashMap, HashSet};

// ─────────────────────────────────────────────────────────────────────────────
//  HulkType — tipo semántico resuelto
//  Distinto de TypeName (AST): aquí ya no hay spans ni strings crudos.
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HulkType {
    Number,
    StringT,               // "String" es keyword en Rust
    Boolean,
    Null,
    Object,
    Vector(Box<HulkType>),
    UserDefined(String),   // tipos declarados con `type`
    Protocol(String),      // protocolos
    Unknown,               // tipo aún no inferido
    Never,                 // tipo de error — no propaga chequeos
}

impl HulkType {
    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Number | Self::StringT | Self::Boolean)
    }

    pub fn is_never(&self) -> bool {
        matches!(self, Self::Never)
    }

    pub fn name(&self) -> String {
        match self {
            Self::Number           => "Number".into(),
            Self::StringT          => "String".into(),
            Self::Boolean          => "Boolean".into(),
            Self::Null             => "Null".into(),
            Self::Object           => "Object".into(),
            Self::Vector(t)        => format!("{}[]", t.name()),
            Self::UserDefined(n)   => n.clone(),
            Self::Protocol(n)      => n.clone(),
            Self::Unknown          => "<unknown>".into(),
            Self::Never            => "<never>".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Firma de función / método
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct FuncSignature {
    pub params:      Vec<(String, HulkType)>,  // (nombre, tipo)
    pub return_type: HulkType,
}

// ─────────────────────────────────────────────────────────────────────────────
//  Información de un tipo registrado
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name:               String,
    pub parent:             Option<String>,
    /// Parámetros del constructor en orden: `type T(a: Number, b: String)` → [(a,Number),(b,String)]
    pub constructor_params: Vec<(String, HulkType)>,
    pub attributes:         HashMap<String, HulkType>,
    pub methods:            HashMap<String, FuncSignature>,
    pub is_builtin:         bool,
}

// ─────────────────────────────────────────────────────────────────────────────
//  Información de un protocolo registrado
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    pub name:    String,
    pub extends: Option<String>,
    pub methods: HashMap<String, FuncSignature>,
}

// ─────────────────────────────────────────────────────────────────────────────
//  TypeHierarchy — jerarquía completa
// ─────────────────────────────────────────────────────────────────────────────
pub struct TypeHierarchy {
    pub types:     HashMap<String, TypeInfo>,
    pub protocols: HashMap<String, ProtocolInfo>,
}

impl TypeHierarchy {
    pub fn new() -> Self {
        let mut h = Self {
            types:     HashMap::new(),
            protocols: HashMap::new(),
        };
        h.register_builtins();
        h
    }

    // ── Conformance ───────────────────────────────────────────────────────────
    //
    // ¿`child` conforma con `ancestor`?
    // - Todo conforma con Object
    // - Se usa subtipado nominal para tipos
    // - Se usa chequeo estructural para protocolos
    // - Never conforma con todo (evita cascada de errores)

    pub fn conforms(&self, child: &HulkType, ancestor: &HulkType) -> bool {
        // Never suprime errores en cascada
        if child.is_never() || ancestor.is_never() { return true; }
        // Unknown: se asume ok (la inferencia lo resolverá)
        if matches!(child, HulkType::Unknown) || matches!(ancestor, HulkType::Unknown) {
            return true;
        }

        if child == ancestor { return true; }

        // Todo conforma con Object
        if matches!(ancestor, HulkType::Object) { return true; }

        // Null conforma con UserDefined (nullable)
        if matches!(child, HulkType::Null) {
            return matches!(ancestor, HulkType::UserDefined(_) | HulkType::Object);
        }

        match (child, ancestor) {
            // Subtipado nominal entre tipos de usuario
            (HulkType::UserDefined(c), HulkType::UserDefined(a)) => {
                self.is_subtype(c, a)
            }
            // Primitivos heredan de Object (ya cubierto arriba), pero
            // explícitamente suben a sus representaciones
            (HulkType::Number, HulkType::UserDefined(a)) => a == "Number",
            (HulkType::StringT, HulkType::UserDefined(a)) => a == "String",
            (HulkType::Boolean, HulkType::UserDefined(a)) => a == "Boolean",

            // Verificación estructural de protocolo
            (HulkType::UserDefined(c), HulkType::Protocol(p)) => {
                self.conforms_protocol(c, p)
            }

            // Vector[T] conforma con Vector[T] (invariante por ahora)
            (HulkType::Vector(a), HulkType::Vector(b)) => self.conforms(a, b),

            _ => false,
        }
    }

    /// Sube la cadena de herencia nominal
    pub fn is_subtype(&self, child: &str, ancestor: &str) -> bool {
        if child == ancestor { return true; }
        if let Some(info) = self.types.get(child) {
            if let Some(parent) = &info.parent {
                return self.is_subtype(parent, ancestor);
            }
        }
        // Todo tipo de usuario es subtype de Object implícitamente
        ancestor == "Object"
    }

    /// Verificación estructural de protocolo
    pub fn conforms_protocol(&self, type_name: &str, protocol: &str) -> bool {
        let proto = match self.protocols.get(protocol) {
            Some(p) => p.clone(),
            None    => return false,
        };
        let tinfo = match self.types.get(type_name) {
            Some(t) => t.clone(),
            None    => return false,
        };

        for (method_name, proto_sig) in &proto.methods {
            match tinfo.methods.get(method_name) {
                None           => return false,
                Some(type_sig) => {
                    if !self.signatures_compatible(type_sig, proto_sig) {
                        return false;
                    }
                }
            }
        }
        // Verificar protocolo padre recursivamente
        if let Some(parent_proto) = proto.extends.clone() {
            return self.conforms_protocol(type_name, &parent_proto);
        }
        true
    }

    /// Busca el método que falta para un protocolo (para mejores mensajes de error)
    pub fn missing_protocol_method(&self, type_name: &str, protocol: &str) -> Option<String> {
        let proto = self.protocols.get(protocol)?;
        let tinfo = self.types.get(type_name)?;
        for (method_name, _) in &proto.methods {
            if !tinfo.methods.contains_key(method_name) {
                return Some(method_name.clone());
            }
        }
        None
    }

    fn signatures_compatible(&self, a: &FuncSignature, b: &FuncSignature) -> bool {
        a.params.len() == b.params.len()
            && self.conforms(&a.return_type, &b.return_type)
            && a.params.iter().zip(&b.params)
                .all(|((_, ta), (_, tb))| ta == tb)
    }

    // ── LCA (Lowest Common Ancestor) ─────────────────────────────────────────
    //
    // Usado para determinar el tipo de if-elif-else:
    //   if (c) expr_A elif (c) expr_B else expr_C
    //   → LCA(LCA(A, B), C)

    pub fn lca(&self, a: &HulkType, b: &HulkType) -> HulkType {
        // Never no participa en LCA
        if a.is_never() { return b.clone(); }
        if b.is_never() { return a.clone(); }
        if a == b       { return a.clone(); }

        let ancestors_a = self.ancestors(a);

        // Subir desde b buscando el primer ancestro compartido con a
        let mut current = b.clone();
        loop {
            if ancestors_a.contains(&current) {
                return current;
            }
            match self.parent_of(&current) {
                Some(p) => current = p,
                None    => return HulkType::Object,
            }
        }
    }

    fn ancestors(&self, t: &HulkType) -> HashSet<HulkType> {
        let mut set = HashSet::new();
        let mut cur = t.clone();
        loop {
            set.insert(cur.clone());
            match self.parent_of(&cur) {
                Some(p) => cur = p,
                None    => break,
            }
        }
        set
    }

    fn parent_of(&self, t: &HulkType) -> Option<HulkType> {
        match t {
            HulkType::UserDefined(name) => {
                let parent_name = self.types.get(name)?.parent.as_ref()?;
                Some(HulkType::UserDefined(parent_name.clone()))
            }
            HulkType::Number  => Some(HulkType::Object),
            HulkType::StringT => Some(HulkType::Object),
            HulkType::Boolean => Some(HulkType::Object),
            HulkType::Null    => Some(HulkType::Object),
            _                 => None,
        }
    }

    // ── Detección de herencia circular ────────────────────────────────────────

    pub fn has_circular_inheritance(&self, type_name: &str) -> bool {
        let mut visited = HashSet::new();
        let mut current = type_name.to_string();
        loop {
            if !visited.insert(current.clone()) {
                return true; // ciclo detectado
            }
            match self.types.get(&current).and_then(|t| t.parent.clone()) {
                Some(parent) => current = parent,
                None         => return false,
            }
        }
    }

    // ── Built-ins ─────────────────────────────────────────────────────────────

    fn register_builtins(&mut self) {
        // Tipos built-in con sus padres
        let builtin_types = [
            ("Object",  None),
            ("Number",  Some("Object")),
            ("String",  Some("Object")),
            ("Boolean", Some("Object")),
            ("Range",   Some("Object")),
        ];

        for (name, parent) in builtin_types {
            self.types.insert(name.into(), TypeInfo {
                name:               name.into(),
                parent:             parent.map(|s| s.into()),
                constructor_params: vec![],   // built-ins no tienen constructor declarado
                attributes:         HashMap::new(),
                methods:            self.builtin_methods_for(name),
                is_builtin:         true,
            });
        }

        // Protocolo Iterable
        self.protocols.insert("Iterable".into(), ProtocolInfo {
            name:    "Iterable".into(),
            extends: None,
            methods: {
                let mut m = HashMap::new();
                m.insert("next".into(), FuncSignature {
                    params:      vec![],
                    return_type: HulkType::Boolean,
                });
                m.insert("current".into(), FuncSignature {
                    params:      vec![],
                    return_type: HulkType::Object,
                });
                m
            },
        });

        // Range implementa Iterable
        if let Some(range) = self.types.get_mut("Range") {
            range.methods.insert("next".into(), FuncSignature {
                params:      vec![],
                return_type: HulkType::Boolean,
            });
            range.methods.insert("current".into(), FuncSignature {
                params:      vec![],
                return_type: HulkType::Number,
            });
        }
    }

    fn builtin_methods_for(&self, type_name: &str) -> HashMap<String, FuncSignature> {
        let mut m = HashMap::new();
        match type_name {
            "Object" => {
                m.insert("toString".into(), FuncSignature {
                    params:      vec![],
                    return_type: HulkType::StringT,
                });
            }
            "Number" => {
                m.insert("toString".into(), FuncSignature {
                    params:      vec![],
                    return_type: HulkType::StringT,
                });
            }
            "String" => {
                m.insert("size".into(), FuncSignature {
                    params:      vec![],
                    return_type: HulkType::Number,
                });
                m.insert("toString".into(), FuncSignature {
                    params:      vec![],
                    return_type: HulkType::StringT,
                });
            }
            "Boolean" => {
                m.insert("toString".into(), FuncSignature {
                    params:      vec![],
                    return_type: HulkType::StringT,
                });
            }
            _ => {}
        }
        m
    }
}