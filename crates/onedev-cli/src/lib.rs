//! OneDev Admin CLI (stack control, docker compose, dev tools)

use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;
use strum::Display;

#[derive(Parser, Debug)]
#[command(name = "onedev")]
#[command(about = "OneDev Admin CLI - Stack Control, Docker Compose, Dev Tools")]
#[command(version, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Configuration file path
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output format
    #[arg(short, long, global = true, default_value = "table")]
    pub output: OutputFormat,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
    Csv,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Stack management (docker compose)
    Stack(StackCommands),

    /// Project management
    Project(ProjectCommands),

    /// User management
    User(UserCommands),

    /// Build management
    Build(BuildCommands),

    /// Git operations
    Git(GitCommands),

    /// Database operations
    Db(DbCommands),

    /// Configuration management
    Config(ConfigCommands),

    /// Backup and restore
    Backup(BackupCommands),

    /// System administration
    Admin(AdminCommands),

    /// Development tools
    Dev(DevCommands),

    /// Generate shell completions
    Completion(CompletionArgs),
}

#[derive(Debug, Args)]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

/// Stack management commands
#[derive(Debug, Args)]
pub struct StackCommands {
    #[command(subcommand)]
    pub command: StackSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum StackSubcommand {
    /// Start the stack
    Up {
        /// Detach mode
        #[arg(short, long)]
        detach: bool,

        /// Build images before starting
        #[arg(long)]
        build: bool,

        /// Services to start (default: all)
        #[arg(value_name = "SERVICE")]
        services: Vec<String>,
    },

    /// Stop the stack
    Down {
        /// Remove volumes
        #[arg(long)]
        volumes: bool,

        /// Remove images
        #[arg(long)]
        rmi: bool,

        /// Services to stop (default: all)
        #[arg(value_name = "SERVICE")]
        services: Vec<String>,
    },

    /// Restart services
    Restart {
        /// Services to restart (default: all)
        #[arg(value_name = "SERVICE")]
        services: Vec<String>,
    },

    /// Show stack status
    Status {
        /// Show all services (including stopped)
        #[arg(short, long)]
        all: bool,
    },

    /// View logs
    Logs {
        /// Follow logs
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, long, default_value = "100")]
        tail: usize,

        /// Services to show logs for (default: all)
        #[arg(value_name = "SERVICE")]
        services: Vec<String>,
    },

    /// Execute command in service
    Exec {
        /// Service name
        #[arg(value_name = "SERVICE")]
        service: String,

        /// Command to execute
        #[arg(value_name = "COMMAND", last = true)]
        command: Vec<String>,
    },

    /// Show running processes
    Ps {
        /// Show all services (including stopped)
        #[arg(short, long)]
        all: bool,
    },

    /// Pull latest images
    Pull {
        /// Services to pull (default: all)
        #[arg(value_name = "SERVICE")]
        services: Vec<String>,
    },

    /// Build images
    Build {
        /// Services to build (default: all)
        #[arg(value_name = "SERVICE")]
        services: Vec<String>,

        /// Don't use cache
        #[arg(long)]
        no_cache: bool,
    },
}

/// Project management commands
#[derive(Debug, Args)]
pub struct ProjectCommands {
    #[command(subcommand)]
    pub command: ProjectSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectSubcommand {
    /// List projects
    List {
        /// Filter by visibility
        #[arg(long, value_enum)]
        visibility: Option<VisibilityFilter>,

        /// Search query
        #[arg(short, long)]
        search: Option<String>,
    },

    /// Show project details
    Show {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,
    },

    /// Create a new project
    Create {
        /// Project name
        #[arg(value_name = "NAME")]
        name: String,

        /// Description
        #[arg(short, long)]
        description: Option<String>,

        /// Visibility
        #[arg(long, value_enum, default_value = "private")]
        visibility: Visibility,

        /// Initialize with README
        #[arg(long)]
        readme: bool,

        /// Template to use
        #[arg(long)]
        template: Option<String>,
    },

    /// Update project
    Update {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// New name
        #[arg(long)]
        name: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New visibility
        #[arg(long, value_enum)]
        visibility: Option<Visibility>,
    },

    /// Delete project
    Delete {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Force delete without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Fork project
    Fork {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Target namespace
        #[arg(long)]
        namespace: Option<String>,
    },

    /// Transfer project
    Transfer {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Target user/group
        #[arg(value_name = "TARGET")]
        target: String,
    },

    /// Archive/unarchive project
    Archive {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Unarchive instead of archive
        #[arg(long)]
        unarchive: bool,
    },
}

/// User management commands
#[derive(Debug, Args)]
pub struct UserCommands {
    #[command(subcommand)]
    pub command: UserSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum UserSubcommand {
    /// List users
    List {
        /// Filter by status
        #[arg(long, value_enum)]
        status: Option<UserStatusFilter>,

        /// Search query
        #[arg(short, long)]
        search: Option<String>,
    },

    /// Show user details
    Show {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,
    },

    /// Create a new user
    Create {
        /// Username
        #[arg(value_name = "USERNAME")]
        username: String,

        /// Email
        #[arg(value_name = "EMAIL")]
        email: String,

        /// Full name
        #[arg(short, long)]
        name: Option<String>,

        /// Password
        #[arg(long)]
        password: Option<String>,

        /// Make admin
        #[arg(long)]
        admin: bool,
    },

    /// Update user
    Update {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// New email
        #[arg(long)]
        email: Option<String>,

        /// New full name
        #[arg(long)]
        name: Option<String>,

        /// New password
        #[arg(long)]
        password: Option<String>,

        /// Toggle admin status
        #[arg(long)]
        admin: Option<bool>,
    },

    /// Delete user
    Delete {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// Force delete without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Impersonate user
    Impersonate {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,
    },

    /// Manage SSH keys
    SshKey(SshKeyCommands),

    /// Manage GPG keys
    GpgKey(GpgKeyCommands),

    /// Manage personal access tokens
    Token(TokenCommands),
}

/// Build management commands
#[derive(Debug, Args)]
pub struct BuildCommands {
    #[command(subcommand)]
    pub command: BuildSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum BuildSubcommand {
    /// List builds
    List {
        /// Filter by project
        #[arg(long)]
        project: Option<String>,

        /// Filter by status
        #[arg(long, value_enum)]
        status: Option<BuildStatusFilter>,

        /// Filter by branch
        #[arg(long)]
        branch: Option<String>,
    },

    /// Show build details
    Show {
        /// Build ID
        #[arg(value_name = "BUILD_ID")]
        id: String,
    },

    /// Trigger a new build
    Trigger {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Pipeline name
        #[arg(value_name = "PIPELINE")]
        pipeline: String,

        /// Branch or tag
        #[arg(value_name = "REF")]
        ref_name: String,

        /// Build variables (KEY=VALUE)
        #[arg(short, long, value_name = "KEY=VALUE")]
        variables: Vec<String>,
    },

    /// Cancel a running build
    Cancel {
        /// Build ID
        #[arg(value_name = "BUILD_ID")]
        id: String,
    },

    /// Retry a failed build
    Retry {
        /// Build ID
        #[arg(value_name = "BUILD_ID")]
        id: String,
    },

    /// View build logs
    Logs {
        /// Build ID
        #[arg(value_name = "BUILD_ID")]
        id: String,

        /// Follow logs
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, long, default_value = "100")]
        tail: usize,
    },

    /// List artifacts
    Artifacts {
        /// Build ID
        #[arg(value_name = "BUILD_ID")]
        id: String,
    },
}

/// Git operations commands
#[derive(Debug, Args)]
pub struct GitCommands {
    #[command(subcommand)]
    pub command: GitSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum GitSubcommand {
    /// List branches
    Branch {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Show all branches (including remote)
        #[arg(short, long)]
        all: bool,
    },

    /// List tags
    Tag {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,
    },

    /// Show commit details
    Commit {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Commit SHA
        #[arg(value_name = "SHA")]
        sha: String,
    },

    /// List files in tree
    Ls {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Git ref (branch, tag, commit)
        #[arg(value_name = "REF")]
        ref_name: String,

        /// Path within repository
        #[arg(value_name = "PATH")]
        path: Option<String>,
    },

    /// Show file content
    Cat {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Git ref (branch, tag, commit)
        #[arg(value_name = "REF")]
        ref_name: String,

        /// File path
        #[arg(value_name = "PATH")]
        path: String,
    },

    /// Show commit history
    Log {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Git ref (branch, tag, commit)
        #[arg(value_name = "REF")]
        ref_name: Option<String>,

        /// Number of commits to show
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// File path to filter
        #[arg(value_name = "PATH")]
        path: Option<String>,
    },

    /// Show diff
    Diff {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Base ref
        #[arg(value_name = "BASE")]
        base: String,

        /// Head ref
        #[arg(value_name = "HEAD")]
        head: String,

        /// File path to filter
        #[arg(value_name = "PATH")]
        path: Option<String>,
    },

    /// Manage hooks
    Hook(GitHookCommands),
}

/// Database operations commands
#[derive(Debug, Args)]
pub struct DbCommands {
    #[command(subcommand)]
    pub command: DbSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum DbSubcommand {
    /// Run migrations
    Migrate {
        /// Target migration version (default: latest)
        #[arg(value_name = "VERSION")]
        version: Option<String>,
    },

    /// Create migration
    CreateMigration {
        /// Migration name
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// Show migration status
    Status,

    /// Rollback migration
    Rollback {
        /// Number of steps to rollback
        #[arg(value_name = "STEPS", default_value = "1")]
        steps: u32,
    },

    /// Backup database
    Backup {
        /// Output file path
        #[arg(value_name = "PATH", default_value = "onedev_backup.sql")]
        path: PathBuf,
    },

    /// Restore database
    Restore {
        /// Input file path
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// Drop existing tables first
        #[arg(long)]
        drop: bool,
    },

    /// Vacuum database
    Vacuum {
        /// Full vacuum
        #[arg(long)]
        full: bool,
    },

    /// Show database statistics
    Stats,
}

/// Configuration management commands
#[derive(Debug, Args)]
pub struct ConfigCommands {
    #[command(subcommand)]
    pub command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigSubcommand {
    /// Show current configuration
    Show {
        /// Show all (including sensitive)
        #[arg(long)]
        all: bool,
    },

    /// Get a configuration value
    Get {
        /// Configuration key
        #[arg(value_name = "KEY")]
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        #[arg(value_name = "KEY")]
        key: String,

        /// Configuration value
        #[arg(value_name = "VALUE")]
        value: String,
    },

    /// Reset configuration to defaults
    Reset {
        /// Confirm reset
        #[arg(long)]
        confirm: bool,
    },

    /// Validate configuration
    Validate,
}

/// Backup and restore commands
#[derive(Debug, Args)]
pub struct BackupCommands {
    #[command(subcommand)]
    pub command: BackupSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum BackupSubcommand {
    /// Create a full backup
    Create {
        /// Output directory
        #[arg(value_name = "DIR", default_value = "./backups")]
        dir: PathBuf,

        /// Include repositories
        #[arg(long)]
        include_repos: bool,

        /// Include artifacts
        #[arg(long)]
        include_artifacts: bool,

        /// Include LFS objects
        #[arg(long)]
        include_lfs: bool,
    },

    /// Restore from backup
    Restore {
        /// Backup directory
        #[arg(value_name = "DIR")]
        dir: PathBuf,

        /// Restore repositories
        #[arg(long)]
        include_repos: bool,

        /// Restore artifacts
        #[arg(long)]
        include_artifacts: bool,

        /// Restore LFS objects
        #[arg(long)]
        include_lfs: bool,
    },

    /// List backups
    List {
        /// Backup directory
        #[arg(value_name = "DIR", default_value = "./backups")]
        dir: PathBuf,
    },

    /// Verify backup integrity
    Verify {
        /// Backup directory
        #[arg(value_name = "DIR")]
        dir: PathBuf,
    },
}

/// System administration commands
#[derive(Debug, Args)]
pub struct AdminCommands {
    #[command(subcommand)]
    pub command: AdminSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum AdminSubcommand {
    /// Show system information
    Info,

    /// Show system health
    Health,

    /// Show system metrics
    Metrics {
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },

    /// Manage users
    User(UserAdminCommands),

    /// Manage projects
    Project(ProjectAdminCommands),

    /// Manage groups
    Group(GroupAdminCommands),

    /// Rebuild search index
    Reindex {
        /// Project ID (default: all)
        #[arg(value_name = "PROJECT_ID")]
        project: Option<String>,
    },

    /// Cleanup old data
    Cleanup {
        /// Days to keep (default: 30)
        #[arg(long, default_value = "30")]
        days: u32,

        /// Dry run
        #[arg(long)]
        dry_run: bool,
    },

    /// Maintenance mode
    Maintenance {
        /// Enable or disable
        #[arg(value_enum)]
        action: MaintenanceAction,
    },
}

/// Development tools commands
#[derive(Debug, Args)]
pub struct DevCommands {
    #[command(subcommand)]
    pub command: DevSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum DevSubcommand {
    /// Generate code
    Generate {
        /// Generator to use
        #[arg(value_enum)]
        generator: GeneratorType,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
    },

    /// Run tests
    Test {
        /// Test filter
        #[arg(value_name = "FILTER")]
        filter: Option<String>,

        /// Run with coverage
        #[arg(long)]
        coverage: bool,
    },

    /// Lint code
    Lint {
        /// Fix automatically
        #[arg(long)]
        fix: bool,
    },

    /// Format code
    Format {
        /// Check only (don't write)
        #[arg(long)]
        check: bool,
    },

    /// Start development server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "46315")]
        port: u16,

        /// Open browser
        #[arg(long)]
        open: bool,
    },
}

/// User admin commands
#[derive(Debug, Args)]
pub struct UserAdminCommands {
    #[command(subcommand)]
    pub command: UserAdminSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum UserAdminSubcommand {
    /// List all users
    List {
        /// Include inactive
        #[arg(long)]
        inactive: bool,
    },

    /// Create admin user
    CreateAdmin {
        /// Username
        #[arg(value_name = "USERNAME")]
        username: String,

        /// Email
        #[arg(value_name = "EMAIL")]
        email: String,

        /// Password
        #[arg(value_name = "PASSWORD")]
        password: String,
    },

    /// Reset user password
    ResetPassword {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// New password
        #[arg(value_name = "PASSWORD")]
        password: String,
    },

    /// Toggle user active status
    ToggleActive {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,
    },

    /// Manage user roles
    Role {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// Role to add/remove
        #[arg(value_name = "ROLE")]
        role: String,

        /// Add or remove
        #[arg(value_enum)]
        action: RoleAction,
    },
}

/// Project admin commands
#[derive(Debug, Args)]
pub struct ProjectAdminCommands {
    #[command(subcommand)]
    pub command: ProjectAdminSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectAdminSubcommand {
    /// List all projects
    List {
        /// Include archived
        #[arg(long)]
        archived: bool,
    },

    /// Delete project (force)
    Delete {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,
    },

    /// Transfer ownership
    Transfer {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// New owner
        #[arg(value_name = "OWNER")]
        owner: String,
    },

    /// Archive/unarchive
    Archive {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Unarchive instead of archive
        #[arg(long)]
        unarchive: bool,
    },
}

/// Group admin commands
#[derive(Debug, Args)]
pub struct GroupAdminCommands {
    #[command(subcommand)]
    pub command: GroupAdminSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum GroupAdminSubcommand {
    /// List all groups
    List,

    /// Create group
    Create {
        /// Group name
        #[arg(value_name = "NAME")]
        name: String,

        /// Parent group
        #[arg(long)]
        parent: Option<String>,
    },

    /// Delete group
    Delete {
        /// Group ID or name
        #[arg(value_name = "GROUP")]
        group: String,
    },

    /// Manage members
    Member {
        /// Group ID or name
        #[arg(value_name = "GROUP")]
        group: String,

        /// User to add/remove
        #[arg(value_name = "USER")]
        user: String,

        /// Add or remove
        #[arg(value_enum)]
        action: MemberAction,
    },
}

/// Git hook commands
#[derive(Debug, Args)]
pub struct GitHookCommands {
    #[command(subcommand)]
    pub command: GitHookSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum GitHookSubcommand {
    /// List hooks
    List {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,
    },

    /// Add hook
    Add {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Hook name
        #[arg(value_name = "NAME")]
        name: String,

        /// Hook script
        #[arg(value_name = "SCRIPT")]
        script: String,
    },

    /// Remove hook
    Remove {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Hook name
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// Enable/disable hook
    Toggle {
        /// Project ID or path
        #[arg(value_name = "PROJECT")]
        project: String,

        /// Hook name
        #[arg(value_name = "NAME")]
        name: String,

        /// Enable or disable
        #[arg(value_enum)]
        action: ToggleAction,
    },
}

/// SSH key commands
#[derive(Debug, Args)]
pub struct SshKeyCommands {
    #[command(subcommand)]
    pub command: SshKeySubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SshKeySubcommand {
    /// List SSH keys
    List {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,
    },

    /// Add SSH key
    Add {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// Key title
        #[arg(value_name = "TITLE")]
        title: String,

        /// Public key file or key content
        #[arg(value_name = "KEY")]
        key: String,
    },

    /// Remove SSH key
    Remove {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// Key ID
        #[arg(value_name = "KEY_ID")]
        key_id: String,
    },
}

/// GPG key commands
#[derive(Debug, Args)]
pub struct GpgKeyCommands {
    #[command(subcommand)]
    pub command: GpgKeySubcommand,
}

#[derive(Debug, Subcommand)]
pub enum GpgKeySubcommand {
    /// List GPG keys
    List {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,
    },

    /// Add GPG key
    Add {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// Key ID
        #[arg(value_name = "KEY_ID")]
        key_id: String,
    },

    /// Remove GPG key
    Remove {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// Key ID
        #[arg(value_name = "KEY_ID")]
        key_id: String,
    },
}

/// Token commands
#[derive(Debug, Args)]
pub struct TokenCommands {
    #[command(subcommand)]
    pub command: TokenSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TokenSubcommand {
    /// List tokens
    List {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,
    },

    /// Create token
    Create {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// Token name
        #[arg(value_name = "NAME")]
        name: String,

        /// Scopes
        #[arg(value_name = "SCOPE")]
        scopes: Vec<String>,

        /// Expiry in days
        #[arg(long, default_value = "30")]
        expiry: u32,
    },

    /// Revoke token
    Revoke {
        /// User ID or username
        #[arg(value_name = "USER")]
        user: String,

        /// Token ID
        #[arg(value_name = "TOKEN_ID")]
        token_id: String,
    },
}

/// Supporting types
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum VisibilityFilter {
    Private,
    Internal,
    Public,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum Visibility {
    Private,
    Internal,
    Public,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum UserStatusFilter {
    Active,
    Inactive,
    Blocked,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum BuildStatusFilter {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum MaintenanceAction {
    Enable,
    Disable,
}

#[derive(Debug, Clone, clap::ValueEnum, Display)]
pub enum RoleAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, clap::ValueEnum, Display)]
pub enum MemberAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, clap::ValueEnum, Display)]
pub enum ToggleAction {
    Enable,
    Disable,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum GeneratorType {
    Project,
    Controller,
    Service,
    Model,
    Migration,
}

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub database_url: Option<String>,
    pub log_level: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            database_url: Some("postgres://onedev:onedev@localhost:5432/onedev".to_string()),
            log_level: Some("info".to_string()),
        }
    }
}

/// Load configuration
async fn load_config(_path: Option<&std::path::Path>) -> anyhow::Result<CliConfig> {
    Ok(CliConfig::default())
}

/// Run CLI
pub async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    // Initialize config
    let config = load_config(cli.config.as_deref()).await?;

    // Set up logging
    if cli.verbose {
        unsafe {
            std::env::set_var("RUST_LOG", "debug");
        }
    }

    match cli.command {
        Commands::Stack(cmd) => handle_stack(cmd, &config).await,
        Commands::Project(cmd) => handle_project(cmd, &config).await,
        Commands::User(cmd) => handle_user(cmd, &config).await,
        Commands::Build(cmd) => handle_build(cmd, &config).await,
        Commands::Git(cmd) => handle_git(cmd, &config).await,
        Commands::Db(cmd) => handle_db(cmd, &config).await,
        Commands::Config(cmd) => handle_config(cmd, &config).await,
        Commands::Backup(cmd) => handle_backup(cmd, &config).await,
        Commands::Admin(cmd) => handle_admin(cmd, &config).await,
        Commands::Dev(cmd) => handle_dev(cmd, &config).await,
        Commands::Completion(args) => handle_completion(args),
    }
}

/// Handler functions
async fn handle_stack(cmd: StackCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        StackSubcommand::Up {
            detach,
            build,
            services,
        } => {
            println!(
                "Starting stack (detach={}, build={}, services={:?})",
                detach, build, services
            );
        }
        StackSubcommand::Down {
            volumes,
            rmi,
            services,
        } => {
            println!(
                "Stopping stack (volumes={}, rmi={}, services={:?})",
                volumes, rmi, services
            );
        }
        StackSubcommand::Restart { services } => {
            println!("Restarting services: {:?}", services);
        }
        StackSubcommand::Status { all } => {
            println!("Stack status (all={})", all);
        }
        StackSubcommand::Logs {
            follow,
            tail,
            services,
        } => {
            println!(
                "Viewing logs (follow={}, tail={}, services={:?})",
                follow, tail, services
            );
        }
        StackSubcommand::Exec { service, command } => {
            println!("Executing in {}: {:?}", service, command);
        }
        StackSubcommand::Ps { all } => {
            println!("Processes (all={})", all);
        }
        StackSubcommand::Pull { services } => {
            println!("Pulling images for: {:?}", services);
        }
        StackSubcommand::Build { services, no_cache } => {
            println!(
                "Building images (services={:?}, no_cache={})",
                services, no_cache
            );
        }
    }
    Ok(())
}

async fn handle_project(cmd: ProjectCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        ProjectSubcommand::List { visibility, search } => {
            println!(
                "Listing projects (visibility={:?}, search={:?})",
                visibility, search
            );
        }
        ProjectSubcommand::Show { project } => {
            println!("Showing project: {}", project);
        }
        ProjectSubcommand::Create {
            name,
            description: _,
            visibility,
            readme,
            template,
        } => {
            println!(
                "Creating project: {} (visibility={:?}, readme={}, template={:?})",
                name, visibility, readme, template
            );
        }
        ProjectSubcommand::Update {
            project,
            name,
            description: _,
            visibility,
        } => {
            println!(
                "Updating project: {} (name={:?}, visibility={:?})",
                project, name, visibility
            );
        }
        ProjectSubcommand::Delete { project, force } => {
            println!("Deleting project: {} (force={})", project, force);
        }
        ProjectSubcommand::Fork { project, namespace } => {
            println!("Forking project: {} (namespace={:?})", project, namespace);
        }
        ProjectSubcommand::Transfer { project, target } => {
            println!("Transferring project: {} to {}", project, target);
        }
        ProjectSubcommand::Archive { project, unarchive } => {
            println!(
                "{} project: {}",
                if unarchive {
                    "Unarchiving"
                } else {
                    "Archiving"
                },
                project
            );
        }
    }
    Ok(())
}

async fn handle_user(cmd: UserCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        UserSubcommand::List { status, search } => {
            println!("Listing users (status={:?}, search={:?})", status, search);
        }
        UserSubcommand::Show { user } => {
            println!("Showing user: {}", user);
        }
        UserSubcommand::Create {
            username,
            email,
            name: _,
            password: _,
            admin,
        } => {
            println!(
                "Creating user: {} (email={}, admin={})",
                username, email, admin
            );
        }
        UserSubcommand::Update {
            user,
            email,
            name: _,
            password: _,
            admin,
        } => {
            println!(
                "Updating user: {} (email={:?}, admin={:?})",
                user, email, admin
            );
        }
        UserSubcommand::Delete { user, force } => {
            println!("Deleting user: {} (force={})", user, force);
        }
        UserSubcommand::Impersonate { user } => {
            println!("Impersonating user: {}", user);
        }
        UserSubcommand::SshKey(cmd) => handle_ssh_key(cmd).await?,
        UserSubcommand::GpgKey(cmd) => handle_gpg_key(cmd).await?,
        UserSubcommand::Token(cmd) => handle_token(cmd).await?,
    }
    Ok(())
}

async fn handle_ssh_key(cmd: SshKeyCommands) -> anyhow::Result<()> {
    match cmd.command {
        SshKeySubcommand::List { user } => {
            println!("Listing SSH keys for user: {}", user);
        }
        SshKeySubcommand::Add {
            user,
            title,
            key: _,
        } => {
            println!("Adding SSH key for user: {} (title={})", user, title);
        }
        SshKeySubcommand::Remove { user, key_id } => {
            println!("Removing SSH key {} for user: {}", key_id, user);
        }
    }
    Ok(())
}

async fn handle_gpg_key(cmd: GpgKeyCommands) -> anyhow::Result<()> {
    match cmd.command {
        GpgKeySubcommand::List { user } => {
            println!("Listing GPG keys for user: {}", user);
        }
        GpgKeySubcommand::Add { user, key_id } => {
            println!("Adding GPG key for user: {} (key_id={})", user, key_id);
        }
        GpgKeySubcommand::Remove { user, key_id } => {
            println!("Removing GPG key {} for user: {}", key_id, user);
        }
    }
    Ok(())
}

async fn handle_token(cmd: TokenCommands) -> anyhow::Result<()> {
    match cmd.command {
        TokenSubcommand::List { user } => {
            println!("Listing tokens for user: {}", user);
        }
        TokenSubcommand::Create {
            user,
            name,
            scopes,
            expiry,
        } => {
            println!(
                "Creating token for user: {} (name={}, scopes={:?}, expiry={})",
                user, name, scopes, expiry
            );
        }
        TokenSubcommand::Revoke { user, token_id } => {
            println!("Revoking token {} for user: {}", token_id, user);
        }
    }
    Ok(())
}

async fn handle_build(cmd: BuildCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        BuildSubcommand::List {
            project,
            status,
            branch,
        } => {
            println!(
                "Listing builds (project={:?}, status={:?}, branch={:?})",
                project, status, branch
            );
        }
        BuildSubcommand::Show { id } => {
            println!("Showing build: {}", id);
        }
        BuildSubcommand::Trigger {
            project,
            pipeline,
            ref_name,
            variables,
        } => {
            println!(
                "Triggering build: project={}, pipeline={}, ref={}, variables={:?}",
                project, pipeline, ref_name, variables
            );
        }
        BuildSubcommand::Cancel { id } => {
            println!("Cancelling build: {}", id);
        }
        BuildSubcommand::Retry { id } => {
            println!("Retrying build: {}", id);
        }
        BuildSubcommand::Logs { id, follow, tail } => {
            println!(
                "Viewing logs for build: {} (follow={}, tail={})",
                id, follow, tail
            );
        }
        BuildSubcommand::Artifacts { id } => {
            println!("Listing artifacts for build: {}", id);
        }
    }
    Ok(())
}

async fn handle_git(cmd: GitCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        GitSubcommand::Branch { project, all } => {
            println!("Listing branches for project: {} (all={})", project, all);
        }
        GitSubcommand::Tag { project } => {
            println!("Listing tags for project: {}", project);
        }
        GitSubcommand::Commit { project, sha } => {
            println!("Showing commit {} for project: {}", sha, project);
        }
        GitSubcommand::Ls {
            project,
            ref_name,
            path,
        } => {
            println!(
                "Listing files for project: {} at {} (path={:?})",
                project, ref_name, path
            );
        }
        GitSubcommand::Cat {
            project,
            ref_name,
            path,
        } => {
            println!(
                "Showing file {} for project: {} at {}",
                path, project, ref_name
            );
        }
        GitSubcommand::Log {
            project,
            ref_name,
            limit,
            path,
        } => {
            println!(
                "Showing log for project: {} (ref={:?}, limit={}, path={:?})",
                project, ref_name, limit, path
            );
        }
        GitSubcommand::Diff {
            project,
            base,
            head,
            path,
        } => {
            println!(
                "Showing diff for project: {} ({}..{}) (path={:?})",
                project, base, head, path
            );
        }
        GitSubcommand::Hook(cmd) => handle_git_hook(cmd).await?,
    }
    Ok(())
}

async fn handle_git_hook(cmd: GitHookCommands) -> anyhow::Result<()> {
    match cmd.command {
        GitHookSubcommand::List { project } => {
            println!("Listing hooks for project: {}", project);
        }
        GitHookSubcommand::Add {
            project,
            name,
            script: _,
        } => {
            println!("Adding hook {} for project: {}", name, project);
        }
        GitHookSubcommand::Remove { project, name } => {
            println!("Removing hook {} for project: {}", name, project);
        }
        GitHookSubcommand::Toggle {
            project,
            name,
            action,
        } => {
            println!("{} hook {} for project: {}", action, name, project);
        }
    }
    Ok(())
}

async fn handle_db(cmd: DbCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        DbSubcommand::Migrate { version } => {
            println!("Running migrations (version={:?})", version);
        }
        DbSubcommand::CreateMigration { name } => {
            println!("Creating migration: {}", name);
        }
        DbSubcommand::Status => {
            println!("Migration status");
        }
        DbSubcommand::Rollback { steps } => {
            println!("Rolling back {} steps", steps);
        }
        DbSubcommand::Backup { path } => {
            println!("Backing up database to: {:?}", path);
        }
        DbSubcommand::Restore { path, drop } => {
            println!("Restoring database from: {:?} (drop={})", path, drop);
        }
        DbSubcommand::Vacuum { full } => {
            println!("Vacuuming database (full={})", full);
        }
        DbSubcommand::Stats => {
            println!("Database statistics");
        }
    }
    Ok(())
}

async fn handle_config(cmd: ConfigCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        ConfigSubcommand::Show { all } => {
            println!("Showing configuration (all={})", all);
        }
        ConfigSubcommand::Get { key } => {
            println!("Getting config: {}", key);
        }
        ConfigSubcommand::Set { key, value } => {
            println!("Setting config: {} = {}", key, value);
        }
        ConfigSubcommand::Reset { confirm } => {
            println!("Resetting config (confirm={})", confirm);
        }
        ConfigSubcommand::Validate => {
            println!("Validating config");
        }
    }
    Ok(())
}

async fn handle_backup(cmd: BackupCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        BackupSubcommand::Create {
            dir,
            include_repos,
            include_artifacts,
            include_lfs,
        } => {
            println!(
                "Creating backup in {:?} (repos={}, artifacts={}, lfs={})",
                dir, include_repos, include_artifacts, include_lfs
            );
        }
        BackupSubcommand::Restore {
            dir,
            include_repos,
            include_artifacts,
            include_lfs,
        } => {
            println!(
                "Restoring backup from {:?} (repos={}, artifacts={}, lfs={})",
                dir, include_repos, include_artifacts, include_lfs
            );
        }
        BackupSubcommand::List { dir } => {
            println!("Listing backups in {:?}", dir);
        }
        BackupSubcommand::Verify { dir } => {
            println!("Verifying backup in {:?}", dir);
        }
    }
    Ok(())
}

async fn handle_admin(cmd: AdminCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        AdminSubcommand::Info => {
            println!("System information");
        }
        AdminSubcommand::Health => {
            println!("System health");
        }
        AdminSubcommand::Metrics { format } => {
            println!("System metrics (format={:?})", format);
        }
        AdminSubcommand::User(cmd) => handle_user_admin(cmd).await?,
        AdminSubcommand::Project(cmd) => handle_project_admin(cmd).await?,
        AdminSubcommand::Group(cmd) => handle_group_admin(cmd).await?,
        AdminSubcommand::Reindex { project } => {
            println!("Rebuilding search index (project={:?})", project);
        }
        AdminSubcommand::Cleanup { days, dry_run } => {
            println!("Cleaning up old data (days={}, dry_run={})", days, dry_run);
        }
        AdminSubcommand::Maintenance { action } => {
            println!("Maintenance mode: {:?}", action);
        }
    }
    Ok(())
}

async fn handle_user_admin(cmd: UserAdminCommands) -> anyhow::Result<()> {
    match cmd.command {
        UserAdminSubcommand::List { inactive } => {
            println!("Listing users (inactive={})", inactive);
        }
        UserAdminSubcommand::CreateAdmin {
            username,
            email,
            password: _,
        } => {
            println!("Creating admin user: {} ({})", username, email);
        }
        UserAdminSubcommand::ResetPassword { user, password: _ } => {
            println!("Resetting password for user: {}", user);
        }
        UserAdminSubcommand::ToggleActive { user } => {
            println!("Toggling active status for user: {}", user);
        }
        UserAdminSubcommand::Role { user, role, action } => {
            println!("{} role {} for user: {}", action, role, user);
        }
    }
    Ok(())
}

async fn handle_project_admin(cmd: ProjectAdminCommands) -> anyhow::Result<()> {
    match cmd.command {
        ProjectAdminSubcommand::List { archived } => {
            println!("Listing projects (archived={})", archived);
        }
        ProjectAdminSubcommand::Delete { project } => {
            println!("Force deleting project: {}", project);
        }
        ProjectAdminSubcommand::Transfer { project, owner } => {
            println!("Transferring project {} to owner: {}", project, owner);
        }
        ProjectAdminSubcommand::Archive { project, unarchive } => {
            println!(
                "{} project: {}",
                if unarchive {
                    "Unarchiving"
                } else {
                    "Archiving"
                },
                project
            );
        }
    }
    Ok(())
}

async fn handle_group_admin(cmd: GroupAdminCommands) -> anyhow::Result<()> {
    match cmd.command {
        GroupAdminSubcommand::List => {
            println!("Listing groups");
        }
        GroupAdminSubcommand::Create { name, parent } => {
            println!("Creating group: {} (parent={:?})", name, parent);
        }
        GroupAdminSubcommand::Delete { group } => {
            println!("Deleting group: {}", group);
        }
        GroupAdminSubcommand::Member {
            group,
            user,
            action,
        } => {
            println!("{} user {} to/from group: {}", action, user, group);
        }
    }
    Ok(())
}

async fn handle_dev(cmd: DevCommands, _config: &CliConfig) -> anyhow::Result<()> {
    match cmd.command {
        DevSubcommand::Generate { generator, output } => {
            println!("Generating {:?} in {:?}", generator, output);
        }
        DevSubcommand::Test { filter, coverage } => {
            println!("Running tests (filter={:?}, coverage={})", filter, coverage);
        }
        DevSubcommand::Lint { fix } => {
            println!("Linting (fix={})", fix);
        }
        DevSubcommand::Format { check } => {
            println!("Formatting (check={})", check);
        }
        DevSubcommand::Serve { port, open } => {
            println!("Starting dev server on port {} (open={})", port, open);
        }
    }
    Ok(())
}

fn handle_completion(args: CompletionArgs) -> anyhow::Result<()> {
    let mut cmd = <Cli as clap::CommandFactory>::command();
    clap_complete::generate(args.shell, &mut cmd, "onedev", &mut std::io::stdout());
    Ok(())
}
