use slugarch_tile_model::{export_corpus, CorpusConfig, FaultKind, RecordMode, WORKLOAD_SEED};
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = None;
    let mut tiles = None;
    let mut mode = None;
    let mut seed = WORKLOAD_SEED;
    let mut fault = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => output = arguments.next().map(PathBuf::from),
            "--tiles" => tiles = arguments.next().map(|value| value.parse()).transpose()?,
            "--mode" => {
                mode = arguments
                    .next()
                    .map(|value| parse_mode(&value))
                    .transpose()?
            }
            "--seed" => seed = arguments.next().ok_or("--seed requires a value")?.parse()?,
            "--fault" => {
                fault = arguments
                    .next()
                    .map(|value| parse_fault(&value))
                    .transpose()?
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }

    let config = CorpusConfig {
        tiles: tiles.ok_or("--tiles is required")?,
        record_mode: mode.ok_or("--mode is required")?,
        seed,
        fault,
    };
    let output = output.ok_or("--output is required")?;
    let exported = export_corpus(&config, &output)?;
    println!("{}", hex(&exported.sha256));
    Ok(())
}

fn parse_mode(value: &str) -> Result<RecordMode, String> {
    match value {
        "validation" => Ok(RecordMode::Validation),
        "delta" => Ok(RecordMode::Delta),
        "full" => Ok(RecordMode::Full),
        _ => Err(format!("unsupported record mode: {value}")),
    }
}

fn parse_fault(value: &str) -> Result<FaultKind, String> {
    match value {
        "missing-invalidate-ack" => Ok(FaultKind::MissingInvalidateAck),
        "stale-line-version" => Ok(FaultKind::StaleLineVersion),
        "reordered-completion" => Ok(FaultKind::ReorderedCompletion),
        "fence-omission" => Ok(FaultKind::FenceOmission),
        "policy-digest-mismatch" => Ok(FaultKind::PolicyDigestMismatch),
        "required-record-drop" => Ok(FaultKind::RequiredRecordDrop),
        _ => Err(format!("unsupported fault: {value}")),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
