//! Render an analysis as text or as JSON, the port of the Python
//! report module. Both renderings match the Python renderings
//! byte for byte on every recorded case.

use crate::jsonvalue::dumps_sorted_indented;
use crate::model::{Analysis, FileVerdict, FunctionVerdict, MutantOutcome};

/// Render the analysis as a JSON string.
pub fn to_json(analysis: &Analysis) -> String {
    dumps_sorted_indented(&analysis.to_dict())
}

fn mutant_lines(outcomes: &[&MutantOutcome], indent: &str, lines: &mut Vec<String>) {
    for outcome in outcomes {
        let mutant = &outcome.mutant;
        lines.push(format!(
            "{}mutant that walked through - {} - {} at line {} - {}",
            indent, outcome.source_path, mutant.kind, mutant.line, mutant.detail
        ));
    }
}

fn function_lines(function: &FunctionVerdict, lines: &mut Vec<String>) {
    lines.push(format!(
        "  function - {} - {}",
        function.name,
        function.status()
    ));
    match function.status() {
        "baseline_failed" => {
            lines.push(
                "    the unmutated run of this function already fails so it is not judged decorative"
                    .to_string(),
            );
        }
        "no_applicable_mutants" => {
            lines.push("    no mutant in the fixed panel applied to this function".to_string());
        }
        "decorative_candidate" => {
            lines.push(
                "    decorative candidate - this function stayed green under every applicable mutant in the fixed panel"
                    .to_string(),
            );
            mutant_lines(&function.survivors(), "    ", lines);
        }
        _ => {
            let survivors = function.survivors();
            let killed = function.outcomes.len() - survivors.len();
            lines.push(format!(
                "    this function turned red under {} of {} mutants in the fixed panel",
                killed,
                function.outcomes.len()
            ));
            mutant_lines(&survivors, "    ", lines);
        }
    }
}

fn verdict_lines(verdict: &FileVerdict, lines: &mut Vec<String>) {
    lines.push(format!("{} - {}", verdict.path, verdict.status()));
    for function in &verdict.functions {
        function_lines(function, lines);
    }
    if verdict.partial_selection {
        lines.push(
            "  this file holds test shaped definitions the selector does not name, and the verdict covers the named items only"
                .to_string(),
        );
    }
    if verdict.no_tests {
        lines.push(
            "  this file holds no test pytest could collect, so no mutant in the panel has a test to be judged by"
                .to_string(),
        );
    }
    if verdict.status() == "baseline_failed" {
        lines.push(
            "  the unmutated run already fails so this file is not judged decorative".to_string(),
        );
    } else if verdict.outcomes.is_empty() {
        lines.push("  no mutant in the fixed panel applied".to_string());
    } else if verdict.status() == "decorative_candidate" {
        lines.push(
            "  decorative candidate - every judged function in this file stayed green under every applicable mutant in the fixed panel"
                .to_string(),
        );
    } else {
        let survivors = verdict.survivors();
        let killed = verdict.outcomes.len() - survivors.len();
        lines.push(format!(
            "  this file turned red under {} of {} mutants in the fixed panel",
            killed,
            verdict.outcomes.len()
        ));
    }
}

/// Render the analysis as a human readable text report.
pub fn to_text(analysis: &Analysis) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("cantfail report".to_string());
    lines.push(format!("project - {}", analysis.project));
    lines.push("schema - cantfail-report/3".to_string());
    if !analysis.collection_settings.is_empty() {
        let parts: Vec<String> = analysis
            .collection_settings
            .iter()
            .map(|(key, patterns)| format!("{} {}", key, patterns.join(" ")))
            .collect();
        lines.push(format!("collection settings - {}", parts.join(", ")));
        if let Some(source) = &analysis.settings_source {
            lines.push(format!("collection settings source - {}", source));
        }
    }
    lines.push(String::new());
    lines.push("static findings".to_string());
    if analysis.static_findings.is_empty() {
        lines.push("none".to_string());
    }
    for finding in &analysis.static_findings {
        lines.push(format!(
            "{} line {} - {} - {}",
            finding.path, finding.line, finding.kind, finding.detail
        ));
    }
    lines.push(String::new());
    lines.push("test file verdicts".to_string());
    if analysis.verdicts.is_empty() {
        lines.push("no test files found".to_string());
    }
    for verdict in &analysis.verdicts {
        verdict_lines(verdict, &mut lines);
    }
    lines.push(String::new());
    lines.push(
        "A surviving mutant is evidence about this fixed panel only. It does not prove a test is worthless and a killed mutant does not prove a test is strong."
            .to_string(),
    );
    lines.join("\n") + "\n"
}
