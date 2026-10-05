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
pub struct CImport {
    pub root: Graph,
    pub nested_graphs: BTreeMap<String, Graph>,
}

impl CImport {
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
pub enum CImportError {
    Language(tree_sitter::LanguageError),
    ParseReturnedNone,
    Core(GraphError),
}

impl fmt::Display for CImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Language(error) => write!(f, "failed to load C grammar: {error}"),
            Self::ParseReturnedNone => write!(f, "Tree-sitter returned no parse tree"),
            Self::Core(error) => write!(f, "derived Algoram graph is invalid: {error}"),
        }
    }
}

impl Error for CImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Language(error) => Some(error),
            Self::Core(error) => Some(error),
            Self::ParseReturnedNone => None,
        }
    }
}

impl From<tree_sitter::LanguageError> for CImportError {
    fn from(value: tree_sitter::LanguageError) -> Self {
        Self::Language(value)
    }
}

impl From<GraphError> for CImportError {
    fn from(value: GraphError) -> Self {
        Self::Core(value)
    }
}

pub fn import_c(
    artifact_id: impl Into<String>,
    origin: impl Into<String>,
    source: &str,
) -> Result<CImport, CImportError> {
    let artifact_id = artifact_id.into();
    let origin = origin.into();

    let mut parser = Parser::new();
    let language = tree_sitter_c::LANGUAGE;
    parser.set_language(&language.into())?;
    let tree = parser
        .parse(source, None)
        .ok_or(CImportError::ParseReturnedNone)?;

    let artifact = SourceArtifact {
        id: artifact_id.clone(),
        origin: origin.clone(),
        language: "c".to_owned(),
        revision: None,
        content_hash: None,
        repository_origin: None,
        license: None,
    };

    let root_node = tree.root_node();
    let root_graph_id = format!("graph:c:{artifact_id}:document");
    let translation_graph_id = format!("graph:c:{artifact_id}:translation-unit");
    let file_block_id = format!("c:{artifact_id}:file");

    let mut builder = Builder {
        source,
        artifact_id: &artifact_id,
        artifact: artifact.clone(),
        nested_graphs: BTreeMap::new(),
    };

    let translation_graph = builder.build_scope(
        &translation_graph_id,
        Some(format!("C translation unit: {origin}")),
        &file_block_id,
        named_children(root_node),
    )?;
    builder
        .nested_graphs
        .insert(translation_graph_id.clone(), translation_graph);

    let mut root = graph_with_artifact(
        root_graph_id,
        Some(format!("C source document: {origin}")),
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
    file_block.internal_graph_ref = Some(translation_graph_id);

    if root_node.has_error() {
        root.diagnostics.push(Diagnostic {
            code: "c.parse_incomplete".to_owned(),
            message:
                "Tree-sitter reported one or more syntax-error regions; useful structure was preserved where possible."
                    .to_owned(),
            source_anchor: Some(anchor_for(root_node, &artifact_id)),
        });
    }

    root.blocks.push(file_block);
    root.validate()?;

    let imported = CImport {
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
    ) -> Result<Graph, CImportError> {
        let mut graph = graph_with_artifact(graph_id.to_owned(), label, &self.artifact);
        let mut ids = IdAllocator::default();
        let mut previous_flow_block: Option<String> = None;

        for node in nodes {
            if node.kind() == "comment" {
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
    ) -> Result<Option<Block>, CImportError> {
        match node.kind() {
            "function_definition" => self.import_function(node, parent_identity, ids).map(Some),
            "declaration" => Ok(Some(self.import_declaration(node, parent_identity, ids))),
            "return_statement" => Ok(Some(self.import_return(node, parent_identity, ids))),
            "expression_statement" => Ok(Some(self.import_expression_statement(
                node,
                parent_identity,
                ids,
            ))),
            "call_expression" => Ok(Some(self.import_call(node, parent_identity, ids))),
            "if_statement" if node.child_by_field_name("alternative").is_none() => {
                self.import_if(node, parent_identity, ids).map(Some)
            }
            "comment" => Ok(None),
            _ => Ok(Some(self.import_opaque(node, parent_identity, ids))),
        }
    }

    fn import_function<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, CImportError> {
        let declarator = node.child_by_field_name("declarator");
        let function_declarator = declarator.and_then(|value| {
            if value.kind() == "function_declarator" {
                Some(value)
            } else {
                find_first_kind(value, "function_declarator")
            }
        });

        let name = function_declarator
            .and_then(|value| value.child_by_field_name("declarator"))
            .and_then(|value| first_identifier_text(value, self.source))
            .or_else(|| declarator.and_then(|value| first_identifier_text(value, self.source)))
            .unwrap_or("<anonymous>");

        let return_type = child_text(node, "type", self.source).unwrap_or("unknown");
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
            "function_definition",
            json!({
                "kind": "function_definition",
                "name": name,
                "return_type": return_type
            }),
        );
        block.internal_graph_ref = Some(graph_id.clone());

        let parameter_nodes = function_declarator
            .and_then(|value| value.child_by_field_name("parameters"))
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|parameter| parameter.kind() == "parameter_declaration")
            .collect::<Vec<_>>();

        for parameter in &parameter_nodes {
            let parameter_type = child_text(*parameter, "type", self.source).unwrap_or("unknown");
            let parameter_name = parameter
                .child_by_field_name("declarator")
                .and_then(|value| first_identifier_text(value, self.source));

            if parameter_name.is_none() && parameter_type.trim() == "void" {
                continue;
            }

            let port_name = parameter_name.unwrap_or("<unnamed>");
            block.ports.push(Port {
                id: format!("param:{}", stable_fragment(port_name)),
                direction: PortDirection::In,
                channel: PortChannel::Data,
                contract: Some(json!(format!(
                    "c:{}",
                    stable_fragment(parameter_type.trim())
                ))),
                extensions: Extensions::new(),
            });
        }

        block.ports.push(Port {
            id: "return".to_owned(),
            direction: PortDirection::Out,
            channel: PortChannel::Data,
            contract: Some(json!(format!("c:{}", stable_fragment(return_type.trim())))),
            extensions: Extensions::new(),
        });

        let mut nested = graph_with_artifact(
            graph_id.clone(),
            Some(format!("C function: {name}")),
            &self.artifact,
        );

        let mut parameter_ids = IdAllocator::default();
        for parameter in parameter_nodes {
            let parameter_type = child_text(parameter, "type", self.source).unwrap_or("unknown");
            let parameter_name = parameter
                .child_by_field_name("declarator")
                .and_then(|value| first_identifier_text(value, self.source));

            if parameter_name.is_none() && parameter_type.trim() == "void" {
                continue;
            }

            let display_name = parameter_name.unwrap_or("<unnamed>");
            let id = parameter_ids.allocate(
                &block_id,
                &format!("parameter:{}", stable_fragment(display_name)),
            );
            nested.blocks.push(block_for_node(
                id,
                format!("parameter {display_name}"),
                parameter,
                self.artifact_id,
                "parameter",
                json!({
                    "kind": "parameter",
                    "name": display_name,
                    "type": parameter_type
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

    fn import_declaration<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        let data_type = child_text(node, "type", self.source).unwrap_or("unknown");
        let declarator = node.child_by_field_name("declarator");
        let name = declarator.and_then(|value| first_identifier_text(value, self.source));
        let declarator_text = declarator.map(|value| node_text(value, self.source));

        let base = name
            .map(|value| format!("declaration:{}", stable_fragment(value)))
            .unwrap_or_else(|| format!("declaration:{}", stable_fragment(data_type)));

        let mut block = block_for_node(
            ids.allocate(parent_identity, &base),
            name.map(|value| format!("declare {value}"))
                .unwrap_or_else(|| "declaration".to_owned()),
            node,
            self.artifact_id,
            "declaration",
            json!({
                "kind": "declaration",
                "name": name,
                "type": data_type,
                "declarator": declarator_text
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
        let mut metadata = json!({"kind": "return_statement"});
        if let Some(value) = value {
            metadata["value_kind"] = json!(value.kind());
            metadata["value_text"] = json!(node_text(value, self.source));
        }

        let mut block = block_for_node(
            ids.allocate(parent_identity, "return"),
            compact_label("return", value.map(|item| node_text(item, self.source))),
            node,
            self.artifact_id,
            "return_statement",
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
    ) -> Result<Block, CImportError> {
        let block_id = ids.allocate(parent_identity, "if");
        let graph_id = format!("graph:{block_id}");
        let condition = child_text(node, "condition", self.source).unwrap_or("<condition>");

        let mut block = block_for_node(
            block_id.clone(),
            format!("if {condition}"),
            node,
            self.artifact_id,
            "if_statement",
            json!({
                "kind": "if_statement",
                "condition": condition
            }),
        );
        add_flow_ports(&mut block);
        block.internal_graph_ref = Some(graph_id.clone());

        let nested = if let Some(consequence) = node.child_by_field_name("consequence") {
            let children = if consequence.kind() == "compound_statement" {
                named_children(consequence)
            } else {
                vec![consequence]
            };
            self.build_scope(
                &graph_id,
                Some(format!("If body: {condition}")),
                &block_id,
                children,
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
                code: "c.parse_region_incomplete".to_owned(),
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
    c_metadata: Value,
) -> Block {
    let mut extensions = Extensions::new();
    extensions.insert(
        "c".to_owned(),
        json!({
            "semantic_kind": semantic_kind,
            "syntax": c_metadata
        }),
    );

    let mut diagnostics = Vec::new();
    if node.is_error() || node.has_error() {
        diagnostics.push(Diagnostic {
            code: "c.parse_region_incomplete".to_owned(),
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

fn first_identifier_text<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    if node.kind() == "identifier" || node.kind() == "field_identifier" {
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

fn is_structural_opaque(kind: &str) -> bool {
    kind.starts_with("preproc_")
        || matches!(
            kind,
            "type_definition"
                | "struct_specifier"
                | "union_specifier"
                | "enum_specifier"
                | "linkage_specification"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c_syntax(block: &Block) -> &Value {
        &block.extensions["c"]["syntax"]
    }

    fn kind(block: &Block) -> Option<&str> {
        c_syntax(block)["kind"].as_str()
    }

    fn all_blocks(imported: &CImport) -> Vec<&Block> {
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

    fn function_named<'a>(imported: &'a CImport, name: &str) -> &'a Block {
        all_blocks(imported)
            .into_iter()
            .find(|block| {
                kind(block) == Some("function_definition") && c_syntax(block)["name"] == json!(name)
            })
            .unwrap_or_else(|| panic!("missing function {name}"))
    }

    #[test]
    fn imports_c_function_hierarchy_and_supported_constructs() {
        let source = r#"
#include <stdio.h>

int double_value(int x) {
    int y = x;
    puts("ok");
    if (y) {
        return y * 2;
    }
    return 0;
}
"#;

        let imported = import_c("artifact:c-basic", "basic.c", source).unwrap();
        imported.validate().unwrap();

        let function = function_named(&imported, "double_value");
        let function_graph = imported
            .graph(function.internal_graph_ref.as_deref().unwrap())
            .unwrap();

        assert_eq!(
            function
                .ports
                .iter()
                .filter(|port| port.direction == PortDirection::In)
                .count(),
            1
        );
        assert!(function_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("parameter")));
        assert!(function_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("declaration")));
        assert!(function_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("call_expression")));

        let if_block = function_graph
            .blocks
            .iter()
            .find(|block| kind(block) == Some("if_statement"))
            .unwrap();
        let if_graph = imported
            .graph(if_block.internal_graph_ref.as_deref().unwrap())
            .unwrap();
        assert!(if_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("return_statement")));

        assert!(function_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("return_statement")));
    }

    #[test]
    fn every_imported_block_is_source_anchored_and_graphs_validate() {
        let source = "int double_value(int x) { int y = x; return y * 2; }\n";
        let imported = import_c("artifact:c-anchors", "anchors.c", source).unwrap();

        for block in all_blocks(&imported) {
            let anchor = block
                .source_anchor
                .as_ref()
                .unwrap_or_else(|| panic!("missing anchor for {}", block.id));
            assert_eq!(anchor.artifact_id, "artifact:c-anchors");
            assert!(anchor.end_byte >= anchor.start_byte);
        }

        imported.validate().unwrap();
    }

    #[test]
    fn named_function_ids_survive_leading_comments_and_blank_lines() {
        let base = "int f(int x) { return x; }\n";
        let shifted = "/* documentation */\n\nint f(int x) { return x; }\n";

        let first = import_c("artifact:c-stable", "stable.c", base).unwrap();
        let second = import_c("artifact:c-stable", "stable.c", shifted).unwrap();

        assert_eq!(
            function_named(&first, "f").id,
            function_named(&second, "f").id
        );
    }

    #[test]
    fn preprocessor_construct_is_preserved_as_opaque_source_block() {
        let source = "#include <stdio.h>\nint f(void) { return 1; }\n";
        let imported = import_c("artifact:c-opaque", "opaque.c", source).unwrap();
        let blocks = all_blocks(&imported);

        let include = blocks
            .into_iter()
            .find(|block| kind(block) == Some("preproc_include"))
            .unwrap();
        assert_eq!(c_syntax(include)["opaque"], json!(true));
        assert!(include.source_anchor.is_some());
        assert!(!has_flow_ports(include));
    }

    #[test]
    fn incomplete_source_keeps_useful_structure_and_reports_diagnostic() {
        let source = "int ok(int x) { return x; }\nint broken(\n";
        let imported = import_c("artifact:c-broken", "broken.c", source).unwrap();

        assert!(!imported.root.diagnostics.is_empty());
        assert_eq!(function_named(&imported, "ok").label, "fn ok");
    }

    #[test]
    fn import_only_path_does_not_require_compilation_or_execution() {
        let source = r#"
int system(const char *);
int dangerous(void) {
    system("this string is parsed, not executed");
    return 0;
}
"#;
        let imported = import_c("artifact:c-no-exec", "no_exec.c", source).unwrap();
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
