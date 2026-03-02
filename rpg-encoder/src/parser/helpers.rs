use crate::core::SourceLocation;
use std::path::Path;

pub trait TsNodeExt {
    fn text<'a>(&self, source: &'a [u8]) -> &'a str;
    fn to_location(&self, file: &Path) -> SourceLocation;
}

impl<'a> TsNodeExt for tree_sitter::Node<'a> {
    fn text<'b>(&self, source: &'b [u8]) -> &'b str {
        self.utf8_text(source).unwrap_or("")
    }

    fn to_location(&self, file: &Path) -> SourceLocation {
        let start = self.start_position();
        let end = self.end_position();
        SourceLocation::new(
            file.to_path_buf(),
            start.row + 1,
            start.column + 1,
            end.row + 1,
            end.column + 1,
        )
    }
}

pub fn walk_tree<F>(node: &tree_sitter::Node, source: &[u8], file: &Path, visitor: &mut F)
where
    F: FnMut(&tree_sitter::Node, &[u8], &Path),
{
    visitor(node, source, file);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            walk_tree(&child, source, file, visitor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse_rust(source: &str) -> Option<tree_sitter::Tree> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .ok()?;
        parser.parse(source, None)
    }

    #[test]
    fn test_text_extraction() {
        let source = b"fn main() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source[..], None).unwrap();
        let root = tree.root_node();

        assert_eq!(root.text(source), "fn main() {}");
    }

    #[test]
    fn test_text_invalid_utf8() {
        let source = &[0xFF, 0xFE];
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source[..], None).unwrap();
        let root = tree.root_node();

        // Should return empty string for invalid UTF-8
        assert_eq!(root.text(source), "");
    }

    #[test]
    fn test_to_location() {
        let source = "fn main() {}";
        let tree = parse_rust(source).unwrap();
        let root = tree.root_node();

        let file = PathBuf::from("test.rs");
        let loc = root.to_location(&file);

        assert_eq!(loc.file, file);
        assert_eq!(loc.start_line, 1);
        assert_eq!(loc.start_column, 1);
    }

    #[test]
    fn test_to_location_multiline() {
        let source = "fn main() {\n    let x = 1;\n}";
        let tree = parse_rust(source).unwrap();
        let root = tree.root_node();

        let file = PathBuf::from("multi.rs");
        let loc = root.to_location(&file);

        assert_eq!(loc.start_line, 1);
        assert!(loc.end_line >= 3);
    }

    #[test]
    fn test_walk_tree_visits_all_nodes() {
        let source = "fn foo() {}\nfn bar() {}";
        let tree = parse_rust(source).unwrap();
        let root = tree.root_node();
        let file = PathBuf::from("test.rs");

        let mut count = 0;
        walk_tree(
            &root,
            source.as_bytes(),
            &file,
            &mut |_node, _src, _path| {
                count += 1;
            },
        );

        assert!(count > 1);
    }

    #[test]
    fn test_walk_tree_provides_correct_source() {
        let source = "fn test() {}";
        let tree = parse_rust(source).unwrap();
        let root = tree.root_node();
        let file = PathBuf::from("test.rs");

        let mut found_source = false;
        walk_tree(&root, source.as_bytes(), &file, &mut |node, src, path| {
            if node.kind() == "source_file" {
                assert_eq!(src, source.as_bytes());
                assert_eq!(path, &file);
                found_source = true;
            }
        });

        assert!(found_source);
    }
}
