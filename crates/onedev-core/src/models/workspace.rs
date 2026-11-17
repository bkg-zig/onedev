//! Workspace Domain Model

use super::*;

/// Workspace status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WorkspaceStatus {
    Creating,
    Running,
    Stopped,
    Stopping,
    Failed,
    Deleting,
    Deleted,
}

/// Workspace type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WorkspaceType {
    VSCode,
    IntelliJ,
    Jupyter,
    Terminal,
    Custom,
}

/// Core Workspace entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub user_id: i64,
    pub name: String,
    pub workspace_type: WorkspaceType,
    pub status: WorkspaceStatus,
    pub spec: WorkspaceSpec,
    pub container_id: Option<String>,
    pub container_image: String,
    pub cpu_limit: Option<u32>,
    pub memory_limit: Option<u64>, // bytes
    pub storage_limit: Option<u64>, // bytes
    pub ports: Vec<WorkspacePort>,
    pub volumes: Vec<WorkspaceVolume>,
    pub environment: HashMap<String, String>,
    pub git_ref: Option<String>,
    pub git_sha: Option<String>,
    pub working_dir: String,
    pub idle_timeout: u32, // minutes
    pub last_activity_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Workspace specification
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WorkspaceSpec {
    pub image: String,
    pub shell: Option<String>,
    pub extensions: Vec<String>,
    pub settings: serde_json::Value,
    pub env: HashMap<String, String>,
    pub ports: Vec<WorkspacePortSpec>,
    pub volumes: Vec<WorkspaceVolumeSpec>,
    pub resources: Option<WorkspaceResources>,
    pub lifecycle: Option<WorkspaceLifecycle>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WorkspaceResources {
    pub cpu: Option<u32>,
    pub memory: Option<String>, // e.g., "4Gi"
    pub storage: Option<String>, // e.g., "10Gi"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WorkspaceLifecycle {
    pub idle_timeout: Option<u32>, // minutes
    pub max_lifetime: Option<u32>, // minutes
    pub auto_stop: bool,
    pub auto_delete: bool,
}

/// Workspace port
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspacePort {
    pub name: String,
    pub container_port: u16,
    pub host_port: Option<u16>,
    pub protocol: PortProtocol,
    pub public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspacePortSpec {
    pub name: String,
    pub container_port: u16,
    pub protocol: PortProtocol,
    pub public: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum PortProtocol {
    Tcp,
    Udp,
}

/// Workspace volume
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceVolume {
    pub name: String,
    pub mount_path: String,
    pub volume_type: VolumeType,
    pub source: Option<String>, // PVC name, host path, etc.
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceVolumeSpec {
    pub name: String,
    pub mount_path: String,
    pub volume_type: VolumeType,
    pub size: Option<String>, // e.g., "5Gi"
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum VolumeType {
    EmptyDir,
    PersistentVolumeClaim,
    HostPath,
    ConfigMap,
    Secret,
    GitRepo,
}

/// Lightweight workspace reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceRef {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub project_path: String,
    pub name: String,
    pub workspace_type: WorkspaceType,
    pub status: WorkspaceStatus,
    pub container_image: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Workspace creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateWorkspace {
    #[validate(length(min = 1, max = 100))]
    #[validate(regex(path = *crate::utils::validation::WORKSPACE_NAME_REGEX))]
    pub name: String,

    pub workspace_type: WorkspaceType,
    pub spec: WorkspaceSpec,
    pub git_ref: Option<String>,
    pub idle_timeout: Option<u32>,
}

/// Workspace query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceQuery {
    pub status: Option<WorkspaceStatus>,
    pub workspace_type: Option<WorkspaceType>,
    pub project_id: Option<i64>,
    pub user_id: Option<i64>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

impl Workspace {
    pub fn is_active(&self) -> bool {
        matches!(self.status, WorkspaceStatus::Running | WorkspaceStatus::Creating)
    }

    pub fn is_stopped(&self) -> bool {
        matches!(self.status, WorkspaceStatus::Stopped)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.status, WorkspaceStatus::Failed | WorkspaceStatus::Deleted)
    }

    pub fn url(&self, base_url: &str) -> Option<String> {
        if self.status == WorkspaceStatus::Running {
            Some(format!("{}/-/workspaces/{}", base_url.trim_end_matches('/'), self.id))
        } else {
            None
        }
    }
}