#![allow(dead_code)]

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub struct FixtureConfig {
    pub file_count: usize,
    pub functions_per_file: usize,
    pub structs_per_file: usize,
    pub imports_per_file: usize,
    pub call_depth: usize,
    pub nesting_depth: usize,
}

impl Default for FixtureConfig {
    fn default() -> Self {
        Self {
            file_count: 10,
            functions_per_file: 5,
            structs_per_file: 2,
            imports_per_file: 3,
            call_depth: 2,
            nesting_depth: 2,
        }
    }
}

impl FixtureConfig {
    pub fn small() -> Self {
        Self {
            file_count: 10,
            functions_per_file: 3,
            structs_per_file: 1,
            imports_per_file: 2,
            call_depth: 1,
            nesting_depth: 1,
        }
    }

    pub fn medium() -> Self {
        Self {
            file_count: 50,
            functions_per_file: 8,
            structs_per_file: 3,
            imports_per_file: 5,
            call_depth: 3,
            nesting_depth: 2,
        }
    }

    pub fn large() -> Self {
        Self {
            file_count: 200,
            functions_per_file: 15,
            structs_per_file: 5,
            imports_per_file: 8,
            call_depth: 5,
            nesting_depth: 3,
        }
    }
}

pub struct RustCodeGenerator {
    config: FixtureConfig,
    function_names: Vec<String>,
    struct_names: Vec<String>,
}

impl RustCodeGenerator {
    pub fn new(config: FixtureConfig) -> Self {
        Self {
            config,
            function_names: Vec::new(),
            struct_names: Vec::new(),
        }
    }

    pub fn generate_file(&mut self, file_index: usize) -> String {
        let mut code = String::new();

        code.push_str("// GENERATED FIXTURE - DO NOT EDIT\n");
        code.push_str("#![allow(dead_code)]\n");
        code.push_str("#![allow(unused_variables)]\n\n");

        for i in 0..self.config.imports_per_file {
            code.push_str(&format!("use std::collections::HashMap as Map{};\n", i));
            code.push_str(&format!("use std::vec::Vec as Vector{};\n", i));
        }
        code.push('\n');

        for i in 0..self.config.structs_per_file {
            let struct_name = format!("Struct{}_{}", file_index, i);
            self.struct_names.push(struct_name.clone());
            code.push_str(&self.generate_struct(&struct_name, file_index));
            code.push('\n');
        }

        for i in 0..self.config.functions_per_file {
            let fn_name = format!("func{}_{}", file_index, i);
            self.function_names.push(fn_name.clone());
            code.push_str(&self.generate_function(&fn_name, file_index, 0));
            code.push('\n');
        }

        code
    }

    fn generate_struct(&self, name: &str, _file_index: usize) -> String {
        let mut code = format!("/// Documentation for {}\n", name);
        code.push_str("#[derive(Debug, Clone)]\n");
        code.push_str(&format!("pub struct {} {{\n", name));

        for i in 0..4 {
            code.push_str(&format!("    pub field_{}: i32,\n", i));
        }

        code.push_str("    pub data: Vec<String>,\n");
        code.push_str("    pub mapping: std::collections::HashMap<String, i32>,\n");
        code.push_str("}\n\n");

        code.push_str(&format!("impl {} {{\n", name));
        code.push_str(&format!("    pub fn new() -> Self {{\n"));
        code.push_str(&format!("        Self {{\n"));
        for i in 0..4 {
            code.push_str(&format!("            field_{}: 0,\n", i));
        }
        code.push_str("            data: Vec::new(),\n");
        code.push_str("            mapping: std::collections::HashMap::new(),\n");
        code.push_str("        }\n");
        code.push_str("    }\n\n");

        code.push_str("    pub fn process(&mut self, input: &str) -> Result<i32, String> {\n");
        code.push_str("        self.data.push(input.to_string());\n");
        code.push_str("        let value = input.len() as i32;\n");
        code.push_str("        self.mapping.insert(input.to_string(), value);\n");
        code.push_str("        Ok(value * 2)\n");
        code.push_str("    }\n");

        code.push_str("}\n");

        code
    }

    fn generate_function(&self, name: &str, file_index: usize, depth: usize) -> String {
        let indent = "    ".repeat(depth);
        let mut code = String::new();

        if depth == 0 {
            code.push_str(&format!("/// Documentation for {}\n", name));
            code.push_str("#[allow(clippy::all)]\n");
        }

        if depth == 0 {
            code.push_str(&format!("pub fn {}(", name));
        } else {
            code.push_str(&format!("{}fn nested_{}_{}(", indent, name, depth));
        }

        code.push_str("input: &str, count: usize) -> Result<Vec<String>, String> {\n");

        code.push_str(&format!(
            "{}    let mut results = Vec::with_capacity(count);\n",
            indent
        ));
        code.push_str(&format!("{}    let mut accumulator = 0i32;\n\n", indent));

        code.push_str(&format!("{}    for i in 0..count {{\n", indent));
        code.push_str(&format!(
            "{}        let item = format!(\"{{}}_{{}}\", input, i);\n",
            indent
        ));
        code.push_str(&format!("{}        results.push(item.clone());\n", indent));
        code.push_str(&format!("{}        accumulator += i as i32;\n", indent));

        if depth < self.config.call_depth && !self.function_names.is_empty() {
            let callee_idx = (file_index + depth) % self.function_names.len().max(1);
            if let Some(callee) = self.function_names.get(callee_idx) {
                code.push_str(&format!("{}        if i % 3 == 0 {{\n", indent));
                code.push_str(&format!(
                    "{}            let _ = {}(\"inner\", 1);\n",
                    indent, callee
                ));
                code.push_str(&format!("{}        }}\n", indent));
            }
        }

        code.push_str(&format!("{}    }}\n\n", indent));

        if depth < self.config.nesting_depth {
            code.push_str(&self.generate_nested_closure(&indent, depth + 1));
        }

        code.push_str(&format!("{}    if accumulator > 100 {{\n", indent));
        code.push_str(&format!("{}        Ok(results)\n", indent));
        code.push_str(&format!("{}    }} else {{\n", indent));
        code.push_str(&format!(
            "{}        Err(\"accumulator too low\".to_string())\n",
            indent
        ));
        code.push_str(&format!("{}    }}\n", indent));
        code.push_str(&format!("{}}}\n\n", indent));

        code
    }

    fn generate_nested_closure(&self, base_indent: &str, _depth: usize) -> String {
        let indent = format!("{}    ", base_indent);
        let mut code = String::new();

        code.push_str(&format!(
            "{}let process_nested = |x: i32| -> i32 {{\n",
            base_indent
        ));
        code.push_str(&format!("{}    let mut sum = x;\n", indent));
        for i in 0..3 {
            code.push_str(&format!("{}    sum += {};\n", indent, i * 10));
        }
        code.push_str(&format!("{}    sum * 2\n", indent));
        code.push_str(&format!("{}}};\n\n", base_indent));

        code.push_str(&format!(
            "{}let _ = process_nested(accumulator);\n\n",
            base_indent
        ));

        code
    }
}

pub struct FixtureSet {
    pub base_path: PathBuf,
    pub files: Vec<PathBuf>,
}

impl FixtureSet {
    pub fn generate(name: &str, config: FixtureConfig) -> std::io::Result<Self> {
        let temp_dir = std::env::temp_dir().join("rpg_benchmarks").join(name);

        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
        }
        fs::create_dir_all(&temp_dir)?;

        let mut generator = RustCodeGenerator::new(config);
        let mut files = Vec::new();

        for i in 0..config.file_count {
            let file_name = format!("module_{:04}.rs", i);
            let file_path = temp_dir.join(&file_name);

            let code = generator.generate_file(i);

            let mut file = File::create(&file_path)?;
            file.write_all(code.as_bytes())?;

            files.push(file_path);
        }

        let lib_path = temp_dir.join("lib.rs");
        let mut lib_content = String::new();
        for i in 0..config.file_count {
            lib_content.push_str(&format!("pub mod module_{:04};\n", i));
        }
        let mut lib_file = File::create(&lib_path)?;
        lib_file.write_all(lib_content.as_bytes())?;
        files.insert(0, lib_path);

        Ok(Self {
            base_path: temp_dir,
            files,
        })
    }

    pub fn cleanup(&self) -> std::io::Result<()> {
        if self.base_path.exists() {
            fs::remove_dir_all(&self.base_path)
        } else {
            Ok(())
        }
    }
}

impl Drop for FixtureSet {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub fn empty_rust_code() -> &'static str {
    ""
}

pub fn typical_function() -> &'static str {
    r#"
/// Processes input data and returns transformed results.
/// 
/// # Arguments
/// * `input` - The input string to process
/// * `count` - Number of iterations
/// 
/// # Returns
/// A vector of processed strings
pub fn process_data(input: &str, count: usize) -> Result<Vec<String>, String> {
    let mut results = Vec::with_capacity(count);
    
    for i in 0..count {
        let item = format!("{}_{}", input, i);
        results.push(item);
    }
    
    if !results.is_empty() {
        Ok(results)
    } else {
        Err("no results generated".to_string())
    }
}
"#
}

pub fn complex_struct() -> &'static str {
    r#"
/// Configuration for the data processor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessorConfig {
    pub name: String,
    pub batch_size: usize,
    pub timeout_ms: u64,
    pub retry_count: u32,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            batch_size: 100,
            timeout_ms: 5000,
            retry_count: 3,
            enabled: true,
            tags: vec!["default".to_string()],
            metadata: std::collections::HashMap::new(),
        }
    }
}

impl ProcessorConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("name cannot be empty".to_string());
        }
        if self.batch_size == 0 {
            return Err("batch_size must be greater than 0".to_string());
        }
        Ok(())
    }
}
"#
}

pub fn deeply_nested() -> &'static str {
    r#"
pub fn deeply_nested_function(data: &mut [i32]) -> Result<i64, String> {
    let mut outer_sum = 0i64;
    
    'outer: for (idx, value) in data.iter_mut().enumerate() {
        let mut inner_sum = 0i64;
        
        'inner: for j in 0..10 {
            match j % 3 {
                0 => {
                    if let Some(v) = data.get(idx + j) {
                        inner_sum += *v as i64;
                    }
                }
                1 => {
                    let closure = |x: i32| -> i32 {
                        let nested = || x * 2;
                        nested()
                    };
                    inner_sum += closure(*value);
                }
                _ => {
                    if idx > 0 {
                        if let Some(prev) = data.get(idx - 1) {
                            inner_sum += *prev as i64;
                        }
                    }
                }
            }
        }
        
        *value = inner_sum as i32;
        outer_sum += inner_sum;
    }
    
    Ok(outer_sum)
}
"#
}

pub fn many_imports() -> &'static str {
    r#"
use std::collections::{HashMap, HashSet, BTreeMap, BTreeSet, VecDeque, LinkedList};
use std::sync::{Arc, Mutex, RwLock, Condvar, Barrier};
use std::thread::{self, JoinHandle, spawn};
use std::io::{self, Read, Write, BufReader, BufWriter};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::marker::{PhantomData, PhantomPinned};
use std::ops::{Deref, DerefMut, Range, RangeInclusive};
use std::fmt::{self, Debug, Display, Formatter};
use std::error::Error;
use std::result::Result;
use std::option::Option;
use std::convert::{From, Into, TryFrom, TryInto};
use std::str::{self, FromStr};
use std::string::String;
use std::vec::Vec;
use std::boxed::Box;
"#
}

pub fn generate_sample_graph_nodes(count: usize) -> Vec<(String, String)> {
    (0..count)
        .map(|i| {
            let category = match i % 5 {
                0 => "function",
                1 => "struct",
                2 => "enum",
                3 => "trait",
                _ => "module",
            };
            (format!("node_{}", i), category.to_string())
        })
        .collect()
}

pub fn generate_sample_embeddings(count: usize, dims: usize) -> Vec<Vec<f32>> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    (0..count)
        .map(|_| (0..dims).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect())
        .collect()
}
