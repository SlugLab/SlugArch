use crate::dispatch::build_gemm_dispatch_stream;
use crate::result::decode_results;
use crate::{GemmJob, HostError};
use serde::{Deserialize, Serialize};
use slugarch_cxl_wire::{decode, encode, CxlMsg, S2MDRSOp, S2MNDROp, FLIT_BYTES};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QemuType2Expected {
    pub workload: String,
    pub request_count: usize,
    pub flit_bytes: usize,
    pub request_tags: Vec<u16>,
    pub expected_c: [[u32; 4]; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QemuType2Summary {
    pub status: String,
    pub request_count: usize,
    pub response_count: usize,
    pub tag_mismatches: usize,
    pub dispatch_failures: usize,
    pub result_c: [[u32; 4]; 4],
    pub expected_c: [[u32; 4]; 4],
    pub requests_path: String,
    pub responses_path: String,
}

pub fn expected_for_job(job: &GemmJob) -> QemuType2Expected {
    let stream = build_gemm_dispatch_stream(job, 0);
    QemuType2Expected {
        workload: "slugcxl_gemm_4x4".to_string(),
        request_count: stream.len(),
        flit_bytes: FLIT_BYTES,
        request_tags: stream.iter().map(CxlMsg::tag).collect(),
        expected_c: matmul(job),
    }
}

pub fn export_requests(job: &GemmJob, out_dir: &Path) -> Result<QemuType2Expected, HostError> {
    fs::create_dir_all(out_dir).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("create {}: {e}", out_dir.display()),
    })?;
    let stream = build_gemm_dispatch_stream(job, 0);
    let mut bytes = Vec::with_capacity(stream.len() * FLIT_BYTES);
    for msg in &stream {
        bytes.extend_from_slice(&encode(msg));
    }
    fs::write(out_dir.join("requests.bin"), bytes).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("write requests.bin: {e}"),
    })?;
    let expected = expected_for_job(job);
    let expected_json =
        serde_json::to_vec_pretty(&expected).map_err(|e| HostError::DispatchFailed {
            tag: 0,
            reason: format!("serialize expected.json: {e}"),
        })?;
    fs::write(out_dir.join("expected.json"), expected_json).map_err(|e| {
        HostError::DispatchFailed {
            tag: 0,
            reason: format!("write expected.json: {e}"),
        }
    })?;
    Ok(expected)
}

pub fn validate_responses(
    job: &GemmJob,
    responses_path: &Path,
    out_dir: &Path,
) -> Result<QemuType2Summary, HostError> {
    let bytes = fs::read(responses_path).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("read {}: {e}", responses_path.display()),
    })?;
    if bytes.len() % FLIT_BYTES != 0 {
        return Err(HostError::DispatchFailed {
            tag: 0,
            reason: format!(
                "responses.bin length must be a multiple of 64, got {}",
                bytes.len()
            ),
        });
    }
    let response_count = bytes.len() / FLIT_BYTES;
    let requests = build_gemm_dispatch_stream(job, 0);
    let mut decoded = Vec::with_capacity(response_count);
    for chunk in bytes.chunks_exact(FLIT_BYTES) {
        let mut flit = [0u8; FLIT_BYTES];
        flit.copy_from_slice(chunk);
        decoded.push(decode(&flit)?);
    }
    let mut tag_mismatches = 0usize;
    let mut dispatch_failures = 0usize;
    let mut read_responses = Vec::new();
    for (idx, response) in decoded.iter().enumerate() {
        if let Some(request) = requests.get(idx) {
            if response.tag() != request.tag() {
                tag_mismatches += 1;
            }
        }
        match response {
            CxlMsg::S2MNDR {
                opcode: S2MNDROp::Cmp,
                ..
            } if idx < 33 => {}
            CxlMsg::S2MDRS {
                opcode: S2MDRSOp::MemData,
                ..
            } if idx >= 33 => {
                read_responses.push(response.clone());
            }
            CxlMsg::S2MNDR {
                opcode: S2MNDROp::DispatchFailed,
                ..
            } => {
                dispatch_failures += 1;
            }
            _ => {
                dispatch_failures += 1;
            }
        }
    }
    let result_c = if read_responses.len() == 16 {
        decode_results(&read_responses)?
    } else {
        [[0u32; 4]; 4]
    };
    let expected_c = matmul(job);
    let status = if response_count == requests.len()
        && tag_mismatches == 0
        && dispatch_failures == 0
        && result_c == expected_c
    {
        "pass"
    } else {
        "fail"
    }
    .to_string();
    let summary = QemuType2Summary {
        status,
        request_count: requests.len(),
        response_count,
        tag_mismatches,
        dispatch_failures,
        result_c,
        expected_c,
        requests_path: out_dir.join("requests.bin").display().to_string(),
        responses_path: responses_path.display().to_string(),
    };
    fs::create_dir_all(out_dir).map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("create {}: {e}", out_dir.display()),
    })?;
    fs::write(
        out_dir.join("summary.json"),
        serde_json::to_vec_pretty(&summary).map_err(|e| HostError::DispatchFailed {
            tag: 0,
            reason: format!("serialize summary.json: {e}"),
        })?,
    )
    .map_err(|e| HostError::DispatchFailed {
        tag: 0,
        reason: format!("write summary.json: {e}"),
    })?;
    fs::write(out_dir.join("summary.csv"), summary_csv(&summary)).map_err(|e| {
        HostError::DispatchFailed {
            tag: 0,
            reason: format!("write summary.csv: {e}"),
        }
    })?;
    Ok(summary)
}

fn matmul(job: &GemmJob) -> [[u32; 4]; 4] {
    let mut out = [[0u32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            let mut sum = 0u32;
            for k in 0..4 {
                sum += job.a[r][k] as u32 * job.b[k][c] as u32;
            }
            out[r][c] = sum;
        }
    }
    out
}

fn summary_csv(summary: &QemuType2Summary) -> String {
    format!(
        "status,request_count,response_count,tag_mismatches,dispatch_failures\n{},{},{},{},{}\n",
        summary.status,
        summary.request_count,
        summary.response_count,
        summary.tag_mismatches,
        summary.dispatch_failures
    )
}
