use lifthrasir_pak::SourceEntries;
use std::{error::Error, fmt, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
}

/// Primary published artifacts and operation-level diagnostics, not every sidecar/progress tick.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperationReport {
    pub outputs: Vec<PathBuf>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct PackInputs {
    pub entries: SourceEntries,
    pub skipped: usize,
    pub excluded: usize,
    pub diagnostics: Vec<Diagnostic>,
}

/// Successful outputs and diagnostics remain inspectable when a batch is incomplete.
#[derive(Debug)]
pub struct BatchFailure {
    pub report: OperationReport,
    pub failures: Vec<String>,
}

impl fmt::Display for BatchFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "conversion failed for {} item(s): {}",
            self.failures.len(),
            self.failures.join("; ")
        )
    }
}
impl Error for BatchFailure {}

impl OperationReport {
    pub(crate) fn info(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Info,
            message,
        });
    }
    pub(crate) fn warning(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            message,
        });
    }
    pub(crate) fn merge(&mut self, other: Self) {
        self.outputs.extend(other.outputs);
        self.diagnostics.extend(other.diagnostics);
    }
    pub(crate) fn include(
        &mut self,
        result: anyhow::Result<Self>,
        failures: &mut Vec<String>,
    ) -> bool {
        match result {
            Ok(report) => {
                self.merge(report);
                true
            }
            Err(error) => {
                match error.downcast::<BatchFailure>() {
                    Ok(batch) => {
                        self.merge(batch.report);
                        failures.extend(batch.failures);
                    }
                    Err(error) => {
                        let message = format!("{error:#}");
                        self.warning(message.clone());
                        failures.push(message);
                    }
                }
                false
            }
        }
    }
    pub(crate) fn finish(self, failures: Vec<String>) -> anyhow::Result<Self> {
        if failures.is_empty() {
            Ok(self)
        } else {
            Err(BatchFailure {
                report: self,
                failures,
            }
            .into())
        }
    }
}

pub(crate) fn add_diagnostics(
    result: anyhow::Result<OperationReport>,
    diagnostics: Vec<Diagnostic>,
) -> anyhow::Result<OperationReport> {
    let mut report = OperationReport::default();
    let mut failures = Vec::new();
    report.include(result, &mut failures);
    report.diagnostics.extend(diagnostics);
    report.finish(failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregation_keeps_successes_and_nested_partial_failures() {
        let mut combined = OperationReport::default();
        let mut failures = Vec::new();
        let mut success = OperationReport {
            outputs: vec!["first.glb".into()],
            ..Default::default()
        };
        success.info("first converted".into());
        assert!(combined.include(Ok(success), &mut failures));
        let mut partial = OperationReport {
            outputs: vec!["second.glb".into()],
            ..Default::default()
        };
        partial.warning("missing optional texture".into());
        assert!(
            !combined.include(
                Err(BatchFailure {
                    report: partial,
                    failures: vec!["bad source: missing.rsm".into()]
                }
                .into()),
                &mut failures
            )
        );
        let error = combined.finish(failures).unwrap_err();
        let batch = error.downcast_ref::<BatchFailure>().unwrap();
        assert_eq!(
            batch.report.outputs,
            vec![PathBuf::from("first.glb"), PathBuf::from("second.glb")]
        );
        assert_eq!(batch.report.diagnostics.len(), 2);
        assert_eq!(batch.failures, vec!["bad source: missing.rsm"]);
    }
}
