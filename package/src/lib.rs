use algoram_composite::{CompositeDefinition, CompositeError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

pub const PACKAGE_SCHEMA_VERSION: &str = "algoram.block-package/0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageRef {
    pub package_id: String,
    pub package_version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencySummary {
    pub implementation_refs: Vec<String>,
    pub definition_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockPackage {
    pub schema_version: String,
    pub package_id: String,
    pub package_version: String,
    #[serde(default)]
    pub provenance: PackageProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<PackageRef>,
    pub definition: CompositeDefinition,
}

impl BlockPackage {
    pub fn new(
        package_id: impl Into<String>,
        package_version: impl Into<String>,
        definition: CompositeDefinition,
    ) -> Self {
        Self {
            schema_version: PACKAGE_SCHEMA_VERSION.to_owned(),
            package_id: package_id.into(),
            package_version: package_version.into(),
            provenance: PackageProvenance::default(),
            derived_from: None,
            definition,
        }
    }

    pub fn with_provenance(mut self, provenance: PackageProvenance) -> Self {
        self.provenance = provenance;
        self
    }

    pub fn with_derived_from(mut self, derived_from: PackageRef) -> Self {
        self.derived_from = Some(derived_from);
        self
    }

    pub fn validate(&self) -> Result<(), PackageError> {
        if self.schema_version != PACKAGE_SCHEMA_VERSION {
            return Err(PackageError::UnsupportedSchemaVersion {
                found: self.schema_version.clone(),
            });
        }
        if self.package_id.trim().is_empty() {
            return Err(PackageError::EmptyPackageId);
        }
        if self.package_version.trim().is_empty() {
            return Err(PackageError::EmptyPackageVersion);
        }

        self.definition
            .validate()
            .map_err(PackageError::Composite)?;

        Ok(())
    }

    pub fn to_json_pretty(&self) -> Result<String, PackageError> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(input: &str) -> Result<Self, PackageError> {
        let package: Self = serde_json::from_str(input)?;
        package.validate()?;
        Ok(package)
    }

    pub fn dependency_summary(&self) -> PackageDependencySummary {
        let mut implementation_refs = BTreeSet::new();
        let mut definition_refs = BTreeSet::new();

        for block in &self.definition.internal_graph.blocks {
            if let Some(implementation_ref) = &block.implementation_ref {
                implementation_refs.insert(implementation_ref.clone());
            }
            if let Some(definition_ref) = &block.definition_ref {
                if definition_ref != &self.definition.id {
                    definition_refs.insert(definition_ref.clone());
                }
            }
        }

        PackageDependencySummary {
            implementation_refs: implementation_refs.into_iter().collect(),
            definition_refs: definition_refs.into_iter().collect(),
        }
    }
}

#[derive(Debug)]
pub enum PackageError {
    Json(serde_json::Error),
    Composite(CompositeError),
    UnsupportedSchemaVersion { found: String },
    EmptyPackageId,
    EmptyPackageVersion,
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid package JSON: {error}"),
            Self::Composite(error) => write!(f, "invalid packaged CompositeDefinition: {error}"),
            Self::UnsupportedSchemaVersion { found } => write!(
                f,
                "unsupported package schema_version '{found}', expected '{PACKAGE_SCHEMA_VERSION}'"
            ),
            Self::EmptyPackageId => write!(f, "package_id must not be empty"),
            Self::EmptyPackageVersion => write!(f, "package_version must not be empty"),
        }
    }
}

impl Error for PackageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Composite(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for PackageError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoram_core::{Block, Extensions, Graph, SourceAnchor, SourceArtifact};
    use serde_json::json;

    fn source_artifact() -> SourceArtifact {
        SourceArtifact {
            id: "artifact:package-source".to_owned(),
            origin: "src/example.py".to_owned(),
            language: "python".to_owned(),
            revision: Some("abc123".to_owned()),
            content_hash: Some("sha256:0123456789".to_owned()),
            repository_origin: Some("example/repository".to_owned()),
            license: Some("MIT".to_owned()),
        }
    }

    fn block(id: &str) -> Block {
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

    fn fixture_definition() -> CompositeDefinition {
        let mut graph = Graph::new("graph:package-fixture");
        graph.source_artifacts.push(source_artifact());
        graph
            .extensions
            .insert("future.graph".to_owned(), json!({"preserve": true}));

        let mut executable = block("block:executable");
        executable.implementation_ref = Some("impl:zeta".to_owned());
        executable.source_anchor = Some(SourceAnchor {
            artifact_id: "artifact:package-source".to_owned(),
            start_byte: 10,
            end_byte: 42,
            start_line: Some(1),
            start_column: Some(0),
            end_line: Some(3),
            end_column: Some(1),
            semantic_key: Some("python:example.fn".to_owned()),
        });

        let mut duplicate_impl = block("block:duplicate-impl");
        duplicate_impl.implementation_ref = Some("impl:zeta".to_owned());

        let mut other_impl = block("block:other-impl");
        other_impl.implementation_ref = Some("impl:alpha".to_owned());

        let mut external_definition = block("block:external-definition");
        external_definition.definition_ref = Some("definition:external".to_owned());

        let mut own_definition = block("block:own-definition");
        own_definition.definition_ref = Some("definition:package-fixture".to_owned());

        graph.blocks.extend([
            executable,
            duplicate_impl,
            other_impl,
            external_definition,
            own_definition,
        ]);

        CompositeDefinition::new(
            "definition:package-fixture",
            "Package fixture",
            graph,
            Vec::new(),
            Vec::new(),
        )
    }

    fn fixture_package() -> BlockPackage {
        BlockPackage::new("example.package", "0.1.0", fixture_definition()).with_provenance(
            PackageProvenance {
                origin: Some("https://example.invalid/package".to_owned()),
                revision: Some("pkg-rev-1".to_owned()),
                license: Some("Apache-2.0".to_owned()),
            },
        )
    }

    #[test]
    fn package_round_trip_preserves_definition_extensions_and_provenance() {
        let package = fixture_package();
        package.validate().unwrap();

        let json = package.to_json_pretty().unwrap();
        let restored = BlockPackage::from_json(&json).unwrap();

        assert_eq!(restored, package);
        assert_eq!(restored.provenance.license.as_deref(), Some("Apache-2.0"));

        let artifact = &restored.definition.internal_graph.source_artifacts[0];
        assert_eq!(artifact.origin, "src/example.py");
        assert_eq!(artifact.revision.as_deref(), Some("abc123"));
        assert_eq!(artifact.content_hash.as_deref(), Some("sha256:0123456789"));
        assert_eq!(
            artifact.repository_origin.as_deref(),
            Some("example/repository")
        );
        assert_eq!(artifact.license.as_deref(), Some("MIT"));

        let anchored = restored
            .definition
            .internal_graph
            .blocks
            .iter()
            .find(|block| block.id == "block:executable")
            .unwrap();
        assert_eq!(
            anchored
                .source_anchor
                .as_ref()
                .and_then(|anchor| anchor.semantic_key.as_deref()),
            Some("python:example.fn")
        );
        assert_eq!(
            restored.definition.internal_graph.extensions["future.graph"],
            json!({"preserve": true})
        );
    }

    #[test]
    fn dependency_summary_is_derived_sorted_unique_and_excludes_own_definition() {
        let package = fixture_package();
        let summary = package.dependency_summary();

        assert_eq!(
            summary.implementation_refs,
            vec!["impl:alpha".to_owned(), "impl:zeta".to_owned()]
        );
        assert_eq!(
            summary.definition_refs,
            vec!["definition:external".to_owned()]
        );
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let mut package = fixture_package();
        package.schema_version = "algoram.block-package/99".to_owned();

        assert!(matches!(
            package.validate(),
            Err(PackageError::UnsupportedSchemaVersion { .. })
        ));
    }

    #[test]
    fn empty_package_identity_is_rejected() {
        let mut empty_id = fixture_package();
        empty_id.package_id = "   ".to_owned();
        assert!(matches!(
            empty_id.validate(),
            Err(PackageError::EmptyPackageId)
        ));

        let mut empty_version = fixture_package();
        empty_version.package_version = String::new();
        assert!(matches!(
            empty_version.validate(),
            Err(PackageError::EmptyPackageVersion)
        ));
    }

    #[test]
    fn invalid_composite_definition_is_rejected() {
        let mut package = fixture_package();
        package
            .definition
            .internal_graph
            .blocks
            .push(block("block:executable"));

        assert!(matches!(
            package.validate(),
            Err(PackageError::Composite(_))
        ));
    }

    #[test]
    fn derived_from_round_trip_is_supported_without_affecting_dependency_summary() {
        let package = fixture_package().with_derived_from(PackageRef {
            package_id: "upstream.package".to_owned(),
            package_version: "1".to_owned(),
        });

        let restored = BlockPackage::from_json(&package.to_json_pretty().unwrap()).unwrap();
        assert_eq!(restored, package);
        assert_eq!(restored.dependency_summary(), package.dependency_summary());
    }
}
