//! Defines a compilation unit of rapira26

use std::collections::HashMap;

use crate::ast::{FunctionDefinition, ProcedureDefinition, Spannable, Statement, TypeDefinition};

/// A map of imported module names to the names of their exported definitions
type ImportInfo = HashMap<String, Vec<String>>;

// This is a compilation unit
#[derive(Debug)]
pub struct Module {
    pub name: String,
    pub functions: Vec<Spannable<FunctionDefinition>>,
    pub procedures: Vec<Spannable<ProcedureDefinition>>,
    pub types: Vec<Spannable<TypeDefinition>>,
    pub toplevel: Vec<Spannable<Statement>>,
    pub imports: ImportInfo,
}

impl Module {
    pub fn new(name: String) -> Self {
        Self {
            name,
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
