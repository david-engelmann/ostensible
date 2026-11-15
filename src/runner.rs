//! The execution machinery of the Python runner, ported whole
//! under PORT10. Discovery, the conftest chain fold, the pairing
//! of sources, the scratch copies, the runs of pytest over those
//! copies with their timeout discipline, and the assembly of the
//! analysis from the runs, filling the model of PORT-2 exactly as
//! the Python machinery fills its own.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::model::{aggregate_outcomes, Analysis, FileVerdict, FunctionVerdict, MutantOutcome};
use crate::mutations::generate_mutants;
use crate::pyscan::scan_file;
use crate::pyselect::{has_unselected_test_shapes, selectable_test_items};
use crate::settings::{
    file_matches, folder_source, load_collection_settings_with_source, CollectionSettings,
};

/// A refusal of the machinery. Where the Python raises, the port
/// refuses in its own wording, the standing port discipline.
#[derive(Debug, Clone, PartialEq)]
pub struct RunnerError {
    pub message: String,
}

impl RunnerError {
    fn new(message: &str) -> Self {
        RunnerError {
            message: message.to_string(),
        }
    }
}

type Result<T> = std::result::Result<T, RunnerError>;

fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|_| RunnerError::new("the file could not be read"))?;
    String::from_utf8(bytes).map_err(|_| RunnerError::new("the file does not decode as UTF-8"))
}

fn relative_posix(path: &Path, base: &Path) -> String {
    let relative = path.strip_prefix(base).unwrap_or(path);
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<String>>()
        .join("/")
}

fn is_test_file(path: &Path, settings: &CollectionSettings, project: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(patterns) = &settings.python_files {
        let relative = relative_posix(path, project);
        return file_matches(&relative, &name, patterns);
    }
    name.starts_with("test_") || name.ends_with("_test.py")
}

fn discover(project: &Path, settings: &CollectionSettings) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut all: Vec<PathBuf> = Vec::new();
    let mut pending = vec![project.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let entries = std::fs::read_dir(&folder)
            .map_err(|_| RunnerError::new("the project could not be walked"))?;
        for entry in entries {
            let entry = entry.map_err(|_| RunnerError::new("the project could not be walked"))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|_| RunnerError::new("the project could not be walked"))?;
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() && path.extension().map(|ext| ext == "py").unwrap_or(false) {
                all.push(path);
            }
        }
    }
    all.sort();
    let mut tests: Vec<PathBuf> = Vec::new();
    let mut sources: Vec<PathBuf> = Vec::new();
    for path in all {
        let dotted = path
            .components()
            .any(|part| part.as_os_str().to_string_lossy().starts_with('.'));
        if dotted {
            continue;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if is_test_file(&path, settings, project) || name == "conftest.py" {
            tests.push(path);
        } else {
            sources.push(path);
        }
    }
    let test_files: Vec<PathBuf> = tests
        .into_iter()
        .filter(|path| is_test_file(path, settings, project))
        .collect();
    Ok((test_files, sources))
}

fn chain_floor(project: &Path, test_file: &Path) -> PathBuf {
    let mut folder = test_file
        .parent()
        .map(|parent| parent.to_path_buf())
        .unwrap_or_else(|| project.to_path_buf());
    loop {
        if folder_source(&folder).is_some() || folder.join("pyproject.toml").is_file() {
            return folder;
        }
        if folder == project {
            return project.to_path_buf();
        }
        match folder.parent() {
            Some(parent) => folder = parent.to_path_buf(),
            None => return project.to_path_buf(),
        }
    }
}

fn conftest_chain(project: &Path, test_file: &Path) -> Result<Vec<String>> {
    let floor = chain_floor(project, test_file);
    let mut folders: Vec<PathBuf> = Vec::new();
    let mut folder = test_file
        .parent()
        .map(|parent| parent.to_path_buf())
        .unwrap_or_else(|| project.to_path_buf());
    while folder != floor {
        folders.push(folder.clone());
        match folder.parent() {
            Some(parent) => folder = parent.to_path_buf(),
            None => break,
        }
    }
    folders.push(floor);
    let mut sources: Vec<String> = Vec::new();
    for folder in folders.iter().rev() {
        let candidate = folder.join("conftest.py");
        if candidate.is_file() {
            sources.push(read_text(&candidate)?);
        }
    }
    Ok(sources)
}

fn paired_sources(test_file: &Path, sources: &[PathBuf]) -> Vec<PathBuf> {
    let stem = test_file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let wanted = if let Some(rest) = stem.strip_prefix("test_") {
        rest.to_string()
    } else if let Some(rest) = stem.strip_suffix("_test.py") {
        format!("{rest}.py")
    } else {
        stem.clone()
    };
    let matched: Vec<PathBuf> = sources
        .iter()
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy() == wanted)
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    if matched.is_empty() {
        sources.to_vec()
    } else {
        matched
    }
}

static COPY_COUNTER: AtomicU64 = AtomicU64::new(0);

fn copy_project(project: &Path, governing_file: Option<&Path>) -> Result<(PathBuf, PathBuf)> {
    let serial = COPY_COUNTER.fetch_add(1, Ordering::SeqCst);
    let scratch = std::env::temp_dir().join(format!("ostensible-{}-{serial}", std::process::id()));
    let target = scratch.join("project");
    let outcome = (|| -> Result<()> {
        copy_tree(project, &target)?;
        if let Some(governing) = governing_file {
            let name = governing
                .file_name()
                .map(|name| name.to_os_string())
                .unwrap_or_default();
            std::fs::copy(governing, scratch.join(name))
                .map_err(|_| RunnerError::new("the governing source could not be copied"))?;
        }
        Ok(())
    })();
    match outcome {
        Ok(()) => Ok((scratch, target)),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&scratch);
            Err(error)
        }
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)
        .map_err(|_| RunnerError::new("the scratch copy could not be made"))?;
    let entries = std::fs::read_dir(from)
        .map_err(|_| RunnerError::new("the scratch copy could not be made"))?;
    for entry in entries {
        let entry = entry.map_err(|_| RunnerError::new("the scratch copy could not be made"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "__pycache__" || name == ".git" || name == ".pytest_cache" {
            continue;
        }
        let kind = entry
            .file_type()
            .map_err(|_| RunnerError::new("the scratch copy could not be made"))?;
        let destination = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else if kind.is_file() {
            if name.ends_with(".pyc") {
                continue;
            }
            std::fs::copy(entry.path(), &destination)
                .map_err(|_| RunnerError::new("the scratch copy could not be made"))?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RunOutcome {
    Passed,
    Failed,
    Timeout,
    Empty,
}

fn run_pytest(
    interpreter: &Path,
    project_copy: &Path,
    relative_test: &Path,
    timeout: Duration,
    item_name: Option<&str>,
) -> Result<RunOutcome> {
    let mut target = relative_posix(relative_test, Path::new(""));
    if target.is_empty() {
        target = relative_test.to_string_lossy().into_owned();
    }
    if let Some(item) = item_name {
        target = format!("{target}::{item}");
    }
    let mut child = Command::new(interpreter)
        .args(["-m", "pytest", "-q", "-p", "no:cacheprovider", &target])
        .current_dir(project_copy)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| RunnerError::new("the pytest run could not be started"))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(match status.code() {
                    Some(0) => RunOutcome::Passed,
                    Some(5) => RunOutcome::Empty,
                    _ => RunOutcome::Failed,
                });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(RunOutcome::Timeout);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => {
                return Err(RunnerError::new("the pytest run could not be read"));
            }
        }
    }
}

fn whole_file_outcomes(
    interpreter: &Path,
    project: &Path,
    test_file: &Path,
    sources: &[PathBuf],
    timeout: Duration,
    governing_file: Option<&Path>,
) -> Result<(bool, Vec<MutantOutcome>, bool)> {
    let relative_test = test_file.strip_prefix(project).unwrap_or(test_file);
    let (scratch, copy) = copy_project(project, governing_file)?;
    let baseline = run_pytest(interpreter, &copy, relative_test, timeout, None);
    let _ = std::fs::remove_dir_all(&scratch);
    let baseline = baseline?;
    if baseline == RunOutcome::Empty {
        return Ok((false, Vec::new(), true));
    }
    if baseline != RunOutcome::Passed {
        return Ok((false, Vec::new(), false));
    }
    let mut outcomes: Vec<MutantOutcome> = Vec::new();
    for source_file in paired_sources(test_file, sources) {
        let relative_source = source_file.strip_prefix(project).unwrap_or(&source_file);
        let text = read_text(&source_file)?;
        let mutants =
            generate_mutants(&text).map_err(|refusal| RunnerError::new(&refusal.message))?;
        for mutant in mutants {
            let (scratch, copy) = copy_project(project, governing_file)?;
            let written = (|| -> Result<RunOutcome> {
                let target = copy.join(relative_source);
                std::fs::write(&target, format!("{}\n", mutant.source))
                    .map_err(|_| RunnerError::new("the mutant could not be written"))?;
                run_pytest(interpreter, &copy, relative_test, timeout, None)
            })();
            let _ = std::fs::remove_dir_all(&scratch);
            let outcome = written?;
            outcomes.push(MutantOutcome {
                source_path: relative_posix(relative_source, Path::new("")),
                mutant,
                killed: outcome == RunOutcome::Failed,
                timed_out: outcome == RunOutcome::Timeout,
            });
        }
    }
    Ok((true, outcomes, false))
}

/// Analyse one project directory and return the full analysis,
/// as the Python machinery returns it.
pub fn analyse_project(
    project: &Path,
    timeout_seconds: f64,
    interpreter: &Path,
) -> Result<Analysis> {
    let project = std::fs::canonicalize(project)
        .map_err(|_| RunnerError::new("the project could not be read"))?;
    let timeout = Duration::from_secs_f64(timeout_seconds.max(0.0));
    let (settings, source_file) = load_collection_settings_with_source(&project);
    let mut governing_file: Option<PathBuf> = None;
    let mut settings_source: Option<String> = None;
    if let Some(found) = &source_file {
        let parent = found.parent().map(|parent| parent.to_path_buf());
        if parent.as_deref() != Some(project.as_path()) {
            governing_file = Some(found.clone());
        }
        if !settings.non_defaults().is_empty() {
            settings_source = Some(match parent {
                Some(parent) if parent == project => "project root".to_string(),
                Some(parent) => parent.to_string_lossy().into_owned(),
                None => "project root".to_string(),
            });
        }
    }
    let (test_files, sources) = discover(&project, &settings)?;

    let mut static_findings = Vec::new();
    for test_file in &test_files {
        let display = relative_posix(test_file, &project);
        let chain = conftest_chain(&project, test_file)?;
        let findings = scan_file(test_file, &display, Some(&settings), Some(&chain))
            .map_err(|refusal| RunnerError::new(&refusal.message))?;
        static_findings.extend(findings);
    }

    let mut verdicts: Vec<FileVerdict> = Vec::new();
    for test_file in &test_files {
        let relative_test = test_file.strip_prefix(&project).unwrap_or(test_file);
        let source_text = read_text(test_file)?;
        let items = selectable_test_items(&source_text, Some(&settings))
            .map_err(|refusal| RunnerError::new(&refusal.message))?;
        if items.is_empty() {
            let (whole_ok, whole_outcomes, no_tests) = whole_file_outcomes(
                interpreter,
                &project,
                test_file,
                &sources,
                timeout,
                governing_file.as_deref(),
            )?;
            verdicts.push(FileVerdict {
                path: relative_posix(relative_test, Path::new("")),
                test_functions: Vec::new(),
                baseline_ok: whole_ok,
                outcomes: whole_outcomes,
                functions: Vec::new(),
                whole_file: true,
                no_tests,
                partial_selection: false,
            });
            continue;
        }
        let mut function_verdicts: Vec<FunctionVerdict> = Vec::new();
        let (scratch, copy) = copy_project(&project, governing_file.as_deref())?;
        let baselines = (|| -> Result<()> {
            for name in &items {
                let baseline = run_pytest(interpreter, &copy, relative_test, timeout, Some(name))?;
                function_verdicts.push(FunctionVerdict {
                    name: name.clone(),
                    baseline_ok: baseline == RunOutcome::Passed,
                    outcomes: Vec::new(),
                    baseline_timed_out: baseline == RunOutcome::Timeout,
                });
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&scratch);
        baselines?;

        let judged_any = function_verdicts.iter().any(|item| item.baseline_ok);
        if judged_any {
            let mut mutants: Vec<(PathBuf, crate::model::Mutant)> = Vec::new();
            for source_file in paired_sources(test_file, &sources) {
                let relative_source = source_file.strip_prefix(&project).unwrap_or(&source_file);
                let text = read_text(&source_file)?;
                let generated = generate_mutants(&text)
                    .map_err(|refusal| RunnerError::new(&refusal.message))?;
                for mutant in generated {
                    mutants.push((relative_source.to_path_buf(), mutant));
                }
            }
            for (relative_source, mutant) in &mutants {
                let (scratch, copy) = copy_project(&project, governing_file.as_deref())?;
                let runs = (|| -> Result<Vec<(usize, RunOutcome)>> {
                    let target = copy.join(relative_source);
                    std::fs::write(&target, format!("{}\n", mutant.source))
                        .map_err(|_| RunnerError::new("the mutant could not be written"))?;
                    let mut done = Vec::new();
                    for (index, function) in function_verdicts.iter().enumerate() {
                        if !function.baseline_ok {
                            continue;
                        }
                        let outcome = run_pytest(
                            interpreter,
                            &copy,
                            relative_test,
                            timeout,
                            Some(&function.name),
                        )?;
                        done.push((index, outcome));
                    }
                    Ok(done)
                })();
                let _ = std::fs::remove_dir_all(&scratch);
                for (index, outcome) in runs? {
                    function_verdicts[index].outcomes.push(MutantOutcome {
                        source_path: relative_posix(relative_source, Path::new("")),
                        mutant: mutant.clone(),
                        killed: outcome == RunOutcome::Failed,
                        timed_out: outcome == RunOutcome::Timeout,
                    });
                }
            }
        }

        let baseline_ok = function_verdicts.iter().any(|item| item.baseline_ok);
        let outcomes = aggregate_outcomes(&function_verdicts);
        let partial_selection = has_unselected_test_shapes(&source_text, &items, Some(&settings))
            .map_err(|refusal| RunnerError::new(&refusal.message))?;
        verdicts.push(FileVerdict {
            path: relative_posix(relative_test, Path::new("")),
            test_functions: items,
            baseline_ok,
            outcomes,
            functions: function_verdicts,
            whole_file: false,
            no_tests: false,
            partial_selection,
        });
    }

    Ok(Analysis {
        project: project
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        static_findings,
        verdicts,
        collection_settings: settings.non_defaults(),
        settings_source,
    })
}
