//! Definition (theorem, lemma, def, example) discovery.

use serde::{Deserialize, Serialize};

/// Information about the enclosing definition (theorem, lemma, def, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionInfo {
    /// Kind of definition (theorem, lemma, def, example)
    #[serde(default)]
    pub kind: Option<String>,
    /// Name of the definition
    pub name: String,
    /// Line where the definition starts
    #[serde(default)]
    pub line: Option<u32>,
}
