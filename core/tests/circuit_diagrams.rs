use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CircuitDiagram {
    schema: String,
    model: String,
    title: String,
    status: String,
    source_of_truth: String,
    nodes: Vec<CircuitNode>,
    edges: Vec<CircuitEdge>,
}

#[derive(Debug, Deserialize)]
struct CircuitNode {
    id: String,
    label: String,
    kind: String,
    #[serde(default)]
    ports: Vec<String>,
    #[serde(default)]
    spice: Option<SpiceExport>,
}

#[derive(Debug, Deserialize)]
struct CircuitEdge {
    from: String,
    to: String,
}

#[derive(Debug, Deserialize)]
struct SpiceExport {
    primitive: String,
    name: String,
    nodes: Vec<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[test]
fn circuit_diagram_json5_files_are_valid_graphs() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core crate should live under the workspace root");
    let knowledge_root = repo_root.join("knowledge");
    let diagrams = diagram_files(&knowledge_root);

    assert!(
        !diagrams.is_empty(),
        "expected at least one *.diagram.json5 file under knowledge/"
    );

    for path in diagrams {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
        let diagram: CircuitDiagram = json5::from_str(&text)
            .unwrap_or_else(|error| panic!("could not parse {}: {error}", path.display()));

        assert_eq!(
            diagram.schema,
            "greybound.circuit-diagram.v1",
            "{} uses an unsupported schema",
            path.display()
        );
        assert!(
            !diagram.model.trim().is_empty(),
            "{} must name its model",
            path.display()
        );
        assert!(
            !diagram.title.trim().is_empty(),
            "{} must have a title",
            path.display()
        );
        assert!(
            matches!(
                diagram.status.as_str(),
                "documentation" | "draft" | "validated"
            ),
            "{} has an unknown status '{}'",
            path.display(),
            diagram.status
        );
        assert!(
            matches!(
                diagram.source_of_truth.as_str(),
                "rust-model" | "reference-selection" | "measured-hardware"
            ),
            "{} has an unknown sourceOfTruth '{}'",
            path.display(),
            diagram.source_of_truth
        );
        assert!(
            !diagram.nodes.is_empty(),
            "{} must contain at least one node",
            path.display()
        );

        let mut nodes = HashMap::new();
        for node in &diagram.nodes {
            assert!(
                !node.id.trim().is_empty(),
                "{} contains a node with an empty id",
                path.display()
            );
            assert!(
                !node.label.trim().is_empty(),
                "{} node '{}' has an empty label",
                path.display(),
                node.id
            );
            assert!(
                !node.kind.trim().is_empty(),
                "{} node '{}' has an empty kind",
                path.display(),
                node.id
            );
            assert!(
                nodes.insert(node.id.as_str(), node).is_none(),
                "{} contains duplicate node id '{}'",
                path.display(),
                node.id
            );

            if let Some(spice) = &node.spice {
                assert!(
                    !spice.primitive.trim().is_empty(),
                    "{} node '{}' has an empty SPICE primitive",
                    path.display(),
                    node.id
                );
                assert!(
                    !spice.name.trim().is_empty(),
                    "{} node '{}' has an empty SPICE name",
                    path.display(),
                    node.id
                );
                assert!(
                    spice.nodes.len() >= 2,
                    "{} node '{}' SPICE export needs at least two nodes",
                    path.display(),
                    node.id
                );
                assert!(
                    spice.value.is_some() || spice.model.is_some(),
                    "{} node '{}' SPICE export needs either value or model",
                    path.display(),
                    node.id
                );
            }
        }

        for edge in &diagram.edges {
            assert_endpoint(&path, &nodes, &edge.from);
            assert_endpoint(&path, &nodes, &edge.to);
        }
    }
}

fn assert_endpoint(path: &Path, nodes: &HashMap<&str, &CircuitNode>, endpoint: &str) {
    let (node_id, port_id) = endpoint
        .split_once('.')
        .unwrap_or_else(|| panic!("{} endpoint '{endpoint}' must be node.port", path.display()));
    let node = nodes.get(node_id).unwrap_or_else(|| {
        panic!(
            "{} endpoint '{endpoint}' references unknown node '{node_id}'",
            path.display()
        )
    });
    assert!(
        node.ports.iter().any(|port| port == port_id),
        "{} endpoint '{}' references unknown port '{}' on node '{}'",
        path.display(),
        endpoint,
        port_id,
        node_id
    );
}

fn diagram_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_diagram_files(root, &mut files);
    files.sort();
    files
}

fn collect_diagram_files(path: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("could not read directory {}: {error}", path.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("could not read directory entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            collect_diagram_files(&path, files);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".diagram.json5"))
        {
            files.push(path);
        }
    }
}
