//! The analysis model of the Python runner, ported whole. Every
//! reading the Python model computes, the status of a function
//! verdict and of a file verdict, the survivors, the judged
//! reading, the decorative candidate readings, and the fold of
//! per function outcomes into file outcomes, is computed here
//! from the same fields in the same way.

use crate::jsonvalue::JsonValue;

/// One static finding in one test file.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticFinding {
    pub kind: String,
    pub path: String,
    pub line: i64,
    pub detail: String,
}

impl StaticFinding {
    pub fn to_dict(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("kind".to_string(), JsonValue::Str(self.kind.clone())),
            ("path".to_string(), JsonValue::Str(self.path.clone())),
            ("line".to_string(), JsonValue::Int(self.line)),
            ("detail".to_string(), JsonValue::Str(self.detail.clone())),
        ])
    }
}

/// One mutant of one source module. Only the record is ported in
/// this slice. Mutant generation stays Python only.
#[derive(Debug, Clone, PartialEq)]
pub struct Mutant {
    pub kind: String,
    pub line: i64,
    pub detail: String,
}

impl Mutant {
    pub fn to_dict(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("kind".to_string(), JsonValue::Str(self.kind.clone())),
            ("line".to_string(), JsonValue::Int(self.line)),
            ("detail".to_string(), JsonValue::Str(self.detail.clone())),
        ])
    }
}

/// The outcome of one mutant against one test file or function.
#[derive(Debug, Clone, PartialEq)]
pub struct MutantOutcome {
    pub source_path: String,
    pub mutant: Mutant,
    pub killed: bool,
    pub timed_out: bool,
}

impl MutantOutcome {
    pub fn result(&self) -> &'static str {
        if self.timed_out {
            "timeout"
        } else if self.killed {
            "killed"
        } else {
            "survived"
        }
    }

    pub fn to_dict(&self) -> JsonValue {
        let mut pairs = match self.mutant.to_dict() {
            JsonValue::Object(pairs) => pairs,
            _ => unreachable!(),
        };
        pairs.push((
            "source".to_string(),
            JsonValue::Str(self.source_path.clone()),
        ));
        pairs.push((
            "result".to_string(),
            JsonValue::Str(self.result().to_string()),
        ));
        JsonValue::Object(pairs)
    }
}

/// The verdict for one test function.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionVerdict {
    pub name: String,
    pub baseline_ok: bool,
    pub outcomes: Vec<MutantOutcome>,
    pub baseline_timed_out: bool,
}

impl FunctionVerdict {
    pub fn survivors(&self) -> Vec<&MutantOutcome> {
        self.outcomes
            .iter()
            .filter(|item| !item.killed && !item.timed_out)
            .collect()
    }

    pub fn decorative_candidate(&self) -> bool {
        self.baseline_ok
            && !self.outcomes.is_empty()
            && self
                .outcomes
                .iter()
                .all(|item| !item.killed && !item.timed_out)
    }

    /// A function is judged when mutants ran against a passing
    /// baseline. Timed out outcomes still count as judged runs.
    pub fn judged(&self) -> bool {
        self.baseline_ok && !self.outcomes.is_empty()
    }

    pub fn status(&self) -> &'static str {
        if self.baseline_timed_out {
            return "inconclusive";
        }
        if !self.baseline_ok {
            return "baseline_failed";
        }
        if self.outcomes.is_empty() {
            return "no_applicable_mutants";
        }
        if self.decorative_candidate() {
            return "decorative_candidate";
        }
        if self.outcomes.iter().any(|item| item.killed) {
            return "can_fail";
        }
        if self.outcomes.iter().any(|item| item.timed_out) {
            return "inconclusive";
        }
        "can_fail"
    }

    pub fn to_dict(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("name".to_string(), JsonValue::Str(self.name.clone())),
            (
                "status".to_string(),
                JsonValue::Str(self.status().to_string()),
            ),
            (
                "mutants".to_string(),
                JsonValue::List(self.outcomes.iter().map(|item| item.to_dict()).collect()),
            ),
            (
                "surviving_mutants".to_string(),
                JsonValue::List(self.survivors().iter().map(|item| item.to_dict()).collect()),
            ),
        ])
    }
}

/// Fold per function outcomes into one outcome per mutant.
///
/// A mutant counts as killed at file level when at least one
/// judged function turned red under it. A mutant counts as timed
/// out when no function killed it and at least one run timed out.
pub fn aggregate_outcomes(functions: &[FunctionVerdict]) -> Vec<MutantOutcome> {
    let mut order: Vec<(String, String, i64, String)> = Vec::new();
    let mut grouped: Vec<Vec<MutantOutcome>> = Vec::new();
    for function in functions {
        for outcome in &function.outcomes {
            let key = (
                outcome.source_path.clone(),
                outcome.mutant.kind.clone(),
                outcome.mutant.line,
                outcome.mutant.detail.clone(),
            );
            match order.iter().position(|existing| *existing == key) {
                Some(index) => grouped[index].push(outcome.clone()),
                None => {
                    order.push(key);
                    grouped.push(vec![outcome.clone()]);
                }
            }
        }
    }
    let mut aggregated = Vec::new();
    for items in &grouped {
        let first = &items[0];
        let killed = items.iter().any(|item| item.killed);
        let timed_out = !killed && items.iter().any(|item| item.timed_out);
        aggregated.push(MutantOutcome {
            source_path: first.source_path.clone(),
            mutant: first.mutant.clone(),
            killed,
            timed_out,
        });
    }
    aggregated
}

/// The verdict for one test file, rolled up from its functions.
#[derive(Debug, Clone, PartialEq)]
pub struct FileVerdict {
    pub path: String,
    pub test_functions: Vec<String>,
    pub baseline_ok: bool,
    pub outcomes: Vec<MutantOutcome>,
    pub functions: Vec<FunctionVerdict>,
    pub whole_file: bool,
    pub no_tests: bool,
    pub partial_selection: bool,
}

impl FileVerdict {
    pub fn survivors(&self) -> Vec<&MutantOutcome> {
        self.outcomes
            .iter()
            .filter(|item| !item.killed && !item.timed_out)
            .collect()
    }

    pub fn decorative_candidate(&self) -> bool {
        if !self.functions.is_empty() && !self.whole_file {
            return self.status() == "decorative_candidate";
        }
        self.baseline_ok
            && !self.outcomes.is_empty()
            && self
                .outcomes
                .iter()
                .all(|item| !item.killed && !item.timed_out)
    }

    pub fn status(&self) -> &'static str {
        if self.whole_file {
            if self.no_tests {
                return "no_applicable_mutants";
            }
            if !self.baseline_ok {
                return "baseline_failed";
            }
            if self.outcomes.is_empty() {
                return "no_applicable_mutants";
            }
            if self.decorative_candidate() {
                return "decorative_candidate";
            }
            if self.outcomes.iter().any(|item| item.killed) {
                return "can_fail";
            }
            if self.outcomes.iter().any(|item| item.timed_out) {
                return "inconclusive";
            }
            return "can_fail";
        }
        if self.functions.is_empty() && self.outcomes.is_empty() {
            return "no_applicable_mutants";
        }
        if !self.functions.is_empty() {
            let judged: Vec<&FunctionVerdict> =
                self.functions.iter().filter(|item| item.judged()).collect();
            let has_baseline_failed = self
                .functions
                .iter()
                .any(|item| item.status() == "baseline_failed");
            if !judged.is_empty()
                && judged
                    .iter()
                    .all(|item| item.status() == "decorative_candidate")
                && !has_baseline_failed
            {
                return "decorative_candidate";
            }
            if self.functions.iter().any(|item| item.status() == "can_fail") {
                return "can_fail";
            }
            if has_baseline_failed {
                return "baseline_failed";
            }
            if self
                .functions
                .iter()
                .any(|item| item.status() == "inconclusive")
            {
                return "inconclusive";
            }
            return "no_applicable_mutants";
        }
        if !self.baseline_ok {
            return "baseline_failed";
        }
        if self.outcomes.is_empty() {
            return "no_applicable_mutants";
        }
        if self.decorative_candidate() {
            return "decorative_candidate";
        }
        "can_fail"
    }

    pub fn to_dict(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("path".to_string(), JsonValue::Str(self.path.clone())),
            (
                "test_functions".to_string(),
                JsonValue::List(
                    self.test_functions
                        .iter()
                        .map(|name| JsonValue::Str(name.clone()))
                        .collect(),
                ),
            ),
            (
                "status".to_string(),
                JsonValue::Str(self.status().to_string()),
            ),
            ("whole_file".to_string(), JsonValue::Bool(self.whole_file)),
            ("no_tests".to_string(), JsonValue::Bool(self.no_tests)),
            (
                "partial_selection".to_string(),
                JsonValue::Bool(self.partial_selection),
            ),
            (
                "functions".to_string(),
                JsonValue::List(self.functions.iter().map(|item| item.to_dict()).collect()),
            ),
            (
                "mutants".to_string(),
                JsonValue::List(self.outcomes.iter().map(|item| item.to_dict()).collect()),
            ),
            (
                "surviving_mutants".to_string(),
                JsonValue::List(self.survivors().iter().map(|item| item.to_dict()).collect()),
            ),
        ])
    }
}

/// The full analysis of one project. The collection settings are
/// held as the ordered list of key and patterns pairs the Python
/// model holds in its settings dict, which the runner fills from
/// the non default reading in the key order the settings module
/// states.
#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub project: String,
    pub static_findings: Vec<StaticFinding>,
    pub verdicts: Vec<FileVerdict>,
    pub collection_settings: Vec<(String, Vec<String>)>,
    pub settings_source: Option<String>,
}

impl Analysis {
    pub fn to_dict(&self) -> JsonValue {
        let mut pairs = vec![
            (
                "schema".to_string(),
                JsonValue::Str("cantfail-report/3".to_string()),
            ),
            (
                "project".to_string(),
                JsonValue::Str(self.project.clone()),
            ),
            (
                "static_findings".to_string(),
                JsonValue::List(
                    self.static_findings
                        .iter()
                        .map(|item| item.to_dict())
                        .collect(),
                ),
            ),
            (
                "verdicts".to_string(),
                JsonValue::List(self.verdicts.iter().map(|item| item.to_dict()).collect()),
            ),
        ];
        if !self.collection_settings.is_empty() {
            let settings = JsonValue::Object(
                self.collection_settings
                    .iter()
                    .map(|(key, patterns)| {
                        (
                            key.clone(),
                            JsonValue::List(
                                patterns
                                    .iter()
                                    .map(|pattern| JsonValue::Str(pattern.clone()))
                                    .collect(),
                            ),
                        )
                    })
                    .collect(),
            );
            pairs.push(("collection_settings".to_string(), settings));
            if let Some(source) = &self.settings_source {
                pairs.push((
                    "collection_settings_source".to_string(),
                    JsonValue::Str(source.clone()),
                ));
            }
        }
        JsonValue::Object(pairs)
    }
}
