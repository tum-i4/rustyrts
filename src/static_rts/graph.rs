use itertools::Itertools;
use queues::{IsQueue, Queue};

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::str::FromStr;

use internment::Arena;
use internment::ArenaIntern;

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum EdgeType {
    Call,
    Unsize,
    Contained,
    Drop,
    Static,
    ReifyPtr,
    FnPtr,

    Asm,
    ClosurePtr,
    Intrinsic,
    LangItem,

    Trimmed,
}

impl AsRef<str> for EdgeType {
    fn as_ref(&self) -> &str {
        match self {
            EdgeType::Call => "[color = black]",
            EdgeType::Unsize => "[color = blue]",
            EdgeType::Contained => "[color = orange]",
            EdgeType::Drop => "[color = yellow]",
            EdgeType::Static => "[color = green]",
            EdgeType::ReifyPtr => "[color = magenta]",
            EdgeType::FnPtr => "[color = cyan]",

            EdgeType::Asm => "[color = grey]",
            EdgeType::ClosurePtr => "[color = grey]",
            EdgeType::Intrinsic => "[color = grey]",
            EdgeType::LangItem => "[color = grey]",

            EdgeType::Trimmed => "[color = red]",
        }
    }
}

impl FromStr for EdgeType {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "Call" => Ok(Self::Call),
            "Unsize" => Ok(Self::Unsize),
            "Contained" => Ok(Self::Contained),
            "Drop" => Ok(Self::Drop),
            "Static" => Ok(Self::Static),
            "ReifyPtr" => Ok(Self::ReifyPtr),
            "FnPtr" => Ok(Self::FnPtr),

            "ClosurePtr" => Ok(Self::ClosurePtr),
            "Asm" => Ok(Self::Asm),
            "Intrinsic" => Ok(Self::Intrinsic),
            "LangItem" => Ok(Self::LangItem),

            "Trimmed" => Ok(Self::Trimmed),
            _ => Err(()),
        }
    }
}

pub struct DependencyGraph<'arena, T: Eq + Hash> {
    arena: &'arena Arena<T>,
    nodes: HashSet<ArenaIntern<'arena, T>>,
    backwards_edges:
        HashMap<ArenaIntern<'arena, T>, HashMap<ArenaIntern<'arena, T>, HashSet<EdgeType>>>,
}

impl<'arena, 'a, T: Eq + Hash> DependencyGraph<'arena, T> {
    pub fn new(arena: &'arena Arena<T>) -> Self {
        Self {
            arena,
            nodes: HashSet::new(),
            backwards_edges: HashMap::new(),
        }
    }

    fn add_node(&mut self, node: T) -> ArenaIntern<'arena, T> {
        let interned = self.arena.intern(node);

        if self.nodes.insert(interned) {
            self.backwards_edges.insert(interned, HashMap::new());
        }

        interned
    }

    pub fn add_edge(&mut self, start: T, end: T, edge_type: EdgeType) {
        let end = self.add_node(end);
        let start = self.add_node(start);

        let ingoing = self.backwards_edges.get_mut(&end).unwrap();

        if let None = ingoing.get(&start) {
            ingoing.insert(start.clone(), HashSet::new());
        }

        let types = ingoing.get_mut(&start).unwrap();
        types.insert(edge_type);
    }

    // #[allow(unused)]
    // pub fn affected_tests<'b: 'c, 'c, S>(
    //     &'b self,
    //     starting_points: S,
    //     tests: HashSet<&T>,
    // ) -> (HashSet<&'b T>, HashMap<&'c T, Vec<&'b T>>)
    // where
    //     S: IntoIterator<Item = &'b T>,
    // {
    //     let mut reached = HashSet::new();
    //     let mut affected = HashMap::new();
    //     let mut acc = Vec::new();

    //     for ele in starting_points {
    //         if reached.insert(ele) {
    //             self.affected_tests_helper(ele, &tests, &mut reached, &mut affected, &mut acc);
    //         }
    //     }

    //     (reached, affected)
    // }

    // fn affected_tests_helper<'b: 'c, 'c>(
    //     &'b self,
    //     node: &'b T,
    //     tests: &HashSet<&T>,
    //     reached: &mut HashSet<&'b T>,
    //     affected: &mut HashMap<&'c T, Vec<&'b T>>,
    //     acc: &mut Vec<&'b T>,
    // ) {
    //     acc.push(node);

    //     if tests.contains(node) && !affected.keys().contains(&node) {
    //         affected.insert(node, acc.clone());
    //     }

    //     if let Some(edges) = self.backwards_edges.get(node) {
    //         for (start, _types) in edges {
    //             if reached.insert(start) {
    //                 self.affected_tests_helper(start, tests, reached, affected, acc);
    //             }
    //         }
    //     }

    //     acc.pop();
    // }

    #[allow(unused)]
    pub fn reachable_nodes(
        &self,
        starting_points: impl IntoIterator<Item = ArenaIntern<'arena, T>>,
    ) -> HashSet<ArenaIntern<'arena, T>> {
        let mut queue: Queue<ArenaIntern<'arena, T>> = Queue::new();
        let mut reached: HashSet<ArenaIntern<'arena, T>> = HashSet::new();

        for ele in starting_points {
            queue.add(ele).unwrap();
        }

        while let Ok(node) = queue.remove() {
            if !reached.insert(node) {
                // We already processed this node before
                continue;
            }

            if let Some(edges) = self.backwards_edges.get(&node) {
                for (start, _types) in edges {
                    if !reached.contains(start) {
                        queue.add(*start).unwrap();
                    }
                }
            }
        }

        reached
    }
}

impl<'arena> DependencyGraph<'arena, String> {
    pub fn import_nodes(&mut self, lines: impl IntoIterator<Item = String>) {
        // Parse nodes
        // TODO: improve error handling
        for line in lines {
            let message_fn = || format!("Found malformed node line: {})", line);

            let node = line.strip_prefix("\"").expect(&message_fn());
            let node = node.strip_suffix("\"").expect(&message_fn());

            self.add_node(node.to_string());
        }
    }

    pub fn import_edges(&mut self, lines: impl IntoIterator<Item = String>) {
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

    pub fn pretty(&self, checksum_nodes: HashSet<String>) -> String {
        let mut result = String::new();

        result.push_str("digraph {\n");

        for node in self.nodes.iter().sorted_by(|a, b| Ord::cmp(&***b, &***a)) {
            let format = if checksum_nodes.contains(&***node) {
                " [penwidth = 2.5]"
            } else {
                ""
            };
            result.push_str(format!("\"{}\"{}\n", node, format).as_str())
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
            for typ in *types {
                result.push_str(
                    format!(
                        "\"{}\" -> \"{}\" {} // {:?}\n",
                        start,
                        end,
                        typ.as_ref(),
                        typ
                    )
                    .as_str(),
                )
            }
        }

        result.push_str("}\n");

        result
    }
}

impl<'arena> ToString for DependencyGraph<'arena, String> {
    fn to_string(&self) -> String {
        let mut result = String::new();

        result.push_str("digraph {\n");

        for node in self.nodes.iter().sorted_by(|a, b| Ord::cmp(&***b, &***a)) {
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

impl<'arena> DependencyGraph<'arena, String> {
    pub fn from_str(arena: &'arena Arena<String>, s: &str) -> Result<Self, ()> {
        let content = s.trim();
        let (edges, nodes): (Vec<_>, Vec<_>) = content
            .split("\n")
            .filter(|l| !l.trim_start().starts_with("\\")) // Remove Comments
            .filter(|l| l != &"digraph {")
            .filter(|l| l != &"}")
            .map(|s| s.to_string())
            .partition(|l| l.contains("\" -> \""));

        let mut result = Self::new(arena);

        result.import_nodes(nodes.into_iter().filter(|l| !l.is_empty()));
        result.import_edges(edges);

        return Ok(result);
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use internment::Arena;

    use crate::static_rts::graph::{DependencyGraph, EdgeType};

    #[test]
    pub fn test_graph_deserialization() {
        let arena = Arena::new();
        let mut graph: DependencyGraph<String> = DependencyGraph::new(&arena);

        graph.add_node("lonely_node".to_string());
        graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::Call);
        graph.add_edge("start1".to_string(), "end2".to_string(), EdgeType::Unsize);
        graph.add_edge("start2".to_string(), "end2".to_string(), EdgeType::Drop);

        // let serialized = graph.to_string();
        // let deserialized = DependencyGraph::from_str(&serialized).unwrap();

        // assert_eq!(graph, deserialized);
    }
}
