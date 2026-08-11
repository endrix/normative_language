use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::Instant;

use ndf_compiler::index::{BuildProvenance, NdfIndex, build_index};
use ndf_compiler::pto_impact::pto_release_impact;
use sha2::{Digest, Sha256};

#[path = "../../../tests/scale/generate.rs"]
mod generate;

#[test]
fn release_scale_contract() {
    if std::env::var("NDF_RUN_SCALE_TESTS").as_deref() != Ok("1") {
        eprintln!("set NDF_RUN_SCALE_TESTS=1 to run the release scale contract");
        return;
    }
    let graph = generate::graph();
    let provenance = BuildProvenance {
        project_commits: vec![(
            "scale".to_owned(),
            "0123456789abcdef0123456789abcdef01234567".to_owned(),
        )],
        format_version: "0.2".to_owned(),
        ir_version: "0.1".to_owned(),
        tool_version: "0.1.0".to_owned(),
        normative_roots: Vec::new(),
    };
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("first.sqlite");
    let second_path = directory.path().join("second.sqlite");
    let pto_before_path = directory.path().join("pto-before.sqlite");
    let pto_after_path = directory.path().join("pto-after.sqlite");
    let consumer_path = directory.path().join("consumer.sqlite");

    let started = Instant::now();
    let first = build_index(&graph, &first_path, &provenance).unwrap();
    let first_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let second = build_index(&graph, &second_path, &provenance).unwrap();
    let second_seconds = started.elapsed().as_secs_f64();

    assert_eq!(first.node_count, generate::NODE_COUNT);
    assert_eq!(first.edge_count, generate::EDGE_COUNT);
    assert_eq!(first.semantic_hash, second.semantic_hash);
    assert_eq!(file_hash(&first_path), file_hash(&second_path));
    assert!(
        first_seconds < 10.0 && second_seconds < 10.0,
        "scale builds took {first_seconds:.3}s and {second_seconds:.3}s"
    );

    build_index(
        &generate::pto_graph(false),
        &pto_before_path,
        &impact_provenance("pto-spec"),
    )
    .unwrap();
    build_index(
        &generate::pto_graph(true),
        &pto_after_path,
        &impact_provenance("pto-spec"),
    )
    .unwrap();
    build_index(
        &generate::consumer_graph(),
        &consumer_path,
        &impact_provenance("davincioo"),
    )
    .unwrap();
    let pto_before = NdfIndex::open(&pto_before_path).unwrap();
    let pto_after = NdfIndex::open(&pto_after_path).unwrap();
    let consumer = NdfIndex::open(&consumer_path).unwrap();

    let started = Instant::now();
    let first_impact = pto_release_impact(&pto_before, &pto_after, &consumer).unwrap();
    let first_impact_seconds = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let second_impact = pto_release_impact(&pto_before, &pto_after, &consumer).unwrap();
    let second_impact_seconds = started.elapsed().as_secs_f64();
    let first_impact_json = serde_json::to_vec(&first_impact).unwrap();
    let second_impact_json = serde_json::to_vec(&second_impact).unwrap();

    assert_eq!(
        first_impact.conformance_targets.len(),
        generate::CONFORMANCE_EDGE_COUNT
    );
    assert_eq!(
        first_impact.affected_consumers.len(),
        generate::CONSUMER_NODE_COUNT
    );
    assert_eq!(first_impact_json, second_impact_json);
    assert!(
        first_impact_seconds < 10.0 && second_impact_seconds < 10.0,
        "scale impacts took {first_impact_seconds:.3}s and {second_impact_seconds:.3}s"
    );
    println!(
        "NDF_SCALE first_seconds={first_seconds:.6} second_seconds={second_seconds:.6} impact_first_seconds={first_impact_seconds:.6} impact_second_seconds={second_impact_seconds:.6} bytes={} impact_bytes={}",
        first_path.metadata().unwrap().len(),
        first_impact_json.len()
    );
}

fn impact_provenance(project: &str) -> BuildProvenance {
    BuildProvenance {
        project_commits: vec![(
            project.to_owned(),
            "0123456789abcdef0123456789abcdef01234567".to_owned(),
        )],
        format_version: "0.2".to_owned(),
        ir_version: "0.1".to_owned(),
        tool_version: "0.1.0".to_owned(),
        normative_roots: Vec::new(),
    }
}

fn file_hash(path: &Path) -> [u8; 32] {
    let mut reader = BufReader::new(File::open(path).unwrap());
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    hash.finalize().into()
}
