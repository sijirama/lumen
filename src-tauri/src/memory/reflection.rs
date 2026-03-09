//INFO: Reflection engine - synthesizes higher-level insights from observations
//NOTE: Triggered latently when observation count hits mod 50

//INFO: Build the reflection synthesis prompt from a batch of observations
pub fn build_reflection_prompt(observations: &[String], user_name: &str) -> String {
    let obs_block = observations
        .iter()
        .enumerate()
        .map(|(i, o)| format!("{}. {}", i + 1, o))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are Lumen's internal reflection engine. Lumen is the AI sidekick, and {} is the user.

Below are the 50 most recent observations about {}.
Your job is to synthesize 2-3 HIGH-LEVEL DENSE REFLECTIONS. Look for deep patterns, routines, shifting goals, or evolving expertise.

RULES:
- NO REDUNDANCY: Do not repeat facts from the observations. Look for the "why" and the "logic" behind the patterns.
- NAMES: Use "{}" and "Lumen" explicitly. NEVER say "the user".
- DENSITY: Each reflection must be detailed, non-generic, and capture a broad, stark pattern across multiple observations.
- Return ONLY valid JSON array.

FORMAT:
[
  {{"content": "{}'s research transition from NLP to low-level ML systems is accelerating, evidenced by their increasing focus on CUDA kernels and MoE scaling issues.", "importance": 9}}
]

RECENT OBSERVATIONS:
{}

Synthesize deep reflections now:"#,
        user_name, user_name, user_name, user_name, obs_block
    )
}

//INFO: Build the DailySummary synthesis prompt from bucketed briefings
pub fn build_daily_summary_prompt(briefings: &[(String, String)], user_name: &str) -> String {
    let parts: Vec<String> = briefings
        .iter()
        .map(|(bucket, content)| format!("### {} Briefing\n{}", bucket.to_uppercase(), content))
        .collect();
    let briefing_block = parts.join("\n\n");

    format!(
        r#"You are Lumen's daily synthesis engine. Lumen is the AI sidekick, and {} is the user.

Below are the briefings from a full day in {}'s life.
Your job is to synthesize ONE STARK, DENSE DailySummary that captures the core themes, major breakthroughs, and psychological state of {} for that day.

RULES:
- DENSITY: Be extremely detailed but efficient. Capture the 'pulse' of the day.
- NAMES: Use "{}" and "Lumen". NEVER say "the user".
- ESSENTIALS ONLY: Skip the fluff. Focus on projects, named entities, and key decision points.
- Return ONLY valid JSON object.

FORMAT:
{{"content": "Today {} spent 6 hours debugging Triton kernels for MoE, successfully reducing latency by 15%. They discussed moving to Paris for Mistral AI, reflecting a high-confidence career pivot.", "importance": 9}}

YESTERDAY'S BRIEFINGS:
{}

Synthesize the daily summary now:"#,
        user_name, user_name, user_name, user_name, user_name, briefing_block
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
