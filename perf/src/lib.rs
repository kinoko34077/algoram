use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
}

#[derive(Debug, Clone)]
pub struct MemoCache<K, V> {
    entries: BTreeMap<K, V>,
    hits: u64,
    misses: u64,
}

impl<K: Ord + Clone, V> Default for MemoCache<K, V> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            hits: 0,
            misses: 0,
        }
    }
}

impl<K: Ord + Clone, V> MemoCache<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_try_insert_with<E, F>(&mut self, key: K, build: F) -> Result<&V, E>
    where
        F: FnOnce() -> Result<V, E>,
    {
        if self.entries.contains_key(&key) {
            self.hits += 1;
            return Ok(self
                .entries
                .get(&key)
                .expect("cache key checked immediately before lookup"));
        }

        self.misses += 1;
        let value = build()?;
        self.entries.insert(key.clone(), value);
        Ok(self
            .entries
            .get(&key)
            .expect("cache value inserted immediately before lookup"))
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceCacheKey {
    pub language: String,
    pub artifact_id: String,
    pub origin: String,
    pub source: String,
}

impl SourceCacheKey {
    pub fn new(
        language: impl Into<String>,
        artifact_id: impl Into<String>,
        origin: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            language: language.into(),
            artifact_id: artifact_id.into(),
            origin: origin.into(),
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlanCacheKey {
    pub graph_revision: String,
    pub implementation_revision: String,
    pub route_revision: String,
    pub environment_revision: String,
}

impl PlanCacheKey {
    pub fn new(
        graph_revision: impl Into<String>,
        implementation_revision: impl Into<String>,
        route_revision: impl Into<String>,
        environment_revision: impl Into<String>,
    ) -> Self {
        Self {
            graph_revision: graph_revision.into(),
            implementation_revision: implementation_revision.into(),
            route_revision: route_revision.into(),
            environment_revision: environment_revision.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use algoram_core::{Block, Extensions, Graph};
    use algoram_interop::RouteRegistry;
    use algoram_python_importer::import_python;
    use algoram_runtime::{
        ExecutionPlan, ExecutionStep, ImplementationRegistry, Planner, ProcessAction,
        ProcessRuntime, TraceStatus,
    };

    fn executable_graph() -> Graph {
        let mut graph = Graph::new("graph:cache-test");
        graph.blocks.push(Block {
            id: "block:run".to_owned(),
            label: "run".to_owned(),
            ports: Vec::new(),
            internal_graph_ref: None,
            implementation_ref: Some("impl:run".to_owned()),
            definition_ref: None,
            source_anchor: None,
            extensions: Extensions::new(),
            diagnostics: Vec::new(),
        });
        graph
    }

    #[test]
    fn unchanged_source_reuses_imported_result() {
        let source = "def f(x):\n    return x\n";
        let key = SourceCacheKey::new("python", "artifact:test", "test.py", source);
        let mut cache = MemoCache::new();
        let mut importer_calls = 0usize;

        for _ in 0..3 {
            let imported = cache
                .get_or_try_insert_with(key.clone(), || {
                    importer_calls += 1;
                    import_python("artifact:test", "test.py", source)
                })
                .unwrap();
            imported.validate().unwrap();
        }

        assert_eq!(importer_calls, 1);
        assert_eq!(cache.stats(), CacheStats { hits: 2, misses: 1 });
    }

    #[test]
    fn changed_source_misses_import_cache() {
        let mut cache = MemoCache::new();
        let mut importer_calls = 0usize;

        for source in ["def f(x):\n    return x\n", "def f(x):\n    return x + 1\n"] {
            let key = SourceCacheKey::new("python", "artifact:test", "test.py", source);
            cache
                .get_or_try_insert_with(key, || {
                    importer_calls += 1;
                    import_python("artifact:test", "test.py", source)
                })
                .unwrap();
        }

        assert_eq!(importer_calls, 2);
        assert_eq!(cache.stats().misses, 2);
    }

    #[test]
    fn unchanged_plan_key_does_not_rerun_planner() {
        let graph = executable_graph();
        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:run",
                ProcessAction::new("python3", ["-c", "print('ok')"]),
            )
            .unwrap();

        let routes = RouteRegistry::new();
        let key = PlanCacheKey::new("graph:v1", "impl:v1", "routes:v1", "env:v1");
        let mut cache = MemoCache::<PlanCacheKey, ExecutionPlan>::new();
        let mut planner_calls = 0usize;

        for _ in 0..3 {
            cache
                .get_or_try_insert_with(key.clone(), || {
                    planner_calls += 1;
                    Planner::lower(&graph, &implementations, &routes)
                })
                .unwrap();
        }

        assert_eq!(planner_calls, 1);
        assert_eq!(cache.stats().hits, 2);
    }

    #[test]
    fn changed_plan_revision_misses_cache() {
        let graph = executable_graph();
        let mut implementations = ImplementationRegistry::new();
        implementations
            .register(
                "impl:run",
                ProcessAction::new("python3", ["-c", "print('ok')"]),
            )
            .unwrap();
        let routes = RouteRegistry::new();
        let mut cache = MemoCache::<PlanCacheKey, ExecutionPlan>::new();
        let mut planner_calls = 0usize;

        for graph_revision in ["graph:v1", "graph:v2"] {
            let key = PlanCacheKey::new(graph_revision, "impl:v1", "routes:v1", "env:v1");
            cache
                .get_or_try_insert_with(key, || {
                    planner_calls += 1;
                    Planner::lower(&graph, &implementations, &routes)
                })
                .unwrap();
        }

        assert_eq!(planner_calls, 2);
        assert_eq!(cache.stats().misses, 2);
    }

    #[test]
    fn many_origin_blocks_can_share_one_process_dispatch() {
        let origin_block_ids = (0..128)
            .map(|index| format!("block:{index}"))
            .collect::<Vec<_>>();
        let plan = ExecutionPlan {
            reference_graph_id: "graph:dispatch-proof".to_owned(),
            steps: vec![ExecutionStep {
                id: "step:one-process".to_owned(),
                implementation_ref: "impl:one-process".to_owned(),
                action: ProcessAction::new("python3", ["-c", "print('single-dispatch')"]),
                origin_block_ids: origin_block_ids.clone(),
                source_anchors: Vec::new(),
                route_connector_ids: Vec::new(),
                    argv_bindings: Vec::new(),
            }],
        };

        let trace = ProcessRuntime::execute(&plan);

        assert_eq!(trace.entries.len(), 1);
        assert_eq!(trace.entries[0].status, TraceStatus::Succeeded);
        assert_eq!(trace.entries[0].origin_block_ids, origin_block_ids);
        assert_eq!(trace.entries[0].stdout.trim(), "single-dispatch");
    }
}
