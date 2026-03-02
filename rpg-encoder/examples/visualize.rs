use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use rpg_encoder::{EdgeType, NodeCategory, NodeId, RpgEncoder};

struct DotConfig {
    color_by_category: bool,
    cluster_directories: bool,
    simplify: bool,
    show_edge_labels: bool,
    max_depth: Option<usize>,
}

impl Default for DotConfig {
    fn default() -> Self {
        Self {
            color_by_category: true,
            cluster_directories: true,
            simplify: true,
            show_edge_labels: false,
            max_depth: None,
        }
    }
}

impl DotConfig {
    fn from_args(args: &[String]) -> (Self, PathBuf, Option<PathBuf>) {
        let mut config = Self::default();
        let mut path = PathBuf::from(".");
        let mut output: Option<PathBuf> = None;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--simplify" => config.simplify = true,
                "--no-simplify" => config.simplify = false,
                "--no-colors" => config.color_by_category = false,
                "--no-clusters" => config.cluster_directories = false,
                "--edge-labels" => config.show_edge_labels = true,
                "--output" => {
                    i += 1;
                    if i < args.len() {
                        output = Some(PathBuf::from(&args[i]));
                    }
                }
                "--max-depth" => {
                    i += 1;
                    if i < args.len() {
                        config.max_depth = args[i].parse().ok();
                    }
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                arg if !arg.starts_with('-') => {
                    path = PathBuf::from(arg);
                }
                _ => {}
            }
            i += 1;
        }

        (config, path, output)
    }
}

fn print_usage() {
    println!("rpg-visualize - Generate DOT graph visualization from RPG");
    println!();
    println!("Usage: rpg-visualize [OPTIONS] <PATH>");
    println!();
    println!("Options:");
    println!("  --simplify          Hide imports, collapse trivial paths (default)");
    println!("  --no-simplify       Show all nodes and edges");
    println!("  --no-colors         Disable category-based coloring");
    println!("  --no-clusters       Disable directory clustering");
    println!("  --edge-labels       Show edge type labels");
    println!("  --output <FILE>     Write output to file instead of stdout");
    println!("  --max-depth <N>     Limit graph depth");
    println!("  --help, -h          Show this help message");
    println!();
    println!("Example:");
    println!("  cargo run --example visualize -- ./src | dot -Tsvg > graph.svg");
}

fn category_color(category: NodeCategory) -> &'static str {
    match category {
        NodeCategory::Repository => "#f0f0f0",
        NodeCategory::Directory => "#e8e8e8",
        NodeCategory::File => "#fffacd",
        NodeCategory::Module => "#98fb98",
        NodeCategory::Type => "#87ceeb",
        NodeCategory::Function => "#90ee90",
        NodeCategory::Variable => "#ffb6c1",
        NodeCategory::Import => "#d3d3d3",
        NodeCategory::Constant => "#dda0dd",
        NodeCategory::Field => "#ffe4b5",
        NodeCategory::Parameter => "#e6e6fa",
        NodeCategory::Feature => "#ffd700",
        NodeCategory::Component => "#ff6347",
        NodeCategory::FunctionalCentroid => "#9c27b0",
    }
}

fn category_shape(category: NodeCategory) -> &'static str {
    match category {
        NodeCategory::Repository => "folder",
        NodeCategory::Directory => "folder",
        NodeCategory::File => "note",
        NodeCategory::Module => "component",
        NodeCategory::Type => "box",
        NodeCategory::Function => "ellipse",
        NodeCategory::Variable => "diamond",
        NodeCategory::Import => "note",
        NodeCategory::Constant => "box",
        NodeCategory::Field => "diamond",
        NodeCategory::Parameter => "diamond",
        NodeCategory::Component => "box3d",
        NodeCategory::FunctionalCentroid => "doublecircle",
        NodeCategory::Feature => "hexagon",
    }
}

fn escape_dot_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('|', "\\|")
        .replace('<', "\\<")
        .replace('>', "\\>")
}

struct GraphData<'a> {
    nodes: Vec<&'a rpg_encoder::Node>,
    edges: Vec<(NodeId, NodeId, &'a rpg_encoder::Edge)>,
}

impl<'a> GraphData<'a> {
    fn from_graph(graph: &'a rpg_encoder::RpgGraph, _config: &DotConfig) -> Self {
        let nodes: Vec<_> = graph.nodes().collect();
        let edges: Vec<_> = graph.edges().collect();

        Self { nodes, edges }
    }

    fn should_hide_node(&self, node: &rpg_encoder::Node, config: &DotConfig) -> bool {
        if config.simplify && node.category == NodeCategory::Import {
            return true;
        }
        false
    }

    fn get_hidden_nodes(&self, config: &DotConfig) -> HashSet<NodeId> {
        self.nodes
            .iter()
            .filter(|n| self.should_hide_node(n, config))
            .map(|n| n.id)
            .collect()
    }
}

fn graph_to_dot(graph: &rpg_encoder::RpgGraph, config: &DotConfig) -> String {
    let data = GraphData::from_graph(graph, config);
    let hidden = data.get_hidden_nodes(config);

    let mut output = String::new();
    output.push_str("digraph RPG {\n");
    output.push_str("  compound=true;\n");
    output.push_str("  rankdir=TB;\n");
    output.push_str("  fontname=\"Helvetica,Arial,sans-serif\";\n");
    output.push_str("  node [fontname=\"Helvetica,Arial,sans-serif\", fontsize=10];\n");
    output.push_str("  edge [fontname=\"Helvetica,Arial,sans-serif\", fontsize=9];\n\n");

    let mut clusters: HashMap<PathBuf, Vec<NodeId>> = HashMap::new();
    let mut root_nodes: Vec<NodeId> = Vec::new();

    if config.cluster_directories {
        for node in &data.nodes {
            if hidden.contains(&node.id) {
                continue;
            }
            if let Some(ref path) = node.path {
                if node.category == NodeCategory::Directory {
                    if let Some(parent) = path.parent() {
                        if parent != PathBuf::from(".") && !parent.as_os_str().is_empty() {
                            clusters
                                .entry(parent.to_path_buf())
                                .or_default()
                                .push(node.id);
                        } else {
                            root_nodes.push(node.id);
                        }
                    } else {
                        root_nodes.push(node.id);
                    }
                }
            }
        }
    }

    output.push_str("  // Nodes\n");
    for node in &data.nodes {
        if hidden.contains(&node.id) {
            continue;
        }

        let node_id = format!("n{}", node.id.index());
        let label = if node.name.is_empty() {
            node.kind.clone()
        } else {
            format!("{}\n{}", node.kind, node.name)
        };

        output.push_str(&format!("  {} [\n", node_id));
        output.push_str(&format!("    label=\"{}\",\n", escape_dot_label(&label)));
        output.push_str(&format!("    shape={},\n", category_shape(node.category)));

        if config.color_by_category {
            output.push_str(&format!("    style=\"filled,rounded\",\n"));
            output.push_str(&format!(
                "    fillcolor=\"{}\",\n",
                category_color(node.category)
            ));
        } else {
            output.push_str("    style=\"rounded\",\n");
        }

        if node.category == NodeCategory::Import {
            output.push_str("    style=\"dashed\",\n");
        }

        output.push_str("  ];\n");
    }

    if config.cluster_directories && !clusters.is_empty() {
        output.push_str("\n  // Clusters\n");
        let mut cluster_idx = 0;
        let mut seen_paths: HashSet<PathBuf> = HashSet::new();

        fn write_cluster(
            path: &PathBuf,
            clusters: &HashMap<PathBuf, Vec<NodeId>>,
            output: &mut String,
            idx: &mut i32,
            seen: &mut HashSet<PathBuf>,
            graph: &rpg_encoder::RpgGraph,
            hidden: &HashSet<NodeId>,
        ) {
            if seen.contains(path) {
                return;
            }
            seen.insert(path.clone());

            if let Some(parent) = path.parent() {
                if parent != PathBuf::from(".") && !parent.as_os_str().is_empty() {
                    write_cluster(
                        &parent.to_path_buf(),
                        clusters,
                        output,
                        idx,
                        seen,
                        graph,
                        hidden,
                    );
                }
            }

            if let Some(node_ids) = clusters.get(path) {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("cluster");

                output.push_str(&format!("  subgraph cluster_{} {{\n", *idx));
                output.push_str(&format!("    label=\"{}\";\n", escape_dot_label(dir_name)));
                output.push_str("    style=\"rounded,filled\";\n");
                output.push_str("    fillcolor=\"#f5f5f5\";\n");
                output.push_str("    color=\"#cccccc\";\n");

                for &nid in node_ids {
                    if !hidden.contains(&nid) {
                        output.push_str(&format!("    n{};\n", nid.index()));
                    }
                }

                output.push_str("  }\n");
                *idx += 1;
            }
        }

        let mut paths: Vec<_> = clusters.keys().cloned().collect();
        paths.sort();

        for path in &paths {
            write_cluster(
                path,
                &clusters,
                &mut output,
                &mut cluster_idx,
                &mut seen_paths,
                graph,
                &hidden,
            );
        }
    }

    output.push_str("\n  // Edges\n");
    for (src, tgt, edge) in &data.edges {
        if hidden.contains(src) || hidden.contains(tgt) {
            continue;
        }

        let src_id = format!("n{}", src.index());
        let tgt_id = format!("n{}", tgt.index());

        output.push_str(&format!("  {} -> {}", src_id, tgt_id));

        let mut attrs: Vec<String> = Vec::new();

        if config.show_edge_labels {
            let label = format!("{:?}", edge.edge_type).to_lowercase();
            attrs.push(format!("label=\"{}\"", label));
        }

        match edge.edge_type {
            EdgeType::Contains => {
                attrs.push("style=\"solid\"".to_string());
                attrs.push("color=\"#666666\"".to_string());
            }
            EdgeType::Imports => {
                attrs.push("style=\"dashed\"".to_string());
                attrs.push("color=\"#999999\"".to_string());
            }
            EdgeType::Calls => {
                attrs.push("style=\"solid\"".to_string());
                attrs.push("color=\"#4CAF50\"".to_string());
            }
            EdgeType::Extends | EdgeType::Implements => {
                attrs.push("style=\"solid\"".to_string());
                attrs.push("color=\"#2196F3\"".to_string());
                attrs.push("arrowhead=\"onormal\"".to_string());
            }
            EdgeType::Defines => {
                attrs.push("style=\"solid\"".to_string());
                attrs.push("color=\"#9C27B0\"".to_string());
            }
            EdgeType::References => {
                attrs.push("style=\"dotted\"".to_string());
                attrs.push("color=\"#FF9800\"".to_string());
            }
            EdgeType::FfiBinding => {
                attrs.push("style=\"bold\"".to_string());
                attrs.push("color=\"#F44336\"".to_string());
            }
            EdgeType::DependsOn | EdgeType::Uses => {
                attrs.push("style=\"dashed\"".to_string());
                attrs.push("color=\"#607D8B\"".to_string());
            }
            EdgeType::UsesType => {
                attrs.push("style=\"dotted\"".to_string());
                attrs.push("color=\"#E91E63\"".to_string());
            }
            EdgeType::ImplementsFeature => {
                attrs.push("style=\"solid\"".to_string());
                attrs.push("color=\"#FFD700\"".to_string());
            }
            EdgeType::BelongsToComponent => {
                attrs.push("style=\"bold\"".to_string());
                attrs.push("color=\"#FF6347\"".to_string());
            }
            EdgeType::BelongsToFeature => {
                attrs.push("style=\"solid\"".to_string());
                attrs.push("color=\"#9C27B0\"".to_string());
            }
            EdgeType::ContainsFeature => {
                attrs.push("style=\"solid\"".to_string());
                attrs.push("color=\"#673AB7\"".to_string());
            }
            #[cfg(feature = "semantic")]
            EdgeType::RequiresFeature | EdgeType::EnablesFeature => {
                attrs.push("style=\"dashed\"".to_string());
                attrs.push("color=\"#00BCD4\"".to_string());
            }
            #[cfg(feature = "semantic")]
            EdgeType::RelatedFeature => {
                attrs.push("style=\"dotted\"".to_string());
                attrs.push("color=\"#8BC34A\"".to_string());
            }
        }

        if !attrs.is_empty() {
            output.push_str(&format!(" [{}]", attrs.join(", ")));
        }

        output.push_str(";\n");
    }

    output.push_str("}\n");
    output
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (config, path, output_path) = DotConfig::from_args(&args);

    if !path.exists() {
        eprintln!("Error: Path does not exist: {}", path.display());
        std::process::exit(1);
    }

    eprintln!("Encoding repository: {}", path.display());

    let mut encoder = match RpgEncoder::new() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to initialize encoder: {}", e);
            std::process::exit(1);
        }
    };
    let result = match encoder.encode(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error encoding repository: {}", e);
            std::process::exit(1);
        }
    };

    let graph = &result.graph;

    eprintln!(
        "Encoded {} nodes, {} edges",
        graph.node_count(),
        graph.edge_count()
    );

    let dot = graph_to_dot(graph, &config);

    if let Some(ref output_file) = output_path {
        match std::fs::write(output_file, &dot) {
            Ok(_) => eprintln!("Written to {}", output_file.display()),
            Err(e) => {
                eprintln!("Error writing output: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!("{}", dot);
    }
}
