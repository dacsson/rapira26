//! Defines a compilation unit of rapira26

use petgraph::{
    algo::toposort,
    dot::{Config, Dot},
    graph::{DiGraph, NodeIndex},
};
use std::{collections::HashMap, path::PathBuf};

use crate::ast::{FunctionDefinition, ProcedureDefinition, Spannable, Statement, TypeDefinition};

/// A map of imported module names to the names of their exported definitions
type ImportInfo = HashMap<String, Vec<String>>;

/// A directed graph representing the dependencies between modules
type DependencyGraph = DiGraph<Module, Vec<String>>;

// This is a compilation unit
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub path: PathBuf,
    pub functions: Vec<Spannable<FunctionDefinition>>,
    pub procedures: Vec<Spannable<ProcedureDefinition>>,
    pub types: Vec<Spannable<TypeDefinition>>,
    pub toplevel: Vec<Spannable<Statement>>,
    pub imports: ImportInfo,
}

impl Module {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            functions: Vec::new(),
            procedures: Vec::new(),
            types: Vec::new(),
            toplevel: Vec::new(),
            imports: HashMap::new(),
        }
    }

    pub fn add_function(&mut self, function: Spannable<FunctionDefinition>) {
        self.functions.push(function);
    }

    pub fn add_procedure(&mut self, procedure: Spannable<ProcedureDefinition>) {
        self.procedures.push(procedure);
    }

    pub fn add_type(&mut self, type_def: Spannable<TypeDefinition>) {
        self.types.push(type_def);
    }

    pub fn add_toplevel(&mut self, statement: Spannable<Statement>) {
        self.toplevel.push(statement);
    }

    pub fn add_import(&mut self, statement: Spannable<Statement>) {
        if let Statement::Import { name, definitions } = &statement.node {
            self.imports.insert(name.clone(), definitions.clone());
        } else {
            panic!(
                "Упс, ошибка: Странное подключение модуля: {:?} в модуле {}",
                statement.node, self.name
            );
        }
    }
}

impl std::fmt::Display for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Модуль: {}", self.name)?;
        write!(f, "  путь: {}", self.path.display())
    }
}

pub enum DependencyError {
    ModuleNotFound(String),
    CyclicDependency,
}

/// Build a directed graph of module dependencies
pub fn build_dependency_graph(
    modules: Vec<Module>,
) -> Result<(DependencyGraph, Vec<Module>), DependencyError> {
    let mut graph = DependencyGraph::new();
    modules.into_iter().for_each(|m| _ = graph.add_node(m));

    let mut edges = Vec::<(NodeIndex, NodeIndex, Vec<String>)>::new();

    for node_idx in graph.node_indices() {
        let imports = &graph.node_weight(node_idx).unwrap().imports;
        for (import_mod_name, import_module_deps) in imports {
            let import_node_idx = graph
                .node_indices()
                .find(|n| {
                    graph.node_weight(*n).map_or(false, |n| {
                        let import_with_ext = format!("{}.{}", import_mod_name, "рап");
                        n.name == import_with_ext
                    })
                })
                .ok_or(DependencyError::ModuleNotFound(import_mod_name.clone()))?;

            edges.push((node_idx, import_node_idx, import_module_deps.clone()));
        }
    }

    edges
        .into_iter()
        .for_each(|(src, dst, deps)| _ = graph.add_edge(src, dst, deps));

    // TODO: detect cycles in DAG

    // Topological sort
    let sorted_idxs = match toposort(&graph, None) {
        Ok(sorted) => sorted,
        Err(_) => return Err(DependencyError::CyclicDependency),
    };

    let sorted = sorted_idxs
        .into_iter()
        .map(|idx| graph.node_weight(idx).unwrap())
        .cloned()
        .collect::<Vec<_>>();

    Ok((graph, sorted))
}

/// Dumps dependcy graph in graphviz dot format into stdout
pub fn dump_dependency_graph(graph: &DependencyGraph) {
    let dot = Dot::with_attr_getters(
        graph,
        &[Config::NodeNoLabel, Config::EdgeNoLabel],
        &|_graph, edge| {
            let deps = edge.weight().join(", ");
            format!("label = \"Зависимости: {}\"", deps)
        },
        &|_graph, node| format!("label = \"{}\"", node.1),
    );
    println!("{:?}", dot);
}

impl std::fmt::Display for DependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyError::ModuleNotFound(name) => write!(f, "Не нашёл модуль: {}", name),
            DependencyError::CyclicDependency => write!(f, "Циклическая зависимость"),
        }
    }
}
