//! Package Registry Domain Model

use super::*;

/// Package type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum PackageType {
    Maven,
    Npm,
    PyPI,
    NuGet,
    Cargo,
    Go,
    Helm,
    Docker,
    Gem,
    Composer,
    Conan,
    Conda,
    Generic,
}

/// Package status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PackageStatus {
    Active,
    Hidden,
    Deleted,
}

/// Core Package entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Package {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub name: String,
    pub version: String,
    pub package_type: PackageType,
    pub status: PackageStatus,
    pub description: Option<String>,
    pub metadata: serde_json::Value, // Type-specific metadata
    // Tags/Labels
    pub tags: Vec<String>,
    // Files
    pub files: Vec<PackageFile>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by_id: Option<i64>,
    // Statistics
    pub download_count: u64,
    pub size: u64, // Total size in bytes
}

/// Package file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackageFile {
    pub id: i64,
    pub uuid: Uuid,
    pub package_id: i64,
    pub file_name: String,
    pub file_path: String, // Storage path
    pub size: u64,
    pub sha256: String,
    pub md5: Option<String>,
    pub mime_type: Option<String>,
    pub file_type: PackageFileType,
    pub created_at: DateTime<Utc>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PackageFileType {
    Primary,
    Asset,
    Signature,
    Checksum,
    Metadata,
    Documentation,
    Source,
    Other,
}

/// Package reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackageRef {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub project_path: String,
    pub name: String,
    pub version: String,
    pub package_type: PackageType,
    pub status: PackageStatus,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub download_count: u64,
}

/// Package query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageQuery {
    pub term: Option<String>,
    pub package_type: Option<PackageType>,
    pub status: Option<PackageStatus>,
    pub project_id: Option<i64>,
    pub tags: Vec<String>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Package version
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackageVersion {
    pub id: i64,
    pub uuid: Uuid,
    pub package_id: i64,
    pub version: String,
    pub status: PackageStatus,
    pub release: bool,
    pub metadata: serde_json::Value,
    pub files: Vec<PackageFile>,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by_id: Option<i64>,
}

/// Package creation (via registry API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPackage {
    pub package_type: PackageType,
    pub name: String,
    pub version: String,
    pub metadata: serde_json::Value,
    pub files: Vec<PublishPackageFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPackageFile {
    pub file_name: String,
    pub content: String, // Base64 encoded
    pub file_type: PackageFileType,
}

/// Maven-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MavenMetadata {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    pub packaging: String,
    pub classifier: Option<String>,
    pub dependencies: Vec<MavenDependency>,
    pub plugins: Vec<MavenPlugin>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MavenDependency {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    pub scope: Option<String>,
    pub optional: bool,
    pub exclusions: Vec<MavenExclusion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MavenExclusion {
    pub group_id: String,
    pub artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MavenPlugin {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    pub configuration: Option<serde_json::Value>,
}

/// NPM-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpmMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub main: Option<String>,
    pub scripts: HashMap<String, String>,
    pub dependencies: HashMap<String, String>,
    pub dev_dependencies: HashMap<String, String>,
    pub peer_dependencies: HashMap<String, String>,
    pub optional_dependencies: HashMap<String, String>,
    pub bundled_dependencies: Vec<String>,
    pub bin: HashMap<String, String>,
    pub files: Vec<String>,
    pub engines: HashMap<String, String>,
    pub publish_config: Option<serde_json::Value>,
}

/// PyPI-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PypiMetadata {
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub description_content_type: Option<String>,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub maintainer: Option<String>,
    pub maintainer_email: Option<String>,
    pub license: Option<String>,
    pub classifiers: Vec<String>,
    pub keywords: Vec<String>,
    pub home_page: Option<String>,
    pub download_url: Option<String>,
    pub project_urls: HashMap<String, String>,
    pub requires_python: Option<String>,
    pub requires_dist: Vec<String>,
    pub provides_dist: Vec<String>,
    pub obsoletes_dist: Vec<String>,
    pub platform: Option<String>,
    pub supported_platforms: Vec<String>,
}

/// Docker/OCI Image metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerImageMetadata {
    pub name: String,
    pub tag: String,
    pub digest: String,
    pub size: u64,
    pub architecture: String,
    pub os: String,
    pub layers: Vec<DockerLayer>,
    pub config: serde_json::Value,
    pub created: DateTime<Utc>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerLayer {
    pub digest: String,
    pub size: u64,
    pub media_type: String,
    pub instruction: Option<String>,
}

/// Generic package metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenericMetadata {
    pub format: String,
    pub attributes: HashMap<String, serde_json::Value>,
}

impl Package {
    pub fn reference(&self) -> String {
        match self.package_type {
            PackageType::Maven => format!(
                "{}:{}:{}",
                self.metadata
                    .get("group_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                self.name,
                self.version
            ),
            PackageType::Npm => format!("{}@{}", self.name, self.version),
            PackageType::PyPI => format!("{}=={}", self.name, self.version),
            PackageType::Docker => format!("{}:{}", self.name, self.version),
            _ => format!("{}@{}", self.name, self.version),
        }
    }

    pub fn is_published(&self) -> bool {
        self.published_at.is_some() && self.status == PackageStatus::Active
    }

    pub fn primary_file(&self) -> Option<&PackageFile> {
        self.files
            .iter()
            .find(|f| f.file_type == PackageFileType::Primary)
    }
}
