//! ostensible asks whether a test can fail.
//!
//! This crate is the Rust port of the Python package of the same
//! name, built slice by slice against the landed Python, which
//! stands as the behavioural specification. The crate carries
//! the report reading whole, the collection settings reading
//! whole, and the static scan whole with the Python source
//! parser it carries. The mutant generation, the execution
//! machinery, and the command line with the package front doors
//! stay Python only, and the contract states that boundary by
//! name.

pub mod globmatch;
pub mod ini;
pub mod jsonvalue;
pub mod model;
pub mod pyast;
pub mod pydata;
pub mod pyparse;
pub mod pyscan;
pub mod pyselect;
pub mod pytoken;
pub mod pyunparse;
pub mod pyvalue;
pub mod pywalk;
pub mod report;
pub mod settings;
pub mod toml;

pub use model::{
    aggregate_outcomes, Analysis, FileVerdict, FunctionVerdict, Mutant, MutantOutcome,
    StaticFinding,
};
pub use settings::{
    file_matches, load_collection_settings, load_collection_settings_with_source, name_matches,
    CollectionSettings,
};
