pub mod base;
pub mod docs;
pub mod helpers;
pub mod r#trait;
pub mod types;

pub use base::{collect_types, collect_types_with_scoped, CachedParser, TreeSitterParser};
pub use docs::extract_documentation;
pub use helpers::TsNodeExt;
pub use r#trait::{LanguageParser, ParserRegistry};
pub use types::{
    CallInfo, CallKind, DefinitionInfo, ImportInfo, ParseResult, ReferenceInfo, TypeRefInfo,
    TypeRefKind,
};
