//! Type registry for cross-file type consistency.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::TypeDefinition;

pub struct TypeRegistry {
    types: HashMap<String, TypeDefinition>,
    strict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeConflict {
    pub type_name: String,
    pub existing_file: PathBuf,
    pub new_file: PathBuf,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            strict: true,
        }
    }

    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub fn register(&mut self, def: TypeDefinition) -> std::result::Result<(), TypeConflict> {
        if let Some(existing) = self.types.get(&def.name) {
            if existing.source_file != def.source_file {
                let conflict = TypeConflict {
                    type_name: def.name.clone(),
                    existing_file: existing.source_file.clone(),
                    new_file: def.source_file.clone(),
                };

                if self.strict {
                    return Err(conflict);
                }
            }
        }

        self.types.insert(def.name.clone(), def);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&TypeDefinition> {
        self.types.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    pub fn all_types(&self) -> impl Iterator<Item = &TypeDefinition> {
        self.types.values()
    }

    pub fn types_by_file(&self, file: &PathBuf) -> Vec<&TypeDefinition> {
        self.types
            .values()
            .filter(|t| &t.source_file == file)
            .collect()
    }

    pub fn into_hashmap(self) -> HashMap<String, TypeDefinition> {
        self.types
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl From<TypeRegistry> for HashMap<String, TypeDefinition> {
    fn from(registry: TypeRegistry) -> Self {
        registry.into_hashmap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_type() {
        let mut registry = TypeRegistry::new();

        let def = TypeDefinition {
            name: "User".to_string(),
            source_file: PathBuf::from("src/models/user.rs"),
            kind: "struct".to_string(),
            fields: vec![],
        };

        assert!(registry.register(def).is_ok());
        assert!(registry.contains("User"));
    }

    #[test]
    fn test_conflict_detection() {
        let mut registry = TypeRegistry::new();

        let def1 = TypeDefinition {
            name: "User".to_string(),
            source_file: PathBuf::from("src/models/user.rs"),
            kind: "struct".to_string(),
            fields: vec![],
        };

        let def2 = TypeDefinition {
            name: "User".to_string(),
            source_file: PathBuf::from("src/api/user.rs"),
            kind: "struct".to_string(),
            fields: vec![],
        };

        registry.register(def1).unwrap();
        let result = registry.register(def2);

        assert!(result.is_err());
        let conflict = result.unwrap_err();
        assert_eq!(conflict.type_name, "User");
    }

    #[test]
    fn test_non_strict_mode() {
        let mut registry = TypeRegistry::new().with_strict(false);

        let def1 = TypeDefinition {
            name: "User".to_string(),
            source_file: PathBuf::from("src/models/user.rs"),
            kind: "struct".to_string(),
            fields: vec![],
        };

        let def2 = TypeDefinition {
            name: "User".to_string(),
            source_file: PathBuf::from("src/api/user.rs"),
            kind: "struct".to_string(),
            fields: vec![],
        };

        assert!(registry.register(def1).is_ok());
        assert!(registry.register(def2).is_ok());
    }
}
