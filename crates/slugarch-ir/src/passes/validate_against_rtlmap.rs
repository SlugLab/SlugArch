//! validate_against_rtlmap: diffs a SlugIR function against a pipeline rtlmap.json.

use crate::error::RtlmapDiff;
use crate::module::{Function, Module};
use crate::pass::Pass;
use crate::types::{IpId, OpId};
use crate::IrError;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct PipelineRtlmap {
    pub name: String,
    pub nodes: Vec<PipelineNode>,
    pub edges: Vec<PipelineEdge>,
}

#[derive(Debug, Deserialize)]
pub struct PipelineNode {
    pub node_id: String,
    pub op: String,
    pub selected_ip: String,
}

#[derive(Debug, Deserialize)]
pub struct PipelineEdge {
    pub from: String,
    pub to: String,
    pub channel: String,
}

impl PipelineRtlmap {
    pub fn from_json_file(path: &std::path::Path) -> Result<Self, IrError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| IrError::Deserialize(format!("reading {}: {}", path.display(), e)))?;
        serde_json::from_str(&text).map_err(|e| IrError::Deserialize(e.to_string()))
    }
}

/// Maps a Gemma IP catalog name (e.g. "systolic_array_16x16") to the enum variant.
pub fn parse_ip_id(name: &str) -> Option<IpId> {
    IpId::all()
        .iter()
        .copied()
        .find(|ip| ip.catalog_name() == name)
}

pub struct ValidateAgainstRtlmap {
    pub oracle: PipelineRtlmap,
    pub function_name: String,
    /// Maps source_hint strings attached to ops to rtlmap node_ids.
    pub node_id_of_hint: HashMap<String, String>,
}

impl Pass for ValidateAgainstRtlmap {
    fn name(&self) -> &'static str {
        "validate_against_rtlmap"
    }

    fn run(&mut self, module: &mut Module) -> Result<(), IrError> {
        let f = module
            .functions
            .iter()
            .find(|f| f.name == self.function_name)
            .ok_or_else(|| IrError::PassFailed {
                pass: "validate_against_rtlmap",
                node: None,
                msg: format!("function {} not found", self.function_name),
            })?;
        let diff = diff_function(f, &self.oracle, &self.node_id_of_hint);
        if !diff.is_empty() {
            return Err(IrError::OracleMismatch {
                diff: serde_json::to_string_pretty(&diff).unwrap_or_else(|_| format!("{:?}", diff)),
            });
        }
        Ok(())
    }
}

fn diff_function(
    f: &Function,
    oracle: &PipelineRtlmap,
    node_id_of_hint: &HashMap<String, String>,
) -> RtlmapDiff {
    let mut diff = RtlmapDiff::default();

    // Resolve (OpId -> node_id) and (node_id -> OpId) via hint map.
    let mut op_to_node: HashMap<OpId, String> = HashMap::new();
    for (id, meta) in &f.meta {
        if let Some(hint) = &meta.source_hint {
            if let Some(node_id) = node_id_of_hint.get(hint) {
                op_to_node.insert(*id, node_id.clone());
            }
        }
    }
    let node_to_op: HashMap<String, OpId> =
        op_to_node.iter().map(|(k, v)| (v.clone(), *k)).collect();

    // Node presence
    let oracle_ids: std::collections::HashSet<&String> =
        oracle.nodes.iter().map(|n| &n.node_id).collect();
    let actual_ids: std::collections::HashSet<&String> = node_to_op.keys().collect();

    for nid in oracle_ids.difference(&actual_ids) {
        diff.missing_nodes.push((*nid).clone());
    }
    for nid in actual_ids.difference(&oracle_ids) {
        diff.extra_nodes.push((*nid).clone());
    }

    // Per-node IP correctness
    for node in &oracle.nodes {
        let Some(op_id) = node_to_op.get(&node.node_id) else {
            continue;
        };
        let actual_ip = f
            .meta
            .get(op_id)
            .and_then(|m| m.backend)
            .map(|bc| bc.0.catalog_name().to_string())
            .unwrap_or_else(|| "<unassigned>".into());
        if actual_ip != node.selected_ip {
            diff.wrong_ip
                .push((node.node_id.clone(), node.selected_ip.clone(), actual_ip));
        }
    }

    // Edge presence (token edges only)
    let mut oracle_edges: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for e in &oracle.edges {
        if e.channel == "token" {
            oracle_edges.insert((e.from.clone(), e.to.clone()));
        }
    }
    let mut actual_edges: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for edge in &f.edges {
        let (Some(s), Some(d)) = (op_to_node.get(&edge.src()), op_to_node.get(&edge.dst())) else {
            continue;
        };
        actual_edges.insert((s.clone(), d.clone()));
    }
    for e in oracle_edges.difference(&actual_edges) {
        diff.edge_diff
            .push(format!("missing token edge: {} -> {}", e.0, e.1));
    }
    for e in actual_edges.difference(&oracle_edges) {
        diff.edge_diff
            .push(format!("extra token edge: {} -> {}", e.0, e.1));
    }

    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Edge;
    use crate::module::{Context, FunctionBuilder};
    use crate::op::{Op, OpMeta, StateKind};
    use crate::types::{BackendChoice, IpId};

    fn hand_made_oracle() -> PipelineRtlmap {
        PipelineRtlmap {
            name: "test".into(),
            nodes: vec![
                PipelineNode {
                    node_id: "n0".into(),
                    op: "rms_norm".into(),
                    selected_ip: "npu_array_v4_seed_g".into(),
                },
                PipelineNode {
                    node_id: "n1".into(),
                    op: "gemm_qkv".into(),
                    selected_ip: "systolic_array_16x16".into(),
                },
            ],
            edges: vec![PipelineEdge {
                from: "n0".into(),
                to: "n1".into(),
                channel: "token".into(),
            }],
        }
    }

    #[test]
    fn matching_module_produces_empty_diff() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "test");
        let a = b.add_op(Op::StateStep {
            kind: StateKind::RmsNorm,
            operands: vec![],
        });
        let c = b.add_op(Op::TensorTile {
            kind: crate::op::TileKind::Gemm,
            shape: crate::types::Shape(vec![64, 64]),
            dtype: crate::types::Dtype::F16,
            operands: vec![],
        });
        b.add_edge(Edge::Data(a, c));
        let mut f = b.finish();
        f.meta.insert(
            a,
            OpMeta {
                backend: Some(BackendChoice(IpId::NpuArrayV4SeedG)),
                source_hint: Some("h0".into()),
                ..Default::default()
            },
        );
        f.meta.insert(
            c,
            OpMeta {
                backend: Some(BackendChoice(IpId::SystolicArray16x16)),
                source_hint: Some("h1".into()),
                ..Default::default()
            },
        );
        let mut m = Module::default();
        m.functions.push(f);

        let mut node_ids = HashMap::new();
        node_ids.insert("h0".to_string(), "n0".to_string());
        node_ids.insert("h1".to_string(), "n1".to_string());

        let mut pass = ValidateAgainstRtlmap {
            oracle: hand_made_oracle(),
            function_name: "test".into(),
            node_id_of_hint: node_ids,
        };
        pass.run(&mut m).expect("should match");
    }

    #[test]
    fn wrong_ip_produces_error() {
        let mut ctx = Context::new();
        let mut b = FunctionBuilder::new(&mut ctx, "test");
        let a = b.add_op(Op::StateStep {
            kind: StateKind::RmsNorm,
            operands: vec![],
        });
        let c = b.add_op(Op::StateStep {
            kind: StateKind::RmsNorm,
            operands: vec![],
        });
        b.add_edge(Edge::Data(a, c));
        let mut f = b.finish();
        // Intentionally wrong IP for n1
        f.meta.insert(
            a,
            OpMeta {
                backend: Some(BackendChoice(IpId::NpuArrayV4SeedG)),
                source_hint: Some("h0".into()),
                ..Default::default()
            },
        );
        f.meta.insert(
            c,
            OpMeta {
                backend: Some(BackendChoice(IpId::NoCMesh)), // should have been SystolicArray16x16
                source_hint: Some("h1".into()),
                ..Default::default()
            },
        );
        let mut m = Module::default();
        m.functions.push(f);

        let mut node_ids = HashMap::new();
        node_ids.insert("h0".to_string(), "n0".to_string());
        node_ids.insert("h1".to_string(), "n1".to_string());
        let mut pass = ValidateAgainstRtlmap {
            oracle: hand_made_oracle(),
            function_name: "test".into(),
            node_id_of_hint: node_ids,
        };
        match pass.run(&mut m) {
            Err(IrError::OracleMismatch { diff }) => {
                assert!(diff.contains("noc_mesh"));
                assert!(diff.contains("systolic_array_16x16"));
            }
            other => panic!("expected OracleMismatch, got {:?}", other),
        }
    }

    #[test]
    fn loads_real_qwen_decode_pipeline_from_vendored_file() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(
                "vendor/gemma-generated/generated/mappings/pipelines/qwen_decode_token.rtlmap.json",
            );
        let oracle = PipelineRtlmap::from_json_file(&path).expect("load vendored pipeline");
        assert_eq!(oracle.name, "qwen_decode_token");
        // Sanity: the pipeline has >=3 nodes and references at least one NPU IP.
        assert!(oracle.nodes.len() >= 3);
        assert!(oracle
            .nodes
            .iter()
            .any(|n| n.selected_ip == "npu_array_v4_seed_g"));
    }
}
