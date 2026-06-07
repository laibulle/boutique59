use greybound::neural_cell::ExperimentalNeuralCell;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct VectorFile {
    cases: Vec<VectorCase>,
    tolerance_abs: f32,
}

#[derive(Debug, Deserialize)]
struct VectorCase {
    input_v: f32,
    expected_output_v: f32,
}

#[test]
fn generated_neural_cell_vectors_match_rust_loader() {
    let Some(descriptor_path) = env_path("GREYBOUND_NEURAL_CELL_DESCRIPTOR") else {
        return;
    };
    let Some(vectors_path) = env_path("GREYBOUND_NEURAL_CELL_VECTORS") else {
        return;
    };
    let cell = ExperimentalNeuralCell::from_descriptor_path(&descriptor_path)
        .expect("failed to load generated neural-cell descriptor");
    let text =
        fs::read_to_string(&vectors_path).expect("failed to read generated neural-cell vectors");
    let vectors: VectorFile =
        json5::from_str(&text).expect("failed to parse generated neural-cell vectors");
    assert!(
        !vectors.cases.is_empty(),
        "generated neural-cell vector file has no cases"
    );

    for case in vectors.cases {
        let actual = cell.process_sample(case.input_v);
        let error = (actual - case.expected_output_v).abs();
        assert!(
            error <= vectors.tolerance_abs,
            "input {} expected {} got {} error {} > {}",
            case.input_v,
            case.expected_output_v,
            actual,
            error,
            vectors.tolerance_abs
        );
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}
