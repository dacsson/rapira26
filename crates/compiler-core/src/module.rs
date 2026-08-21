//! Defines a compilation unit of rapira26 as well as
//! usefull functions for building and manipulating multi-module projects

use log::debug;
use petgraph::{
    algo::toposort,
    dot::{Config, Dot},
    graph::{DiGraph, NodeIndex},
};
use std::rc::Rc;
use std::{collections::HashMap, path::PathBuf};

use crate::ast::{FunctionDefinition, Spannable, Statement, TypeDefinition};

/// A map of imported module names to the names of their exported definitions
type ImportInfo = HashMap<String, Vec<String>>;

/// A directed graph representing the dependencies between modules
type DependencyGraph = DiGraph<Module, Vec<String>>;

/// Canonical path to a module
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct AbsolutModulePath(pub PathBuf);

impl AbsolutModulePath {
    pub fn get(&self) -> &PathBuf {
        &self.0
    }
}

/// Module name is a "mangled" string of modules path
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ModuleName(pub String);

impl ModuleName {
    pub fn get(&self) -> &str {
        &self.0
    }
}

/// This is a compilation unit
#[derive(Debug, Clone)]
pub struct Module {
    pub name: ModuleName,
    pub path: AbsolutModulePath,
    pub functions: Vec<Spannable<FunctionDefinition>>,
    pub types: Vec<Spannable<TypeDefinition>>,
    pub toplevel: Vec<Spannable<Statement>>,

    // Dependencies
    pub imports_info: ImportInfo,
    pub imported_functions: Vec<Rc<Spannable<FunctionDefinition>>>,
    pub imported_types: Vec<Rc<Spannable<TypeDefinition>>>,
}

impl Module {
    pub fn new(path: AbsolutModulePath) -> Self {
        Self {
            name: Module::mangle_module_name(&path),
            path,
            functions: Vec::new(),
            types: Vec::new(),
            toplevel: Vec::new(),
            imports_info: HashMap::new(),
            imported_functions: Vec::new(),
            imported_types: Vec::new(),
        }
    }

    pub fn mangle_module_name(path: &AbsolutModulePath) -> ModuleName {
        let path_str = path.get().with_extension("").to_string_lossy().to_string();
        ModuleName(path_str.replace("/", "_").replace(".", "_"))
    }

    pub fn add_function(&mut self, function: Spannable<FunctionDefinition>) {
        self.functions.push(function);
    }

    pub fn find_function(&self, name: &str) -> Option<&Spannable<FunctionDefinition>> {
        self.functions
            .iter()
            .find(|f| f.node.name.as_ref().map(|n| n == name).is_some())
    }

    pub fn add_imported_function(&mut self, function: Rc<Spannable<FunctionDefinition>>) {
        self.imported_functions.push(function);
    }

    pub fn find_imported_function(&self, name: &str) -> Option<&Rc<Spannable<FunctionDefinition>>> {
        self.imported_functions
            .iter()
            .find(|f| f.node.name.as_ref().map(|n| n == name).is_some())
    }

    pub fn add_type(&mut self, type_def: Spannable<TypeDefinition>) {
        self.types.push(type_def);
    }

    pub fn find_type(&self, name: &str) -> Option<&Spannable<TypeDefinition>> {
        self.types.iter().find(|t| t.node.name == name)
    }

    pub fn add_imported_type(&mut self, type_def: Rc<Spannable<TypeDefinition>>) {
        self.imported_types.push(type_def);
    }

    pub fn find_imported_type(&self, name: &str) -> Option<&Rc<Spannable<TypeDefinition>>> {
        self.imported_types.iter().find(|t| t.node.name == name)
    }

    pub fn add_toplevel(&mut self, statement: Spannable<Statement>) {
        self.toplevel.push(statement);
    }

    pub fn add_import(&mut self, statement: Spannable<Statement>) {
        if let Statement::Import { name, definitions } = &statement.node {
            self.imports_info.insert(name.clone(), definitions.clone());
        } else {
            panic!(
                "Упс, ошибка: Странное подключение модуля: {:?} в модуле {:?}",
                statement.node, self.name
            );
        }
    }
}

impl std::fmt::Display for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Модуль: {:?}", self.name)?;
        write!(f, "  путь: {}", self.path.get().display())
    }
}

pub enum DependencyError {
    ModuleNotFound(String),
    CyclicDependency,
}

impl std::fmt::Display for DependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyError::ModuleNotFound(name) => write!(f, "Не нашёл модуль: {}", name),
            DependencyError::CyclicDependency => write!(f, "Циклическая зависимость"),
        }
    }
}

/// Build a directed graph of module dependencies
pub fn build_dependency_graph(
    modules: Vec<Module>,
) -> Result<(DependencyGraph, Vec<Module>), DependencyError> {
    let mut graph = DependencyGraph::new();
    modules.into_iter().for_each(|m| _ = graph.add_node(m));

    // Gathers definitions relations (example: A --imports func "func_name"--> B)
    let mut edges = Vec::<(NodeIndex, NodeIndex, Vec<String>)>::new();

    let mut imported_functions =
        HashMap::<NodeIndex, Vec<Rc<Spannable<FunctionDefinition>>>>::new();
    let mut imported_types = HashMap::<NodeIndex, Vec<Rc<Spannable<TypeDefinition>>>>::new();

    for node_idx in graph.node_indices() {
        // Use import metadata to find dependencies
        let imports = &graph.node_weight(node_idx).unwrap().imports_info;
        for (import_mod_name, import_module_deps) in imports {
            let import_node_idx = graph
                .node_indices()
                .find(|n| {
                    graph
                        .node_weight(*n)
                        .map_or(false, |n| n.name.get() == import_mod_name)
                })
                .ok_or(DependencyError::ModuleNotFound(import_mod_name.clone()))?;

            // Expand import info and fill concrete import definitions
            let imported_module = graph.node_weight(import_node_idx).unwrap();
            for import_def_name in import_module_deps {
                // It can be either a function, procedure, or type
                if let Some(imported_func) = imported_module.find_function(import_def_name) {
                    imported_functions
                        .entry(node_idx)
                        .or_default()
                        // TODO: refactor Rc usage, rn its pointless
                        .push(Rc::new(imported_func.clone()));
                }
            }

            // Add to dependency graph
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

    let mut sorted = sorted_idxs
        .into_iter()
        .map(|idx| graph.node_weight(idx).unwrap())
        .cloned()
        .collect::<Vec<_>>();

    debug!("Отсортированные модули: {:?}", sorted);

    sorted.reverse();

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
