//! Push hooks implemented as Rust functions.
//!
//! Vanilla OneDev encodes protection checks in Java listeners. Here the same
//! checks run before the pack is accepted; there are deliberately no shell
//! hooks in the repository (`core.hooksPath` is disabled for all Git commands).

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::errors::{GitError, GitResult};
use crate::models::{
    GitActor, HookDecision, PostReceiveListener, ProtectionRules, PushContext, PushEvent, RefUpdate,
};
use crate::utils::{
    PktLine, decode_pkt_lines, glob_matches, repository_path, run_git, validate_ref_name,
};
use chrono::Utc;

const ZERO_OID_40: &str = "0000000000000000000000000000000000000000";
const ZERO_OID_64: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn is_zero(oid: &str) -> bool {
    oid == ZERO_OID_40 || oid == ZERO_OID_64 || oid.bytes().all(|b| b == b'0')
}

/// External checks a project can require before accepting a push.
///
/// The default implementation denies every required check, so a rule is never
/// silently bypassed when the host application has not wired it up yet.
pub trait PushPolicy: Send + Sync {
    fn approvals(&self, _project_path: &str, _branch: &str, _commit: &str) -> GitResult<u32> {
        Ok(0)
    }

    fn build_succeeded(&self, _project_path: &str, _commit: &str) -> GitResult<bool> {
        Ok(false)
    }

    fn signature_verified(&self, _project_path: &str, _commit: &str) -> GitResult<bool> {
        Ok(false)
    }
}

/// Default policy: no approvals, builds or signatures are known.
#[derive(Debug, Default)]
pub struct NoPushPolicy;

impl PushPolicy for NoPushPolicy {}

/// Pre- and post-receive hook engine.
pub struct HookEngine {
    rules: RwLock<ProtectionRules>,
    policy: Arc<dyn PushPolicy>,
    listeners: Vec<Arc<dyn PostReceiveListener>>,
    repos_root: PathBuf,
    git_binary: PathBuf,
}

impl HookEngine {
    pub fn new(repos_root: PathBuf, git_binary: PathBuf) -> Self {
        HookEngine {
            rules: RwLock::new(ProtectionRules::default()),
            policy: Arc::new(NoPushPolicy),
            listeners: Vec::new(),
            repos_root,
            git_binary,
        }
    }

    pub fn with_policy(mut self, policy: Arc<dyn PushPolicy>) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_listener(mut self, listener: Arc<dyn PostReceiveListener>) -> Self {
        self.listeners.push(listener);
        self
    }

    pub fn set_rules(&self, rules: ProtectionRules) {
        if let Ok(mut guard) = self.rules.write() {
            *guard = rules;
        }
    }

    pub fn rules(&self) -> ProtectionRules {
        self.rules
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Validate all ref updates of a push before the pack is written.
    pub fn pre_receive(&self, context: &PushContext) -> GitResult<()> {
        for update in &context.updates {
            validate_ref_name(&update.ref_name)?;
            let decision = self.check_update(context, update)?;
            if let HookDecision::Deny(reason) = decision {
                return Err(GitError::HookRejected(reason));
            }
        }
        Ok(())
    }

    /// Notify listeners after a push has been accepted.
    pub fn post_receive(&self, context: &PushContext) {
        let event = PushEvent {
            context: context.clone(),
            accepted_at: Utc::now(),
        };
        for listener in &self.listeners {
            if let Err(err) = listener.on_push(&event) {
                tracing::warn!(error = %err, "post-receive listener failed");
            }
        }
    }

    fn check_update(&self, context: &PushContext, update: &RefUpdate) -> GitResult<HookDecision> {
        let snapshot = self
            .rules
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone());
        let (rules, branch) = if let Some(name) = update.ref_name.strip_prefix("refs/heads/") {
            (&snapshot.branches, name)
        } else if let Some(name) = update.ref_name.strip_prefix("refs/tags/") {
            (&snapshot.tags, name)
        } else {
            // Pushes to notes/pull refs are not covered by protection rules.
            return Ok(HookDecision::Allow);
        };

        let old = update.old_id.as_deref().filter(|id| !is_zero(id));
        let new = update.new_id.as_deref().filter(|id| !is_zero(id));

        for rule in rules {
            if !glob_matches(&rule.pattern, branch) {
                continue;
            }
            if rule
                .bypass_users
                .iter()
                .any(|user| user == &context.actor.name)
            {
                continue;
            }

            let deny = |reason: String| Ok(HookDecision::Deny(format!("{}: {}", branch, reason)));

            if new.is_none() && rule.prevent_deletion {
                return deny(format!(
                    "deletion is not allowed (protected by '{}')",
                    rule.pattern
                ));
            }
            if old.is_none() && rule.prevent_creation {
                return deny(format!(
                    "creation is not allowed (protected by '{}')",
                    rule.pattern
                ));
            }
            if let (Some(old), Some(new)) = (old, new)
                && rule.prevent_force_push
                && !self.is_ancestor(context, old, new)?
            {
                return deny("non-fast-forward push is not allowed".to_owned());
            }
            if let Some(commit) = new {
                if rule.require_pull_request {
                    let needed = rule.required_approvals.max(1);
                    let approvals = self
                        .policy
                        .approvals(&context.project_path, branch, commit)?;
                    if approvals < needed {
                        return deny(format!(
                            "requires a pull request with at least {needed} approval(s), found {approvals}"
                        ));
                    }
                }
                if rule.require_successful_build
                    && !self.policy.build_succeeded(&context.project_path, commit)?
                {
                    return deny("a successful build is required".to_owned());
                }
                if rule.require_signed_commits
                    && !self
                        .policy
                        .signature_verified(&context.project_path, commit)?
                {
                    return deny("a verified signature is required".to_owned());
                }
            }
        }

        Ok(HookDecision::Allow)
    }

    fn is_ancestor(&self, context: &PushContext, old: &str, new: &str) -> GitResult<bool> {
        let repo = repository_path(&self.repos_root, &context.project_path)?;
        let output = run_git(
            &self.git_binary,
            &repo,
            &["merge-base", "--is-ancestor", old, new],
            None,
        );
        match output {
            Ok(_) => Ok(true),
            Err(GitError::CommandFailed { .. }) => Ok(false),
            Err(err) => Err(err),
        }
    }
}

/// Extract ref updates from a receive-pack pkt-line stream (protocol v0/v2).
pub fn parse_push_updates(body: &[u8]) -> GitResult<Vec<RefUpdate>> {
    let mut updates = Vec::new();
    for line in decode_pkt_lines(body)? {
        let PktLine::Data(data) = line else { continue };
        let text = String::from_utf8_lossy(&data);
        let text = text.trim_end_matches('\0');

        // Protocol v2: "command refs/heads/main:<old> <new>"
        if let Some(rest) = text.strip_prefix("command ")
            && let Some((target, new)) = rest.split_once(' ')
            && let Some((ref_name, old)) = target.rsplit_once(':')
        {
            updates.push(RefUpdate {
                ref_name: ref_name.to_owned(),
                old_id: normalize_oid(old),
                new_id: normalize_oid(new.trim()),
            });
            continue;
        }

        // Protocol v0: "<old> <new> <ref>" (capabilities follow a NUL byte).
        let parts: Vec<&str> = text.split(' ').collect();
        if parts.len() == 3
            && parts[0].len() >= 40
            && parts[0].bytes().all(|b| b.is_ascii_hexdigit())
        {
            updates.push(RefUpdate {
                ref_name: parts[2].to_owned(),
                old_id: normalize_oid(parts[0]),
                new_id: normalize_oid(parts[1]),
            });
        }
    }
    Ok(updates)
}

fn normalize_oid(value: &str) -> Option<String> {
    let value = value.trim();
    if is_zero(value) || value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

/// Parse Git protocol v0/v2 push options from a request body.
pub fn parse_push_options(body: &[u8]) -> GitResult<Vec<String>> {
    let mut options = Vec::new();
    for line in decode_pkt_lines(body)? {
        let PktLine::Data(data) = line else { continue };
        let text = String::from_utf8_lossy(&data);
        if let Some(value) = text.strip_prefix("push-option ") {
            options.push(value.trim().to_owned());
        }
    }
    Ok(options)
}

/// Convenience: build a push context for an actor and a raw receive-pack body.
pub fn push_context(project_path: &str, actor: GitActor, body: &[u8]) -> GitResult<PushContext> {
    Ok(PushContext {
        project_path: project_path.to_owned(),
        actor,
        updates: parse_push_updates(body)?,
        push_options: parse_push_options(body)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProtectionRule;
    use crate::utils::encode_pkt_line;

    #[test]
    fn parses_v0_push_commands() {
        let body = [
            encode_pkt_line(format!("{ZERO_OID_40} {} refs/heads/main", "1".repeat(40)).as_bytes()),
            encode_pkt_line(b"0000"),
        ]
        .concat();
        let updates = parse_push_updates(&body).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].ref_name, "refs/heads/main");
        assert!(updates[0].is_create());
    }

    #[test]
    fn parses_v2_push_commands_and_options() {
        let body = [
            encode_pkt_line(
                format!("command refs/heads/main:{ZERO_OID_40} {}", "a".repeat(40)).as_bytes(),
            ),
            encode_pkt_line(b"push-option ci.skip"),
            encode_pkt_line(b"0000"),
        ]
        .concat();
        let updates = parse_push_updates(&body).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].new_id.as_deref(), Some("a".repeat(40).as_str()));
        assert_eq!(parse_push_options(&body).unwrap(), vec!["ci.skip"]);
    }

    #[test]
    fn protection_rules_deny_deletions() {
        let engine = HookEngine::new(PathBuf::from("/tmp/git"), PathBuf::from("git"));
        let rules = ProtectionRules {
            branches: vec![ProtectionRule {
                pattern: "main".to_owned(),
                prevent_deletion: true,
                ..ProtectionRule::default()
            }],
            tags: Vec::new(),
        };
        engine.set_rules(rules);
        let context = PushContext {
            project_path: "acme/app".to_owned(),
            actor: GitActor {
                id: "1".to_owned(),
                name: "alice".to_owned(),
            },
            updates: vec![RefUpdate {
                ref_name: "refs/heads/main".to_owned(),
                old_id: Some("a".repeat(40)),
                new_id: None,
            }],
            push_options: Vec::new(),
        };
        assert!(matches!(
            engine.pre_receive(&context),
            Err(GitError::HookRejected(_))
        ));
    }

    #[test]
    fn bypass_users_skip_rules() {
        let engine = HookEngine::new(PathBuf::from("/tmp/git"), PathBuf::from("git"));
        engine.set_rules(ProtectionRules {
            branches: vec![ProtectionRule {
                pattern: "*".to_owned(),
                prevent_deletion: true,
                bypass_users: vec!["bot".to_owned()],
                ..ProtectionRule::default()
            }],
            tags: Vec::new(),
        });
        let context = PushContext {
            project_path: "acme/app".to_owned(),
            actor: GitActor {
                id: "2".to_owned(),
                name: "bot".to_owned(),
            },
            updates: vec![RefUpdate {
                ref_name: "refs/heads/main".to_owned(),
                old_id: Some("a".repeat(40)),
                new_id: None,
            }],
            push_options: Vec::new(),
        };
        assert!(engine.pre_receive(&context).is_ok());
    }
}
