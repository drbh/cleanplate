use minijinja::machinery;
use minijinja::machinery::ast::Const;
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Core structure to represent template analysis results
#[derive(Debug, Clone)]
pub struct TemplateAnalysis {
    pub external_vars: BTreeSet<String>,
    pub internal_vars: BTreeSet<String>,
    pub loop_vars: HashMap<String, String>,
    pub object_shapes_json: Value,
}

/// Analyzes a template source string and returns structured analysis data
pub fn analyze(
    template_content: &str,
    verbose: bool,
) -> Result<TemplateAnalysis, Box<dyn std::error::Error>> {
    if verbose {
        eprintln!("TEMPLATE ANALYSIS: Starting template analysis with verbose tracing");
    }

    // Parse the template content to get the AST
    let ast = machinery::parse(
        template_content,
        "<string>",
        Default::default(),
        Default::default(),
    )?;

    // Initialize variable tracker
    let mut variable_tracker = VariableTracker::new();
    variable_tracker.verbose = verbose;

    // Collect all variables and track their reads/sets
    collect_variables(&ast, &mut variable_tracker);

    // Convert to neat analysis result
    let analysis = variable_tracker.to_analysis();

    if verbose {
        eprintln!("TEMPLATE ANALYSIS: Completed template analysis with {} external variables, {} internal variables, and {} loop variables",
            analysis.external_vars.len(),
            analysis.internal_vars.len(),
            analysis.loop_vars.len()
        );
    }

    Ok(analysis)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VarAccess {
    Read,
    Set,
    SetAlias(String), // Track when internal vars point to external vars
    LoopVar(String),  // Loop variable with the iterable name
}

struct VariableTracker {
    // Track variable accesses in order
    access_log: Vec<(String, VarAccess)>,

    // Sets of variables categorized
    internal_vars: HashSet<String>,
    external_vars: HashSet<String>,
    loop_vars: HashMap<String, String>, // loop_var -> iterable

    // Track attributes of objects and their hierarchical relationships
    object_attrs: HashMap<String, BTreeSet<String>>,

    // Track aliases of objects
    object_aliases: HashMap<String, String>,

    // Map to track parent-child relationships (variable -> attributes)
    var_hierarchy: HashMap<String, HashSet<String>>,

    // To track first access of each variable
    first_access: HashMap<String, VarAccess>,

    // Flag to enable verbose debug output
    verbose: bool,
}

impl VariableTracker {
    fn new() -> Self {
        Self {
            access_log: Vec::new(),
            internal_vars: HashSet::new(),
            external_vars: HashSet::new(),
            loop_vars: HashMap::new(),
            object_attrs: HashMap::new(),
            object_aliases: HashMap::new(),
            var_hierarchy: HashMap::new(),
            first_access: HashMap::new(),
            verbose: false,
        }
    }

    fn track_access(&mut self, var_name: &str, access: VarAccess) {
        // if len of var_name is 0, return
        if var_name.is_empty() {
            return;
        }

        // Special case for the `loop` keyword
        if var_name.starts_with("loop.") || var_name == "loop" {
            return;
        }

        // TODO: handle other special cases

        // Debug logging when verbose mode is enabled
        if self.verbose {
            let access_desc = match &access {
                VarAccess::Read => "READ".to_string(),
                VarAccess::Set => "SET".to_string(),
                VarAccess::SetAlias(alias) => format!("SET ALIAS to {alias}"),
                VarAccess::LoopVar(iterable) => format!("LOOP VAR from {iterable}"),
            };
            eprintln!("VARIABLE TRACKER: {var_name} => {access_desc}");
        }

        // Add to access log
        self.access_log.push((var_name.to_string(), access.clone()));

        // Process attribute access and build hierarchy
        if let Some(idx) = var_name.rfind('.') {
            let (parent, attr) = var_name.split_at(idx);
            let attr = &attr[1..]; // Remove leading dot

            // Build hierarchical relationship
            self.var_hierarchy
                .entry(parent.to_string())
                .or_default()
                .insert(attr.to_string());

            // Also track base variable and its immediate attribute
            if let Some(base_idx) = parent.find('.') {
                let (base, _) = parent.split_at(base_idx);
                self.var_hierarchy
                    .entry(base.to_string())
                    .or_default()
                    .insert(parent[base_idx + 1..].to_string());
            }

            // If the parent is a loop variable, associate the attribute with the iterable
            if let Some(iterable) = self.loop_vars.get(parent) {
                self.object_attrs
                    .entry(iterable.clone())
                    .or_default()
                    .insert(attr.to_string());
            } else {
                // Track attribute for regular objects too
                self.object_attrs
                    .entry(parent.to_string())
                    .or_default()
                    .insert(attr.to_string());
            }
        }

        // Track first access for classification
        if !self.first_access.contains_key(var_name) {
            self.first_access
                .insert(var_name.to_string(), access.clone());

            // Immediately classify based on first access
            match access {
                VarAccess::Read => {
                    // Only add base variable name to external vars
                    let base_name = var_name.split('.').next().unwrap_or(var_name);

                    let is_a_loop_var = self.loop_vars.contains_key(base_name);
                    if is_a_loop_var {
                        return;
                    }

                    self.external_vars.insert(base_name.to_string());
                }
                VarAccess::Set => {
                    self.internal_vars.insert(var_name.to_string());
                }
                VarAccess::SetAlias(alias) => {
                    self.object_aliases
                        .insert(alias.to_string(), var_name.to_string());
                    self.internal_vars.insert(var_name.to_string());
                }
                VarAccess::LoopVar(iterable) => {
                    self.internal_vars.insert(var_name.to_string());
                    self.loop_vars.insert(var_name.to_string(), iterable);
                }
            }
        }
    }

    fn to_analysis(&self) -> TemplateAnalysis {
        // Convert to BTreeSet for deterministic ordering
        let external_vars = BTreeSet::from_iter(self.external_vars.iter().cloned());
        let internal_vars = BTreeSet::from_iter(self.internal_vars.iter().cloned());

        // Create a TemplateData struct to use with build_nested_object
        let data = TemplateData {
            internal_vars: self.internal_vars.clone(),
            external_vars: self.external_vars.clone(),
            loop_vars: self.loop_vars.clone(),
            object_attrs: self.object_attrs.clone(),
            object_aliases: self.object_aliases.clone(),
        };

        // Build the object shapes JSON representation
        let object_shapes_json = build_nested_object(&data);

        TemplateAnalysis {
            external_vars,
            internal_vars,
            loop_vars: self.loop_vars.clone(),
            object_shapes_json,
        }
    }
}

#[derive(Debug, Clone)]
struct TemplateData {
    #[allow(dead_code)]
    internal_vars: HashSet<String>,
    external_vars: HashSet<String>,
    loop_vars: HashMap<String, String>,
    object_attrs: HashMap<String, BTreeSet<String>>,
    object_aliases: HashMap<String, String>,
}

fn build_nested_object(data: &TemplateData) -> Value {
    let mut result = Map::new();

    // Process all external_vars as top-level keys
    for var in &data.external_vars {
        // Resolve the actual variable name through aliases (possibly multiple levels)
        let resolved_var = resolve_alias_chain(var, &data.object_aliases);

        // Check if this variable (or its resolved alias) is an iterated variable
        let iterated_var = find_iterated_var(&resolved_var, data);

        if let Some(iterated) = iterated_var {
            // This is an iterated variable or aliases to one
            if data.object_attrs.contains_key(&iterated) {
                let item_obj = build_object_from_attrs(&iterated, data);
                result.insert(var.clone(), json!([item_obj]));
            } else {
                result.insert(var.clone(), json!([]));
            }
        } else if data.object_attrs.contains_key(&resolved_var) {
            // This is a non-iterated object
            result.insert(var.clone(), build_object_from_attrs(&resolved_var, data));
        } else {
            // This is a simple value
            result.insert(var.clone(), json!(""));
        }
    }

    Value::Object(result)
}

// Recursively resolves aliases until reaching a non-aliased variable
fn resolve_alias_chain(var: &str, aliases: &HashMap<String, String>) -> String {
    let mut current = var;
    let mut visited = HashSet::new();

    while let Some(alias) = aliases.get(current) {
        if visited.contains(alias) {
            // Detected a cycle, break out
            break;
        }
        visited.insert(alias);
        current = alias;
    }

    current.to_string()
}

// Find if a variable is iterated or aliases to an iterated variable
fn find_iterated_var(var: &str, data: &TemplateData) -> Option<String> {
    // Direct check if this var is being iterated
    if data.loop_vars.values().any(|v| v == var) {
        return Some(var.to_string());
    }

    // Check if this var is an alias of an iterated var
    for iterable in data.loop_vars.values() {
        let resolved_iterable = resolve_alias_chain(iterable, &data.object_aliases);
        if &resolved_iterable == var {
            return Some(resolved_iterable);
        }
    }

    None
}

// Function to build an object from its attributes
fn build_object_from_attrs(obj_key: &str, data: &TemplateData) -> Value {
    let mut obj = Map::new();

    if let Some(attrs) = data.object_attrs.get(obj_key) {
        for attr in attrs {
            // Build the potential nested key
            let nested_key = format!("{obj_key}.{attr}");

            // Find corresponding loop variable
            let corresponding_loop_var = find_corresponding_loop_var(&nested_key, data);

            // Check if there are attributes for either form of the path
            let has_nested_attrs = data.object_attrs.contains_key(&nested_key);

            // Determine which key to use for attributes
            let key_to_use = if has_nested_attrs {
                Some(nested_key.clone())
            } else {
                None
            };

            // Determine if this should be an array
            let should_be_array = corresponding_loop_var.is_some() || attr == "tool_calls";

            if let Some(key) = key_to_use {
                // Has nested attributes
                if should_be_array {
                    let nested_obj = build_object_from_attrs(&key, data);
                    obj.insert(attr.clone(), json!([nested_obj]));
                } else {
                    obj.insert(attr.clone(), build_object_from_attrs(&key, data));
                }
            } else {
                // No nested attributes
                obj.insert(attr.clone(), json!(""));
            }
        }
    }

    Value::Object(obj)
}

// Function to find corresponding loop variable
fn find_corresponding_loop_var<'a>(path: &str, data: &'a TemplateData) -> Option<&'a String> {
    // Direct match in loop_vars values
    for (loop_var, iterable) in &data.loop_vars {
        if iterable == path {
            return Some(loop_var);
        }
    }

    None
}

fn collect_variables(node: &machinery::ast::Stmt, tracker: &mut VariableTracker) {
    match node {
        machinery::ast::Stmt::Template(template) => {
            for child in &template.children {
                collect_variables(child, tracker);
            }
        }
        machinery::ast::Stmt::Block(block) => {
            for child in &block.body {
                collect_variables(child, tracker);
            }
        }
        machinery::ast::Stmt::EmitExpr(expr) => {
            collect_var_reads(&expr.expr, tracker);
        }
        machinery::ast::Stmt::ForLoop(for_loop) => {
            // Track reads in the iterable expression
            collect_var_reads(&for_loop.iter, tracker);

            // Get the loop variable name
            let loop_var = match extract_var_name(&format!("{:?}", for_loop.target)) {
                Some(name) => name,
                None => "loop_var".to_string(), // Fallback
            };

            // Get what we're iterating over
            let iter_expr = get_attribute_path(&for_loop.iter);

            // Track as loop variable
            tracker.track_access(&loop_var, VarAccess::LoopVar(iter_expr));

            // Process the loop body
            for child in &for_loop.body {
                collect_variables(child, tracker);
            }
        }
        machinery::ast::Stmt::IfCond(if_cond) => {
            // Track reads in condition
            collect_var_reads(&if_cond.expr, tracker);

            // Process true body
            for child in &if_cond.true_body {
                collect_variables(child, tracker);
            }
            // Process false body if it exists
            for child in &if_cond.false_body {
                collect_variables(child, tracker);
            }
        }
        machinery::ast::Stmt::WithBlock(with_block) => {
            // Process all assignments
            for (name, expr) in &with_block.assignments {
                // Track reads in the expression
                collect_var_reads(expr, tracker);

                // Track setting of the target
                if let Some(var_name) = extract_var_name(&format!("{name:?}")) {
                    tracker.track_access(&var_name, VarAccess::Set);
                }
            }

            // Process the body
            for child in &with_block.body {
                collect_variables(child, tracker);
            }
        }
        machinery::ast::Stmt::Set(set) => {
            // Track reads in the expression
            collect_var_reads(&set.expr, tracker);

            // Track setting of the target
            if let Some(var_name) = extract_var_name(&format!("{:?}", set.target)) {
                match &set.expr {
                    machinery::ast::Expr::Var(var) => {
                        tracker.track_access(&var_name, VarAccess::SetAlias(var.id.to_string()));
                    }
                    _ => {
                        tracker.track_access(&var_name, VarAccess::Set);
                    }
                }
            }
        }
        machinery::ast::Stmt::SetBlock(set_block) => {
            // Track setting of the target
            if let Some(var_name) = extract_var_name(&format!("{:?}", set_block.target)) {
                tracker.track_access(&var_name, VarAccess::Set);
            }

            // Process the body
            for child in &set_block.body {
                collect_variables(child, tracker);
            }
        }
        machinery::ast::Stmt::AutoEscape(auto_escape) => {
            for child in &auto_escape.body {
                collect_variables(child, tracker);
            }
        }
        machinery::ast::Stmt::FilterBlock(filter_block) => {
            // Track reads in filter
            collect_var_reads(&filter_block.filter, tracker);

            // Process the body
            for child in &filter_block.body {
                collect_variables(child, tracker);
            }
        }
        _ => {}
    }
}

// Track variable reads in expressions
fn collect_var_reads(expr: &machinery::ast::Expr, tracker: &mut VariableTracker) {
    match expr {
        machinery::ast::Expr::Var(var) => {
            // Track variable read
            tracker.track_access(var.id, VarAccess::Read);
        }
        machinery::ast::Expr::GetAttr(get_attr) => {
            // Get the full attribute path
            let attr_path = get_attribute_path(expr);

            // Track read of the full path
            tracker.track_access(&attr_path, VarAccess::Read);

            // Also track read of base expression (needed for attribute tracking)
            collect_var_reads(&get_attr.expr, tracker);
        }
        machinery::ast::Expr::GetItem(get_item) => {
            let access_in_get = {
                let mut left = String::new();

                // First check if we have a variable expression
                let has_var = match &get_item.expr {
                    machinery::ast::Expr::Var(var) => {
                        left.push_str(var.id);
                        left.push('.');
                        true
                    }
                    _ => false, // Skip if not a variable
                };

                // Only continue if we found a variable
                if has_var {
                    match &get_item.subscript_expr {
                        machinery::ast::Expr::Const(constant) => {
                            let Const { value } = &**constant;
                            if value.is_number() {
                                None
                            } else {
                                left.push_str(&format!("{value}"));
                                Some(left)
                            }
                        }
                        _ => None, // Skip if not a constant
                    }
                } else {
                    None
                }
            };

            if let Some(access_in_get) = access_in_get {
                // Track read of the full path
                tracker.track_access(&access_in_get, VarAccess::Read);
            }

            collect_var_reads(&get_item.expr, tracker);
            collect_var_reads(&get_item.subscript_expr, tracker);
        }
        machinery::ast::Expr::Call(call) => {
            collect_var_reads(&call.expr, tracker);

            // Process call arguments
            for arg in &call.args {
                // Use extract_vars_from_debug_str instead of direct call to handle CallArg type
                let arg_str = format!("{arg:?}");
                extract_vars_from_debug_str(&arg_str, tracker);
            }
        }
        machinery::ast::Expr::Filter(filter) => {
            if let Some(expr) = &filter.expr {
                collect_var_reads(expr, tracker);
            }

            // Process filter arguments
            for arg in &filter.args {
                // Use extract_vars_from_debug_str instead of direct call to handle CallArg type
                let arg_str = format!("{arg:?}");
                extract_vars_from_debug_str(&arg_str, tracker);
            }
        }
        machinery::ast::Expr::Test(test) => {
            collect_var_reads(&test.expr, tracker);

            // Process test arguments
            for arg in &test.args {
                // Use extract_vars_from_debug_str instead of direct call to handle CallArg type
                let arg_str = format!("{arg:?}");
                extract_vars_from_debug_str(&arg_str, tracker);
            }
        }
        machinery::ast::Expr::BinOp(bin_op) => {
            collect_var_reads(&bin_op.left, tracker);
            collect_var_reads(&bin_op.right, tracker);
        }
        machinery::ast::Expr::UnaryOp(unary_op) => {
            collect_var_reads(&unary_op.expr, tracker);
        }
        machinery::ast::Expr::List(list) => {
            for item in &list.items {
                collect_var_reads(item, tracker);
            }
        }
        machinery::ast::Expr::Map(map) => {
            for key in &map.keys {
                collect_var_reads(key, tracker);
            }
            for value in &map.values {
                collect_var_reads(value, tracker);
            }
        }
        machinery::ast::Expr::Const(_) => {}
        _ => {}
    }
}

// Helper function to recursively build the full attribute path
fn get_attribute_path(expr: &machinery::ast::Expr) -> String {
    match expr {
        machinery::ast::Expr::Var(var) => var.id.to_string(),
        machinery::ast::Expr::GetAttr(get_attr) => {
            let base_path = get_attribute_path(&get_attr.expr);
            if !base_path.is_empty() {
                format!("{}.{}", base_path, get_attr.name)
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

// Helper to extract a clean variable name from a debug string
fn extract_var_name(debug_str: &str) -> Option<String> {
    if let Some(start) = debug_str.find("id: \"") {
        if let Some(end) = debug_str[start + 5..].find('\"') {
            return Some(debug_str[start + 5..start + 5 + end].to_string());
        }
    }
    None
}

// Extract variable reads from debug strings
fn extract_vars_from_debug_str(debug_str: &str, tracker: &mut VariableTracker) {
    // Try to extract variable names from debug output
    if let Some(var_name) = extract_var_name(debug_str) {
        tracker.track_access(&var_name, VarAccess::Read);
    }

    // Try to extract attribute paths
    if debug_str.contains("GetAttr") {
        let mut path_parts = Vec::new();

        // Find base variable
        if let Some(var_start) = debug_str.find("id: \"") {
            if let Some(var_end) = debug_str[var_start + 5..].find('\"') {
                let var_name = &debug_str[var_start + 5..var_start + 5 + var_end];
                path_parts.push(var_name.to_string());

                // Find all attributes
                let mut pos = var_start + 5 + var_end;
                while let Some(attr_start) = debug_str[pos..].find("name: \"") {
                    pos += attr_start + 7;
                    if let Some(attr_end) = debug_str[pos..].find('\"') {
                        let attr_name = &debug_str[pos..pos + attr_end];
                        path_parts.push(attr_name.to_string());
                        pos += attr_end;
                    } else {
                        break;
                    }
                }

                // Build and add each level of the path
                if !path_parts.is_empty() {
                    let mut full_path = path_parts[0].clone();
                    tracker.track_access(&full_path, VarAccess::Read);

                    for i in 1..path_parts.len() {
                        full_path = format!("{}.{}", full_path, path_parts[i]);
                        tracker.track_access(&full_path, VarAccess::Read);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_variable_detection() {
        let template = "{{ user.name }}";
        let analysis = analyze(template, false).unwrap();
        assert!(analysis.external_vars.contains("user"));
    }

    #[test]
    fn test_internal_variable_detection() {
        let template = "{% set title = 'Hello' %}{{ title }}";
        let analysis = analyze(template, false).unwrap();
        assert!(analysis.internal_vars.contains("title"));
        assert!(!analysis.external_vars.contains("title"));
    }

    #[test]
    fn test_loop_variable_detection() {
        let template = "{% for item in items %}{{ item.name }}{% endfor %}";
        let analysis = analyze(template, false).unwrap();
        assert!(analysis.loop_vars.contains_key("item"));
        assert_eq!(analysis.loop_vars.get("item").unwrap(), "items");
        assert!(analysis.external_vars.contains("items"));
    }

    #[test]
    fn test_nested_object_shapes() {
        let template = "{% for item in items %}{{ item.name }}{% endfor %}";
        // need to ensure that name is in the object shapes
        let analysis = analyze(template, false).unwrap();
        let object_shapes = analysis.object_shapes_json.as_object().unwrap();
        assert!(object_shapes.contains_key("items"));
        assert!(!object_shapes["items"].as_array().unwrap().is_empty());
        assert!(object_shapes["items"][0]
            .as_object()
            .unwrap()
            .contains_key("name"));
    }
}
