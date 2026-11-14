//! Read the collection naming settings of an analysed project,
//! the port of the Python settings module whole. The search
//! walks from the project folder upward to the root of the
//! filesystem, and the first folder holding a governing source
//! decides, and within a folder the source forms stand in the
//! order the Python reading tries them.

use crate::globmatch::fnmatchcase;
use crate::ini::{load_ini, IniLoad};
use crate::pyvalue::{py_str, PyValue};
use crate::toml::load_toml;
use std::path::{Path, PathBuf};

/// The three naming settings, in the order the module states
/// them.
pub const KEYS: [&str; 3] = ["python_functions", "python_classes", "python_files"];

fn default_patterns(key: &str) -> Vec<String> {
    match key {
        "python_functions" => vec!["test".to_string()],
        "python_classes" => vec!["Test".to_string()],
        _ => vec!["test_*.py".to_string(), "*_test.py".to_string()],
    }
}

/// The naming settings in force for one analysed project. Each
/// field is the tuple of patterns a settings source gave, or
/// None when no setting is in force and the default applies.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CollectionSettings {
    pub python_functions: Option<Vec<String>>,
    pub python_classes: Option<Vec<String>>,
    pub python_files: Option<Vec<String>>,
}

impl CollectionSettings {
    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        match key {
            "python_functions" => self.python_functions.as_ref(),
            "python_classes" => self.python_classes.as_ref(),
            _ => self.python_files.as_ref(),
        }
    }

    fn set(&mut self, key: &str, value: Vec<String>) {
        match key {
            "python_functions" => self.python_functions = Some(value),
            "python_classes" => self.python_classes = Some(value),
            _ => self.python_files = Some(value),
        }
    }

    /// The settings in force whose patterns differ from the
    /// defaults, in the stated key order, for the report
    /// disclosure. A setting restated at exactly its default
    /// value is not a difference and is not named.
    pub fn non_defaults(&self) -> Vec<(String, Vec<String>)> {
        let mut found = Vec::new();
        for key in KEYS {
            if let Some(value) = self.get(key) {
                if *value != default_patterns(key) {
                    found.push((key.to_string(), value.clone()));
                }
            }
        }
        found
    }
}

/// Return true when a function or class name matches a pattern.
/// A name matches when it starts with the pattern or matches the
/// pattern as a glob.
pub fn name_matches(name: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| name.starts_with(pattern.as_str()) || fnmatchcase(name, pattern))
}

/// Return true when a file matches a python_files pattern. A
/// pattern holding no path separator matches the file name alone
/// as a glob. A pattern holding a separator matches the project
/// relative path as a glob.
pub fn file_matches(relative: &str, filename: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if pattern.contains('/') {
            if fnmatchcase(relative, pattern) {
                return true;
            }
        } else if fnmatchcase(filename, pattern) {
            return true;
        }
    }
    false
}

/// Read one setting value as a tuple of patterns. A string splits
/// on whitespace, the ini reading. A list keeps its items as
/// strings, the native TOML reading. Any other scalar becomes
/// its string form.
pub fn split_words(value: &PyValue) -> Vec<String> {
    match value {
        PyValue::Str(text) => {
            // Python str.split takes the Python whitespace set.
            split_python_whitespace(text)
        }
        PyValue::List(items) => items.iter().map(py_str).collect(),
        other => vec![py_str(other)],
    }
}

fn split_python_whitespace(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if crate::ini::py_isspace(ch) {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn settings_from_table(table: &PyValue) -> CollectionSettings {
    let mut settings = CollectionSettings::default();
    if let PyValue::Table(_) = table {
        for key in KEYS {
            if let Some(value) = table.get(key) {
                settings.set(key, split_words(value));
            }
        }
    }
    settings
}

fn settings_from_parser(parser: &crate::ini::IniFile, section: &str) -> CollectionSettings {
    let mut settings = CollectionSettings::default();
    for key in KEYS {
        if parser.has_option(section, key) {
            let value = parser.get(section, key).unwrap_or_default();
            settings.set(key, split_python_whitespace(&value));
        }
    }
    settings
}

fn ini_source(path: &Path, section: &str, always: bool) -> Option<CollectionSettings> {
    let parser = match load_ini(path) {
        IniLoad::Refused => return Some(CollectionSettings::default()),
        IniLoad::Parsed(parser) => parser,
    };
    if !parser.has_section(section) {
        return if always {
            Some(CollectionSettings::default())
        } else {
            None
        };
    }
    Some(settings_from_parser(&parser, section))
}

fn pytest_toml_source(path: &Path) -> CollectionSettings {
    let data = match load_toml(path) {
        Some(data) => data,
        None => return CollectionSettings::default(),
    };
    let empty = PyValue::Table(Vec::new());
    let table = data.get("pytest").unwrap_or(&empty);
    if !table.is_table() {
        return CollectionSettings::default();
    }
    settings_from_table(table)
}

fn pyproject_source(path: &Path) -> Option<CollectionSettings> {
    let data = match load_toml(path) {
        Some(data) => data,
        None => return Some(CollectionSettings::default()),
    };
    let tool = data.get("tool");
    if let Some(tool) = tool {
        if !tool.is_table() {
            return Some(CollectionSettings::default());
        }
    }
    let tool = match tool {
        Some(tool) if tool.is_table() => tool,
        _ => return None,
    };
    let table = tool.get("pytest")?;
    if !table.is_table() {
        return Some(CollectionSettings::default());
    }
    let entries = match table {
        PyValue::Table(entries) => entries,
        _ => unreachable!(),
    };
    let has_native = entries.iter().any(|(key, _)| key != "ini_options");
    let ini_options = table.get("ini_options");
    if has_native && ini_options.is_some() {
        return Some(CollectionSettings::default());
    }
    if has_native {
        return Some(settings_from_table(table));
    }
    if let Some(ini_options) = ini_options {
        if !ini_options.is_table() {
            return Some(CollectionSettings::default());
        }
        return Some(settings_from_table(ini_options));
    }
    None
}

/// The governing source of one folder, or None. The seven source
/// forms are tried in their stated order, and the first form
/// that counts as a source decides for the folder, a broken
/// source included.
pub fn folder_source(folder: &Path) -> Option<(CollectionSettings, PathBuf)> {
    for name in ["pytest.toml", ".pytest.toml"] {
        let candidate = folder.join(name);
        if candidate.is_file() {
            return Some((pytest_toml_source(&candidate), candidate));
        }
    }
    for name in ["pytest.ini", ".pytest.ini"] {
        let candidate = folder.join(name);
        if candidate.is_file() {
            if let Some(settings) = ini_source(&candidate, "pytest", true) {
                return Some((settings, candidate));
            }
        }
    }
    let pyproject = folder.join("pyproject.toml");
    if pyproject.is_file() {
        if let Some(settings) = pyproject_source(&pyproject) {
            return Some((settings, pyproject));
        }
    }
    let tox_ini = folder.join("tox.ini");
    if tox_ini.is_file() {
        if let Some(settings) = ini_source(&tox_ini, "pytest", false) {
            return Some((settings, tox_ini));
        }
    }
    let setup_cfg = folder.join("setup.cfg");
    if setup_cfg.is_file() {
        if let Some(settings) = ini_source(&setup_cfg, "tool:pytest", false) {
            return Some((settings, setup_cfg));
        }
    }
    None
}

/// Read the naming settings in force for one project, with the
/// governing source file that contributed them. The search walks
/// from the project folder upward through its parent folders to
/// the filesystem root, and the first folder holding a governing
/// source decides. The second value is that source file, or None
/// when no folder on the walk holds a source.
pub fn load_collection_settings_with_source(
    project: &Path,
) -> (CollectionSettings, Option<PathBuf>) {
    let mut folder = project.to_path_buf();
    loop {
        if let Some(found) = folder_source(&folder) {
            return (found.0, Some(found.1));
        }
        match folder.parent() {
            Some(parent) if parent != folder => {
                folder = parent.to_path_buf();
            }
            _ => return (CollectionSettings::default(), None),
        }
    }
}

/// Read the naming settings in force for one project.
pub fn load_collection_settings(project: &Path) -> CollectionSettings {
    load_collection_settings_with_source(project).0
}
