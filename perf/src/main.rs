use algoram_c_importer::import_c;
use algoram_core::{
    Block, Connection, Extensions, Graph, Port, PortChannel, PortDirection, PortRef,
};
use algoram_interop::{Connector, ContractId, RouteRegistry, TransferMode};
use algoram_perf_baseline::{MemoCache, PlanCacheKey, SourceCacheKey};
use algoram_python_importer::{import_python, PythonImport};
use algoram_runtime::{
    ExecutionPlan, ImplementationRegistry, Planner, ProcessAction, ProcessRuntime,
};
use serde_json::json;
use std::hint::black_box;
use std::time::{Duration, Instant};

fn main() {
    println!(
        "{}",
        json!({
            "kind": "environment",
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "crate": env!("CARGO_PKG_NAME"),
            "crate_version": env!("CARGO_PKG_VERSION")
        })
    );

    for (size, iterations) in [(10usize, 20usize), (100, 10), (500, 3)] {
        let source = python_source(size);
        measure("python_import_cold", size, iterations, || {
            let imported =
                import_python("artifact:perf-python", "perf.py", black_box(&source)).unwrap();
            black_box(imported.root.blocks.len());
        });

        let source = c_source(size);
        measure("c_import_cold", size, iterations, || {
            let imported = import_c("artifact:perf-c", "perf.c", black_box(&source)).unwrap();
            black_box(imported.root.blocks.len());
        });
    }

    measure_import_cache_hit();
    measure_plan_cold_and_cached();
    measure_single_process_runtime();
}

fn measure(metric: &str, size: usize, iterations: usize, mut operation: impl FnMut()) {
    operation();
    let start = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    emit_metric(metric, size, iterations, start.elapsed(), json!({}));
}

fn emit_metric(
    metric: &str,
    size: usize,
    iterations: usize,
    elapsed: Duration,
    extra: serde_json::Value,
) {
    let elapsed_ns = elapsed.as_nanos().min(u64::MAX as u128) as u64;
    let ns_per_iteration = if iterations == 0 {
        0
    } else {
        elapsed_ns / iterations as u64
    };

    println!(
        "{}",
        json!({
            "kind": "metric",
            "metric": metric,
            "size": size,
            "iterations": iterations,
            "elapsed_ns": elapsed_ns,
            "ns_per_iteration": ns_per_iteration,
            "extra": extra
        })
    );
}

fn python_source(function_count: usize) -> String {
    let mut source = String::new();
    for index in 0..function_count {
        source.push_str(&format!(
            "def f_{index}(x):\n    y = x + 1\n    if y:\n        return y\n\n"
        ));
    }
    source
}

fn c_source(function_count: usize) -> String {
    let mut source = String::from("#include <stdint.h>\n\n");
    for index in 0..function_count {
        source.push_str(&format!(
            "int32_t f_{index}(int32_t x) {{ int32_t y = x + 1; if (y) {{ return y; }} return 0; }}\n"
        ));
    }
    source
}

fn measure_import_cache_hit() {
    let source = python_source(100);
    let key = SourceCacheKey::new(
        "python",
        "artifact:perf-cache",
        "perf-cache.py",
        source.clone(),
    );
    let mut cache = MemoCache::<SourceCacheKey, PythonImport>::new();

    cache
        .get_or_try_insert_with(key.clone(), || {
            import_python("artifact:perf-cache", "perf-cache.py", &source)
        })
        .unwrap();

    let iterations = 10_000usize;
    let start = Instant::now();
    for _ in 0..iterations {
        let imported = cache
            .get_or_try_insert_with(key.clone(), || {
                import_python("artifact:perf-cache", "perf-cache.py", &source)
            })
            .unwrap();
        black_box(imported.root.id.as_str());
    }

    let stats = cache.stats();
    emit_metric(
        "python_import_cache_hit",
        100,
        iterations,
        start.elapsed(),
        json!({
            "cache_hits": stats.hits,
            "cache_misses": stats.misses
        }),
    );
}

fn measure_plan_cold_and_cached() {
    let (graph, implementations, routes) = planning_fixture();

    measure("execution_plan_cold", graph.blocks.len(), 2_000, || {
        let plan = Planner::lower(&graph, &implementations, &routes).unwrap();
        black_box(plan.steps.len());
    });

    let key = PlanCacheKey::new("graph:v1", "impl:v1", "routes:v1", "env:linux");
    let mut cache = MemoCache::<PlanCacheKey, ExecutionPlan>::new();
    cache
        .get_or_try_insert_with(key.clone(), || {
            Planner::lower(&graph, &implementations, &routes)
        })
        .unwrap();

    let iterations = 10_000usize;
    let start = Instant::now();
    for _ in 0..iterations {
        let plan = cache
            .get_or_try_insert_with(key.clone(), || {
                Planner::lower(&graph, &implementations, &routes)
            })
            .unwrap();
        black_box(plan.steps.len());
    }
    let stats = cache.stats();

    emit_metric(
        "execution_plan_cache_hit",
        graph.blocks.len(),
        iterations,
        start.elapsed(),
        json!({
            "cache_hits": stats.hits,
            "cache_misses": stats.misses
        }),
    );
}

fn planning_fixture() -> (Graph, ImplementationRegistry, RouteRegistry) {
    let mut graph = Graph::new("graph:perf-plan");

    let source = Block {
        id: "block:source".to_owned(),
        label: "source".to_owned(),
        ports: vec![port(
            "out",
            PortDirection::Out,
            PortChannel::Data,
            Some("python:ctypes:c_int"),
        )],
        internal_graph_ref: None,
        implementation_ref: None,
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    };

    let target = Block {
        id: "block:target".to_owned(),
        label: "target".to_owned(),
        ports: vec![port(
            "in",
            PortDirection::In,
            PortChannel::Data,
            Some("c:function:perf:int32"),
        )],
        internal_graph_ref: None,
        implementation_ref: Some("impl:target".to_owned()),
        definition_ref: None,
        source_anchor: None,
        extensions: Extensions::new(),
        diagnostics: Vec::new(),
    };

    graph.blocks.extend([source, target]);
    graph.connections.push(Connection {
        id: "data:source-target".to_owned(),
        source: PortRef {
            block_id: "block:source".to_owned(),
            port_id: "out".to_owned(),
        },
        target: PortRef {
            block_id: "block:target".to_owned(),
            port_id: "in".to_owned(),
        },
        extensions: Extensions::new(),
    });

    let mut implementations = ImplementationRegistry::new();
    implementations
        .register("impl:target", ProcessAction::new("python3", ["-c", "pass"]))
        .unwrap();

    let mut routes = RouteRegistry::new();
    routes
        .register(
            Connector::new(
                "python-ctypes-c-abi",
                ContractId::from("python:ctypes:c_int"),
                ContractId::from("c:abi:int32"),
                "python:ctypes",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();
    routes
        .register(
            Connector::new(
                "c-abi-perf",
                ContractId::from("c:abi:int32"),
                ContractId::from("c:function:perf:int32"),
                "c:abi",
            )
            .with_transfer(TransferMode::Copy),
        )
        .unwrap();

    (graph, implementations, routes)
}

fn port(id: &str, direction: PortDirection, channel: PortChannel, contract: Option<&str>) -> Port {
    Port {
        id: id.to_owned(),
        direction,
        channel,
        contract: contract.map(|value| json!(value)),
        extensions: Extensions::new(),
    }
}

fn measure_single_process_runtime() {
    let plan = ExecutionPlan {
        reference_graph_id: "graph:perf-runtime".to_owned(),
        steps: vec![algoram_runtime::ExecutionStep {
            id: "step:python-loop".to_owned(),
            implementation_ref: "impl:python-loop".to_owned(),
            action: ProcessAction::new(
                "python3",
                ["-c", "x = 0\nfor i in range(10000):\n    x += i\nprint(x)"],
            ),
            origin_block_ids: (0..100).map(|index| format!("block:{index}")).collect(),
            source_anchors: Vec::new(),
            route_connector_ids: Vec::new(),
        }],
    };

    measure("single_process_many_origins", 100, 5, || {
        let trace = ProcessRuntime::execute(&plan);
        assert!(trace.succeeded());
        assert_eq!(trace.entries.len(), 1);
        black_box(trace.entries[0].stdout.as_str());
    });
}
