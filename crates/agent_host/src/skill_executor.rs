//! Async skill execution wrapper.
//!
//! Provides timeout handling, progress reporting, and execution management
//! for skill invocations (different from executor.rs which handles shell commands).

use anyhow::Result;
use shared::events::SkillEvent;
use shared::skill::{ExecutionStatus, Skill, SkillContext, SkillError, SkillExecution, SkillInput};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Default execution timeout (60 seconds)
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Skill executor with timeout and progress reporting.
pub struct SkillExecutor {
    /// Default timeout for skill execution
    default_timeout: Duration,
    /// Channel for sending skill events
    event_sender: Option<mpsc::UnboundedSender<SkillEvent>>,
}

impl SkillExecutor {
    /// Create a new skill executor with default settings.
    pub fn new() -> Self {
        Self {
            default_timeout: DEFAULT_TIMEOUT,
            event_sender: None,
        }
    }

    /// Create a new skill executor with an event channel.
    pub fn with_events(event_sender: mpsc::UnboundedSender<SkillEvent>) -> Self {
        Self {
            default_timeout: DEFAULT_TIMEOUT,
            event_sender: Some(event_sender),
        }
    }

    /// Set the default timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Execute a skill with timeout handling.
    pub async fn execute(
        &self,
        skill: &Arc<dyn Skill>,
        input: SkillInput,
        ctx: &SkillContext,
    ) -> Result<SkillExecution, SkillError> {
        self.execute_with_timeout(skill, input, ctx, self.default_timeout)
            .await
    }

    /// Execute a skill with a custom timeout.
    pub async fn execute_with_timeout(
        &self,
        skill: &Arc<dyn Skill>,
        input: SkillInput,
        ctx: &SkillContext,
        timeout: Duration,
    ) -> Result<SkillExecution, SkillError> {
        let execution_id = Uuid::new_v4();
        let skill_id = skill.id().to_string();
        let mode = ctx.mode;

        // Create execution record
        let mut execution = SkillExecution::new(&skill_id, mode, input.clone());
        execution.id = execution_id;

        // Validate input
        skill
            .validate_input(&input)
            .map_err(|e| SkillError::InvalidInput {
                message: e.to_string(),
            })?;

        // Send started event
        self.send_event(SkillEvent::Started {
            execution_id,
            skill_id: skill_id.clone(),
            mode,
        });

        let start = Instant::now();

        // Execute with timeout
        let result = tokio::time::timeout(timeout, skill.execute(input, ctx)).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                self.send_event(SkillEvent::Completed {
                    execution_id,
                    duration_ms,
                });
                Ok(execution.complete(output, duration_ms))
            }
            Ok(Err(e)) => {
                let error_msg = e.to_string();
                self.send_event(SkillEvent::Failed {
                    execution_id,
                    error: error_msg.clone(),
                    duration_ms,
                });
                Ok(execution.fail(error_msg, duration_ms))
            }
            Err(_) => {
                self.send_event(SkillEvent::Timeout {
                    execution_id,
                    duration_ms,
                });
                Ok(execution.timeout(duration_ms))
            }
        }
    }

    /// Execute a skill without timeout (use with caution).
    pub async fn execute_unbounded(
        &self,
        skill: &Arc<dyn Skill>,
        input: SkillInput,
        ctx: &SkillContext,
    ) -> Result<SkillExecution, SkillError> {
        let execution_id = Uuid::new_v4();
        let skill_id = skill.id().to_string();
        let mode = ctx.mode;

        let mut execution = SkillExecution::new(&skill_id, mode, input.clone());
        execution.id = execution_id;

        skill
            .validate_input(&input)
            .map_err(|e| SkillError::InvalidInput {
                message: e.to_string(),
            })?;

        self.send_event(SkillEvent::Started {
            execution_id,
            skill_id: skill_id.clone(),
            mode,
        });

        let start = Instant::now();
        let result = skill.execute(input, ctx).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                self.send_event(SkillEvent::Completed {
                    execution_id,
                    duration_ms,
                });
                Ok(execution.complete(output, duration_ms))
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.send_event(SkillEvent::Failed {
                    execution_id,
                    error: error_msg.clone(),
                    duration_ms,
                });
                Ok(execution.fail(error_msg, duration_ms))
            }
        }
    }

    /// Send a progress update for an execution.
    pub fn send_progress(&self, execution_id: Uuid, message: String, percent: Option<u8>) {
        self.send_event(SkillEvent::Progress {
            execution_id,
            message,
            percent,
        });
    }

    /// Send an event through the channel.
    fn send_event(&self, event: SkillEvent) {
        if let Some(ref sender) = self.event_sender {
            // Ignore send errors (receiver may have dropped)
            let _ = sender.send(event);
        }
    }
}

impl Default for SkillExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a batch skill execution.
pub struct BatchExecutionResult {
    pub successful: Vec<SkillExecution>,
    pub failed: Vec<(String, SkillError)>,
    pub total_duration_ms: u64,
}

impl SkillExecutor {
    /// Execute multiple skills sequentially.
    pub async fn execute_batch(
        &self,
        skills: Vec<(Arc<dyn Skill>, SkillInput)>,
        ctx: &SkillContext,
    ) -> BatchExecutionResult {
        let start = Instant::now();
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for (skill, input) in skills {
            let skill_id = skill.id().to_string();
            match self.execute(&skill, input, ctx).await {
                Ok(execution) => {
                    if execution.status == ExecutionStatus::Completed {
                        successful.push(execution);
                    } else {
                        failed.push((
                            skill_id,
                            SkillError::ExecutionFailed(anyhow::anyhow!(execution
                                .error
                                .unwrap_or_else(|| "Unknown error".to_string()))),
                        ));
                    }
                }
                Err(e) => {
                    failed.push((skill_id, e));
                }
            }
        }

        BatchExecutionResult {
            successful,
            failed,
            total_duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Execute multiple skills concurrently (respecting permissions).
    pub async fn execute_concurrent(
        &self,
        skills: Vec<(Arc<dyn Skill>, SkillInput)>,
        ctx: &SkillContext,
        max_concurrent: usize,
    ) -> BatchExecutionResult {
        use futures::stream::{self, StreamExt};

        let start = Instant::now();
        let results: Vec<_> = stream::iter(skills)
            .map(|(skill, input)| {
                let skill_id = skill.id().to_string();
                async move {
                    let result = self.execute(&skill, input, ctx).await;
                    (skill_id, result)
                }
            })
            .buffer_unordered(max_concurrent)
            .collect()
            .await;

        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for (skill_id, result) in results {
            match result {
                Ok(execution) => {
                    if execution.status == ExecutionStatus::Completed {
                        successful.push(execution);
                    } else {
                        failed.push((
                            skill_id,
                            SkillError::ExecutionFailed(anyhow::anyhow!(execution
                                .error
                                .unwrap_or_else(|| "Unknown error".to_string()))),
                        ));
                    }
                }
                Err(e) => {
                    failed.push((skill_id, e));
                }
            }
        }

        BatchExecutionResult {
            successful,
            failed,
            total_duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use shared::skill::{Mode, PermissionLevel, SkillOutput};
    use std::path::PathBuf;

    struct QuickSkill;

    #[async_trait]
    impl Skill for QuickSkill {
        fn id(&self) -> &'static str {
            "quick_skill"
        }
        fn name(&self) -> &'static str {
            "Quick Skill"
        }
        fn description(&self) -> &'static str {
            "A fast skill"
        }
        fn permission_level(&self) -> PermissionLevel {
            PermissionLevel::Safe
        }
        fn modes(&self) -> &'static [Mode] {
            &[Mode::Find]
        }

        async fn execute(&self, _input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
            Ok(SkillOutput::text("Quick result"))
        }
    }

    struct SlowSkill;

    #[async_trait]
    impl Skill for SlowSkill {
        fn id(&self) -> &'static str {
            "slow_skill"
        }
        fn name(&self) -> &'static str {
            "Slow Skill"
        }
        fn description(&self) -> &'static str {
            "A slow skill"
        }
        fn permission_level(&self) -> PermissionLevel {
            PermissionLevel::Safe
        }
        fn modes(&self) -> &'static [Mode] {
            &[Mode::Find]
        }

        async fn execute(&self, _input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(SkillOutput::text("Slow result"))
        }
    }

    #[tokio::test]
    async fn test_execute_quick_skill() {
        let executor = SkillExecutor::new();
        let skill: Arc<dyn Skill> = Arc::new(QuickSkill);
        let ctx = SkillContext::new(Mode::Find, PathBuf::from("/tmp"));
        let input = SkillInput::from_query("test");

        let result = executor.execute(&skill, input, &ctx).await;
        assert!(result.is_ok());

        let execution = result.unwrap();
        assert_eq!(execution.status, ExecutionStatus::Completed);
        assert!(execution.output.is_some());
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        let executor = SkillExecutor::new().with_timeout(Duration::from_millis(100));
        let skill: Arc<dyn Skill> = Arc::new(SlowSkill);
        let ctx = SkillContext::new(Mode::Find, PathBuf::from("/tmp"));
        let input = SkillInput::from_query("test");

        let result = executor.execute(&skill, input, &ctx).await;
        assert!(result.is_ok());

        let execution = result.unwrap();
        assert_eq!(execution.status, ExecutionStatus::Timeout);
    }

    #[tokio::test]
    async fn test_event_sending() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let executor = SkillExecutor::with_events(tx);
        let skill: Arc<dyn Skill> = Arc::new(QuickSkill);
        let ctx = SkillContext::new(Mode::Find, PathBuf::from("/tmp"));
        let input = SkillInput::from_query("test");

        let _ = executor.execute(&skill, input, &ctx).await;

        // Should receive Started and Completed events
        let started = rx.recv().await;
        assert!(matches!(started, Some(SkillEvent::Started { .. })));

        let completed = rx.recv().await;
        assert!(matches!(completed, Some(SkillEvent::Completed { .. })));
    }
}
