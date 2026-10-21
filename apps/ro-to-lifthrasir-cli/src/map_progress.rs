//! Live progress for the bulk map conversion: a bar on a terminal, plain lines
//! when the output is redirected.

use lifthrasir_tooling::{Diagnostic, DiagnosticLevel, Progress};
use std::io::IsTerminal;
use std::time::Duration;

/// Owns the progress bar for one `convert-maps` run.
pub struct MapProgress {
    bar: Option<indicatif::ProgressBar>,
}

impl Default for MapProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl MapProgress {
    pub fn new() -> Self {
        Self {
            bar: std::io::stderr().is_terminal().then(progress_bar),
        }
    }

    /// Runs `convert` with the progress sink and clears the bar afterwards,
    /// including on failure.
    pub fn track<T>(
        &self,
        convert: impl FnOnce(&(dyn Fn(Progress<'_>) + Sync)) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let result = convert(&|progress| self.render(progress));
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
        result
    }

    fn render(&self, progress: Progress<'_>) {
        let Some(bar) = &self.bar else {
            return print_lines(progress);
        };
        bar.set_length(progress.total as u64);
        bar.set_position(progress.done as u64);
        for diagnostic in info(progress) {
            bar.println(&diagnostic.message);
        }
    }
}

/// Warnings stay in the report, which the host renders once the run ends.
fn info(progress: Progress<'_>) -> impl Iterator<Item = &'_ Diagnostic> {
    progress
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Info)
}

fn progress_bar() -> indicatif::ProgressBar {
    let bar = indicatif::ProgressBar::new(0);
    bar.set_style(
        indicatif::ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} maps ({percent}%)",
        )
        .expect("valid progress template")
        .progress_chars("#>-"),
    );
    // Maps take seconds each; the spinner shows the run is alive between ticks.
    bar.enable_steady_tick(Duration::from_millis(120));
    bar
}

fn print_lines(progress: Progress<'_>) {
    for diagnostic in info(progress) {
        println!("{}", diagnostic.message);
    }
}
