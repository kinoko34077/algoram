use algoram_core::{
    Block, Connection, Diagnostic, Extensions, Graph, GraphError, Port, PortChannel, PortDirection,
    PortRef, SourceAnchor, SourceArtifact,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, PartialEq)]
pub struct RustImport {
    pub root: Graph,
    pub nested_graphs: BTreeMap<String, Graph>,
}

impl RustImport {
    pub fn graph(&self, id: &str) -> Option<&Graph> {
        if self.root.id == id {
            Some(&self.root)
        } else {
            self.nested_graphs.get(id)
        }
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        self.root.validate()?;
        for graph in self.nested_graphs.values() {
            graph.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum RustImportError {
    Language(tree_sitter::LanguageError),
    ParseReturnedNone,
    Core(GraphError),
}

impl fmt::Display for RustImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Language(error) => write!(f, "failed to load Rust grammar: {error}"),
            Self::ParseReturnedNone => write!(f, "Tree-sitter returned no parse tree"),
            Self::Core(error) => write!(f, "derived Algoram graph is invalid: {error}"),
        }
    }
}

impl Error for RustImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Language(error) => Some(error),
            Self::Core(error) => Some(error),
            Self::ParseReturnedNone => None,
        }
    }
}

impl From<tree_sitter::LanguageError> for RustImportError {
    fn from(value: tree_sitter::LanguageError) -> Self {
        Self::Language(value)
    }
}

impl From<GraphError> for RustImportError {
    fn from(value: GraphError) -> Self {
        Self::Core(value)
    }
}

pub fn import_rust(
    artifact_id: impl Into<String>,
    origin: impl Into<String>,
    source: &str,
) -> Result<RustImport, RustImportError> {
    let artifact_id = artifact_id.into();
    let origin = origin.into();

    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let tree = parser
        .parse(source, None)
        .ok_or(RustImportError::ParseReturnedNone)?;

    let artifact = SourceArtifact {
        id: artifact_id.clone(),
        origin: origin.clone(),
        language: "rust".to_owned(),
        revision: None,
        content_hash: None,
        repository_origin: None,
        license: None,
    };

    let root_node = tree.root_node();
    let root_graph_id = format!("graph:rust:{artifact_id}:document");
    let module_graph_id = format!("graph:rust:{artifact_id}:module");
    let file_block_id = format!("rust:{artifact_id}:file");

    let mut builder = Builder {
        source,
        artifact_id: &artifact_id,
        artifact: artifact.clone(),
        nested_graphs: BTreeMap::new(),
    };

    let module_graph = builder.build_scope(
        &module_graph_id,
        Some(format!("Rust module: {origin}")),
        &file_block_id,
        named_children(root_node),
    )?;
    builder
        .nested_graphs
        .insert(module_graph_id.clone(), module_graph);

    let mut root = graph_with_artifact(
        root_graph_id,
        Some(format!("Rust source document: {origin}")),
        &artifact,
    );
    let mut file_block = block_for_node(
        file_block_id,
        origin,
        root_node,
        &artifact_id,
        "source_file",
        json!({"kind": "source_file"}),
    );
    file_block.internal_graph_ref = Some(module_graph_id);

    if root_node.has_error() {
        root.diagnostics.push(Diagnostic {
            code: "rust.parse_incomplete".to_owned(),
            message:
                "Tree-sitter reported one or more syntax-error regions; useful structure was preserved where possible."
                    .to_owned(),
            source_anchor: Some(anchor_for(root_node, &artifact_id)),
        });
    }

    root.blocks.push(file_block);
    root.validate()?;

    let imported = RustImport {
        root,
        nested_graphs: builder.nested_graphs,
    };
    imported.validate()?;
    Ok(imported)
}

struct Builder<'a> {
    source: &'a str,
    artifact_id: &'a str,
    artifact: SourceArtifact,
    nested_graphs: BTreeMap<String, Graph>,
}

impl<'a> Builder<'a> {
    fn build_scope<'tree>(
        &mut self,
        graph_id: &str,
        label: Option<String>,
        parent_identity: &str,
        nodes: Vec<Node<'tree>>,
    ) -> Result<Graph, RustImportError> {
        let mut graph = graph_with_artifact(graph_id.to_owned(), label, &self.artifact);
        let mut ids = IdAllocator::default();
        let mut previous_flow_block: Option<String> = None;

        for node in nodes {
            if is_comment(node.kind()) {
                continue;
            }

            if let Some(block) = self.import_node(node, parent_identity, &mut ids)? {
                if has_flow_ports(&block) {
                    if let Some(previous) = &previous_flow_block {
                        graph.connections.push(flow_connection(previous, &block.id));
                    }
                    previous_flow_block = Some(block.id.clone());
                }
                graph.blocks.push(block);
            }
        }

        graph.validate()?;
        Ok(graph)
    }

    fn import_node<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Option<Block>, RustImportError> {
        match node.kind() {
            "function_item" => self.import_function(node, parent_identity, ids).map(Some),
            "struct_item" => self.import_struct(node, parent_identity, ids).map(Some),
            "impl_item" => self.import_impl(node, parent_identity, ids).map(Some),
            "mod_item" => self.import_module(node, parent_identity, ids).map(Some),
            "let_declaration" => Ok(Some(self.import_let(node, parent_identity, ids))),
            "return_expression" => Ok(Some(self.import_return(node, parent_identity, ids))),
            "expression_statement" => Ok(Some(self.import_expression_statement(
                node,
                parent_identity,
                ids,
            ))),
            "call_expression" => Ok(Some(self.import_call(node, parent_identity, ids))),
            "if_expression" if node.child_by_field_name("alternative").is_none() => {
                self.import_if(node, parent_identity, ids).map(Some)
            }
            kind if is_basic_expression(kind) => {
                Ok(Some(self.import_expression(node, parent_identity, ids)))
            }
            kind if is_comment(kind) => Ok(None),
            _ => Ok(Some(self.import_opaque(node, parent_identity, ids))),
        }
    }

    fn import_function<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, RustImportError> {
        let name = child_text(node, "name", self.source).unwrap_or("<anonymous>");
        let return_type = child_text(node, "return_type", self.source).unwrap_or("()");
        let extern_abi = find_first_kind(node, "extern_modifier")
            .map(|modifier| node_text(modifier, self.source).trim().to_owned());

        let block_id = ids.allocate(
            parent_identity,
            &format!("function:{}", stable_fragment(name)),
        );
        let graph_id = format!("graph:{block_id}");

        let mut block = block_for_node(
            block_id.clone(),
            format!("fn {name}"),
            node,
            self.artifact_id,
            "function_item",
            json!({
                "kind": "function_item",
                "name": name,
                "return_type": return_type,
                "extern_abi": extern_abi
            }),
        );
        block.internal_graph_ref = Some(graph_id.clone());

        let parameter_nodes = node
            .child_by_field_name("parameters")
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind(),
                    "parameter" | "self_parameter" | "variadic_parameter"
                )
            })
            .collect::<Vec<_>>();

        for parameter in &parameter_nodes {
            let (parameter_name, parameter_type) = parameter_signature(*parameter, self.source);
            block.ports.push(Port {
                id: format!("param:{}", stable_fragment(&parameter_name)),
                direction: PortDirection::In,
                channel: PortChannel::Data,
                contract: Some(json!(format!(
                    "rust:{}",
                    stable_fragment(parameter_type.trim())
                ))),
                extensions: Extensions::new(),
            });
        }

        block.ports.push(Port {
            id: "return".to_owned(),
            direction: PortDirection::Out,
            channel: PortChannel::Data,
            contract: Some(json!(format!(
                "rust:{}",
                stable_fragment(return_type.trim())
            ))),
            extensions: Extensions::new(),
        });

        let mut nested = graph_with_artifact(
            graph_id.clone(),
            Some(format!("Rust function: {name}")),
            &self.artifact,
        );

        let mut parameter_ids = IdAllocator::default();
        for parameter in parameter_nodes {
            let (parameter_name, parameter_type) = parameter_signature(parameter, self.source);
            let id = parameter_ids.allocate(
                &block_id,
                &format!("parameter:{}", stable_fragment(&parameter_name)),
            );
            nested.blocks.push(block_for_node(
                id,
                format!("parameter {parameter_name}"),
                parameter,
                self.artifact_id,
                "parameter",
                json!({
                    "kind": "parameter",
                    "name": parameter_name,
                    "type": parameter_type,
                    "syntax_kind": parameter.kind()
                }),
            ));
        }

        if let Some(body) = node.child_by_field_name("body") {
            let body_graph = self.build_scope(
                &format!("{graph_id}:body"),
                Some(format!("Body of {name}")),
                &block_id,
                named_children(body),
            )?;
            append_graph_contents(&mut nested, body_graph);
        }

        nested.validate()?;
        self.nested_graphs.insert(graph_id, nested);
        Ok(block)
    }

    fn import_struct<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, RustImportError> {
        let name = child_text(node, "name", self.source).unwrap_or("<anonymous>");
        let block_id = ids.allocate(parent_identity, &format!("struct:{}", stable_fragment(name)));
        let graph_id = format!("graph:{block_id}");

        let mut block = block_for_node(
            block_id.clone(),
            format!("struct {name}"),
            node,
            self.artifact_id,
            "struct_item",
            json!({"kind": "struct_item", "name": name}),
        );

        if let Some(body) = node.child_by_field_name("body") {
            block.internal_graph_ref = Some(graph_id.clone());
            let nested = self.build_scope(
                &graph_id,
                Some(format!("Rust struct: {name}")),
                &block_id,
                named_children(body),
            )?;
            self.nested_graphs.insert(graph_id, nested);
        }

        Ok(block)
    }

    fn import_impl<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, RustImportError> {
        let target = child_text(node, "type", self.source).unwrap_or("<type>");
        let trait_name = child_text(node, "trait", self.source);
        let identity = match trait_name {
            Some(trait_name) => format!(
                "impl:{}:for:{}",
                stable_fragment(trait_name),
                stable_fragment(target)
            ),
            None => format!("impl:{}", stable_fragment(target)),
        };
        let block_id = ids.allocate(parent_identity, &identity);
        let graph_id = format!("graph:{block_id}");
        let label = match trait_name {
            Some(trait_name) => format!("impl {trait_name} for {target}"),
            None => format!("impl {target}"),
        };

        let mut block = block_for_node(
            block_id.clone(),
            label.clone(),
            node,
            self.artifact_id,
            "impl_item",
            json!({
                "kind": "impl_item",
                "type": target,
                "trait": trait_name
            }),
        );

        if let Some(body) = node.child_by_field_name("body") {
            block.internal_graph_ref = Some(graph_id.clone());
            let nested = self.build_scope(
                &graph_id,
                Some(label),
                &block_id,
                named_children(body),
            )?;
            self.nested_graphs.insert(graph_id, nested);
        }

        Ok(block)
    }

    fn import_module<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, RustImportError> {
        let name = child_text(node, "name", self.source).unwrap_or("<anonymous>");
        let block_id = ids.allocate(parent_identity, &format!("module:{}", stable_fragment(name)));
        let graph_id = format!("graph:{block_id}");

        let mut block = block_for_node(
            block_id.clone(),
            format!("mod {name}"),
            node,
            self.artifact_id,
            "mod_item",
            json!({"kind": "mod_item", "name": name}),
        );

        if let Some(body) = node.child_by_field_name("body") {
            block.internal_graph_ref = Some(graph_id.clone());
            let nested = self.build_scope(
                &graph_id,
                Some(format!("Rust module: {name}")),
                &block_id,
                named_children(body),
            )?;
            self.nested_graphs.insert(graph_id, nested);
        }

        Ok(block)
    }

    fn import_let<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        let pattern = child_text(node, "pattern", self.source).unwrap_or("<pattern>");
        let data_type = child_text(node, "type", self.source);
        let value = child_text(node, "value", self.source);

        let mut block = block_for_node(
            ids.allocate(
                parent_identity,
                &format!("let:{}", stable_fragment(pattern)),
            ),
            format!("let {pattern}"),
            node,
            self.artifact_id,
            "let_declaration",
            json!({
                "kind": "let_declaration",
                "pattern": pattern,
                "type": data_type,
                "value": value
            }),
        );
        add_flow_ports(&mut block);
        block
    }

    fn import_return<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        let value = first_named_child(node);
        let mut metadata = json!({"kind": "return_expression"});
        if let Some(value) = value {
            metadata["value_kind"] = json!(value.kind());
            metadata["value_text"] = json!(node_text(value, self.source));
        }

        let mut block = block_for_node(
            ids.allocate(parent_identity, "return"),
            compact_label("return", value.map(|item| node_text(item, self.source))),
            node,
            self.artifact_id,
            "return_expression",
            metadata,
        );
        add_flow_ports(&mut block);
        block
    }

    fn import_expression_statement<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        match first_named_child(node) {
            Some(child) if child.kind() == "call_expression" => {
                self.import_call(child, parent_identity, ids)
            }
            Some(child) => self.import_expression(child, parent_identity, ids),
            None => self.import_opaque(node, parent_identity, ids),
        }
    }

    fn import_expression<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        let text = node_text(node, self.source);
        let mut block = block_for_node(
            ids.allocate(parent_identity, &format!("expression:{}", node.kind())),
            compact_label("expr", Some(text)),
            node,
            self.artifact_id,
            "expression",
            json!({
                "kind": "expression",
                "syntax_kind": node.kind(),
                "text": text
            }),
        );
        add_flow_ports(&mut block);
        block
    }

    fn import_call<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        let function = child_text(node, "function", self.source).unwrap_or("<callable>");
        let arguments = child_text(node, "arguments", self.source).unwrap_or("()");
        let mut block = block_for_node(
            ids.allocate(
                parent_identity,
                &format!("call:{}", stable_fragment(function)),
            ),
            format!("call {function}{arguments}"),
            node,
            self.artifact_id,
            "call_expression",
            json!({
                "kind": "call_expression",
                "function": function,
                "arguments": arguments
            }),
        );
        add_flow_ports(&mut block);
        block
    }

    fn import_if<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, RustImportError> {
        let block_id = ids.allocate(parent_identity, "if");
        let graph_id = format!("graph:{block_id}");
        let condition = child_text(node, "condition", self.source).unwrap_or("<condition>");

        let mut block = block_for_node(
            block_id.clone(),
            format!("if {condition}"),
            node,
            self.artifact_id,
            "if_expression",
            json!({
                "kind": "if_expression",
                "condition": condition
            }),
        );
        add_flow_ports(&mut block);
        block.internal_graph_ref = Some(graph_id.clone());

        let nested = if let Some(consequence) = node.child_by_field_name("consequence") {
            self.build_scope(
                &graph_id,
                Some(format!("If body: {condition}")),
                &block_id,
                named_children(consequence),
            )?
        } else {
            graph_with_artifact(
                graph_id.clone(),
                Some(format!("If body: {condition}")),
                &self.artifact,
            )
        };

        self.nested_graphs.insert(graph_id, nested);
        Ok(block)
    }

    fn import_opaque<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        let mut block = block_for_node(
            ids.allocate(parent_identity, &format!("opaque:{}", node.kind())),
            format!("opaque {}", node.kind()),
            node,
            self.artifact_id,
            "opaque",
            json!({
                "kind": node.kind(),
                "opaque": true,
                "text": node_text(node, self.source)
            }),
        );

        if node.is_error() || node.has_error() {
            block.diagnostics.push(Diagnostic {
                code: "rust.parse_region_incomplete".to_owned(),
                message: format!(
                    "Tree-sitter reported incomplete syntax in source-backed '{}' region.",
                    node.kind()
                ),
                source_anchor: Some(anchor_for(node, self.artifact_id)),
            });
        }

        if !is_structural_opaque(node.kind()) {
            add_flow_ports(&mut block);
        }

        block
    }
}

#[derive(Default)]
struct IdAllocator {
    counts: BTreeMap<String, usize>,
}

impl IdAllocator {
    fn allocate(&mut self, parent: &str, base: &str) -> String {
        let stem = format!("{parent}/{base}");
        let count = self.counts.entry(stem.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            stem
        } else {
            format!("{stem}#{}", *count)
        }
    }
}

fn graph_with_artifact(
    id: impl Into<String>,
    label: Option<String>,
    artifact: &SourceArtifact,
) -> Graph {
    let mut graph = Graph::new(id);
    graph.label = label;
    graph.source_artifacts.push(artifact.clone());
    graph
}

fn block_for_node(
    id: String,
    label: impl Into<String>,
    node: Node<'_>,
    artifact_id: &str,
    semantic_kind: &str,
    rust_metadata: Value,
) -> Block {
    let mut extensions = Extensions::new();
    extensions.insert(
        "rust".to_owned(),
        json!({
            "semantic_kind": semantic_kind,
            "syntax": rust_metadata
        }),
    );

    let mut diagnostics = Vec::new();
    if node.is_error() || node.has_error() {
        diagnostics.push(Diagnostic {
            code: "rust.parse_region_incomplete".to_owned(),
            message: format!(
                "Tree-sitter reported incomplete syntax in '{}' source region.",
                node.kind()
            ),
            source_anchor: Some(anchor_for(node, artifact_id)),
        });
    }

    Block {
        id,
        label: label.into(),
        ports: Vec::new(),
        internal_graph_ref: None,
        implementation_ref: None,
        definition_ref: None,
        source_anchor: Some(anchor_for(node, artifact_id)),
        extensions,
        diagnostics,
    }
}

fn anchor_for(node: Node<'_>, artifact_id: &str) -> SourceAnchor {
    let start = node.start_position();
    let end = node.end_position();
    SourceAnchor {
        artifact_id: artifact_id.to_owned(),
        start_byte: node.start_byte() as u64,
        end_byte: node.end_byte() as u64,
        start_line: Some(start.row as u64),
        start_column: Some(start.column as u64),
        end_line: Some(end.row as u64),
        end_column: Some(end.column as u64),
        semantic_key: None,
    }
}

fn add_flow_ports(block: &mut Block) {
    block.ports.push(Port {
        id: "flow_in".to_owned(),
        direction: PortDirection::In,
        channel: PortChannel::Flow,
        contract: None,
        extensions: Extensions::new(),
    });
    block.ports.push(Port {
        id: "flow_out".to_owned(),
        direction: PortDirection::Out,
        channel: PortChannel::Flow,
        contract: None,
        extensions: Extensions::new(),
    });
}

fn has_flow_ports(block: &Block) -> bool {
    block
        .ports
        .iter()
        .any(|port| port.id == "flow_in" && port.channel == PortChannel::Flow)
        && block
            .ports
            .iter()
            .any(|port| port.id == "flow_out" && port.channel == PortChannel::Flow)
}

fn flow_connection(source_block: &str, target_block: &str) -> Connection {
    Connection {
        id: format!("flow:{source_block}->{target_block}"),
        source: PortRef {
            block_id: source_block.to_owned(),
            port_id: "flow_out".to_owned(),
        },
        target: PortRef {
            block_id: target_block.to_owned(),
            port_id: "flow_in".to_owned(),
        },
        extensions: Extensions::new(),
    }
}

fn append_graph_contents(target: &mut Graph, source: Graph) {
    target.blocks.extend(source.blocks);
    target.connections.extend(source.connections);
    target.diagnostics.extend(source.diagnostics);
}

fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).next()
}

fn find_first_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_first_kind(child, kind) {
            return Some(found);
        }
    }
    None
}

fn child_text<'a>(node: Node<'_>, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field)
        .map(|child| node_text(child, source))
}

fn parameter_signature(node: Node<'_>, source: &str) -> (String, String) {
    if node.kind() == "self_parameter" {
        return ("self".to_owned(), node_text(node, source).trim().to_owned());
    }

    let pattern = child_text(node, "pattern", source)
        .or_else(|| first_identifier_text(node, source))
        .unwrap_or("<parameter>");
    let data_type = child_text(node, "type", source).unwrap_or("unknown");

    (pattern.to_owned(), data_type.to_owned())
}

fn first_identifier_text<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    if matches!(
        node.kind(),
        "identifier" | "type_identifier" | "field_identifier" | "self"
    ) {
        return Some(node_text(node, source));
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(identifier) = first_identifier_text(child, source) {
            return Some(identifier);
        }
    }
    None
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn stable_fragment(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
            output.push(character);
        } else {
            output.push('_');
        }
    }

    if output.is_empty() {
        "anonymous".to_owned()
    } else {
        output
    }
}

fn compact_label(prefix: &str, value: Option<&str>) -> String {
    match value {
        Some(value) if !value.trim().is_empty() => {
            let value = value.trim().replace('\n', " ");
            let short = if value.chars().count() > 72 {
                let mut shortened: String = value.chars().take(69).collect();
                shortened.push_str("...");
                shortened
            } else {
                value
            };
            format!("{prefix} {short}")
        }
        _ => prefix.to_owned(),
    }
}

fn is_comment(kind: &str) -> bool {
    matches!(kind, "line_comment" | "block_comment")
}

fn is_basic_expression(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "integer_literal"
            | "float_literal"
            | "string_literal"
            | "char_literal"
            | "boolean_literal"
            | "binary_expression"
            | "unary_expression"
            | "field_expression"
            | "reference_expression"
            | "assignment_expression"
            | "compound_assignment_expr"
            | "unit_expression"
    )
}

fn is_structural_opaque(kind: &str) -> bool {
    matches!(
        kind,
        "field_declaration"
            | "attribute_item"
            | "inner_attribute_item"
            | "use_declaration"
            | "macro_definition"
            | "type_item"
            | "trait_item"
            | "enum_item"
            | "union_item"
            | "foreign_mod_item"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_syntax(block: &Block) -> &Value {
        &block.extensions["rust"]["syntax"]
    }

    fn kind(block: &Block) -> Option<&str> {
        rust_syntax(block)["kind"].as_str()
    }

    fn all_blocks(imported: &RustImport) -> Vec<&Block> {
        imported
            .root
            .blocks
            .iter()
            .chain(
                imported
                    .nested_graphs
                    .values()
                    .flat_map(|graph| graph.blocks.iter()),
            )
            .collect()
    }

    fn function_named<'a>(imported: &'a RustImport, name: &str) -> &'a Block {
        all_blocks(imported)
            .into_iter()
            .find(|block| {
                kind(block) == Some("function_item") && rust_syntax(block)["name"] == json!(name)
            })
            .unwrap_or_else(|| panic!("missing function {name}"))
    }

    #[test]
    fn imports_rust_hierarchy_and_supported_constructs() {
        let source = r#"
struct Counter {
    value: i32,
}

impl Counter {
    fn double(&self, x: i32) -> i32 {
        let y: i32 = helper(x);
        helper(y);
        if y > 0 {
            return y * 2;
        }
        0
    }
}

fn helper(x: i32) -> i32 {
    x
}

extern "C" fn exported(x: i32) -> i32 {
    x * 3
}
"#;

        let imported = import_rust("artifact:rust-basic", "basic.rs", source).unwrap();
        imported.validate().unwrap();

        let blocks = all_blocks(&imported);
        assert!(blocks.iter().any(|block| kind(block) == Some("struct_item")));
        assert!(blocks.iter().any(|block| kind(block) == Some("impl_item")));

        let double = function_named(&imported, "double");
        let double_graph = imported
            .graph(double.internal_graph_ref.as_deref().unwrap())
            .unwrap();

        assert!(double_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("parameter")));
        assert!(double_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("let_declaration")));
        assert!(double_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("call_expression")));

        let if_block = double_graph
            .blocks
            .iter()
            .find(|block| kind(block) == Some("if_expression"))
            .unwrap();
        let if_graph = imported
            .graph(if_block.internal_graph_ref.as_deref().unwrap())
            .unwrap();
        assert!(if_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("return_expression")));

        let exported = function_named(&imported, "exported");
        assert_eq!(
            rust_syntax(exported)["extern_abi"],
            json!("extern \"C\"")
        );
    }

    #[test]
    fn every_imported_block_is_source_anchored_and_graphs_validate() {
        let source = "fn double_value(x: i32) -> i32 { let y = x; y * 2 }\n";
        let imported = import_rust("artifact:rust-anchors", "anchors.rs", source).unwrap();

        for block in all_blocks(&imported) {
            let anchor = block
                .source_anchor
                .as_ref()
                .unwrap_or_else(|| panic!("missing anchor for {}", block.id));
            assert_eq!(anchor.artifact_id, "artifact:rust-anchors");
            assert!(anchor.end_byte >= anchor.start_byte);
        }

        imported.validate().unwrap();
    }

    #[test]
    fn named_ids_survive_leading_comments_and_blank_lines() {
        let base = "struct S;\nfn f(x: i32) -> i32 { x }\n";
        let shifted = "// documentation\n\nstruct S;\nfn f(x: i32) -> i32 { x }\n";

        let first = import_rust("artifact:rust-stable", "stable.rs", base).unwrap();
        let second = import_rust("artifact:rust-stable", "stable.rs", shifted).unwrap();

        assert_eq!(function_named(&first, "f").id, function_named(&second, "f").id);

        let first_struct = all_blocks(&first)
            .into_iter()
            .find(|block| kind(block) == Some("struct_item"))
            .unwrap();
        let second_struct = all_blocks(&second)
            .into_iter()
            .find(|block| kind(block) == Some("struct_item"))
            .unwrap();
        assert_eq!(first_struct.id, second_struct.id);
    }

    #[test]
    fn macro_invocation_is_preserved_as_opaque_source_block() {
        let source = "fn f() { println!(\"hello\"); }\n";
        let imported = import_rust("artifact:rust-opaque", "opaque.rs", source).unwrap();

        let macro_block = all_blocks(&imported)
            .into_iter()
            .find(|block| rust_syntax(block)["kind"] == json!("macro_invocation"))
            .unwrap();

        assert_eq!(rust_syntax(macro_block)["opaque"], json!(true));
        assert!(macro_block.source_anchor.is_some());
    }

    #[test]
    fn incomplete_source_keeps_useful_structure_and_reports_diagnostic() {
        let source = "fn ok(x: i32) -> i32 { x }\nfn broken(\n";
        let imported = import_rust("artifact:rust-broken", "broken.rs", source).unwrap();

        assert!(!imported.root.diagnostics.is_empty());
        assert_eq!(function_named(&imported, "ok").label, "fn ok");
    }

    #[test]
    fn import_only_path_does_not_build_or_execute_rust() {
        let source = r#"
fn dangerous() {
    std::process::Command::new("definitely-not-executed").status();
}
"#;

        let imported = import_rust("artifact:rust-no-exec", "no_exec.rs", source).unwrap();
        let function = function_named(&imported, "dangerous");
        let graph = imported
            .graph(function.internal_graph_ref.as_deref().unwrap())
            .unwrap();

        assert!(graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("call_expression")));
    }
}
