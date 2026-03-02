use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::SourceLocation;

#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub module_path: String,
    pub imported_names: Vec<String>,
    pub is_glob: bool,
    pub location: Option<SourceLocation>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ImportInfo {
    pub fn new(module_path: impl Into<String>) -> Self {
        Self {
            module_path: module_path.into(),
            imported_names: Vec::new(),
            is_glob: false,
            location: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_names(mut self, names: Vec<String>) -> Self {
        self.imported_names = names;
        self
    }

    pub fn with_glob(mut self, is_glob: bool) -> Self {
        self.is_glob = is_glob;
        self
    }

    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone)]
pub struct DefinitionInfo {
    pub kind: String,
    pub name: String,
    pub location: Option<SourceLocation>,
    pub parent: Option<String>,
    pub signature: Option<String>,
    pub is_public: bool,
    pub doc: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DefinitionInfo {
    pub fn new(kind: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            location: None,
            parent: None,
            signature: None,
            is_public: true,
            doc: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    pub fn with_visibility(mut self, is_public: bool) -> Self {
        self.is_public = is_public;
        self
    }

    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Direct,
    Method,
    Associated,
    Constructor,
    Macro,
}

#[derive(Debug, Clone)]
pub struct CallInfo {
    pub caller: String,
    pub callee: String,
    pub receiver: Option<String>,
    pub call_kind: CallKind,
    pub location: Option<SourceLocation>,
}

impl CallInfo {
    pub fn new(caller: impl Into<String>, callee: impl Into<String>) -> Self {
        Self {
            caller: caller.into(),
            callee: callee.into(),
            receiver: None,
            call_kind: CallKind::Direct,
            location: None,
        }
    }

    pub fn with_receiver(mut self, receiver: impl Into<String>) -> Self {
        self.receiver = Some(receiver.into());
        self
    }

    pub fn with_kind(mut self, kind: CallKind) -> Self {
        self.call_kind = kind;
        self
    }

    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn method(
        caller: impl Into<String>,
        receiver: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        Self {
            caller: caller.into(),
            callee: method.into(),
            receiver: Some(receiver.into()),
            call_kind: CallKind::Method,
            location: None,
        }
    }

    pub fn associated(
        caller: impl Into<String>,
        type_name: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        let type_str = type_name.into();
        let method_str = method.into();
        Self {
            caller: caller.into(),
            callee: format!("{}::{}", type_str, method_str),
            receiver: Some(type_str),
            call_kind: CallKind::Associated,
            location: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRefKind {
    Parameter,
    Return,
    Field,
    Local,
    GenericArg,
    Bound,
}

#[derive(Debug, Clone)]
pub struct TypeRefInfo {
    pub source: String,
    pub type_name: String,
    pub ref_kind: TypeRefKind,
    pub location: Option<SourceLocation>,
}

impl TypeRefInfo {
    pub fn new(source: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            type_name: type_name.into(),
            ref_kind: TypeRefKind::Local,
            location: None,
        }
    }

    pub fn with_kind(mut self, kind: TypeRefKind) -> Self {
        self.ref_kind = kind;
        self
    }

    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn param(source: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            type_name: type_name.into(),
            ref_kind: TypeRefKind::Parameter,
            location: None,
        }
    }

    pub fn ret(source: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            type_name: type_name.into(),
            ref_kind: TypeRefKind::Return,
            location: None,
        }
    }

    pub fn field(source: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            type_name: type_name.into(),
            ref_kind: TypeRefKind::Field,
            location: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReferenceInfo {
    pub source: String,
    pub target: String,
    pub location: Option<SourceLocation>,
}

impl ReferenceInfo {
    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            location: None,
        }
    }

    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub file_path: PathBuf,
    pub imports: Vec<ImportInfo>,
    pub definitions: Vec<DefinitionInfo>,
    pub calls: Vec<CallInfo>,
    pub type_refs: Vec<TypeRefInfo>,
    pub references: Vec<ReferenceInfo>,
    pub ffi_bindings: Vec<crate::languages::ffi::FfiBinding>,
}

impl ParseResult {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            imports: Vec::new(),
            definitions: Vec::new(),
            calls: Vec::new(),
            type_refs: Vec::new(),
            references: Vec::new(),
            ffi_bindings: Vec::new(),
        }
    }
}
