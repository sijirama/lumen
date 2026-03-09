//INFO: Reflection engine - synthesizes higher-level insights from observations
//NOTE: Triggered latently when observation count hits mod 50

//INFO: Build the reflection synthesis prompt from a batch of observations
pub fn build_reflection_prompt(observations: &[String]) -> String {
    let obs_block = observations
        .iter()
        .enumerate()
        .map(|(i, o)| format!("{}. {}", i + 1, o))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are a reflection synthesis agent for a personal AI assistant called Lumen.

Below are the last 50 observations Lumen has made about the user.
Your job is to synthesize 2-3 HIGH-LEVEL REFLECTIONS about the user's overarching goals, routines, projects, or state of mind.

RULES:
- Each reflection must be VERY DETAILED and capture a broad pattern, not a single event.
- Score each reflection's importance from 7-10 (reflections are inherently important).
- Return ONLY valid JSON array.

FORMAT:
[
  {{"content": "detailed high-level reflection...", "importance": 9}}
]

RECENT OBSERVATIONS:
{}

Synthesize reflections now:"#,
        obs_block
    )
}

//INFO: Build the DailySummary synthesis prompt from bucketed briefings
pub fn build_daily_summary_prompt(briefings: &[(String, String)]) -> String {
    let parts: Vec<String> = briefings
        .iter()
        .map(|(bucket, content)| format!("### {} Briefing\n{}", bucket.to_uppercase(), content))
        .collect();
    let briefing_block = parts.join("\n\n");

    format!(
        r#"You are a daily summary agent for a personal AI assistant called Lumen.

Below are the briefing snapshots from yesterday's 4 time periods (morning, afternoon, evening, night).
Your job is to synthesize ONE highly detailed DailySummary that captures the core themes, projects, accomplishments, and state of the user for that day.

RULES:
- Be VERY DETAILED so this can be retrieved by semantic search.
- Mention specific projects, people, topics, and events by name.
- Score importance 7-10.
- Return ONLY valid JSON object.

FORMAT:
{{"content": "detailed daily summary...", "importance": 8}}

YESTERDAY'S BRIEFINGS:
{}

Synthesize the daily summary now:"#,
        briefing_block
    )
}

//INFO: Parsed reflection from LLM response
#[derive(Debug, serde::Deserialize)]
pub struct ExtractedReflection {
    pub content: String,
    pub importance: f64,
}

//INFO: Parsed daily summary from LLM response
#[derive(Debug, serde::Deserialize)]
pub struct ExtractedDailySummary {
    pub content: String,
    pub importance: f64,
}
