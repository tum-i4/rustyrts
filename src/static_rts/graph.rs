use dot::RenderOption;
use internment::Arena;
use internment::ArenaIntern;
use itertools::Itertools;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use queues::{IsQueue, Queue};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
};
use std::{fmt::Debug, ops::Deref};
use std::{fmt::Display, iter::IntoIterator, vec::Vec};
use std::{hash::Hash, ops::AddAssign};

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, IntoPrimitive, TryFromPrimitive)]
#[repr(u16)]
pub enum EdgeType {
    Call = 1 << 0,
    Unsize = 1 << 1,
    Contained = 1 << 2,
    Drop = 1 << 3,
    Static = 1 << 4,
    ReifyPtr = 1 << 5,
    FnPtr = 1 << 6,

    Asm = 1 << 7,
    ClosurePtr = 1 << 8,
    Intrinsic = 1 << 9,
    LangItem = 1 << 10,

    Trimmed = 1 << 11,
}

impl Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{self:?}"))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EdgeTypes {
    bitmap: u16,
}

impl EdgeTypes {
    fn empty() -> Self {
        Self { bitmap: 0 }
    }

    fn from_raw(raw: u16) -> Self {
        Self { bitmap: raw }
    }
}

impl Deref for EdgeTypes {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.bitmap
    }
}

impl AddAssign<EdgeType> for EdgeTypes {
    #[allow(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, rhs: EdgeType) {
        self.bitmap |= u16::from(rhs);
    }
}

impl IntoIterator for EdgeTypes {
    type Item = EdgeType;

    type IntoIter = <Vec<EdgeType> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        let mut vec = Vec::new();
        let mut i: u16 = 1;
        while i != 0 {
            let d = self.bitmap & i;

            if d != 0 {
                let ty = EdgeType::try_from(i).expect("Found malformed edge_types");
                vec.push(ty);
            }
            i <<= 1;
        }

        vec.into_iter()
    }
}

impl Display for EdgeTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut types = self.into_iter();
        f.write_fmt(format_args!("{}", types.join(", ")))
    }
}

#[derive(Clone)]
pub struct DependencyGraph<'arena, T: Eq + Hash> {
    arena: &'arena Arena<T>,
    nodes: HashSet<ArenaIntern<'arena, T>>,
    backwards_edges: HashMap<ArenaIntern<'arena, T>, HashMap<ArenaIntern<'arena, T>, EdgeTypes>>,
}

impl<'arena, T: Eq + Hash> PartialEq for DependencyGraph<'arena, T> {
    fn eq(&self, other: &Self) -> bool {
        self.nodes == other.nodes && self.backwards_edges == other.backwards_edges
    }
}

impl<'arena, T: Eq + Hash + Debug> Debug for DependencyGraph<'arena, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Graph { ")?;
        f.write_fmt(format_args!("Nodes: {:?}", self.nodes))?;
        f.write_fmt(format_args!("Edges: {:?}", self.backwards_edges))?;
        f.write_str(" }")?;
        Ok(())
    }
}

impl<'arena, T: Eq + Hash> DependencyGraph<'arena, T> {
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

        if ingoing.get(&start).is_none() {
            ingoing.insert(start, EdgeTypes::empty());
        }

        let types = ingoing.get_mut(&start).unwrap();
        *types += edge_type;
    }

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
                for start in edges.keys() {
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
    pub fn render_to<W: Write>(&self, output: &mut W) {
        dot::render_opts(
            self,
            output,
            &[RenderOption::NoEdgeLabels, RenderOption::NoNodeLabels],
        )
        .unwrap();
    }
}

pub mod serialize {
    use std::{
        collections::{HashMap, HashSet},
        str::Utf8Error,
    };

    use internment::Arena;
    use itertools::Itertools;

    use crate::static_rts::graph::EdgeTypes;

    use super::DependencyGraph;

    #[derive(Debug)]
    pub enum DeserializationError {
        SplitError,
        ConversionError(Utf8Error),
        IndexOutOfBounds(usize),
    }

    pub trait ArenaSerializable<'arena, I> {
        fn serialize(self) -> Vec<u8>;
    }

    pub trait ArenaDeserializable<'arena, I> {
        type Error;

        fn deserialize(
            arena: &'arena Arena<I>,
            input: &[u8],
        ) -> Result<DependencyGraph<'arena, String>, Self::Error>;
    }

    impl<'arena> ArenaSerializable<'arena, String> for DependencyGraph<'arena, String> {
        fn serialize(self) -> Vec<u8> {
            let mut out: Vec<u8> = Vec::new();

            let mut nodes_map = HashMap::new();

            // 1. Nodes
            let nodes = self
                .nodes
                .into_iter()
                .enumerate()
                .inspect(|(i, s)| {
                    nodes_map.insert(*s, *i);
                })
                .map(|(_i, s)| s)
                .join("|");
            out.extend_from_slice(nodes.as_bytes());

            out.push(b'~');

            // 2. Edges
            out.extend_from_slice(&self.backwards_edges.len().to_ne_bytes());
            for (end, edges) in self.backwards_edges {
                let i_end = *nodes_map.get(&end).unwrap();
                let num = edges.len();

                out.extend_from_slice(&i_end.to_ne_bytes());
                out.extend_from_slice(&num.to_ne_bytes());

                for (start, types) in edges {
                    let i_start = nodes_map.get(&start).unwrap();

                    out.extend_from_slice(&i_start.to_ne_bytes());
                    out.extend_from_slice(&(*types).to_ne_bytes());
                }
            }

            out
        }
    }

    impl<'arena> ArenaDeserializable<'arena, String> for DependencyGraph<'arena, String> {
        type Error = DeserializationError;

        fn deserialize(arena: &'arena Arena<String>, input: &[u8]) -> Result<Self, Self::Error> {
            let mut input = input;

            let mut nodes = HashSet::new();
            let mut backwards_edges = HashMap::new();

            while !input.is_empty() {
                let (nodes_raw, rest) = input
                    .split_once(|c| *c == b'~')
                    .ok_or(DeserializationError::SplitError)?;
                input = rest;

                // 1. Read nodes
                let mut nodes_map = HashMap::new();
                for (i, n) in nodes_raw.split(|c| *c == b'|').enumerate() {
                    let node =
                        std::str::from_utf8(n).map_err(DeserializationError::ConversionError)?;
                    let interned = arena.intern(node.to_string());
                    nodes_map.insert(i, interned);
                    nodes.insert(interned);
                }

                // 2. Read edges
                let num_edges = {
                    let (num_raw, r) = input.split_at(std::mem::size_of::<usize>());
                    input = r;

                    usize::from_ne_bytes(num_raw.try_into().unwrap())
                };

                for _ in 0..num_edges {
                    let (end_raw, r) = input.split_at(std::mem::size_of::<usize>());
                    input = r;
                    let (num_raw, r) = input.split_at(std::mem::size_of::<usize>());
                    input = r;

                    let end_index = usize::from_ne_bytes(end_raw.try_into().unwrap());
                    let end = *nodes_map
                        .get(&end_index)
                        .ok_or(DeserializationError::IndexOutOfBounds(end_index))?;

                    let num = usize::from_ne_bytes(num_raw.try_into().unwrap());

                    backwards_edges.insert(end, HashMap::new());
                    let inner = backwards_edges.get_mut(&end).unwrap();

                    for _ in 0..num {
                        let (start_raw, r) = input.split_at(std::mem::size_of::<usize>());
                        input = r;
                        let (types_raw, r) = input.split_at(std::mem::size_of::<EdgeTypes>());
                        input = r;

                        let start_index = usize::from_ne_bytes(start_raw.try_into().unwrap());
                        let start = *nodes_map
                            .get(&start_index)
                            .ok_or(DeserializationError::IndexOutOfBounds(start_index))?;

                        let bitmap = u16::from_ne_bytes(types_raw.try_into().unwrap());
                        let types = EdgeTypes::from_raw(bitmap);

                        inner.insert(start, types);
                    }
                }
            }

            Ok(Self {
                arena,
                nodes,
                backwards_edges,
            })
        }
    }
}

pub mod pretty {
    use std::borrow::Cow;

    use dot::{GraphWalk, Id, Labeller};
    use internment::ArenaIntern;
    use itertools::Itertools;

    use super::{DependencyGraph, EdgeType};

    #[derive(Clone)]
    pub(crate) struct Edge<'arena> {
        start: ArenaIntern<'arena, String>,
        end: ArenaIntern<'arena, String>,
        ty: EdgeType,
    }

    impl<'arena, 'a> GraphWalk<'a, ArenaIntern<'arena, String>, Edge<'arena>>
        for DependencyGraph<'arena, String>
    {
        fn nodes(&'a self) -> dot::Nodes<'a, ArenaIntern<'arena, String>> {
            self.nodes
                .iter()
                .copied()
                .sorted_by(|n1, n2| Ord::cmp(n1.as_str(), n2.as_str()))
                .collect_vec()
                .into()
        }

        fn edges(&'a self) -> dot::Edges<'a, Edge<'arena>> {
            let mut vec = Vec::new();

            for (end, edges) in self
                .backwards_edges
                .iter()
                .sorted_by(|(s1, _), (s2, _)| Ord::cmp(s1.as_str(), s2.as_str()))
            {
                for (start, types) in edges
                    .iter()
                    .sorted_by(|(e1, _), (e2, _)| Ord::cmp(e1.as_str(), e2.as_str()))
                {
                    for ty in types.into_iter() {
                        if ty != EdgeType::Trimmed {
                            vec.push(Edge {
                                start: *start,
                                end: *end,
                                ty,
                            });
                        }
                    }
                }
            }

            std::borrow::Cow::Owned(vec)
        }

        fn source(&'a self, edge: &Edge<'arena>) -> ArenaIntern<'arena, String> {
            let Edge {
                start,
                end: _end,
                ty: _types,
            } = edge;
            *start
        }

        fn target(&'a self, edge: &Edge<'arena>) -> ArenaIntern<'arena, String> {
            let Edge {
                start: _start,
                end,
                ty: _types,
            } = edge;
            *end
        }
    }

    impl<'a, 'arena> Labeller<'a, ArenaIntern<'arena, String>, Edge<'arena>>
        for DependencyGraph<'arena, String>
    {
        fn graph_id(&'a self) -> dot::Id<'a> {
            Id::new("DependencyGraph").unwrap()
        }

        fn node_id(&'a self, n: &ArenaIntern<'arena, String>) -> dot::Id<'a> {
            unsafe { unchecked_id(n) }
        }

        fn edge_color(&'a self, e: &Edge<'arena>) -> Option<dot::LabelText<'a>> {
            let color = match e.ty {
                EdgeType::Call => "black",
                EdgeType::Unsize => "blue",
                EdgeType::Contained => "orange",
                EdgeType::Drop => "yellow",
                EdgeType::Static => "green",
                EdgeType::ReifyPtr => "magenta",
                EdgeType::FnPtr => "cyan",
                EdgeType::Asm => "grey",
                EdgeType::ClosurePtr => "grey",
                EdgeType::Intrinsic => "grey",
                EdgeType::LangItem => "grey",
                EdgeType::Trimmed => "red",
            };

            Some(dot::LabelText::LabelStr(Cow::Borrowed(color)))
        }

        fn edge_label(&'a self, e: &Edge<'arena>) -> dot::LabelText<'a> {
            let label = match e.ty {
                EdgeType::Call => "call",
                EdgeType::Unsize => "unsize",
                EdgeType::Contained => "contained",
                EdgeType::Drop => "drop",
                EdgeType::Static => "static",
                EdgeType::ReifyPtr => "reify_ptr",
                EdgeType::FnPtr => "fn_ptr",
                EdgeType::Asm => "asm",
                EdgeType::ClosurePtr => "closure_ptr",
                EdgeType::Intrinsic => "intrinsic",
                EdgeType::LangItem => "lang_item",
                EdgeType::Trimmed => "",
            };

            dot::LabelText::LabelStr(Cow::Borrowed(label))
        }
    }

    unsafe fn unchecked_id<'a, 'arena>(n: &ArenaIntern<'arena, String>) -> dot::Id<'a> {
        let cow: Cow<'a, str> = Cow::Owned(format!("\"{n}\""));
        std::mem::transmute::<Cow<_>, Id>(cow)
    }
}

#[cfg(test)]
mod test {
    use internment::Arena;

    use crate::static_rts::graph::{DependencyGraph, EdgeType};

    use super::serialize::{ArenaDeserializable, ArenaSerializable};

    #[test]
    pub fn test_graph_deserialization() {
        let arena = Arena::new();
        let mut graph: DependencyGraph<String> = DependencyGraph::new(&arena);

        graph.add_node("lonely_node".to_string());
        graph.add_edge("start1".to_string(), "end1".to_string(), EdgeType::Call);
        graph.add_edge("start1".to_string(), "end2".to_string(), EdgeType::Unsize);
        graph.add_edge("start2".to_string(), "end2".to_string(), EdgeType::Drop);

        let serialized = graph.clone().serialize();
        println!("Serialized {serialized:?}");
        let deserialized = DependencyGraph::deserialize(&arena, &serialized).unwrap();
        println!("Deserialized {deserialized:?}");

        assert_eq!(graph, deserialized);
    }
}
