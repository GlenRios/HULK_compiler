use std::collections::{HashMap, HashSet};

/// Tipo semántico resuelto (distinto de TypeName del AST)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HulkType {
    Number,
    StringT,
    Boolean,
    Null,
    Object,
    Vector(Box<HulkType>),
    UserDefined(String),     // tipos declarados con `type`
    Protocol(String),        // protocolos
    Unknown,                 // antes de inferir
    Never,                   // para errores de tipo — no propaga
}

impl HulkType {
    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Number | Self::StringT | Self::Boolean)
    }

    pub fn name(&self) -> String {
        match self {
            Self::Number          => "Number".into(),
            Self::StringT         => "String".into(),
            Self::Boolean         => "Boolean".into(),
            Self::Null            => "Null".into(),
            Self::Object          => "Object".into(),
            Self::Vector(t)       => format!("{}[]", t.name()),
            Self::UserDefined(n)  => n.clone(),
            Self::Protocol(n)     => n.clone(),
            Self::Unknown         => "<unknown>".into(),
            Self::Never           => "<never>".into(),
        }
    }
}

/// Información de un tipo registrado
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name:       String,
    pub parent:     Option<String>,          // herencia simple
    pub attributes: HashMap<String, HulkType>,
    pub methods:    HashMap<String, FuncSignature>,
    pub protocols:  Vec<String>,             // protocolos que conforma
    pub is_builtin: bool,
}

#[derive(Debug, Clone)]
pub struct FuncSignature {
    pub params:      Vec<(String, HulkType)>,
    pub return_type: HulkType,
}

/// La jerarquía completa de tipos
pub struct TypeHierarchy {
    pub types:     HashMap<String, TypeInfo>,
    pub protocols: HashMap<String, ProtocolInfo>,
}

#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    pub name:    String,
    pub extends: Option<String>,
    pub methods: HashMap<String, FuncSignature>,
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

    /// Conformance: ¿`child` conforma con `ancestor`?
    /// Sube por la cadena de herencia.
    pub fn conforms(&self, child: &HulkType, ancestor: &HulkType) -> bool {
        if child == ancestor { return true; }
        if ancestor == &HulkType::Object { return true; } // todo conforma con Object

        match (child, ancestor) {
            (HulkType::UserDefined(c), HulkType::UserDefined(a)) => {
                self.is_subtype(c, a)
            }
            (HulkType::UserDefined(c), HulkType::Protocol(p)) => {
                self.conforms_protocol(c, p)
            }
            _ => false,
        }
    }

    /// Sube la cadena de herencia para verificar subtipado nominal
    fn is_subtype(&self, child: &str, ancestor: &str) -> bool {
        if child == ancestor { return true; }
        if let Some(info) = self.types.get(child) {
            if let Some(parent) = &info.parent {
                return self.is_subtype(parent, ancestor);
            }
        }
        false
    }

    /// Verificación estructural de protocolo
    fn conforms_protocol(&self, type_name: &str, protocol: &str) -> bool {
        let Some(proto) = self.protocols.get(protocol) else { return false; };
        let Some(tinfo) = self.types.get(type_name)   else { return false; };

        // Todos los métodos del protocolo deben existir en el tipo con firma compatible
        for (method_name, proto_sig) in &proto.methods {
            match tinfo.methods.get(method_name) {
                None => return false,
                Some(type_sig) => {
                    if !self.signatures_compatible(type_sig, proto_sig) {
                        return false;
                    }
                }
            }
        }
        // Revisar protocolo padre también
        if let Some(parent_proto) = &proto.extends {
            return self.conforms_protocol(type_name, parent_proto);
        }
        true
    }

    fn signatures_compatible(&self, a: &FuncSignature, b: &FuncSignature) -> bool {
        a.params.len() == b.params.len()
            && a.return_type == b.return_type
            && a.params.iter().zip(&b.params)
                .all(|((_, ta), (_, tb))| ta == tb)
    }

    /// LCA: Lowest Common Ancestor — para if-elif-else
    pub fn lca(&self, a: &HulkType, b: &HulkType) -> HulkType {
        if a == b { return a.clone(); }

        // Construir ancestros de `a`
        let ancestors_a = self.ancestors(a);

        // Subir `b` hasta encontrar uno en ancestors_a
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
                self.types.get(name)?
                    .parent.as_ref()
                    .map(|p| HulkType::UserDefined(p.clone()))
            }
            HulkType::Number | HulkType::StringT | HulkType::Boolean => {
                Some(HulkType::Object)
            }
            _ => None,
        }
    }

    fn register_builtins(&mut self) {
        // Object, Number, String, Boolean con sus métodos built-in
        // (toString, etc.)
        for name in ["Object", "Number", "String", "Boolean"] {
            self.types.insert(name.into(), TypeInfo {
                name:       name.into(),
                parent:     if name == "Object" { None } else { Some("Object".into()) },
                attributes: HashMap::new(),
                methods:    HashMap::new(),
                is_builtin: true,
                protocols:  vec![],
            });
        }
        // Protocolo Iterable
        self.protocols.insert("Iterable".into(), ProtocolInfo {
            name:    "Iterable".into(),
            extends: None,
            methods: HashMap::new(), // next(), current(), etc.
        });
    }
}