use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::EXPECTED_SCHEMA_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeState {
    pub version: String,
    pub installed_at: DateTime<Utc>,
    pub mirror: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstalledDsh {
    pub version: String,
    pub installed_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DshState {
    pub current: Option<String>,
    pub known_good: Option<String>,
    pub pending: Option<String>,
    #[serde(default = "default_registry")]
    pub registry: String,
    #[serde(default)]
    pub installed: Vec<InstalledDsh>,
}

fn default_registry() -> String {
    "https://registry.npmjs.org".to_string()
}

impl Default for DshState {
    fn default() -> Self {
        Self {
            current: None,
            known_good: None,
            pending: None,
            registry: default_registry(),
            installed: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BootstrapPlan {
    pub dsh_version: String,
    pub registry: String,
    pub engines_node: Option<String>,
    pub node_version: String,
    pub requirement_source: String,
    pub resolved_at: DateTime<Utc>,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppState {
    pub schema_version: u8,
    #[serde(default)]
    pub bootstrap_plan: Option<BootstrapPlan>,
    #[serde(default)]
    pub node: Option<NodeState>,
    #[serde(default)]
    pub node_mirror: Option<String>,
    #[serde(default)]
    pub dsh: DshState,
    #[serde(default)]
    pub crash_counter: u32,
    #[serde(default)]
    pub last_crash_at: Option<DateTime<Utc>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            schema_version: EXPECTED_SCHEMA_VERSION,
            bootstrap_plan: None,
            node: None,
            node_mirror: None,
            dsh: DshState::default(),
            crash_counter: 0,
            last_crash_at: None,
        }
    }
}

#[derive(Debug)]
pub enum StateStatus {
    FirstRun,
    Loaded(Box<AppState>),
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn load_from(path: impl AsRef<std::path::Path>) -> crate::error::Result<StateStatus> {
        super::persistence::load_from(path)
    }
    pub fn load() -> crate::error::Result<StateStatus> {
        super::persistence::load()
    }
    pub fn save_to(&self, path: impl AsRef<std::path::Path>) -> crate::error::Result<()> {
        super::persistence::save_to(self, path)
    }
    pub fn save(&self) -> crate::error::Result<()> {
        super::persistence::save(self)
    }
}
