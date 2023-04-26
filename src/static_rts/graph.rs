use itertools::Itertools;
use queues::{IsQueue, Queue};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::str::FromStr;

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum EdgeType {
    Unevaluated,
    Scalar,
    Closure,
    Generator,
    FnDef,
    FnPtr, // TODO: not sure if this is necessary
    Impl,
    Adt,
    //Foreign,
    //Opaque,
}

impl FromStr for EdgeType {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "Unevaluated" => Ok(Self::Unevaluated),
            "Scalar" => Ok(Self::Scalar),
            "Closure" => Ok(Self::Closure),
            "Generator" => Ok(Self::Generator),
            "FnDef" => Ok(Self::FnDef),
            "FnPtr" => Ok(Self::FnPtr),
            "Impl" => Ok(Self::Impl),
            "Adt" => Ok(Self::Adt),
            //"Foreign" => Ok(Self::Foreign),
            //"Opaque" => Ok(Self::Opaque),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct DependencyGraph<T: Eq + Hash + Clone> {
    nodes: HashSet<T>,
    backwards_edges: HashMap<T, HashMap<T, HashSet<EdgeType>>>,
}

impl<'a, T: Eq + Hash + Clone> DependencyGraph<T> {
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

    pub fn get_nodes(&self) -> &HashSet<T> {
        &self.nodes
    }

    pub fn get_node(&self, node: &T) -> Option<&T> {
        self.nodes.get(node)
    }

    pub fn get_edges_to(&self, to_node: &T) -> Option<&HashMap<T, HashSet<EdgeType>>> {
        self.backwards_edges.get(to_node)
    }

    pub fn reachable_nodes<'b, S>(&'b self, starting_points: S) -> HashSet<&'b T>
    where
        S: IntoIterator<Item = &'b T>,
    {
        let mut queue: Queue<&'b T> = Queue::new();
        let mut reached: HashSet<&'b T> = HashSet::new();

        for ele in starting_points {
            queue.add(ele).unwrap();
        }

        while let Ok(node) = queue.remove() {
            if !reached.insert(node) {
                // We already processed this node before
                continue;
            }

            if let Some(edges) = self.backwards_edges.get(node) {
                for (start, _types) in edges {
                    if !reached.contains(start) {
                        queue.add(start).unwrap();
                    }
                }
            }
        }

        reached
    }
}

impl DependencyGraph<String> {
    pub fn import_nodes<T>(&mut self, lines: T)
    where
        T: IntoIterator<Item = String>,
    {
        // Parse nodes
        for line in lines {
            let message_fn = || format!("Found malformed node line: {})", line);

            let node = line.strip_prefix("\"").expect(&message_fn());
            let node = node.strip_suffix("\"").expect(&message_fn());

            self.add_node(node.to_string());
        }
    }

    pub fn import_edges<T>(&mut self, lines: T)
    where
        T: IntoIterator<Item = String>,
    {
        // Parse edges
        for line in lines {
            let message_fn = || format!("Found malformed edge line: {})", line);

            let (edge_str, edge_types_str) = line.split_once(" //").expect(&message_fn());

            if let Some(edge_types) = edge_types_str
                .strip_prefix(" {")
                .and_then(|s| s.strip_suffix("}"))
                .and_then(|s| Some(s.split(", ")))
            {
                let (start_str, end_str) = edge_str.split_once("\" -> \"").unwrap();
                let start = start_str.strip_prefix("\"").expect(&message_fn());
                let end = end_str.strip_suffix("\"").expect(&message_fn());

                for edge_type in edge_types {
                    self.add_edge(
                        start.to_string(),
                        end.to_string(),
                        edge_type.parse().expect(&message_fn()),
                    );
                }
            }
        }
    }
}

impl ToString for DependencyGraph<String> {
    fn to_string(&self) -> String {
        let mut result = String::new();

        result.push_str("digraph {\n");

        for node in self.nodes.iter().sorted_by(|a, b| Ord::cmp(&b, &a)) {
            result.push_str(format!("\"{}\"\n", node).as_str())
        }

        let mut unsorted: Vec<(&String, &String, &HashSet<EdgeType>)> = Vec::new();

        for (end, edge) in self.backwards_edges.iter() {
            for (start, types) in edge.iter() {
                unsorted.push((start, end, types));
            }
        }

        for (start, end, types) in unsorted
            .iter()
            .sorted_by(|a, b| Ord::cmp(&b.0, &a.0).then(Ord::cmp(&b.1, &a.1)))
        {
            result.push_str(format!("\"{}\" -> \"{}\" // {:?}\n", start, end, types).as_str())
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
            let (edges, nodes): (Vec<_>, Vec<_>) = content
                .split("\n")
                .filter(|l| !l.trim_start().starts_with("\\")) // Remove Comments
                .map(|s| s.to_string())
                .partition(|l| l.contains("\" -> \""));

            let mut result = Self::new();

            result.import_nodes(nodes.into_iter().filter(|l| !l.is_empty()));
            result.import_edges(edges);

            return Ok(result);
        }
        Err(())
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::static_rts::graph::{DependencyGraph, EdgeType};

    #[test]
    pub fn test_graph_deserialization() {
        let mut graph: DependencyGraph<String> = DependencyGraph::new();

        graph.add_node("lonely_node".to_string());
        graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::Closure);
        graph.add_edge("start1".to_string(), "end2".to_string(), EdgeType::Closure);
        graph.add_edge("start2".to_string(), "end2".to_string(), EdgeType::Closure);

        let serialized = graph.to_string();
        let deserialized = DependencyGraph::from_str(&serialized).unwrap();

        assert_eq!(graph, deserialized);
    }

    #[test]
    pub fn test_graph_deserialization_edge_types() {
        let mut graph: DependencyGraph<String> = DependencyGraph::new();

        graph.add_edge(
            "start1".to_string(),
            "end1".to_string(),
            EdgeType::Unevaluated,
        );
        graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::Scalar);
        graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::Closure);
        graph.add_edge(
            "start1".to_string(),
            "end1".to_string(),
            EdgeType::Generator,
        );
        graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::FnDef);
        graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::FnPtr);
        graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::Impl);

        let serialized = graph.to_string();
        let deserialized = DependencyGraph::from_str(&serialized).unwrap();

        assert_eq!(graph, deserialized);
    }
}
