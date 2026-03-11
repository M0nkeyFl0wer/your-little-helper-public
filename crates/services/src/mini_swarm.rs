//! Mini-swarm: stub for a multi-agent research pipeline.
//!
//! The planned architecture is a three-stage pipeline:
//! 1. **Planner** -- decomposes the research question into sub-queries.
//! 2. **Researcher** -- runs parallel searches/LLM calls for each sub-query.
//! 3. **Verifier** -- cross-checks citations and synthesizes the final answer.
//!
//! Currently returns a placeholder; the implementation will land when
//! the Brave Search integration is fully wired up.

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct SwarmConfig {
    pub max_workers: usize,
}

#[derive(Debug, Clone)]
pub struct SwarmOutput {
    pub summary: String,
    pub citations: Vec<String>,
}

pub async fn run_research(_question: String, _cfg: SwarmConfig) -> Result<SwarmOutput> {
    // TODO: implement planner->researcher->verifier pipeline
    Ok(SwarmOutput {
        summary: "Research stub".into(),
        citations: vec![],
    })
}
