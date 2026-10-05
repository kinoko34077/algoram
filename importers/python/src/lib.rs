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
pub struct PythonImport {
    pub root: Graph,
    pub nested_graphs: BTreeMap<String, Graph>,
}

impl PythonImport {
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
pub enum PythonImportError {
    Language(tree_sitter::LanguageError),
    ParseReturnedNone,
    Core(GraphError),
}

impl fmt::Display for PythonImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Language(error) => write!(f, "failed to load Python grammar: {error}"),
            Self::ParseReturnedNone => write!(f, "Tree-sitter returned no parse tree"),
            Self::Core(error) => write!(f, "derived Algoram graph is invalid: {error}"),
        }
    }
}

impl Error for PythonImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Language(error) => Some(error),
            Self::Core(error) => Some(error),
            Self::ParseReturnedNone => None,
        }
    }
}

impl From<tree_sitter::LanguageError> for PythonImportError {
    fn from(value: tree_sitter::LanguageError) -> Self {
        Self::Language(value)
    }
}

impl From<GraphError> for PythonImportError {
    fn from(value: GraphError) -> Self {
        Self::Core(value)
    }
}

pub fn import_python(
    artifact_id: impl Into<String>,
    origin: impl Into<String>,
    source: &str,
) -> Result<PythonImport, PythonImportError> {
    let artifact_id = artifact_id.into();
    let origin = origin.into();

    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE;
    parser.set_language(&language.into())?;
    let tree = parser
        .parse(source, None)
        .ok_or(PythonImportError::ParseReturnedNone)?;

    let artifact = SourceArtifact {
        id: artifact_id.clone(),
        origin: origin.clone(),
        language: "python".to_owned(),
        revision: None,
        content_hash: None,
        repository_origin: None,
        license: None,
    };

    let root_node = tree.root_node();
    let root_graph_id = format!("graph:python:{artifact_id}:document");
    let module_graph_id = format!("graph:python:{artifact_id}:module");
    let file_block_id = format!("python:{artifact_id}:file");

    let mut builder = Builder {
        source,
        artifact_id: &artifact_id,
        artifact: artifact.clone(),
        nested_graphs: BTreeMap::new(),
    };

    let module_graph = builder.build_scope(
        &module_graph_id,
        Some(format!("Python module: {origin}")),
        &file_block_id,
        named_children(root_node),
    )?;
    builder
        .nested_graphs
        .insert(module_graph_id.clone(), module_graph);

    let mut root = graph_with_artifact(
        root_graph_id,
        Some(format!("Python source document: {origin}")),
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
            code: "python.parse_incomplete".to_owned(),
            message: "Tree-sitter reported one or more syntax-error regions; useful structure was preserved where possible.".to_owned(),
            source_anchor: Some(anchor_for(root_node, &artifact_id)),
        });
    }
    root.blocks.push(file_block);
    root.validate()?;

    let imported = PythonImport {
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
    ) -> Result<Graph, PythonImportError> {
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
    ) -> Result<Option<Block>, PythonImportError> {
        match node.kind() {
            "function_definition" => self.import_function(node, parent_identity, ids).map(Some),
            "class_definition" => self.import_class(node, parent_identity, ids).map(Some),
            "if_statement" => self.import_if(node, parent_identity, ids).map(Some),
            "return_statement" => Ok(Some(self.import_return(node, parent_identity, ids))),
            "expression_statement" => Ok(Some(self.import_expression_statement(
                node,
                parent_identity,
                ids,
            ))),
            "assignment" => Ok(Some(self.import_assignment(node, parent_identity, ids))),
            "call" => Ok(Some(self.import_call(node, parent_identity, ids))),
            kind if is_value_kind(kind) => Ok(Some(self.import_value(node, parent_identity, ids))),
            "comment" => Ok(None),
            _ => Ok(Some(self.import_opaque(node, parent_identity, ids))),
        }
    }

    fn import_function<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, PythonImportError> {
        let name = child_text(node, "name", self.source).unwrap_or("<anonymous>");
        let block_id = ids.allocate(parent_identity, &format!("function:{}", stable_fragment(name)));
        let graph_id = format!("graph:{block_id}");

        let mut block = block_for_node(
            block_id.clone(),
            format!("fn {name}"),
            node,
            self.artifact_id,
            "function_definition",
            json!({"kind": "function_definition", "name": name}),
        );
        block.internal_graph_ref = Some(graph_id.clone());

        let parameter_nodes = node
            .child_by_field_name("parameters")
            .map(named_children)
            .unwrap_or_default();
        for parameter in &parameter_nodes {
            if let Some(parameter_name) = first_identifier_text(*parameter, self.source) {
                block.ports.push(Port {
                    id: format!("param:{}", stable_fragment(parameter_name)),
                    direction: PortDirection::In,
                    channel: PortChannel::Data,
                    contract: Some(json!("python:unknown")),
                    extensions: Extensions::new(),
                });
            }
        }
        block.ports.push(Port {
            id: "return".to_owned(),
            direction: PortDirection::Out,
            channel: PortChannel::Data,
            contract: Some(json!("python:unknown")),
            extensions: Extensions::new(),
        });

        let mut nested = graph_with_artifact(
            graph_id.clone(),
            Some(format!("Python function: {name}")),
            &self.artifact,
        );

        let mut parameter_ids = IdAllocator::default();
        for parameter in parameter_nodes {
            let parameter_name =
                first_identifier_text(parameter, self.source).unwrap_or("<parameter>");
            let id = parameter_ids.allocate(
                &block_id,
                &format!("parameter:{}", stable_fragment(parameter_name)),
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

    fn import_class<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, PythonImportError> {
        let name = child_text(node, "name", self.source).unwrap_or("<anonymous>");
        let block_id = ids.allocate(parent_identity, &format!("class:{}", stable_fragment(name)));
        let graph_id = format!("graph:{block_id}");

        let mut block = block_for_node(
            block_id.clone(),
            format!("class {name}"),
            node,
            self.artifact_id,
            "class_definition",
            json!({"kind": "class_definition", "name": name}),
        );
        block.internal_graph_ref = Some(graph_id.clone());

        let nested = if let Some(body) = node.child_by_field_name("body") {
            self.build_scope(
                &graph_id,
                Some(format!("Python class: {name}")),
                &block_id,
                named_children(body),
            )?
        } else {
            graph_with_artifact(
                graph_id.clone(),
                Some(format!("Python class: {name}")),
                &self.artifact,
            )
        };
        self.nested_graphs.insert(graph_id, nested);
        Ok(block)
    }

    fn import_if<'tree>(
        &mut self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Result<Block, PythonImportError> {
        let block_id = ids.allocate(parent_identity, "if");
        let graph_id = format!("graph:{block_id}");
        let condition = child_text(node, "condition", self.source).unwrap_or("<condition>");

        let mut block = block_for_node(
            block_id.clone(),
            format!("if {condition}"),
            node,
            self.artifact_id,
            "if_statement",
            json!({"kind": "if_statement", "condition": condition}),
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
            compact_label("return", value.map(|value| node_text(value, self.source))),
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
            Some(child) if child.kind() == "assignment" => {
                self.import_assignment(child, parent_identity, ids)
            }
            Some(child) if child.kind() == "call" => self.import_call(child, parent_identity, ids),
            Some(child) if is_value_kind(child.kind()) => {
                self.import_value(child, parent_identity, ids)
            }
            _ => self.import_opaque(node, parent_identity, ids),
        }
    }

    fn import_assignment<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        let left = child_text(node, "left", self.source).unwrap_or("<target>");
        let right = node.child_by_field_name("right");
        let mut metadata = json!({
            "kind": "assignment",
            "target": left
        });
        if let Some(right) = right {
            metadata["value_kind"] = json!(right.kind());
            metadata["value_text"] = json!(node_text(right, self.source));
        }
        let mut block = block_for_node(
            ids.allocate(
                parent_identity,
                &format!("assignment:{}", stable_fragment(left)),
            ),
            compact_label("assign", Some(left)),
            node,
            self.artifact_id,
            "assignment",
            metadata,
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
            "call",
            json!({
                "kind": "call",
                "function": function,
                "arguments": arguments
            }),
        );
        add_flow_ports(&mut block);
        block
    }

    fn import_value<'tree>(
        &self,
        node: Node<'tree>,
        parent_identity: &str,
        ids: &mut IdAllocator,
    ) -> Block {
        let text = node_text(node, self.source);
        let mut block = block_for_node(
            ids.allocate(parent_identity, &format!("value:{}", node.kind())),
            compact_label("value", Some(text)),
            node,
            self.artifact_id,
            "value",
            json!({
                "kind": "value",
                "syntax_kind": node.kind(),
                "text": text
            }),
        );
        add_flow_ports(&mut block);
        block
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
                "opaque": true
            }),
        );
        if node.is_error() || node.has_error() {
            block.diagnostics.push(Diagnostic {
                code: "python.parse_region_incomplete".to_owned(),
                message: format!(
                    "Tree-sitter reported incomplete syntax in source-backed '{}' region.",
                    node.kind()
                ),
                source_anchor: Some(anchor_for(node, self.artifact_id)),
            });
        }
        add_flow_ports(&mut block);
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
    python_metadata: Value,
) -> Block {
    let mut extensions = Extensions::new();
    extensions.insert(
        "python".to_owned(),
        json!({
            "semantic_kind": semantic_kind,
            "syntax": python_metadata
        }),
    );

    let mut diagnostics = Vec::new();
    if node.is_error() || node.has_error() {
        diagnostics.push(Diagnostic {
            code: "python.parse_region_incomplete".to_owned(),
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

fn child_text<'a>(node: Node<'_>, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field)
        .map(|child| node_text(child, source))
}

fn first_identifier_text<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    if node.kind() == "identifier" {
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

fn is_value_kind(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "integer"
            | "float"
            | "string"
            | "concatenated_string"
            | "true"
            | "false"
            | "none"
            | "list"
            | "tuple"
            | "dictionary"
            | "set"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn python_syntax(block: &Block) -> &Value {
        &block.extensions["python"]["syntax"]
    }

    fn kind(block: &Block) -> Option<&str> {
        python_syntax(block)["kind"].as_str()
    }

    fn all_blocks(imported: &PythonImport) -> Vec<&Block> {
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

    #[test]
    fn imports_python_hierarchy_and_supported_constructs() {
        let source = r#"
class Greeter:
    def hello(self, name):
        prefix = "hi"
        print(prefix)
        if name:
            return name
"#;
        let imported = import_python("artifact:test", "fixture.py", source).unwrap();
        imported.validate().unwrap();

        let module = imported
            .nested_graphs
            .get("graph:python:artifact:test:module")
            .unwrap();
        let class = module
            .blocks
            .iter()
            .find(|block| kind(block) == Some("class_definition"))
            .unwrap();
        let class_graph = imported.graph(class.internal_graph_ref.as_deref().unwrap()).unwrap();
        let function = class_graph
            .blocks
            .iter()
            .find(|block| kind(block) == Some("function_definition"))
            .unwrap();
        let function_graph = imported
            .graph(function.internal_graph_ref.as_deref().unwrap())
            .unwrap();

        assert_eq!(
            function
                .ports
                .iter()
                .filter(|port| port.direction == PortDirection::In)
                .count(),
            2
        );
        assert!(function_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("parameter")));
        assert!(function_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("assignment")));
        assert!(function_graph
            .blocks
            .iter()
            .any(|block| kind(block) == Some("call")));

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
    }

    #[test]
    fn every_imported_block_is_source_anchored_and_graphs_validate() {
        let source = "def double(x):\n    y = x\n    return y\n";
        let imported = import_python("artifact:anchors", "anchors.py", source).unwrap();

        for block in all_blocks(&imported) {
            let anchor = block
                .source_anchor
                .as_ref()
                .unwrap_or_else(|| panic!("missing anchor for {}", block.id));
            assert_eq!(anchor.artifact_id, "artifact:anchors");
            assert!(anchor.end_byte >= anchor.start_byte);
        }
        imported.validate().unwrap();
    }

    #[test]
    fn named_ids_survive_leading_comments_and_blank_lines() {
        let base = "class A:\n    def f(self):\n        x = 1\n        return x\n";
        let shifted =
            "# inserted documentation\n\nclass A:\n    def f(self):\n        x = 1\n        return x\n";

        let first = import_python("artifact:stable", "stable.py", base).unwrap();
        let second = import_python("artifact:stable", "stable.py", shifted).unwrap();

        let relevant = |imported: &PythonImport| {
            all_blocks(imported)
                .into_iter()
                .filter(|block| {
                    matches!(
                        kind(block),
                        Some("class_definition" | "function_definition" | "assignment")
                    )
                })
                .map(|block| block.id.clone())
                .collect::<Vec<_>>()
        };

        assert_eq!(relevant(&first), relevant(&second));
    }

    #[test]
    fn unsupported_construct_is_preserved_as_opaque_source_block() {
        let source = "for item in [1, 2]:\n    print(item)\n";
        let imported = import_python("artifact:opaque", "opaque.py", source).unwrap();
        let blocks = all_blocks(&imported);

        let opaque = blocks
            .into_iter()
            .find(|block| kind(block) == Some("for_statement"))
            .unwrap();
        assert_eq!(python_syntax(opaque)["opaque"], json!(true));
        assert!(opaque.source_anchor.is_some());
    }

    #[test]
    fn incomplete_source_keeps_useful_structure_and_reports_diagnostic() {
        let source = "def ok(x):\n    return x\n\ndef broken(\n";
        let imported = import_python("artifact:broken", "broken.py", source).unwrap();

        assert!(!imported.root.diagnostics.is_empty());
        assert!(all_blocks(&imported)
            .into_iter()
            .any(|block| kind(block) == Some("function_definition")
                && python_syntax(block)["name"] == json!("ok")));
    }

    #[test]
    fn assignment_call_literal_and_if_metadata_remain_inspectable_without_execution() {
        let source = r#"
danger = open("/definitely-not-opened-by-importer")
answer = 42
if answer:
    print(answer)
"#;
        let imported = import_python("artifact:no-exec", "no_exec.py", source).unwrap();
        let module = imported
            .nested_graphs
            .get("graph:python:artifact:no-exec:module")
            .unwrap();

        let assignments: Vec<_> = module
            .blocks
            .iter()
            .filter(|block| kind(block) == Some("assignment"))
            .collect();
        assert_eq!(assignments.len(), 2);
        assert!(assignments
            .iter()
            .any(|block| python_syntax(block)["value_kind"] == json!("call")));
        assert!(assignments
            .iter()
            .any(|block| python_syntax(block)["value_kind"] == json!("integer")));
        assert!(module
            .blocks
            .iter()
            .any(|block| kind(block) == Some("if_statement")));
    }
}
