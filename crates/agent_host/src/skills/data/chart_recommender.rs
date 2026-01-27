//! Chart recommendation skill for Data mode.
//!
//! Suggests appropriate visualizations based on data characteristics.

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{
    Mode, PermissionLevel, ResultType, Skill, SkillContext, SkillInput, SkillOutput,
};

/// Chart recommendation skill.
pub struct ChartRecommender;

impl ChartRecommender {
    pub fn new() -> Self {
        Self
    }

    /// Recommend charts based on data description
    fn recommend_charts(description: &str) -> Vec<ChartRecommendation> {
        let desc_lower = description.to_lowercase();
        let mut recommendations = Vec::new();

        // Time series data
        if desc_lower.contains("time")
            || desc_lower.contains("date")
            || desc_lower.contains("trend")
            || desc_lower.contains("over")
        {
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::Line,
                reason: "Line charts are ideal for showing trends over time".to_string(),
                when_to_use: "When you have continuous data points across time".to_string(),
            });
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::Area,
                reason: "Area charts emphasize the magnitude of change over time".to_string(),
                when_to_use: "When you want to show cumulative totals or highlight volume"
                    .to_string(),
            });
        }

        // Comparison data
        if desc_lower.contains("compare")
            || desc_lower.contains("category")
            || desc_lower.contains("group")
        {
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::Bar,
                reason: "Bar charts make it easy to compare values across categories".to_string(),
                when_to_use: "When comparing discrete categories or groups".to_string(),
            });
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::GroupedBar,
                reason: "Grouped bar charts compare multiple series across categories".to_string(),
                when_to_use: "When comparing multiple measures for each category".to_string(),
            });
        }

        // Proportion/percentage data
        if desc_lower.contains("percent")
            || desc_lower.contains("proportion")
            || desc_lower.contains("share")
            || desc_lower.contains("breakdown")
        {
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::Pie,
                reason: "Pie charts show parts of a whole".to_string(),
                when_to_use: "When you have 2-6 categories that sum to 100%".to_string(),
            });
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::StackedBar,
                reason: "Stacked bars show composition and allow comparison".to_string(),
                when_to_use: "When showing composition across multiple groups".to_string(),
            });
        }

        // Correlation/relationship data
        if desc_lower.contains("relationship")
            || desc_lower.contains("correlation")
            || desc_lower.contains("scatter")
        {
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::Scatter,
                reason: "Scatter plots reveal relationships between two variables".to_string(),
                when_to_use: "When exploring correlation or distribution of two numeric values"
                    .to_string(),
            });
        }

        // Distribution data
        if desc_lower.contains("distribution")
            || desc_lower.contains("frequency")
            || desc_lower.contains("histogram")
        {
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::Histogram,
                reason: "Histograms show the distribution of a single variable".to_string(),
                when_to_use: "When analyzing frequency distribution of numeric data".to_string(),
            });
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::BoxPlot,
                reason: "Box plots summarize distribution with quartiles and outliers".to_string(),
                when_to_use: "When comparing distributions across groups".to_string(),
            });
        }

        // If no specific matches, give general recommendations
        if recommendations.is_empty() {
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::Bar,
                reason: "Bar charts are versatile and easy to read".to_string(),
                when_to_use: "Good starting point for most categorical comparisons".to_string(),
            });
            recommendations.push(ChartRecommendation {
                chart_type: ChartType::Line,
                reason: "Line charts work well for continuous data".to_string(),
                when_to_use: "Use when data has a natural sequence or order".to_string(),
            });
        }

        recommendations
    }

    /// Format recommendations for display
    fn format_recommendations(query: &str, recommendations: &[ChartRecommendation]) -> String {
        let mut output = String::new();

        output.push_str("## Chart Recommendations\n\n");

        if !query.is_empty() {
            output.push_str(&format!("Based on: *\"{}\"*\n\n", query));
        }

        for (i, rec) in recommendations.iter().enumerate() {
            let icon = rec.chart_type.icon();
            let name = rec.chart_type.name();

            output.push_str(&format!("### {}. {} {}\n\n", i + 1, icon, name));
            output.push_str(&format!("**Why**: {}\n\n", rec.reason));
            output.push_str(&format!("**Best when**: {}\n\n", rec.when_to_use));
        }

        output.push_str("---\n\n");
        output.push_str("**Tips for effective charts:**\n\n");
        output.push_str("- Keep it simple - one main message per chart\n");
        output.push_str("- Label axes clearly\n");
        output.push_str("- Use consistent colors\n");
        output.push_str("- Start bar charts at zero\n");

        output
    }
}

impl Default for ChartRecommender {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
enum ChartType {
    Line,
    Bar,
    Pie,
    Scatter,
    Area,
    GroupedBar,
    StackedBar,
    Histogram,
    BoxPlot,
}

impl ChartType {
    fn name(&self) -> &'static str {
        match self {
            ChartType::Line => "Line Chart",
            ChartType::Bar => "Bar Chart",
            ChartType::Pie => "Pie Chart",
            ChartType::Scatter => "Scatter Plot",
            ChartType::Area => "Area Chart",
            ChartType::GroupedBar => "Grouped Bar Chart",
            ChartType::StackedBar => "Stacked Bar Chart",
            ChartType::Histogram => "Histogram",
            ChartType::BoxPlot => "Box Plot",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            ChartType::Line => "📈",
            ChartType::Bar => "📊",
            ChartType::Pie => "🥧",
            ChartType::Scatter => "🔵",
            ChartType::Area => "📉",
            ChartType::GroupedBar => "📊",
            ChartType::StackedBar => "📊",
            ChartType::Histogram => "📶",
            ChartType::BoxPlot => "📦",
        }
    }
}

struct ChartRecommendation {
    chart_type: ChartType,
    reason: String,
    when_to_use: String,
}

#[async_trait]
impl Skill for ChartRecommender {
    fn id(&self) -> &'static str {
        "chart_recommender"
    }

    fn name(&self) -> &'static str {
        "Chart Recommender"
    }

    fn description(&self) -> &'static str {
        "Suggest appropriate chart types for your data"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Data]
    }

    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> Result<SkillOutput> {
        if input.query.trim().is_empty() {
            return Ok(SkillOutput::text(
                "Tell me about your data and I'll recommend the best chart types.\n\n\
                 Examples:\n\
                 - \"I want to show sales trends over time\"\n\
                 - \"Compare revenue across different regions\"\n\
                 - \"Show the breakdown of expenses by category\"\n\
                 - \"Find correlation between price and quantity\"",
            ));
        }

        let recommendations = Self::recommend_charts(&input.query);
        let formatted = Self::format_recommendations(&input.query, &recommendations);

        Ok(SkillOutput {
            result_type: ResultType::Text,
            text: Some(formatted),
            files: Vec::new(),
            data: Some(serde_json::json!({
                "query": input.query,
                "recommendation_count": recommendations.len(),
                "chart_types": recommendations.iter().map(|r| r.chart_type.name()).collect::<Vec<_>>(),
            })),
            citations: Vec::new(),
            suggested_actions: Vec::new(),
        })
    }
}
