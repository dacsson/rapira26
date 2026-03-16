//! A C code builder, given a name for a variable, function, etc. generates the corresponding C code as a string.
//! This is just a generalized code emitter, it doesn't specialize on Rapira input.

pub struct CBuilder {
    /// The generated C code (single module)
    code: String,
    /// Module name
    name: String,
}

impl CBuilder {
    pub fn new(module_name: &str) -> Self {
        Self {
            code: String::new(),
            name: module_name.to_string(),
        }
    }

    /// Creates a function with the given return type, name, and arguments.
    pub fn create_function(&mut self, return_type: &str, name: &str, args: &[&str]) {
        let mut function = format!("{} {}(", return_type, name);
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                function.push_str(", ");
            }
            function.push_str(arg);
        }
        function.push_str(") {\n");
        self.code.push_str(&function);
    }

    /// Creates a variable with the given type and name.
    pub fn create_variable(&mut self, return_type: &str, name: &str) {
        self.code.push_str(&format!("{} {};\n", return_type, name));
    }

    /// Creates a variable with the given type and name, and initializes it with the given value.
    pub fn create_variable_with_value(&mut self, return_type: &str, name: &str, value: &str) {
        self.code
            .push_str(&format!("{} {} = {};\n", return_type, name, value));
    }

    /// Creates a struct with the given name and fields.
    pub fn create_struct(&mut self, name: &str, fields: &[(&str, &str)]) {
        let mut struct_def = format!("struct {} {{\n", name);
        for (field_name, field_type) in fields {
            struct_def.push_str(&format!("    {} {};\n", field_type, field_name));
        }
        struct_def.push_str("};\n");
        self.code.push_str(&struct_def);
    }

    /// Creates an enum with the given name and variants.
    pub fn create_enum(&mut self, name: &str, variants: &[&str]) {
        let mut enum_def = format!("enum {} {{\n", name);
        for variant in variants {
            enum_def.push_str(&format!("    {},\n", variant));
        }
        enum_def.push_str("};\n");
        self.code.push_str(&enum_def);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_variable() {
        let mut cbuilder = CBuilder::new("main");
        cbuilder.create_variable("int", "x");
        assert_eq!(cbuilder.code, "int x;\n");
    }

    #[test]
    fn test_create_variable_with_value() {
        let mut cbuilder = CBuilder::new("main");
        cbuilder.create_variable_with_value("int", "x", "42");
        assert_eq!(cbuilder.code, "int x = 42;\n");
    }

    #[test]
    fn test_create_struct() {
        let mut cbuilder = CBuilder::new("main");
        cbuilder.create_struct("Point", &[("x", "int"), ("y", "int")]);
        assert_eq!(
            cbuilder.code,
            "struct Point {\n    int x;\n    int y;\n};\n"
        );
    }

    #[test]
    fn test_create_enum() {
        let mut cbuilder = CBuilder::new("main");
        cbuilder.create_enum("Color", &["Red", "Green", "Blue"]);
        assert_eq!(
            cbuilder.code,
            "enum Color {\n    Red,\n    Green,\n    Blue,\n};\n"
        );
    }
}
