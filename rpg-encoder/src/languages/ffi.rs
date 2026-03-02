use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::SourceLocation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FfiKind {
    Export,
    Import,
    Callback,
    TypeBinding,
}

#[derive(Debug, Clone)]
pub struct FfiBinding {
    pub source_lang: String,
    pub target_lang: String,
    pub symbol: String,
    pub kind: FfiKind,
    pub location: Option<SourceLocation>,
    pub native_signature: Option<String>,
}

impl FfiBinding {
    pub fn new(
        source_lang: impl Into<String>,
        target_lang: impl Into<String>,
        symbol: impl Into<String>,
        kind: FfiKind,
    ) -> Self {
        Self {
            source_lang: source_lang.into(),
            target_lang: target_lang.into(),
            symbol: symbol.into(),
            kind,
            location: None,
            native_signature: None,
        }
    }

    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.native_signature = Some(signature.into());
        self
    }

    pub fn to_metadata(&self) -> std::collections::HashMap<String, serde_json::Value> {
        let mut meta = std::collections::HashMap::new();
        meta.insert(
            "ffi_source".to_string(),
            serde_json::Value::String(self.source_lang.clone()),
        );
        meta.insert(
            "ffi_target".to_string(),
            serde_json::Value::String(self.target_lang.clone()),
        );
        meta.insert(
            "ffi_kind".to_string(),
            serde_json::Value::String(serde_json::to_string(&self.kind).unwrap_or_default()),
        );
        if let Some(ref sig) = self.native_signature {
            meta.insert(
                "ffi_signature".to_string(),
                serde_json::Value::String(sig.clone()),
            );
        }
        meta
    }
}

pub struct FfiDetector;

impl FfiDetector {
    pub fn detect_extern_blocks(
        source: &str,
        file: &Path,
        abi_patterns: &[&str],
    ) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            for abi in abi_patterns {
                if trimmed.starts_with(&format!("extern \"{}\"", abi))
                    || trimmed.starts_with(&format!("extern '{}'", abi))
                {
                    if let Some(block_start) = source[trimmed.len()..].find('{') {
                        let remaining = &source[trimmed.len() + block_start..];
                        if let Some(block_end) = remaining.find('}') {
                            let block_content = &remaining[1..block_end];

                            for fn_line in block_content.lines() {
                                let fn_trimmed = fn_line.trim();
                                if fn_trimmed.starts_with("fn ") {
                                    if let Some(fn_name) = Self::extract_fn_name(fn_trimmed) {
                                        let location = SourceLocation::new(
                                            file.to_path_buf(),
                                            line_idx + 1,
                                            1,
                                            line_idx + 2,
                                            1,
                                        );
                                        bindings.push(
                                            FfiBinding::new("rust", *abi, fn_name, FfiKind::Import)
                                                .with_location(location),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        bindings
    }

    pub fn detect_no_mangle(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            if line.trim().contains("#[no_mangle]") {
                for ahead_idx in 1..5.min(lines.len() - line_idx) {
                    let ahead = lines[line_idx + ahead_idx].trim();
                    if ahead.starts_with("pub fn ")
                        || ahead.starts_with("fn ")
                        || ahead.contains(" fn ")
                    {
                        if let Some(fn_name) = Self::extract_fn_name(ahead) {
                            let location = SourceLocation::new(
                                file.to_path_buf(),
                                line_idx + 1,
                                1,
                                line_idx + ahead_idx + 1,
                                ahead.len(),
                            );
                            bindings.push(
                                FfiBinding::new("rust", "c", fn_name, FfiKind::Export)
                                    .with_location(location),
                            );
                        }
                        break;
                    }
                    if ahead.starts_with("#[")
                        || ahead.starts_with("pub unsafe")
                        || ahead.starts_with("pub extern")
                    {
                        continue;
                    }
                    if !ahead.is_empty() && !ahead.starts_with("//") {
                        break;
                    }
                }
            }
        }

        bindings
    }

    pub fn detect_cgo_exports(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        let mut in_cgo_block = false;
        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed == "import \"C\"" || trimmed.contains("import \"C\"") {
                in_cgo_block = true;
            }

            if in_cgo_block && trimmed == ")" {
                in_cgo_block = false;
                continue;
            }

            if trimmed.starts_with("//export ") {
                let symbol = trimmed.trim_start_matches("//export ").trim();
                if !symbol.is_empty() {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("go", "c", symbol, FfiKind::Export).with_location(location),
                    );
                }
            }
        }

        bindings
    }

    pub fn detect_cgo_imports(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let mut in_cgo_block = false;
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed == "import \"C\"" || trimmed.contains("import \"C\"") {
                in_cgo_block = true;
            }

            if in_cgo_block && trimmed == ")" {
                in_cgo_block = false;
            }

            if in_cgo_block && (trimmed.starts_with("C.") || trimmed.contains("C.")) {
                if let Some(symbol) = Self::extract_cgo_call(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("go", "c", symbol, FfiKind::Import).with_location(location),
                    );
                }
            }
        }

        bindings
    }

    pub fn detect_python_ctypes(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        let mut cdll_vars: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut cdll_returning_funcs: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut current_func: Option<String> = None;

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("def ") {
                current_func = Self::extract_python_func_name(trimmed);
            }

            if trimmed.contains("ctypes.CDLL") || trimmed.contains("cdll.LoadLibrary") {
                if let Some(var_name) = Self::extract_ctypes_cdll(trimmed) {
                    cdll_vars.insert(var_name);
                }
                if trimmed.starts_with("return ") {
                    if let Some(ref func) = current_func {
                        cdll_returning_funcs.insert(func.clone());
                    }
                }
            }

            if let Some(var_name) = Self::extract_ctypes_func_assign(trimmed, &cdll_returning_funcs)
            {
                cdll_vars.insert(var_name);
            }

            if let Some((func, returned_var)) = Self::extract_return_var(trimmed, &current_func) {
                if cdll_vars.contains(&returned_var) {
                    cdll_returning_funcs.insert(func);
                }
            }

            for var in &cdll_vars {
                if let Some(symbol) = Self::extract_ctypes_call(trimmed, var) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("python", "c", symbol, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    pub fn detect_python_cffi(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("cffi.FFI()") || trimmed.contains("ffi.cdef(") {
                let next_lines: String = lines[line_idx..]
                    .iter()
                    .take(20)
                    .map(|l| l.trim())
                    .collect::<Vec<_>>()
                    .join(" ");

                for symbol in Self::extract_cffi_symbols(&next_lines) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("python", "c", symbol, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    pub fn detect_ruby_ffi(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("attach_function ") || trimmed.contains(".attach_function ") {
                if let Some(symbol) = Self::extract_ruby_attach_function(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("ruby", "c", symbol, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.starts_with("callback ") {
                if let Some(symbol) = Self::extract_ruby_callback(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("ruby", "c", symbol, FfiKind::Callback)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    pub fn detect_cpp_extern_c(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        let mut in_extern_c = false;
        let mut brace_depth = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("extern \"C\"") {
                in_extern_c = true;
                brace_depth = 0;
            }

            if in_extern_c {
                brace_depth += trimmed.matches('{').count() as i32;
                brace_depth -= trimmed.matches('}').count() as i32;

                if trimmed.starts_with("void ")
                    || trimmed.starts_with("int ")
                    || trimmed.starts_with("char ")
                    || trimmed.starts_with("bool ")
                    || Self::looks_like_function_decl(trimmed)
                {
                    if let Some(fn_name) = Self::extract_cpp_fn_name(trimmed) {
                        let location = SourceLocation::new(
                            file.to_path_buf(),
                            line_idx + 1,
                            1,
                            line_idx + 1,
                            trimmed.len(),
                        );
                        let kind = if trimmed.contains('=') && !trimmed.contains("==") {
                            FfiKind::TypeBinding
                        } else {
                            FfiKind::Export
                        };
                        bindings.push(
                            FfiBinding::new("cpp", "c", fn_name, kind).with_location(location),
                        );
                    }
                }

                if brace_depth <= 0 && trimmed.contains('}') {
                    in_extern_c = false;
                }
            }
        }

        bindings
    }

    fn extract_fn_name(line: &str) -> Option<String> {
        let line = line.trim_start_matches("pub ").trim();
        let line = line.trim_start_matches("unsafe ").trim();
        let line = line.trim_start_matches("async ").trim();

        let line = if let Some(after_extern) = line.strip_prefix("extern") {
            let after_extern = after_extern.trim_start();
            if let Some(stripped) = after_extern.strip_prefix('"') {
                if let Some(end_quote) = stripped.find('"') {
                    stripped[end_quote + 1..].trim_start()
                } else {
                    line
                }
            } else {
                line
            }
        } else {
            line
        };

        if !line.starts_with("fn ") {
            return None;
        }

        let after_fn = line.strip_prefix("fn ")?.trim_start();
        let end = after_fn.find('(')?;
        Some(after_fn[..end].trim().to_string())
    }

    fn extract_cgo_call(line: &str) -> Option<String> {
        let start = line.find("C.")?;
        let after = &line[start + 2..];
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        let symbol = &after[..end];
        if symbol.is_empty() {
            None
        } else {
            Some(symbol.to_string())
        }
    }

    fn extract_ctypes_cdll(line: &str) -> Option<String> {
        if !line.contains("ctypes.CDLL") && !line.contains("cdll.LoadLibrary") {
            return None;
        }

        let eq_pos = line.find('=')?;
        let var_name = line[..eq_pos].trim().to_string();
        Some(var_name)
    }

    fn extract_python_func_name(line: &str) -> Option<String> {
        let after_def = line.strip_prefix("def ")?;
        let end = after_def.find('(')?;
        Some(after_def[..end].trim().to_string())
    }

    fn extract_ctypes_func_assign(
        line: &str,
        cdll_returning_funcs: &std::collections::HashSet<String>,
    ) -> Option<String> {
        let eq_pos = line.find('=')?;
        let var_name = line[..eq_pos].trim().to_string();

        let after_eq = line[eq_pos + 1..].trim();
        let func_call = after_eq.split('(').next()?.trim();

        if cdll_returning_funcs.contains(func_call)
            || cdll_returning_funcs.contains(&format!("_{}", func_call))
        {
            return Some(var_name);
        }

        if let Some(func_name) = func_call.strip_suffix("()") {
            if cdll_returning_funcs.contains(func_name) {
                return Some(var_name);
            }
        }

        None
    }

    fn extract_return_var(line: &str, current_func: &Option<String>) -> Option<(String, String)> {
        let after_return = line.strip_prefix("return ")?;
        let returned_var = after_return.trim().trim_end_matches(';').to_string();

        if returned_var
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        {
            if let Some(func) = current_func {
                return Some((func.clone(), returned_var));
            }
        }

        None
    }

    fn extract_ctypes_call(line: &str, var: &str) -> Option<String> {
        let prefix = format!("{}.", var);
        let start = line.find(&prefix)?;
        let after = &line[start + prefix.len()..];
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        let symbol = &after[..end];
        if symbol.is_empty() {
            None
        } else {
            Some(symbol.to_string())
        }
    }

    fn extract_cffi_symbols(text: &str) -> Vec<String> {
        let mut symbols = Vec::new();

        let c_types = [
            "void",
            "int",
            "char",
            "bool",
            "float",
            "double",
            "long",
            "short",
            "unsigned",
            "signed",
            "size_t",
            "uint8_t",
            "uint16_t",
            "uint32_t",
            "uint64_t",
            "int8_t",
            "int16_t",
            "int32_t",
            "int64_t",
            "intptr_t",
            "uintptr_t",
            "ptrdiff_t",
        ];

        for line in text.lines() {
            let trimmed = line.trim();

            for c_type in &c_types {
                let patterns = [
                    format!("{} ", c_type),
                    format!("{}*", c_type),
                    format!("{} *", c_type),
                ];

                for pattern in &patterns {
                    if let Some(pos) = trimmed.find(pattern.as_str()) {
                        if pos == 0
                            || trimmed
                                .as_bytes()
                                .get(pos - 1)
                                .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_')
                        {
                            let rest = &trimmed[pos + pattern.len()..];
                            if let Some(fn_name) = Self::extract_c_fn_from_rest(rest) {
                                if !symbols.contains(&fn_name) {
                                    symbols.push(fn_name);
                                }
                            }
                        }
                    }
                }
            }
        }
        symbols
    }

    fn extract_c_fn_from_rest(text: &str) -> Option<String> {
        let text = text.trim();

        let mut paren_depth = 0;
        let mut name_end = 0;
        let mut found_name = false;

        for (i, c) in text.char_indices() {
            if c == '(' {
                paren_depth += 1;
                if !found_name {
                    name_end = i;
                    found_name = true;
                }
            } else if c == ')' {
                if paren_depth > 0 {
                    paren_depth -= 1;
                }
            } else if c == ';' && paren_depth == 0 {
                break;
            }
        }

        if found_name && name_end > 0 {
            let name_part = text[..name_end].trim();
            if let Some(last_space) = name_part.rfind(' ') {
                let name = &name_part[last_space + 1..];
                let name = name.trim_start_matches('*').trim();
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    return Some(name.to_string());
                }
            } else if !name_part.is_empty()
                && name_part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Some(name_part.to_string());
            }
        }
        None
    }

    fn extract_c_fn_name(line: &str) -> Option<String> {
        let paren_pos = line.find('(')?;
        let before_paren = &line[..paren_pos];
        let last_space = before_paren.rfind(' ')?;
        Some(before_paren[last_space + 1..].trim().to_string())
    }

    fn extract_ruby_attach_function(line: &str) -> Option<String> {
        let start = if line.starts_with("attach_function ") {
            16
        } else if line.contains(".attach_function ") {
            line.find(".attach_function ")? + 17
        } else {
            return None;
        };

        let after = &line[start..];
        let after = after.trim_start_matches(':').trim();

        if after.starts_with('"') || after.starts_with("'") {
            let quote = after.chars().next()?;
            let end = after[1..].find(quote)?;
            return Some(after[1..end + 1].to_string());
        }

        if let Some(after) = after.strip_prefix(':') {
            let end = after
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(after.len());
            return Some(after[..end].to_string());
        }

        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        if end > 0 {
            Some(after[..end].to_string())
        } else {
            None
        }
    }

    fn extract_ruby_callback(line: &str) -> Option<String> {
        let start = if line.starts_with("callback ") {
            9
        } else {
            return None;
        };

        let after = &line[start..];
        let after = after.trim_start_matches(':').trim();

        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        if end > 0 {
            Some(after[..end].to_string())
        } else {
            None
        }
    }

    fn looks_like_function_decl(line: &str) -> bool {
        line.contains('(')
            && line.contains(')')
            && (line.contains(';') || line.contains('{'))
            && !line.starts_with("//")
            && !line.starts_with("#")
    }

    fn extract_cpp_fn_name(line: &str) -> Option<String> {
        let paren_pos = line.find('(')?;
        let before_paren = &line[..paren_pos];

        for part in before_paren.split_whitespace().rev() {
            if part.contains('*') || part.contains('&') {
                if let Some(name) = part.trim_matches('*').trim_matches('&').split("::").last() {
                    return Some(name.to_string());
                }
            } else if !part.is_empty()
                && part != "static"
                && part != "inline"
                && part != "virtual"
                && part != "const"
                && part != "extern"
            {
                return Some(part.split("::").last().unwrap_or(part).to_string());
            }
        }

        None
    }

    pub fn detect_java_jni(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        let mut loaded_libs: Vec<String> = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("System.loadLibrary(") || trimmed.contains("System.load(") {
                if let Some(lib_name) = Self::extract_java_lib_name(trimmed) {
                    loaded_libs.push(lib_name.clone());
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("java", "native", lib_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if Self::is_native_method(trimmed) {
                if let Some(method_name) = Self::extract_java_method_name(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("java", "jni", method_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("Linker.nativeLinker()")
                || trimmed.contains("downcallHandle")
                || trimmed.contains("SymbolLookup")
            {
                if let Some(symbol) = Self::extract_ffm_symbol(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("java", "ffm", symbol, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    fn extract_java_lib_name(line: &str) -> Option<String> {
        let start = line.find('(')?;
        let end = line.rfind(')')?;
        let content = &line[start + 1..end].trim();
        let content = content.trim_start_matches('"').trim_end_matches('"');
        Some(content.to_string())
    }

    fn is_native_method(line: &str) -> bool {
        line.contains(" native ")
            && line.contains('(')
            && (line.contains(";") || line.ends_with(")"))
    }

    fn extract_java_method_name(line: &str) -> Option<String> {
        let paren_pos = line.find('(')?;
        let before_paren = &line[..paren_pos];
        let words: Vec<&str> = before_paren.split_whitespace().collect();
        words.last().map(|s| s.to_string())
    }

    fn extract_ffm_symbol(line: &str) -> Option<String> {
        if line.contains("findOrThrow(") || line.contains("find(") {
            let start = line.find('"')?;
            let end = line[start + 1..].find('"')?;
            return Some(line[start + 1..start + 1 + end].to_string());
        }
        None
    }

    pub fn detect_node_native(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("require(") && trimmed.contains(".node") {
                if let Some(lib_name) = Self::extract_node_require(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("javascript", "node", lib_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.starts_with("WebAssembly.instantiate")
                || trimmed.starts_with("WebAssembly.Instance")
                || trimmed.starts_with("instantiateStreaming")
            {
                if let Some(wasm_file) = Self::extract_wasm_import(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("javascript", "wasm", wasm_file, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("ffi.Library(") || trimmed.contains("ffi-napi") {
                if let Some(lib_name) = Self::extract_ffi_library(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("javascript", "ffi", lib_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    fn extract_node_require(line: &str) -> Option<String> {
        let start = line.find("require(")?;
        let rest = &line[start + 8..];
        let start_quote = rest.find('"').or_else(|| rest.find("'"))?;
        let rest = &rest[start_quote + 1..];
        let end_quote = rest.find('"').or_else(|| rest.find("'"))?;
        Some(rest[..end_quote].to_string())
    }

    fn extract_wasm_import(line: &str) -> Option<String> {
        let start = line.find('"')?;
        let rest = &line[start + 1..];
        let end = rest.find('"')?;
        let path = &rest[..end];
        if path.ends_with(".wasm") {
            Some(path.to_string())
        } else {
            None
        }
    }

    fn extract_ffi_library(line: &str) -> Option<String> {
        let start = line.find("ffi.Library(")?;
        let rest = &line[start + 12..];
        let start_quote = rest.find('"').or_else(|| rest.find("'"))?;
        let rest = &rest[start_quote + 1..];
        let end_quote = rest.find('"').or_else(|| rest.find("'"))?;
        Some(rest[..end_quote].to_string())
    }

    pub fn detect_swift_ffi(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("import ") {
                let module = trimmed.strip_prefix("import ").unwrap_or("").trim();
                if module.starts_with('C') && module.len() > 1 {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("swift", "c", module, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("@objc") && !trimmed.contains("@objc(") {
                if let Some(func_name) = Self::extract_swift_func_name(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("swift", "objc", func_name, FfiKind::Export)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("@_cdecl(") || trimmed.contains("@_silgen_name(") {
                if let Some(symbol) = Self::extract_swift_ffi_attr(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("swift", "c", symbol, FfiKind::Export)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    fn extract_swift_func_name(line: &str) -> Option<String> {
        if line.contains("func ") {
            let start = line.find("func ")?;
            let rest = &line[start + 5..];
            let end = rest
                .find(|c: char| c == '(' || c.is_whitespace())
                .unwrap_or(rest.len());
            Some(rest[..end].trim().to_string())
        } else {
            None
        }
    }

    fn extract_swift_ffi_attr(line: &str) -> Option<String> {
        let start = line.find('"')?;
        let rest = &line[start + 1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    pub fn detect_luajit_ffi(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut in_cdef = false;

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("ffi.cdef[[") || trimmed.contains("ffi.cdef[[") {
                in_cdef = true;
            }

            if in_cdef && trimmed.contains("]]") {
                in_cdef = false;
                continue;
            }

            if in_cdef {
                if let Some(fn_name) = Self::extract_c_fn_name(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("lua", "c", fn_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("ffi.C.") {
                if let Some(symbol) = Self::extract_ffi_c_call(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("lua", "c", symbol, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("ffi.new(") {
                if let Some(type_name) = Self::extract_ffi_type(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("lua", "c", type_name, FfiKind::TypeBinding)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    fn extract_ffi_c_call(line: &str) -> Option<String> {
        let start = line.find("ffi.C.")?;
        let rest = &line[start + 6..];
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        if end > 0 {
            Some(rest[..end].to_string())
        } else {
            None
        }
    }

    fn extract_ffi_type(line: &str) -> Option<String> {
        let start = line.find("ffi.new(")?;
        let rest = &line[start + 8..];
        let start_quote = rest.find('"').or_else(|| rest.find("'"))?;
        let rest = &rest[start_quote + 1..];
        let end_quote = rest.find('"').or_else(|| rest.find("'"))?;
        Some(rest[..end_quote].to_string())
    }

    pub fn detect_haskell_ffi(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("foreign import ") {
                if let Some(symbol) = Self::extract_haskell_foreign_symbol(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("haskell", "c", symbol, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.starts_with("foreign export ") {
                if let Some(symbol) = Self::extract_haskell_export_symbol(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("haskell", "c", symbol, FfiKind::Export)
                            .with_location(location),
                    );
                }
            }

            if trimmed.starts_with("{-# LANGUAGE") && trimmed.contains("ForeignFunctionInterface") {
                let location = SourceLocation::new(
                    file.to_path_buf(),
                    line_idx + 1,
                    1,
                    line_idx + 1,
                    trimmed.len(),
                );
                bindings.push(
                    FfiBinding::new("haskell", "c", "FFI_ENABLED", FfiKind::TypeBinding)
                        .with_location(location),
                );
            }
        }

        bindings
    }

    fn extract_haskell_foreign_symbol(line: &str) -> Option<String> {
        let start = line.find('"')?;
        let rest = &line[start + 1..];
        let end = rest.find('"')?;

        let full = &rest[..end];
        if let Some(space_pos) = full.rfind(' ') {
            Some(full[space_pos + 1..].to_string())
        } else {
            Some(full.to_string())
        }
    }

    fn extract_haskell_export_symbol(line: &str) -> Option<String> {
        let after_export = line.strip_prefix("foreign export ")?;
        let after_export = after_export.trim();

        let parts: Vec<&str> = after_export.split_whitespace().collect();
        if parts.len() >= 2 {
            Some(parts[1].to_string())
        } else if !parts.is_empty() {
            Some(parts[0].to_string())
        } else {
            None
        }
    }

    pub fn detect_wat(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("(import ") {
                if let Some(symbol) = Self::extract_wat_import(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("wasm", "host", symbol, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.starts_with("(export ") {
                if let Some(symbol) = Self::extract_wat_export(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("wasm", "host", symbol, FfiKind::Export)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    fn extract_wat_import(line: &str) -> Option<String> {
        let start = line.find("(import \"")?;
        let rest = &line[start + 9..];
        let end = rest.find('"')?;
        let module = &rest[..end];

        let rest = &rest[end + 2..];
        let start = rest.find('"')?;
        let rest = &rest[start + 1..];
        let end = rest.find('"')?;
        let name = &rest[..end];

        Some(format!("{}.{}", module, name))
    }

    fn extract_wat_export(line: &str) -> Option<String> {
        let start = line.find("(export \"")?;
        let rest = &line[start + 9..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    pub fn detect_rust_wasm_bindgen(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("#[wasm_bindgen]") {
                for ahead_idx in 1..5.min(lines.len() - line_idx) {
                    let ahead = lines[line_idx + ahead_idx].trim();
                    if ahead.starts_with("pub fn ")
                        || ahead.starts_with("fn ")
                        || ahead.contains(" fn ")
                    {
                        if let Some(fn_name) = Self::extract_fn_name(ahead) {
                            let location = SourceLocation::new(
                                file.to_path_buf(),
                                line_idx + 1,
                                1,
                                line_idx + ahead_idx + 1,
                                ahead.len(),
                            );
                            bindings.push(
                                FfiBinding::new("rust", "wasm", fn_name, FfiKind::Export)
                                    .with_location(location),
                            );
                        }
                        break;
                    }
                    if ahead.starts_with("#[") || ahead.starts_with("pub extern") {
                        continue;
                    }
                    if !ahead.is_empty() && !ahead.starts_with("//") {
                        break;
                    }
                }
            }

            if trimmed.contains("#[link(wasm_import_module") {
                if let Some(module) = Self::extract_wasm_import_module(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("rust", "wasm", module, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }
        }

        bindings
    }

    fn extract_wasm_import_module(line: &str) -> Option<String> {
        let start = line.find('"')?;
        let rest = &line[start + 1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    pub fn detect_csharp_pinvoke(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        let mut in_dll_import = false;
        let mut current_dll: Option<String> = None;

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("[DllImport(") || trimmed.contains("[DllImportAttribute(") {
                if let Some(dll_name) = Self::extract_dllimport_name(trimmed) {
                    current_dll = Some(dll_name.clone());
                    in_dll_import = true;
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("csharp", "native", dll_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if in_dll_import {
                if trimmed.contains("static extern") || trimmed.contains("extern") {
                    if let Some(fn_name) = Self::extract_csharp_extern_fn(trimmed) {
                        let location = SourceLocation::new(
                            file.to_path_buf(),
                            line_idx + 1,
                            1,
                            line_idx + 1,
                            trimmed.len(),
                        );
                        let target = current_dll.clone().unwrap_or_else(|| "native".to_string());
                        bindings.push(
                            FfiBinding::new("csharp", &target, fn_name, FfiKind::Import)
                                .with_location(location),
                        );
                    }
                }
                if !trimmed.contains("extern")
                    && !trimmed.contains("DllImport")
                    && !trimmed.is_empty()
                    && !trimmed.starts_with("[")
                    && !trimmed.starts_with("//")
                {
                    in_dll_import = false;
                    current_dll = None;
                }
            }

            if trimmed.contains("[UnmanagedCallersOnly")
                || trimmed.contains("[UnmanagedCallersOnly(")
            {
                let entry_point = Self::extract_unmanaged_entrypoint(trimmed).or_else(|| {
                    for ahead_idx in 1..3.min(lines.len() - line_idx) {
                        let ahead = lines[line_idx + ahead_idx].trim();
                        if ahead.contains("static") && ahead.contains("(") {
                            return Self::extract_csharp_static_fn_name(ahead);
                        }
                    }
                    None
                });

                if let Some(symbol) = entry_point {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("csharp", "native", symbol, FfiKind::Export)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("[ComImport") {
                if let Some(iface_name) = Self::extract_com_interface_name(&lines, line_idx) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("csharp", "com", iface_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("LoadLibrary") || trimmed.contains("GetProcAddress") {
                let location = SourceLocation::new(
                    file.to_path_buf(),
                    line_idx + 1,
                    1,
                    line_idx + 1,
                    trimmed.len(),
                );
                bindings.push(
                    FfiBinding::new("csharp", "native", "dynamic_load", FfiKind::Import)
                        .with_location(location),
                );
            }
        }

        bindings
    }

    fn extract_dllimport_name(line: &str) -> Option<String> {
        let start = line.find('"')?;
        let rest = &line[start + 1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    fn extract_csharp_extern_fn(line: &str) -> Option<String> {
        let paren_pos = line.find('(')?;
        let before_paren = &line[..paren_pos];
        let words: Vec<&str> = before_paren.split_whitespace().collect();
        words.last().map(|s| s.to_string())
    }

    fn extract_unmanaged_entrypoint(line: &str) -> Option<String> {
        if line.contains("EntryPoint") {
            let start = line.find('"')?;
            let rest = &line[start + 1..];
            let end = rest.find('"')?;
            return Some(rest[..end].to_string());
        }
        None
    }

    fn extract_csharp_static_fn_name(line: &str) -> Option<String> {
        let paren_pos = line.find('(')?;
        let before_paren = &line[..paren_pos];
        for part in before_paren.split_whitespace().rev() {
            if part != "static"
                && part != "public"
                && part != "private"
                && part != "partial"
                && part != "unsafe"
            {
                return Some(part.to_string());
            }
        }
        None
    }

    fn extract_com_interface_name(lines: &[&str], start_idx: usize) -> Option<String> {
        for ahead_idx in 1..5.min(lines.len() - start_idx) {
            let ahead = lines[start_idx + ahead_idx].trim();
            if ahead.starts_with("interface ") || ahead.contains(" interface ") {
                let parts: Vec<&str> = ahead.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "interface" && i + 1 < parts.len() {
                        return Some(parts[i + 1].trim_end_matches('{').to_string());
                    }
                }
            }
        }
        None
    }

    pub fn detect_scala_ffi(source: &str, file: &Path) -> Vec<FfiBinding> {
        let mut bindings = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("Native.load[") || trimmed.contains("Native.load(") {
                if let Some(lib_name) = Self::extract_jna_library(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("scala", "jna", lib_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.starts_with("def ") && trimmed.contains("@native") {
                if let Some(fn_name) = Self::extract_scala_def_name(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("scala", "jni", fn_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("System.loadLibrary(") || trimmed.contains("System.load(") {
                if let Some(lib_name) = Self::extract_java_lib_name(trimmed) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("scala", "jni", lib_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("@extern") || trimmed.contains("@native.extern") {
                if let Some(obj_name) = Self::extract_extern_object_name(&lines, line_idx) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("scala", "native", obj_name, FfiKind::Import)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("@exported") || trimmed.contains("@exported(") {
                if let Some(fn_name) = Self::extract_exported_fn_name(&lines, line_idx) {
                    let location = SourceLocation::new(
                        file.to_path_buf(),
                        line_idx + 1,
                        1,
                        line_idx + 1,
                        trimmed.len(),
                    );
                    bindings.push(
                        FfiBinding::new("scala", "native", fn_name, FfiKind::Export)
                            .with_location(location),
                    );
                }
            }

            if trimmed.contains("unsafe.")
                || trimmed.contains("fromCString")
                || trimmed.contains("toCString")
            {
                let location = SourceLocation::new(
                    file.to_path_buf(),
                    line_idx + 1,
                    1,
                    line_idx + 1,
                    trimmed.len(),
                );
                bindings.push(
                    FfiBinding::new("scala", "native", "unsafe", FfiKind::Import)
                        .with_location(location),
                );
            }
        }

        bindings
    }

    fn extract_jna_library(line: &str) -> Option<String> {
        let start = line.find('"')?;
        let rest = &line[start + 1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    fn extract_scala_def_name(line: &str) -> Option<String> {
        let start = line.find("def ")?;
        let rest = &line[start + 4..].trim_start();
        let end = rest
            .find(|c: char| c == '(' || c == ':' || c.is_whitespace())
            .unwrap_or(rest.len());
        if end > 0 {
            Some(rest[..end].to_string())
        } else {
            None
        }
    }

    fn extract_extern_object_name(lines: &[&str], start_idx: usize) -> Option<String> {
        for ahead_idx in 1..5.min(lines.len() - start_idx) {
            let ahead = lines[start_idx + ahead_idx].trim();
            if ahead.starts_with("object ") || ahead.contains(" object ") {
                let parts: Vec<&str> = ahead.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "object" && i + 1 < parts.len() {
                        return Some(parts[i + 1].trim_end_matches('{').to_string());
                    }
                }
            }
        }
        None
    }

    fn extract_exported_fn_name(lines: &[&str], start_idx: usize) -> Option<String> {
        for ahead_idx in 1..3.min(lines.len() - start_idx) {
            let ahead = lines[start_idx + ahead_idx].trim();
            if ahead.starts_with("def ") {
                return Self::extract_scala_def_name(ahead);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_no_mangle() {
        let source = r#"
#[no_mangle]
pub extern "C" fn my_exported_fn(x: i32) -> i32 {
    x + 1
}
"#;
        let bindings = FfiDetector::detect_no_mangle(source, Path::new("test.rs"));
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].symbol, "my_exported_fn");
        assert_eq!(bindings[0].kind, FfiKind::Export);
    }

    #[test]
    fn test_detect_cgo_exports() {
        let source = r#"
import "C"

//export myGoFunction
func myGoFunction(x C.int) C.int {
    return x + 1
}
"#;
        let bindings = FfiDetector::detect_cgo_exports(source, Path::new("test.go"));
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].symbol, "myGoFunction");
        assert_eq!(bindings[0].kind, FfiKind::Export);
    }

    #[test]
    fn test_detect_ruby_ffi() {
        let source = r#"
require 'ffi'

module MyModule
  extend FFI::Library
  ffi_lib 'mylib'
  attach_function :my_c_function, [:int], :int
end
"#;
        let bindings = FfiDetector::detect_ruby_ffi(source, Path::new("test.rb"));
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].symbol, "my_c_function");
        assert_eq!(bindings[0].kind, FfiKind::Import);
    }

    #[test]
    fn test_detect_csharp_pinvoke() {
        let source = r#"
using System.Runtime.InteropServices;

class NativeMethods {
    [DllImport("kernel32.dll")]
    static extern bool CloseHandle(IntPtr handle);

    [UnmanagedCallersOnly(EntryPoint = "my_export")]
    public static int MyExport(int x) { return x + 1; }
}
"#;
        let bindings = FfiDetector::detect_csharp_pinvoke(source, Path::new("test.cs"));
        assert!(bindings.len() >= 2);
        assert!(bindings
            .iter()
            .any(|b| b.symbol == "kernel32.dll" && b.kind == FfiKind::Import));
    }

    #[test]
    fn test_detect_scala_ffi() {
        let source = r#"
import com.sun.jna.Native

trait MyLib {
  def myFunction(x: Int): Int
}
object MyLib {
  val instance = Native.load[MyLib]("mylib")
}

@extern object CStdLib {
  def puts(s: CString): Int = extern
}
"#;
        let bindings = FfiDetector::detect_scala_ffi(source, Path::new("test.scala"));
        assert!(bindings.len() >= 1);
    }

    #[test]
    fn test_detect_extern_blocks() {
        let source = r#"
extern "C" {
    fn external_function(x: i32) -> i32;
    fn another_external(ptr: *const u8) -> i32;
}
"#;
        let bindings = FfiDetector::detect_extern_blocks(source, Path::new("test.rs"), &["C"]);
        assert!(bindings.len() >= 2);
        assert!(bindings.iter().any(|b| b.symbol == "external_function"));
    }

    #[test]
    fn test_detect_rust_wasm_bindgen() {
        let source = r#"
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn my_exported_function(x: i32) -> i32 {
    x * 2
}
"#;
        let bindings = FfiDetector::detect_rust_wasm_bindgen(source, Path::new("test.rs"));
        assert!(!bindings.is_empty());
        assert!(bindings.iter().any(|b| b.symbol == "my_exported_function"));
    }

    #[test]
    fn test_detect_cgo_imports() {
        let source = r#"
package main

import "C"

func main() {
    C.puts(C.CString("hello"))
    C.free(unsafe.Pointer(nil))
}
"#;
        let bindings = FfiDetector::detect_cgo_imports(source, Path::new("test.go"));
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_detect_python_ctypes() {
        let source = r#"
import ctypes

mylib = ctypes.CDLL("libmylib.so")
result = mylib.add_numbers(1, 2)
"#;
        let bindings = FfiDetector::detect_python_ctypes(source, Path::new("test.py"));
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_detect_python_cffi() {
        let source = r#"from cffi import FFI
ffi = FFI()
ffi.cdef("""
int add_numbers(int a, int b);
""")"#;
        let bindings = FfiDetector::detect_python_cffi(source, Path::new("test.py"));
        assert!(
            bindings.iter().any(|b| b.symbol == "add_numbers"),
            "Should detect add_numbers, got: {:?}",
            bindings
        );
    }

    #[test]
    fn test_detect_cpp_extern_c() {
        let source = r#"
extern "C" {
    int exported_function(int x);
    void another_export(void* ptr);
}
"#;
        let bindings = FfiDetector::detect_cpp_extern_c(source, Path::new("test.cpp"));
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_detect_java_jni() {
        let source = r#"
public class Native {
    static {
        System.loadLibrary("mylib");
    }
    
    public native int addNumbers(int a, int b);
}
"#;
        let bindings = FfiDetector::detect_java_jni(source, Path::new("Test.java"));
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_detect_node_native() {
        let source = r#"
const native = require('./build/Release/native.node');
const wasm = new WebAssembly.Instance(buffer);
"#;
        let bindings = FfiDetector::detect_node_native(source, Path::new("test.js"));
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_detect_swift_ffi() {
        let source = r#"
@_cdecl("exported_function")
func exportedFunction(_ x: Int32) -> Int32 {
    return x * 2
}
"#;
        let bindings = FfiDetector::detect_swift_ffi(source, Path::new("test.swift"));
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_detect_luajit_ffi() {
        let source = r#"
local ffi = require("ffi")
ffi.cdef[[
    int add_numbers(int a, int b);
]]
ffi.C.puts("hello")
"#;
        let bindings = FfiDetector::detect_luajit_ffi(source, Path::new("test.lua"));
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_detect_haskell_ffi() {
        let source = r#"
foreign import ccall "add_numbers"
    c_add_numbers :: CInt -> CInt -> CInt

foreign export ccall
    myExport :: CInt -> CInt
"#;
        let bindings = FfiDetector::detect_haskell_ffi(source, Path::new("Test.hs"));
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_detect_wat() {
        let source = r#"
(module
  (import "env" "log" (func $log (param i32)))
  (export "add" (func $add))
)
"#;
        let bindings = FfiDetector::detect_wat(source, Path::new("test.wat"));
        assert!(bindings.iter().any(|b| b.kind == FfiKind::Import));
        assert!(bindings.iter().any(|b| b.kind == FfiKind::Export));
    }
}

#[cfg(test)]
mod test_repo_tests {
    use super::*;

    #[test]
    fn test_repo_python_ctypes() {
        let source = include_str!("../../examples/test-repo/python/rpg_math.py");
        let bindings = FfiDetector::detect_python_ctypes(source, Path::new("rpg_math.py"));
        println!(
            "ctypes bindings: {:?}",
            bindings.iter().map(|b| &b.symbol).collect::<Vec<_>>()
        );
        assert!(!bindings.is_empty(), "Should detect ctypes bindings");
    }

    #[test]
    fn test_repo_python_cffi() {
        let source = include_str!("../../examples/test-repo/python/rpg_math.py");
        let bindings = FfiDetector::detect_python_cffi(source, Path::new("rpg_math.py"));
        println!(
            "cffi bindings: {:?}",
            bindings.iter().map(|b| &b.symbol).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_repo_rust_exports() {
        let source = include_str!("../../examples/test-repo/rust/math.rs");
        let bindings = FfiDetector::detect_no_mangle(source, Path::new("math.rs"));
        println!(
            "rust exports: {:?}",
            bindings.iter().map(|b| &b.symbol).collect::<Vec<_>>()
        );
    }
}
