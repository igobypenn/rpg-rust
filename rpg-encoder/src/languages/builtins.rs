macro_rules! define_builtins {
    ($($lang:ident => [$($type:literal),* $(,)?])*) => {
        $(
            pub mod $lang {
                #[allow(unreachable_patterns)]
                pub fn is_builtin(name: &str) -> bool {
                    matches!(name, $($type)|*)
                }

                pub const BUILTINS: &[&str] = &[$($type),*];
            }
        )*
    };
}

pub fn is_common_method_call(name: &str) -> bool {
    matches!(
        name,
        "text"
            | "clone"
            | "as_str"
            | "as_ref"
            | "to_string"
            | "into"
            | "unwrap"
            | "unwrap_or"
            | "unwrap_or_else"
            | "unwrap_or_default"
            | "expect"
            | "ok"
            | "err"
            | "is_ok"
            | "is_err"
            | "is_some"
            | "is_none"
            | "map"
            | "filter"
            | "collect"
            | "iter"
            | "into_iter"
            | "flat_map"
            | "fold"
            | "reduce"
            | "for_each"
            | "enumerate"
            | "zip"
            | "chain"
            | "take"
            | "skip"
            | "nth"
            | "last"
            | "count"
            | "sum"
            | "product"
            | "min"
            | "max"
            | "sort"
            | "sort_by"
            | "reverse"
            | "dedup"
            | "contains"
            | "find"
            | "position"
            | "any"
            | "all"
            | "partition"
            | "is_empty"
            | "len"
            | "capacity"
            | "clear"
            | "push"
            | "pop"
            | "insert"
            | "remove"
            | "retain"
            | "extend"
            | "append"
            | "drain"
            | "get"
            | "get_mut"
            | "keys"
            | "values"
            | "entries"
            | "iter_mut"
            | "with_capacity"
            | "from"
            | "as_slice"
            | "as_mut_slice"
            | "format"
            | "println"
            | "print"
            | "write"
            | "flush"
            | "to_lowercase"
            | "to_uppercase"
            | "trim"
            | "trim_start"
            | "trim_end"
            | "split"
            | "lines"
            | "chars"
            | "bytes"
            | "replace"
            | "starts_with"
            | "ends_with"
            | "default"
            | "new"
            | "from_str"
            | "drop"
            | "deref"
            | "clone_from"
            | "eq"
            | "ne"
            | "cmp"
            | "spawn"
            | "join"
            | "lock"
            | "send"
            | "recv"
            | "debug"
            | "dbg"
            | "fmt"
            | "display"
            | "walk"
            | "visit"
            | "traverse"
            | "build"
            | "finish"
            | "create"
            | "make"
            | "init"
            | "setup"
            | "add_node"
            | "add_edge"
            | "node"
            | "edge"
            | "parent"
            | "child"
            | "children"
            | "root"
            | "tn"
            | "cb"
            | "ctx"
            | "buf"
            | "ptr"
            | "gg"
            | "gguf"
            | "ggml"
    )
}

define_builtins! {
    rust => [
        "i8", "i16", "i32", "i64", "i128", "isize",
        "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char", "str",
        "String", "Vec", "Option", "Result",
        "Box", "Rc", "Arc", "Cow", "Cell", "RefCell",
        "HashMap", "HashSet", "BTreeMap", "BTreeSet",
        "VecDeque", "LinkedList", "BinaryHeap",
        "Copy", "Clone", "Debug", "Default", "Drop",
        "Send", "Sync", "Sized", "Unpin",
        "Fn", "FnMut", "FnOnce",
        "Iterator", "IntoIterator", "FromIterator",
        "Deref", "DerefMut", "AsRef", "AsMut",
        "From", "Into", "TryFrom", "TryInto",
        "Error", "Display", "Formatter",
        "Some", "None", "Ok", "Err", "Self",
    ]

    python => [
        "int", "float", "str", "bool", "list", "dict", "set", "tuple",
        "bytes", "bytearray", "memoryview",
        "None", "True", "False", "Ellipsis",
        "type", "object", "classmethod", "staticmethod", "property",
        "Exception", "BaseException", "ValueError", "TypeError",
        "callable", "any", "all", "len", "range", "enumerate",
        "zip", "map", "filter", "sorted", "reversed",
        "print", "input", "open", "dir", "help", "id", "hash",
        "isinstance", "issubclass", "super", "self", "cls",
    ]

    go => [
        "bool", "byte", "rune", "string",
        "int", "int8", "int16", "int32", "int64",
        "uint", "uint8", "uint16", "uint32", "uint64", "uintptr",
        "float32", "float64", "complex64", "complex128",
        "error", "any", "comparable",
        "true", "false", "nil", "iota",
    ]

    java => [
        "byte", "short", "int", "long", "float", "double",
        "char", "boolean", "void",
        "String", "Integer", "Long", "Double", "Float",
        "Boolean", "Character", "Byte", "Short",
        "Object", "Class", "System", "String[]",
        "int[]", "byte[]", "char[]", "long[]", "double[]", "float[]",
        "true", "false", "null", "this", "super",
    ]

    typescript => [
        "string", "number", "boolean", "void", "null", "undefined",
        "any", "unknown", "never", "object", "symbol", "bigint",
        "String", "Number", "Boolean", "Object", "Array",
        "Map", "Set", "WeakMap", "WeakSet", "Promise", "Date",
        "RegExp", "Error", "Function", "Symbol", "BigInt",
        "Partial", "Required", "Readonly", "Record",
        "Pick", "Omit", "Exclude", "Extract",
        "NonNullable", "Parameters", "ConstructorParameters",
        "ReturnType", "InstanceType", "ThisParameterType",
        "OmitThisParameter", "ThisType", "Uppercase", "Lowercase",
        "Capitalize", "Uncapitalize",
        "true", "false", "null", "undefined", "this", "super",
    ]

    c => [
        "void", "bool", "_Bool", "char", "short", "int", "long",
        "float", "double", "signed", "unsigned",
        "size_t", "ptrdiff_t", "intptr_t", "uintptr_t",
        "int8_t", "int16_t", "int32_t", "int64_t",
        "uint8_t", "uint16_t", "uint32_t", "uint64_t",
        "FILE", "va_list", "NULL",
    ]

    cpp => [
        "void", "bool", "char", "short", "int", "long", "float", "double",
        "signed", "unsigned", "size_t", "ptrdiff_t",
        "int8_t", "int16_t", "int32_t", "int64_t",
        "uint8_t", "uint16_t", "uint32_t", "uint64_t",
        "intptr_t", "uintptr_t",
        "auto", "nullptr_t",
        "std", "string", "vector", "map", "set", "list",
        "deque", "queue", "stack", "array", "pair", "tuple",
        "unique_ptr", "shared_ptr", "weak_ptr", "optional", "variant", "any",
        "function", "mutex", "thread", "atomic",
        "nullptr", "this", "true", "false",
    ]

    csharp => [
        "bool", "byte", "sbyte", "char", "decimal", "double", "float",
        "int", "uint", "long", "ulong", "short", "ushort",
        "string", "object", "void", "var", "dynamic",
        "nint", "nuint", "IntPtr", "UIntPtr",
        "String", "Object", "Boolean", "Int32", "Int64",
        "Double", "Single", "Byte", "Char", "Decimal",
        "Task", "Func", "Action", "IEnumerable", "List",
        "Dictionary", "HashSet", "Array", "Exception", "Type", "Guid",
        "DateTime", "TimeSpan", "Nullable", "CancellationToken",
        "true", "false", "null", "this", "base",
    ]

    scala => [
        "Int", "Long", "Short", "Byte", "Float", "Double",
        "Boolean", "Char", "Unit", "String",
        "Any", "AnyVal", "AnyRef", "Null", "Nothing",
        "Option", "Some", "None", "List", "Seq", "Map", "Set",
        "Vector", "Array", "Either", "Left", "Right",
        "Try", "Success", "Failure", "Future", "ExecutionContext",
        "IO", "Task", "scala", "java",
        "true", "false", "null", "this", "super",
    ]

    ruby => [
        "nil", "true", "false", "self",
        "Integer", "Float", "String", "Symbol", "Array",
        "Hash", "Range", "Regexp", "Proc", "Lambda",
        "Class", "Module", "Object", "BasicObject",
        "Exception", "StandardError", "RuntimeError",
        "puts", "print", "p", "pp", "require", "require_relative", "include", "extend",
    ]

    swift => [
        "Int", "Int8", "Int16", "Int32", "Int64",
        "UInt", "UInt8", "UInt16", "UInt32", "UInt64",
        "Float", "Double", "Bool", "Character", "String",
        "Void", "Any", "AnyObject", "Type",
        "Array", "Dictionary", "Set", "Optional",
        "Error", "Result", "some", "any",
        "true", "false", "nil", "self", "Self", "super",
    ]

    lua => [
        "nil", "true", "false",
        "number", "string", "boolean", "table", "function", "thread", "userdata",
        "print", "type", "tostring", "tonumber", "pairs", "ipairs",
        "next", "select", "unpack", "pack", "rawget", "rawset",
        "require", "pcall", "xpcall", "error", "assert",
        "math", "string", "table", "os", "io", "debug",
        "_G", "_VERSION", "self",
    ]

    haskell => [
        "Int", "Integer", "Float", "Double", "Bool", "Char", "String",
        "IO", "Maybe", "Either", "Ordering",
        "Show", "Read", "Eq", "Ord", "Enum", "Bounded",
        "Functor", "Applicative", "Monad", "Monoid", "Semigroup",
        "Foldable", "Traversable", "Num", "Fractional", "Floating",
        "Real", "Integral", "RealFrac", "RealFloat",
        "True", "False", "Nothing", "Just", "Left", "Right",
    ]
}

pub fn is_builtin_for(lang: &str, name: &str) -> bool {
    match lang {
        "rust" => rust::is_builtin(name),
        "python" => python::is_builtin(name),
        "go" => go::is_builtin(name),
        "java" => java::is_builtin(name),
        "typescript" | "javascript" => typescript::is_builtin(name),
        "c" => c::is_builtin(name),
        "cpp" | "c++" => cpp::is_builtin(name),
        "csharp" | "c#" => csharp::is_builtin(name),
        "scala" => scala::is_builtin(name),
        "ruby" => ruby::is_builtin(name),
        "swift" => swift::is_builtin(name),
        "lua" => lua::is_builtin(name),
        "haskell" => haskell::is_builtin(name),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_builtins() {
        assert!(rust::is_builtin("String"));
        assert!(rust::is_builtin("Vec"));
        assert!(rust::is_builtin("Option"));
        assert!(rust::is_builtin("Result"));
        assert!(rust::is_builtin("i32"));
        assert!(rust::is_builtin("HashMap"));
        assert!(!rust::is_builtin("MyCustomType"));
    }

    #[test]
    fn test_python_builtins() {
        assert!(python::is_builtin("str"));
        assert!(python::is_builtin("int"));
        assert!(python::is_builtin("list"));
        assert!(python::is_builtin("dict"));
        assert!(python::is_builtin("None"));
        assert!(!python::is_builtin("MyClass"));
    }

    #[test]
    fn test_go_builtins() {
        assert!(go::is_builtin("int"));
        assert!(go::is_builtin("string"));
        assert!(go::is_builtin("error"));
        assert!(go::is_builtin("nil"));
        assert!(!go::is_builtin("MyStruct"));
    }

    #[test]
    fn test_java_builtins() {
        assert!(java::is_builtin("String"));
        assert!(java::is_builtin("int"));
        assert!(java::is_builtin("void"));
        assert!(java::is_builtin("null"));
        assert!(!java::is_builtin("MyClass"));
    }

    #[test]
    fn test_typescript_builtins() {
        assert!(typescript::is_builtin("string"));
        assert!(typescript::is_builtin("number"));
        assert!(typescript::is_builtin("Promise"));
        assert!(typescript::is_builtin("Record"));
        assert!(!typescript::is_builtin("MyInterface"));
    }

    #[test]
    fn test_c_builtins() {
        assert!(c::is_builtin("int"));
        assert!(c::is_builtin("void"));
        assert!(c::is_builtin("size_t"));
        assert!(c::is_builtin("NULL"));
        assert!(!c::is_builtin("my_struct"));
    }

    #[test]
    fn test_cpp_builtins() {
        assert!(cpp::is_builtin("auto"));
        assert!(cpp::is_builtin("nullptr"));
        assert!(cpp::is_builtin("std"));
        assert!(cpp::is_builtin("vector"));
        assert!(!cpp::is_builtin("MyClass"));
    }

    #[test]
    fn test_csharp_builtins() {
        assert!(csharp::is_builtin("string"));
        assert!(csharp::is_builtin("int"));
        assert!(csharp::is_builtin("Task"));
        assert!(csharp::is_builtin("null"));
        assert!(!csharp::is_builtin("MyClass"));
    }

    #[test]
    fn test_scala_builtins() {
        assert!(scala::is_builtin("Int"));
        assert!(scala::is_builtin("String"));
        assert!(scala::is_builtin("Option"));
        assert!(scala::is_builtin("List"));
        assert!(!scala::is_builtin("MyClass"));
    }

    #[test]
    fn test_ruby_builtins() {
        assert!(ruby::is_builtin("nil"));
        assert!(ruby::is_builtin("String"));
        assert!(ruby::is_builtin("Array"));
        assert!(ruby::is_builtin("puts"));
        assert!(!ruby::is_builtin("MyClass"));
    }

    #[test]
    fn test_swift_builtins() {
        assert!(swift::is_builtin("Int"));
        assert!(swift::is_builtin("String"));
        assert!(swift::is_builtin("nil"));
        assert!(swift::is_builtin("Array"));
        assert!(!swift::is_builtin("MyClass"));
    }

    #[test]
    fn test_lua_builtins() {
        assert!(lua::is_builtin("nil"));
        assert!(lua::is_builtin("print"));
        assert!(lua::is_builtin("table"));
        assert!(lua::is_builtin("require"));
        assert!(!lua::is_builtin("myFunc"));
    }

    #[test]
    fn test_haskell_builtins() {
        assert!(haskell::is_builtin("Int"));
        assert!(haskell::is_builtin("String"));
        assert!(haskell::is_builtin("Maybe"));
        assert!(haskell::is_builtin("IO"));
        assert!(!haskell::is_builtin("MyType"));
    }

    #[test]
    fn test_is_builtin_for() {
        assert!(is_builtin_for("rust", "String"));
        assert!(is_builtin_for("python", "int"));
        assert!(is_builtin_for("go", "error"));
        assert!(is_builtin_for("java", "String"));
        assert!(is_builtin_for("typescript", "string"));
        assert!(is_builtin_for("javascript", "string"));
        assert!(is_builtin_for("c", "int"));
        assert!(is_builtin_for("cpp", "auto"));
        assert!(is_builtin_for("c++", "auto"));
        assert!(is_builtin_for("csharp", "string"));
        assert!(is_builtin_for("c#", "string"));
        assert!(is_builtin_for("scala", "Int"));
        assert!(is_builtin_for("ruby", "nil"));
        assert!(is_builtin_for("swift", "Int"));
        assert!(is_builtin_for("lua", "nil"));
        assert!(is_builtin_for("haskell", "Int"));
        assert!(!is_builtin_for("unknown", "anything"));
        assert!(!is_builtin_for("rust", "MyType"));
    }

    #[test]
    fn test_builtins_list() {
        assert!(!rust::BUILTINS.is_empty());
        assert!(!python::BUILTINS.is_empty());
        assert!(rust::BUILTINS.contains(&"String"));
        assert!(python::BUILTINS.contains(&"int"));
    }
}
