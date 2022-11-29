use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::str::FromStr;

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum EdgeType {
    Call,
    Unevaluated,
    Scalar,
    Closure,
    Generator,
    FnDef,
}

impl FromStr for EdgeType {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "Call" => Ok(Self::Call),
            "Unevaluated" => Ok(Self::Unevaluated),
            "Scalar" => Ok(Self::Scalar),
            "Closure" => Ok(Self::Closure),
            "Generator" => Ok(Self::Generator),
            "FnDef" => Ok(Self::FnDef),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct DependencyGraph<T: Eq + Hash> {
    nodes: HashSet<T>,
    backwards_edges: HashMap<T, HashMap<T, HashSet<EdgeType>>>,
}

impl<T: Eq + Hash + Clone> DependencyGraph<T> {
    pub fn new() -> DependencyGraph<T> {
        DependencyGraph {
            nodes: HashSet::new(),
            backwards_edges: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: T) {
        if self.nodes.insert(node.clone()) {
            self.backwards_edges.insert(node, HashMap::new());
        }
    }

    pub fn add_edge(&mut self, start: T, end: T, edge_type: EdgeType) {
        self.add_node(end.clone());
        self.add_node(start.clone());

        let ingoing = self.backwards_edges.get_mut(&end).unwrap();

        if let None = ingoing.get(&start) {
            ingoing.insert(start.clone(), HashSet::new());
        }
        let types = ingoing.get_mut(&start).unwrap();
        types.insert(edge_type);
    }
}

impl ToString for DependencyGraph<String> {
    fn to_string(&self) -> String {
        let mut result = String::new();

        result.push_str("digraph {\n");
        result.push_str("\n//Nodes\n");

        for node in &self.nodes {
            result.push_str(format!("\"{}\"\n", *node).as_str())
        }

        result.push_str("\n//Edges\n");

        for (end, edge) in &self.backwards_edges {
            for (start, types) in edge {
                result.push_str(format!("\"{}\" -> \"{}\" // {:?}\n", *start, *end, types).as_str())
            }
        }

        result.push_str("}\n");

        result
    }
}

impl FromStr for DependencyGraph<String> {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(content) = s
            .trim()
            .strip_prefix("digraph {") // Filter beginning and ending
            .and_then(|s| s.strip_suffix("}"))
        {
            let lines = content
                .split("\n")
                .filter(|l| !l.trim_start().starts_with("\\")) // Remove Comments
                .filter(|l| l.contains("->")); // Ignore Nodes

            let mut result = Self::new();

            // Parse edges
            for line in lines {
                let (edge_str, edge_types_str) = line.split_once("//").unwrap();

                if let Some(edge_types) = edge_types_str
                    .strip_prefix(" {")
                    .and_then(|s| s.strip_suffix("}"))
                    .and_then(|s| Some(s.split(", ")))
                {
                    let (start_str, end_str) = edge_str.split_once("-> ").unwrap();
                    let start = start_str
                        .strip_prefix("\"")
                        .and_then(|s| s.strip_suffix("\" "))
                        .unwrap();
                    let end = end_str
                        .strip_prefix("\"")
                        .and_then(|s| s.strip_suffix("\" "))
                        .unwrap();

                    for edge_type in edge_types {
                        result.add_edge(
                            start.to_string(),
                            end.to_string(),
                            edge_type.parse().unwrap(),
                        );
                    }
                }
            }

            return Ok(result);
        }
        Err(())
    }
}

#[test]
pub fn test_graph_deserialization() {
    let mut graph: DependencyGraph<String> = DependencyGraph::new();

    graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::Closure);
    graph.add_edge("start1".to_string(), "end2".to_string(), EdgeType::Closure);
    graph.add_edge("start2".to_string(), "end2".to_string(), EdgeType::Closure);

    let serialized = graph.to_string();
    let deserialized = DependencyGraph::from_str(&serialized).unwrap();

    assert_eq!(graph, deserialized);
}
