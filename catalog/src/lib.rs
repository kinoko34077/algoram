use algoram_core::{Port, PortChannel, PortDirection};
use algoram_package::PackageRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CatalogAsset {
    BlockPackage {
        package: PackageRef,
    },
    Implementation {
        implementation_ref: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        logical_implementation_ref: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportedPort {
    pub direction: PortDirection,
    pub channel: PortChannel,
    pub contract: Value,
}

impl SupportedPort {
    pub fn new(direction: PortDirection, channel: PortChannel, contract: Value) -> Self {
        Self {
            direction,
            channel,
            contract,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommercialAvailability {
    Free,
    Paid,
    #[default]
    Unspecified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourceAvailability {
    OpenSource,
    Closed,
    #[default]
    Unspecified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    pub amount_minor: u64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatingSummary {
    pub score_millis: u16,
    pub max_score_millis: u16,
    pub review_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Inspectability {
    Inspectable,
    MetadataOnly,
    #[default]
    Unspecified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MarketplaceMetadata {
    #[serde(default)]
    pub commercial_availability: CommercialAvailability,
    #[serde(default)]
    pub source_availability: SourceAvailability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<RatingSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reputation: Option<String>,
    #[serde(default)]
    pub inspectability: Inspectability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_statement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warranty_statement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityListing {
    pub listing_id: String,
    pub title: String,
    pub supplier: String,
    pub source: String,
    pub version: String,
    pub last_updated: String,
    pub asset: CatalogAsset,
    #[serde(default)]
    pub supported_ports: Vec<SupportedPort>,
    #[serde(default)]
    pub marketplace: MarketplaceMetadata,
}

impl CapabilityListing {
    pub fn validate(&self) -> Result<(), CatalogError> {
        require_nonempty(&self.listing_id, &self.listing_id, "listing_id")?;
        require_nonempty(&self.listing_id, &self.title, "title")?;
        require_nonempty(&self.listing_id, &self.supplier, "supplier")?;
        require_nonempty(&self.listing_id, &self.source, "source")?;
        require_nonempty(&self.listing_id, &self.version, "version")?;
        require_nonempty(&self.listing_id, &self.last_updated, "last_updated")?;

        match &self.asset {
            CatalogAsset::BlockPackage { package } => {
                require_nonempty(&self.listing_id, &package.package_id, "package_id")?;
                require_nonempty(
                    &self.listing_id,
                    &package.package_version,
                    "package_version",
                )?;
            }
            CatalogAsset::Implementation {
                implementation_ref,
                logical_implementation_ref,
            } => {
                require_nonempty(&self.listing_id, implementation_ref, "implementation_ref")?;
                if let Some(logical_ref) = logical_implementation_ref {
                    require_nonempty(&self.listing_id, logical_ref, "logical_implementation_ref")?;
                }
            }
        }

        for (index, port) in self.supported_ports.iter().enumerate() {
            for (other_index, other) in self.supported_ports[..index].iter().enumerate() {
                if port == other {
                    return Err(CatalogError::DuplicateSupportedPort {
                        listing_id: self.listing_id.clone(),
                        first_index: other_index,
                        second_index: index,
                    });
                }
            }
        }

        match self.marketplace.commercial_availability {
            CommercialAvailability::Paid if self.marketplace.price.is_none() => {
                return Err(CatalogError::PaidListingMissingPrice {
                    listing_id: self.listing_id.clone(),
                });
            }
            CommercialAvailability::Free if self.marketplace.price.is_some() => {
                return Err(CatalogError::FreeListingHasPrice {
                    listing_id: self.listing_id.clone(),
                });
            }
            _ => {}
        }

        if let Some(price) = &self.marketplace.price {
            if price.currency.trim().is_empty() {
                return Err(CatalogError::EmptyPriceCurrency {
                    listing_id: self.listing_id.clone(),
                });
            }
        }

        if let Some(rating) = &self.marketplace.rating {
            if rating.max_score_millis == 0 || rating.score_millis > rating.max_score_millis {
                return Err(CatalogError::InvalidRating {
                    listing_id: self.listing_id.clone(),
                    score_millis: rating.score_millis,
                    max_score_millis: rating.max_score_millis,
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CapabilityCatalog {
    #[serde(default)]
    pub listings: Vec<CapabilityListing>,
}

impl CapabilityCatalog {
    pub fn new(listings: impl IntoIterator<Item = CapabilityListing>) -> Self {
        Self {
            listings: listings.into_iter().collect(),
        }
    }

    pub fn validate(&self) -> Result<(), CatalogError> {
        let mut listing_ids = BTreeSet::new();
        for listing in &self.listings {
            listing.validate()?;
            if !listing_ids.insert(listing.listing_id.as_str()) {
                return Err(CatalogError::DuplicateListingId(listing.listing_id.clone()));
            }
        }
        Ok(())
    }

    pub fn to_json_pretty(&self) -> Result<String, CatalogError> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(input: &str) -> Result<Self, CatalogError> {
        let catalog: Self = serde_json::from_str(input)?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn search_logical_implementations(
        &self,
        logical_implementation_ref: &str,
    ) -> Result<Vec<&CapabilityListing>, CatalogError> {
        self.validate()?;

        let mut results = self
            .listings
            .iter()
            .filter(|listing| {
                matches!(
                    &listing.asset,
                    CatalogAsset::Implementation {
                        logical_implementation_ref: Some(logical_ref),
                        ..
                    } if logical_ref == logical_implementation_ref
                )
            })
            .collect::<Vec<_>>();

        results.sort_by(|left, right| left.listing_id.cmp(&right.listing_id));
        Ok(results)
    }

    pub fn search_counterparts(
        &self,
        current_port: &Port,
    ) -> Result<Vec<&CapabilityListing>, CatalogError> {
        self.validate()?;

        let Some(contract) = current_port.contract.as_ref() else {
            return Ok(Vec::new());
        };
        let counterpart_direction = match current_port.direction {
            PortDirection::In => PortDirection::Out,
            PortDirection::Out => PortDirection::In,
        };

        let mut results = self
            .listings
            .iter()
            .filter(|listing| {
                listing.supported_ports.iter().any(|candidate| {
                    candidate.direction == counterpart_direction
                        && candidate.channel == current_port.channel
                        && candidate.contract == *contract
                })
            })
            .collect::<Vec<_>>();

        results.sort_by(|left, right| left.listing_id.cmp(&right.listing_id));
        Ok(results)
    }
}

fn require_nonempty(
    listing_id: &str,
    value: &str,
    field: &'static str,
) -> Result<(), CatalogError> {
    if value.trim().is_empty() {
        return Err(CatalogError::EmptyField {
            listing_id: listing_id.to_owned(),
            field,
        });
    }
    Ok(())
}

#[derive(Debug)]
pub enum CatalogError {
    Json(serde_json::Error),
    EmptyField {
        listing_id: String,
        field: &'static str,
    },
    DuplicateListingId(String),
    DuplicateSupportedPort {
        listing_id: String,
        first_index: usize,
        second_index: usize,
    },
    PaidListingMissingPrice {
        listing_id: String,
    },
    FreeListingHasPrice {
        listing_id: String,
    },
    EmptyPriceCurrency {
        listing_id: String,
    },
    InvalidRating {
        listing_id: String,
        score_millis: u16,
        max_score_millis: u16,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid catalog JSON: {error}"),
            Self::EmptyField { listing_id, field } => {
                write!(f, "listing '{listing_id}' field '{field}' must not be empty")
            }
            Self::DuplicateListingId(listing_id) => {
                write!(f, "duplicate catalog listing_id '{listing_id}'")
            }
            Self::DuplicateSupportedPort {
                listing_id,
                first_index,
                second_index,
            } => write!(
                f,
                "listing '{listing_id}' repeats the same supported Port surface at indexes {first_index} and {second_index}"
            ),
            Self::PaidListingMissingPrice { listing_id } => {
                write!(f, "paid listing '{listing_id}' must declare a price")
            }
            Self::FreeListingHasPrice { listing_id } => {
                write!(f, "free listing '{listing_id}' must not declare a price")
            }
            Self::EmptyPriceCurrency { listing_id } => {
                write!(f, "listing '{listing_id}' price currency must not be empty")
            }
            Self::InvalidRating {
                listing_id,
                score_millis,
                max_score_millis,
            } => write!(
                f,
                "listing '{listing_id}' rating {score_millis}/{max_score_millis} is invalid"
            ),
        }
    }
}

impl Error for CatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for CatalogError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoram_core::{Block, Connection, Extensions, Graph, PortRef};
    use algoram_interop::RouteRegistry;
    use algoram_runtime::{ExecutionPlan, ImplementationRegistry, Planner, ProcessAction, ProcessRuntime};
    use serde_json::json;

    fn current_output_port() -> Port {
        Port {
            id: "out".to_owned(),
            direction: PortDirection::Out,
            channel: PortChannel::Data,
            contract: Some(json!("text:utf8")),
            extensions: Default::default(),
        }
    }

    fn implementation_listing(id: &str, direction: PortDirection) -> CapabilityListing {
        CapabilityListing {
            listing_id: id.to_owned(),
            title: format!("Listing {id}"),
            supplier: "supplier-a".to_owned(),
            source: "https://example.invalid/catalog".to_owned(),
            version: "1.0.0".to_owned(),
            last_updated: "2026-10-06".to_owned(),
            asset: CatalogAsset::Implementation {
                implementation_ref: format!("impl:{id}"),
                logical_implementation_ref: Some("logical:text-transform".to_owned()),
            },
            supported_ports: vec![SupportedPort::new(
                direction,
                PortChannel::Data,
                json!("text:utf8"),
            )],
            marketplace: MarketplaceMetadata {
                commercial_availability: CommercialAvailability::Paid,
                source_availability: SourceAvailability::Closed,
                price: Some(Price {
                    amount_minor: 499,
                    currency: "JPY".to_owned(),
                }),
                rating: Some(RatingSummary {
                    score_millis: 4200,
                    max_score_millis: 5000,
                    review_count: 12,
                }),
                reputation: Some("verified supplier history".to_owned()),
                inspectability: Inspectability::MetadataOnly,
                support_statement: Some("email support".to_owned()),
                warranty_statement: None,
            },
        }
    }

    fn package_listing(id: &str) -> CapabilityListing {
        CapabilityListing {
            listing_id: id.to_owned(),
            title: "Open text package".to_owned(),
            supplier: "supplier-oss".to_owned(),
            source: "https://example.invalid/open-text".to_owned(),
            version: "2.0.0".to_owned(),
            last_updated: "2026-10-05".to_owned(),
            asset: CatalogAsset::BlockPackage {
                package: PackageRef {
                    package_id: "package:text-open".to_owned(),
                    package_version: "2.0.0".to_owned(),
                },
            },
            supported_ports: vec![SupportedPort::new(
                PortDirection::In,
                PortChannel::Data,
                json!("text:utf8"),
            )],
            marketplace: MarketplaceMetadata {
                commercial_availability: CommercialAvailability::Free,
                source_availability: SourceAvailability::OpenSource,
                price: None,
                rating: None,
                reputation: None,
                inspectability: Inspectability::Inspectable,
                support_statement: None,
                warranty_statement: Some("no warranty".to_owned()),
            },
        }
    }

    #[test]
    fn catalog_round_trips_marketplace_metadata_and_asset_refs() {
        let catalog = CapabilityCatalog::new([
            implementation_listing("listing:paid", PortDirection::In),
            package_listing("listing:package"),
        ]);

        let json = catalog.to_json_pretty().unwrap();
        let restored = CapabilityCatalog::from_json(&json).unwrap();

        assert_eq!(restored, catalog);
        assert!(json.contains("logical:text-transform"));
        assert!(json.contains("package:text-open"));
        assert!(json.contains("\"amount_minor\": 499"));
        assert!(json.contains("\"warranty_statement\": \"no warranty\""));
        assert!(!json.contains("\"program\""));
        assert!(!json.contains("\"args\""));
    }

    #[test]
    fn exact_contextual_search_finds_only_counterpart_ports_in_stable_order() {
        let matching_z = implementation_listing("z-listing", PortDirection::In);
        let matching_a = package_listing("a-listing");
        let wrong_direction = implementation_listing("wrong-direction", PortDirection::Out);

        let mut wrong_contract = implementation_listing("wrong-contract", PortDirection::In);
        wrong_contract.supported_ports[0].contract = json!("bytes");

        let mut wrong_channel = implementation_listing("wrong-channel", PortDirection::In);
        wrong_channel.supported_ports[0].channel = PortChannel::Flow;

        let catalog = CapabilityCatalog::new([
            matching_z,
            wrong_direction,
            wrong_contract,
            matching_a,
            wrong_channel,
        ]);
        let before = catalog.clone();

        let results = catalog.search_counterparts(&current_output_port()).unwrap();

        assert_eq!(
            results
                .iter()
                .map(|listing| listing.listing_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a-listing", "z-listing"]
        );
        assert_eq!(catalog, before);
    }

    #[test]
    fn port_without_contract_has_no_contextual_results() {
        let mut port = current_output_port();
        port.contract = None;
        let catalog = CapabilityCatalog::new([package_listing("listing:package")]);

        assert!(catalog.search_counterparts(&port).unwrap().is_empty());
    }

    #[test]
    fn validation_rejects_duplicate_listing_and_supported_port_identity() {
        let listing = package_listing("same");
        let duplicate_catalog = CapabilityCatalog::new([listing.clone(), listing]);
        assert!(matches!(
            duplicate_catalog.validate(),
            Err(CatalogError::DuplicateListingId(id)) if id == "same"
        ));

        let mut duplicate_port = package_listing("duplicate-port");
        duplicate_port
            .supported_ports
            .push(duplicate_port.supported_ports[0].clone());
        assert!(matches!(
            duplicate_port.validate(),
            Err(CatalogError::DuplicateSupportedPort { listing_id, .. })
                if listing_id == "duplicate-port"
        ));
    }

    #[test]
    fn validation_keeps_commercial_terms_explicit_and_separate() {
        let mut paid_without_price = implementation_listing("paid-missing", PortDirection::In);
        paid_without_price.marketplace.price = None;
        assert!(matches!(
            paid_without_price.validate(),
            Err(CatalogError::PaidListingMissingPrice { listing_id })
                if listing_id == "paid-missing"
        ));

        let mut free_with_price = package_listing("free-priced");
        free_with_price.marketplace.price = Some(Price {
            amount_minor: 1,
            currency: "JPY".to_owned(),
        });
        assert!(matches!(
            free_with_price.validate(),
            Err(CatalogError::FreeListingHasPrice { listing_id })
                if listing_id == "free-priced"
        ));
    }

    fn supplier_listing(
        listing_id: &str,
        supplier: &str,
        implementation_ref: &str,
        amount_minor: u64,
        score_millis: u16,
    ) -> CapabilityListing {
        CapabilityListing {
            listing_id: listing_id.to_owned(),
            title: format!("{supplier} text implementation"),
            supplier: supplier.to_owned(),
            source: format!("https://example.invalid/{supplier}"),
            version: "1.0.0".to_owned(),
            last_updated: "2026-10-06".to_owned(),
            asset: CatalogAsset::Implementation {
                implementation_ref: implementation_ref.to_owned(),
                logical_implementation_ref: Some("logical:text-provider".to_owned()),
            },
            supported_ports: vec![SupportedPort::new(
                PortDirection::In,
                PortChannel::Data,
                json!("text:utf8"),
            )],
            marketplace: MarketplaceMetadata {
                commercial_availability: CommercialAvailability::Paid,
                source_availability: SourceAvailability::Closed,
                price: Some(Price {
                    amount_minor,
                    currency: "JPY".to_owned(),
                }),
                rating: Some(RatingSummary {
                    score_millis,
                    max_score_millis: 5000,
                    review_count: 10,
                }),
                reputation: None,
                inspectability: Inspectability::MetadataOnly,
                support_statement: None,
                warranty_statement: None,
            },
        }
    }

    fn supplier_graph() -> Graph {
        let mut graph = Graph::new("graph:supplier-replacement");

        let mut source = Block {
            id: "block:source".to_owned(),
            label: "source".to_owned(),
            ports: Vec::new(),
            internal_graph_ref: None,
            implementation_ref: None,
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        };
        source.ports.push(Port {
            id: "value".to_owned(),
            direction: PortDirection::Out,
            channel: PortChannel::Data,
            contract: Some(json!("text:utf8")),
            extensions: Extensions::new(),
        });

        let mut target = Block {
            id: "block:provider".to_owned(),
            label: "provider".to_owned(),
            ports: Vec::new(),
            internal_graph_ref: None,
            implementation_ref: Some("logical:text-provider".to_owned()),
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        };
        target.ports.push(Port {
            id: "value".to_owned(),
            direction: PortDirection::In,
            channel: PortChannel::Data,
            contract: Some(json!("text:utf8")),
            extensions: Extensions::new(),
        });

        graph.blocks.extend([source, target]);
        graph.connections.push(Connection {
            id: "data:source-provider".to_owned(),
            source: PortRef {
                block_id: "block:source".to_owned(),
                port_id: "value".to_owned(),
            },
            target: PortRef {
                block_id: "block:provider".to_owned(),
                port_id: "value".to_owned(),
            },
            extensions: Extensions::new(),
        });
        graph
    }

    fn registry_with_explicit_supplier(default_ref: &str) -> ImplementationRegistry {
        let mut registry = ImplementationRegistry::new();
        registry
            .register(
                "impl:supplier-a",
                ProcessAction::new(
                    "python3",
                    ["-c".to_owned(), "print('SUPPLIER_A', end='')".to_owned()],
                ),
            )
            .unwrap();
        registry
            .register(
                "impl:supplier-b",
                ProcessAction::new(
                    "python3",
                    ["-c".to_owned(), "print('SUPPLIER_B', end='')".to_owned()],
                ),
            )
            .unwrap();
        registry
            .register_choice(
                "logical:text-provider",
                ["impl:supplier-a", "impl:supplier-b"],
                default_ref,
            )
            .unwrap();
        registry
    }

    fn concrete_ref(listing: &CapabilityListing) -> &str {
        match &listing.asset {
            CatalogAsset::Implementation {
                implementation_ref, ..
            } => implementation_ref,
            CatalogAsset::BlockPackage { .. } => panic!("expected implementation listing"),
        }
    }

    #[test]
    fn logical_query_returns_supplier_candidates_without_marketplace_auto_ranking() {
        let catalog = CapabilityCatalog::new([
            supplier_listing("supplier-b", "B", "impl:supplier-b", 100, 4900),
            supplier_listing("supplier-a", "A", "impl:supplier-a", 9999, 1000),
            package_listing("package-unrelated"),
        ]);

        let before = catalog.clone();
        let results = catalog
            .search_logical_implementations("logical:text-provider")
            .unwrap();

        assert_eq!(
            results
                .iter()
                .map(|listing| listing.listing_id.as_str())
                .collect::<Vec<_>>(),
            vec!["supplier-a", "supplier-b"]
        );
        assert_eq!(
            results.iter().map(|listing| concrete_ref(listing)).collect::<Vec<_>>(),
            vec!["impl:supplier-a", "impl:supplier-b"]
        );
        assert_eq!(catalog, before);

        let mut changed_marketplace = catalog.clone();
        changed_marketplace.listings[0].marketplace.price = Some(Price {
            amount_minor: 1_000_000,
            currency: "JPY".to_owned(),
        });
        changed_marketplace.listings[0].marketplace.rating = Some(RatingSummary {
            score_millis: 100,
            max_score_millis: 5000,
            review_count: 999,
        });
        changed_marketplace.listings[1].marketplace.price = Some(Price {
            amount_minor: 1,
            currency: "JPY".to_owned(),
        });
        changed_marketplace.listings[1].marketplace.rating = Some(RatingSummary {
            score_millis: 5000,
            max_score_millis: 5000,
            review_count: 1,
        });

        assert_eq!(
            changed_marketplace
                .search_logical_implementations("logical:text-provider")
                .unwrap()
                .iter()
                .map(|listing| listing.listing_id.as_str())
                .collect::<Vec<_>>(),
            vec!["supplier-a", "supplier-b"]
        );
    }

    #[test]
    fn explicit_supplier_choice_changes_concrete_plan_without_changing_graph() {
        let catalog = CapabilityCatalog::new([
            supplier_listing("supplier-a", "A", "impl:supplier-a", 9999, 1000),
            supplier_listing("supplier-b", "B", "impl:supplier-b", 100, 4900),
        ]);
        let candidates = catalog
            .search_logical_implementations("logical:text-provider")
            .unwrap();

        let graph = supplier_graph();
        let graph_before = graph.clone();
        let routes = RouteRegistry::new();

        let selected_a = concrete_ref(candidates[0]);
        let plan_a = Planner::lower(
            &graph,
            &registry_with_explicit_supplier(selected_a),
            &routes,
        )
        .unwrap();

        let selected_b = concrete_ref(candidates[1]);
        let plan_b = Planner::lower(
            &graph,
            &registry_with_explicit_supplier(selected_b),
            &routes,
        )
        .unwrap();

        assert_eq!(graph, graph_before);
        assert_eq!(plan_a.steps.len(), 1);
        assert_eq!(plan_b.steps.len(), 1);
        assert_eq!(plan_a.steps[0].implementation_ref, "impl:supplier-a");
        assert_eq!(plan_b.steps[0].implementation_ref, "impl:supplier-b");
        assert_ne!(
            plan_a.steps[0].implementation_ref,
            plan_b.steps[0].implementation_ref
        );
        assert_eq!(graph.blocks[1].implementation_ref.as_deref(), Some("logical:text-provider"));
        assert_eq!(graph.connections, graph_before.connections);
    }

    #[test]
    fn replay_stays_bound_to_selected_supplier_without_catalog_or_registry() {
        let graph = supplier_graph();
        let plan = Planner::lower(
            &graph,
            &registry_with_explicit_supplier("impl:supplier-b"),
            &RouteRegistry::new(),
        )
        .unwrap();

        let json = serde_json::to_string(&plan).unwrap();
        let replayed: ExecutionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(replayed.steps[0].implementation_ref, "impl:supplier-b");

        let trace = ProcessRuntime::execute(&replayed);
        assert!(trace.succeeded());
        assert_eq!(trace.entries[0].implementation_ref, "impl:supplier-b");
        assert_eq!(trace.entries[0].stdout, "SUPPLIER_B");
    }

    #[test]
    fn validation_rejects_empty_implementation_identity_and_invalid_rating() {
        let mut listing = implementation_listing("invalid-impl", PortDirection::In);
        if let CatalogAsset::Implementation {
            implementation_ref, ..
        } = &mut listing.asset
        {
            implementation_ref.clear();
        }
        assert!(matches!(
            listing.validate(),
            Err(CatalogError::EmptyField { field, .. }) if field == "implementation_ref"
        ));

        let mut rating = package_listing("invalid-rating");
        rating.marketplace.rating = Some(RatingSummary {
            score_millis: 6000,
            max_score_millis: 5000,
            review_count: 1,
        });
        assert!(matches!(
            rating.validate(),
            Err(CatalogError::InvalidRating { listing_id, .. })
                if listing_id == "invalid-rating"
        ));
    }
}
