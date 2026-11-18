//! Web UI Models

pub mod admin;
pub mod auth;
pub mod builds;
pub mod dashboard;
pub mod git;
pub mod issues;
pub mod merge_requests;
pub mod profile;
pub mod projects;

// Re-export specific types to avoid ambiguous glob re-exports
pub use admin::*;
pub use auth::User;
pub use builds::{Build, BuildStatus, Pipeline};
pub use dashboard::{ActivityItem, DashboardStats, UserProject};
pub use git::{Branch, Commit, CommitRef, Tag, TreeEntry};
pub use issues::{Issue, IssueComment, Label, Milestone};
pub use merge_requests::MergeRequest;
pub use profile::{AccessToken, GpgKey, Session, SshKey, UpdateSettingsForm, UserProfile};
pub use projects::{Project, ProjectMember};
