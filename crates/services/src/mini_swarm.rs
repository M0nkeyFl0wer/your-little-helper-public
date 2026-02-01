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
    // TODO: implement planner→researcher→verifier pipeline (stub)
    Ok(SwarmOutput {
        summary: "Research stub".into(),
        citations: vec![],
    })
}
