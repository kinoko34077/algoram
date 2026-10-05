use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fmt;

pub const GRAPH_SCHEMA_VERSION: &str = "algoram.graph/0.1";

pub type Extensions = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortDirection {
    In,
    Out,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortChannel {
    Flow,
    Data,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Port {
    pub id: String,
    pub direction: PortDirection,
    pub channel: PortChannel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: Extensions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAnchor {
    pub artifact_id: String,
    pub start_byte: u64,
    pub end_byte: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_column: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceArtifact {
    pub id: String,
    pub origin: String,
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_anchor: Option<SourceAnchor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<Port>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_graph_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_anchor: Option<SourceAnchor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: Extensions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortRef {
    pub block_id: String,
    pub port_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub source: PortRef,
    pub target: PortRef,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: Extensions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub schema_version: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_artifacts: Vec<SourceArtifact>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: Extensions,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub presentation: Extensions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

impl Graph {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            schema_version: GRAPH_SCHEMA_VERSION.to_owned(),
            id: id.into(),
            label: None,
            blocks: Vec::new(),
            connections: Vec::new(),
            source_artifacts: Vec::new(),
            extensions: Extensions::new(),
            presentation: Extensions::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn from_json(input: &str) -> Result<Self, GraphError> {
        let graph: Self = serde_json::from_str(input)?;
        graph.validate()?;
        Ok(graph)
    }

    pub fn to_json_pretty(&self) -> Result<String, GraphError> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        if self.schema_version != GRAPH_SCHEMA_VERSION {
            return Err(GraphError::UnsupportedSchemaVersion {
                found: self.schema_version.clone(),
            });
        }

        let mut artifact_ids = HashSet::new();
        for artifact in &self.source_artifacts {
            if !artifact_ids.insert(artifact.id.as_str()) {
                return Err(GraphError::DuplicateSourceArtifactId(artifact.id.clone()));
            }
        }

        let mut blocks = HashMap::new();
        for block in &self.blocks {
            if blocks.insert(block.id.as_str(), block).is_some() {
                return Err(GraphError::DuplicateBlockId(block.id.clone()));
            }
            let mut port_ids = HashSet::new();
            for port in &block.ports {
                if !port_ids.insert(port.id.as_str()) {
                    return Err(GraphError::DuplicatePortId {
                        block_id: block.id.clone(),
                        port_id: port.id.clone(),
                    });
                }
            }
            if let Some(anchor) = &block.source_anchor {
                validate_anchor(anchor, &artifact_ids, &block.id)?;
            }
        }

        let mut connection_ids = HashSet::new();
        for connection in &self.connections {
            if !connection_ids.insert(connection.id.as_str()) {
                return Err(GraphError::DuplicateConnectionId(connection.id.clone()));
            }

            let source = find_port(&blocks, &connection.source, &connection.id)?;
            let target = find_port(&blocks, &connection.target, &connection.id)?;

            if source.direction != PortDirection::Out || target.direction != PortDirection::In {
                return Err(GraphError::InvalidConnectionDirection {
                    connection_id: connection.id.clone(),
                    source: source.direction.clone(),
                    target: target.direction.clone(),
                });
            }
            if source.channel != target.channel {
                return Err(GraphError::ChannelMismatch {
                    connection_id: connection.id.clone(),
                    source: source.channel.clone(),
                    target: target.channel.clone(),
                });
            }
        }

        Ok(())
    }
}

fn validate_anchor(
    anchor: &SourceAnchor,
    artifact_ids: &HashSet<&str>,
    block_id: &str,
) -> Result<(), GraphError> {
    if anchor.end_byte < anchor.start_byte {
        return Err(GraphError::InvalidSourceAnchorRange {
            block_id: block_id.to_owned(),
            start_byte: anchor.start_byte,
            end_byte: anchor.end_byte,
        });
    }
    if !artifact_ids.contains(anchor.artifact_id.as_str()) {
        return Err(GraphError::MissingSourceArtifact {
            block_id: block_id.to_owned(),
            artifact_id: anchor.artifact_id.clone(),
        });
    }
    Ok(())
}

fn find_port<'a>(
    blocks: &HashMap<&str, &'a Block>,
    reference: &PortRef,
    connection_id: &str,
) -> Result<&'a Port, GraphError> {
    let block = blocks
        .get(reference.block_id.as_str())
        .ok_or_else(|| GraphError::MissingBlock {
            connection_id: connection_id.to_owned(),
            block_id: reference.block_id.clone(),
        })?;
    block
        .ports
        .iter()
        .find(|port| port.id == reference.port_id)
        .ok_or_else(|| GraphError::MissingPort {
            connection_id: connection_id.to_owned(),
            block_id: reference.block_id.clone(),
            port_id: reference.port_id.clone(),
        })
}

#[derive(Debug)]
pub enum GraphError {
    Json(serde_json::Error),
    UnsupportedSchemaVersion {
        found: String,
    },
    DuplicateBlockId(String),
    DuplicatePortId {
        block_id: String,
        port_id: String,
    },
    DuplicateConnectionId(String),
    DuplicateSourceArtifactId(String),
    MissingBlock {
        connection_id: String,
        block_id: String,
    },
    MissingPort {
        connection_id: String,
        block_id: String,
        port_id: String,
    },
    InvalidConnectionDirection {
        connection_id: String,
        source: PortDirection,
        target: PortDirection,
    },
    ChannelMismatch {
        connection_id: String,
        source: PortChannel,
        target: PortChannel,
    },
    InvalidSourceAnchorRange {
        block_id: String,
        start_byte: u64,
        end_byte: u64,
    },
    MissingSourceArtifact {
        block_id: String,
        artifact_id: String,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid graph JSON: {error}"),
            Self::UnsupportedSchemaVersion { found } => {
                write!(f, "unsupported schema_version '{found}', expected '{GRAPH_SCHEMA_VERSION}'")
            }
            Self::DuplicateBlockId(id) => write!(f, "duplicate block id '{id}'"),
            Self::DuplicatePortId { block_id, port_id } => {
                write!(f, "duplicate port id '{port_id}' in block '{block_id}'")
            }
            Self::DuplicateConnectionId(id) => write!(f, "duplicate connection id '{id}'"),
            Self::DuplicateSourceArtifactId(id) => write!(f, "duplicate source artifact id '{id}'"),
            Self::MissingBlock {
                connection_id,
                block_id,
            } => write!(
                f,
                "connection '{connection_id}' references missing block '{block_id}'"
            ),
            Self::MissingPort {
                connection_id,
                block_id,
                port_id,
            } => write!(
                f,
                "connection '{connection_id}' references missing port '{block_id}.{port_id}'"
            ),
            Self::InvalidConnectionDirection {
                connection_id,
                source,
                target,
            } => write!(
                f,
                "connection '{connection_id}' requires out -> in, got {source:?} -> {target:?}"
            ),
            Self::ChannelMismatch {
                connection_id,
                source,
                target,
            } => write!(
                f,
                "connection '{connection_id}' channel mismatch: {source:?} -> {target:?}"
            ),
            Self::InvalidSourceAnchorRange {
                block_id,
                start_byte,
                end_byte,
            } => write!(
                f,
                "block '{block_id}' has invalid source anchor byte range {start_byte}..{end_byte}"
            ),
            Self::MissingSourceArtifact {
                block_id,
                artifact_id,
            } => write!(
                f,
                "block '{block_id}' references missing source artifact '{artifact_id}'"
            ),
        }
    }
}

impl Error for GraphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for GraphError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn port(id: &str, direction: PortDirection, channel: PortChannel) -> Port {
        Port {
            id: id.to_owned(),
            direction,
            channel,
            contract: None,
            extensions: Extensions::new(),
        }
    }

    fn structural_block(id: &str) -> Block {
        Block {
            id: id.to_owned(),
            label: id.to_owned(),
            ports: Vec::new(),
            internal_graph_ref: None,
            implementation_ref: None,
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn structural_non_executable_block_is_valid() {
        let mut graph = Graph::new("graph:structural");
        graph.blocks.push(structural_block("block:file"));
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn json_round_trip_preserves_composite_interface_and_unknown_extensions() {
        let mut graph = Graph::new("graph:root");
        graph.label = Some("Root".to_owned());
        graph.extensions.insert(
            "future.vendor".to_owned(),
            json!({"opaque": [1, 2, 3], "flag": true}),
        );

        let mut block = structural_block("block:composite");
        block.internal_graph_ref = Some("graph:inner".to_owned());
        block.ports.push(port("in", PortDirection::In, PortChannel::Data));
        block.ports.push(port("out", PortDirection::Out, PortChannel::Data));
        block.extensions.insert("future.block".to_owned(), json!({"x": "y"}));
        graph.blocks.push(block);

        let serialized = graph.to_json_pretty().unwrap();
        let restored = Graph::from_json(&serialized).unwrap();
        assert_eq!(restored, graph);
        assert_eq!(
            restored.extensions["future.vendor"],
            json!({"opaque": [1, 2, 3], "flag": true})
        );
    }

    #[test]
    fn source_artifact_and_anchor_round_trip() {
        let mut graph = Graph::new("graph:source");
        graph.source_artifacts.push(SourceArtifact {
            id: "artifact:python".to_owned(),
            origin: "src/example.py".to_owned(),
            language: "python".to_owned(),
            revision: Some("abc123".to_owned()),
            content_hash: Some("sha256:deadbeef".to_owned()),
            repository_origin: Some("example/repo".to_owned()),
            license: Some("MIT".to_owned()),
        });
        let mut block = structural_block("block:function");
        block.source_anchor = Some(SourceAnchor {
            artifact_id: "artifact:python".to_owned(),
            start_byte: 10,
            end_byte: 42,
            start_line: Some(2),
            start_column: Some(0),
            end_line: Some(4),
            end_column: Some(1),
            semantic_key: Some("python:example.fn".to_owned()),
        });
        graph.blocks.push(block);

        let restored = Graph::from_json(&graph.to_json_pretty().unwrap()).unwrap();
        assert_eq!(restored, graph);
    }

    #[test]
    fn valid_out_to_in_connection_passes() {
        let mut graph = Graph::new("graph:connected");
        let mut source = structural_block("a");
        source
            .ports
            .push(port("out", PortDirection::Out, PortChannel::Flow));
        let mut target = structural_block("b");
        target
            .ports
            .push(port("in", PortDirection::In, PortChannel::Flow));
        graph.blocks.extend([source, target]);
        graph.connections.push(Connection {
            id: "connection:a-b".to_owned(),
            source: PortRef {
                block_id: "a".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "b".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });

        assert!(graph.validate().is_ok());
    }

    #[test]
    fn missing_endpoint_fails_deterministically() {
        let mut graph = Graph::new("graph:missing");
        graph.blocks.push(structural_block("a"));
        graph.connections.push(Connection {
            id: "connection:missing".to_owned(),
            source: PortRef {
                block_id: "a".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "missing".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });

        assert!(matches!(
            graph.validate(),
            Err(GraphError::MissingPort { .. }) | Err(GraphError::MissingBlock { .. })
        ));
    }

    #[test]
    fn invalid_direction_fails() {
        for (source_direction, target_direction) in [
            (PortDirection::Out, PortDirection::Out),
            (PortDirection::In, PortDirection::In),
            (PortDirection::In, PortDirection::Out),
        ] {
            let mut graph = Graph::new("graph:direction");
            let mut source = structural_block("a");
            source
                .ports
                .push(port("p", source_direction, PortChannel::Data));
            let mut target = structural_block("b");
            target
                .ports
                .push(port("p", target_direction, PortChannel::Data));
            graph.blocks.extend([source, target]);
            graph.connections.push(Connection {
                id: "connection:direction".to_owned(),
                source: PortRef {
                    block_id: "a".to_owned(),
                    port_id: "p".to_owned(),
                },
                target: PortRef {
                    block_id: "b".to_owned(),
                    port_id: "p".to_owned(),
                },
                extensions: Extensions::new(),
            });
            assert!(matches!(
                graph.validate(),
                Err(GraphError::InvalidConnectionDirection { .. })
            ));
        }
    }

    #[test]
    fn channel_mismatch_fails() {
        let mut graph = Graph::new("graph:channel");
        let mut source = structural_block("a");
        source
            .ports
            .push(port("out", PortDirection::Out, PortChannel::Flow));
        let mut target = structural_block("b");
        target
            .ports
            .push(port("in", PortDirection::In, PortChannel::Data));
        graph.blocks.extend([source, target]);
        graph.connections.push(Connection {
            id: "connection:channel".to_owned(),
            source: PortRef {
                block_id: "a".to_owned(),
                port_id: "out".to_owned(),
            },
            target: PortRef {
                block_id: "b".to_owned(),
                port_id: "in".to_owned(),
            },
            extensions: Extensions::new(),
        });

        assert!(matches!(
            graph.validate(),
            Err(GraphError::ChannelMismatch { .. })
        ));
    }

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let input = r#"{
            "schema_version": "algoram.graph/9.9",
            "id": "graph:future",
            "blocks": [],
            "connections": []
        }"#;

        assert!(matches!(
            Graph::from_json(input),
            Err(GraphError::UnsupportedSchemaVersion { .. })
        ));
    }

    #[test]
    fn source_anchor_requires_known_artifact_and_valid_range() {
        let mut graph = Graph::new("graph:anchor");
        let mut block = structural_block("block:source");
        block.source_anchor = Some(SourceAnchor {
            artifact_id: "artifact:missing".to_owned(),
            start_byte: 20,
            end_byte: 10,
            start_line: None,
            start_column: None,
            end_line: None,
            end_column: None,
            semantic_key: None,
        });
        graph.blocks.push(block);

        assert!(matches!(
            graph.validate(),
            Err(GraphError::InvalidSourceAnchorRange { .. })
        ));

        graph.blocks[0].source_anchor.as_mut().unwrap().end_byte = 30;
        assert!(matches!(
            graph.validate(),
            Err(GraphError::MissingSourceArtifact { .. })
        ));
    }
}
