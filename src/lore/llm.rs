//! LLM integration for rich story generation
//!
//! Connects to an OpenAI-compatible API server to generate rich narratives
//! from the procedural story seeds. Supports parallel/batch requests for
//! efficient generation with vLLM or similar servers.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::export::LlmPrompt;

/// Configuration for the LLM server
#[derive(Clone, Debug)]
pub struct LlmConfig {
    /// Base URL of the LLM server (e.g., "http://192.168.8.59:8000")
    pub base_url: String,
    /// Model name to use (optional, server may have default)
    pub model: Option<String>,
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// Temperature for generation (0.0 = deterministic, 1.0 = creative)
    pub temperature: f32,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Number of parallel requests (vLLM handles these efficiently)
    pub parallel_requests: usize,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            base_url: "http://192.168.8.59:8000".to_string(),
            model: None,
            max_tokens: 1024,
            temperature: 0.8,
            timeout_secs: 120,
            parallel_requests: 8, // vLLM handles parallel requests well
        }
    }
}

/// OpenAI-compatible chat message
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI-compatible chat completion request
#[derive(Serialize, Debug)]
struct ChatCompletionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
}

/// OpenAI-compatible chat completion response
#[derive(Deserialize, Debug)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize, Debug)]
struct ChatChoice {
    message: ChatMessageResponse,
}

/// Response message - handles both standard and reasoning models
#[derive(Deserialize, Debug)]
struct ChatMessageResponse {
    #[serde(default)]
    content: Option<String>,
    /// For reasoning models (o1, etc.) that put response in reasoning field
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

impl ChatMessageResponse {
    /// Get the actual response content, checking multiple fields
    fn get_content(&self) -> Option<String> {
        // Try content first, then reasoning fields
        self.content.clone()
            .filter(|s| !s.is_empty())
            .or_else(|| self.reasoning_content.clone())
            .or_else(|| self.reasoning.clone())
    }
}

/// LLM client for generating stories
pub struct LlmClient {
    config: LlmConfig,
    client: reqwest::blocking::Client,
}

impl LlmClient {
    /// Create a new LLM client with the given configuration
    pub fn new(config: LlmConfig) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Generate a story from an LLM prompt
    pub fn generate_story(&self, prompt: &LlmPrompt) -> Result<String, LlmError> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: prompt.system_context.clone(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt.user_prompt.clone(),
            },
        ];

        self.chat_completion(messages)
    }

    /// Generate a story from custom messages
    pub fn generate_from_messages(&self, messages: Vec<ChatMessage>) -> Result<String, LlmError> {
        self.chat_completion(messages)
    }

    /// Make a chat completion request
    fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String, LlmError> {
        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
        };

        let url = format!("{}/v1/chat/completions", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(LlmError::ApiError(format!(
                "Status {}: {}",
                status, body
            )));
        }

        let completion: ChatCompletionResponse = response
            .json()
            .map_err(|e| LlmError::ParseError(e.to_string()))?;

        completion
            .choices
            .first()
            .and_then(|c| c.message.get_content())
            .ok_or(LlmError::EmptyResponse)
    }

    /// Check if the LLM server is available
    pub fn health_check(&self) -> bool {
        let url = format!("{}/v1/models", self.config.base_url);
        self.client.get(&url).send().map(|r| r.status().is_success()).unwrap_or(false)
    }
}

/// Errors that can occur during LLM operations
#[derive(Debug)]
pub enum LlmError {
    NetworkError(String),
    ApiError(String),
    ParseError(String),
    EmptyResponse,
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::NetworkError(e) => write!(f, "Network error: {}", e),
            LlmError::ApiError(e) => write!(f, "API error: {}", e),
            LlmError::ParseError(e) => write!(f, "Parse error: {}", e),
            LlmError::EmptyResponse => write!(f, "Empty response from LLM"),
        }
    }
}

impl std::error::Error for LlmError {}

/// Tagged prompt for tracking which category a result belongs to
#[derive(Clone)]
struct TaggedPrompt {
    category: PromptCategory,
    prompt: LlmPrompt,
}

#[derive(Clone, Copy, Debug)]
enum PromptCategory {
    WorldOverview,
    CreationMyth,
    Legend,
    SacredPlace,
    ForbiddenZone,
    LostCivilization,
}

/// Generate rich stories from all LLM prompts using parallel requests
/// vLLM and similar servers handle concurrent requests efficiently
pub fn generate_rich_stories(
    prompts: &super::export::LlmPrompts,
    config: &LlmConfig,
    progress_callback: Option<&dyn Fn(usize, usize, &str)>,
) -> RichStories {
    let client = LlmClient::new(config.clone());

    // Check server availability
    if !client.health_check() {
        eprintln!("Warning: LLM server at {} is not available", config.base_url);
        return RichStories::default();
    }

    // Collect all prompts with their categories
    let mut all_prompts: Vec<TaggedPrompt> = Vec::new();

    all_prompts.push(TaggedPrompt {
        category: PromptCategory::WorldOverview,
        prompt: prompts.world_overview_prompt.clone(),
    });

    for p in &prompts.creation_myth_prompts {
        all_prompts.push(TaggedPrompt {
            category: PromptCategory::CreationMyth,
            prompt: p.clone(),
        });
    }
    for p in &prompts.legend_prompts {
        all_prompts.push(TaggedPrompt {
            category: PromptCategory::Legend,
            prompt: p.clone(),
        });
    }
    for p in &prompts.sacred_place_prompts {
        all_prompts.push(TaggedPrompt {
            category: PromptCategory::SacredPlace,
            prompt: p.clone(),
        });
    }
    for p in &prompts.forbidden_zone_prompts {
        all_prompts.push(TaggedPrompt {
            category: PromptCategory::ForbiddenZone,
            prompt: p.clone(),
        });
    }
    for p in &prompts.lost_civilization_prompts {
        all_prompts.push(TaggedPrompt {
            category: PromptCategory::LostCivilization,
            prompt: p.clone(),
        });
    }

    let total = all_prompts.len();
    if let Some(cb) = progress_callback {
        cb(0, total, &format!("Sending {} stories in parallel (concurrency: {})...", total, config.parallel_requests));
    }

    // Use tokio runtime for async parallel requests
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // Shared progress counter
    let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let results = Arc::new(Mutex::new(Vec::new()));

    runtime.block_on(generate_stories_parallel(
        all_prompts,
        config.clone(),
        completed.clone(),
        results.clone(),
    ));

    // Final progress report
    if let Some(cb) = progress_callback {
        let count = completed.load(std::sync::atomic::Ordering::SeqCst);
        cb(count, total, "Done!");
    }

    // Organize results by category
    let mut stories = RichStories::default();
    let final_results = Arc::try_unwrap(results)
        .expect("Arc still has multiple owners")
        .into_inner()
        .unwrap();

    for (category, story) in final_results {
        match category {
            PromptCategory::WorldOverview => stories.world_overview = Some(story),
            PromptCategory::CreationMyth => stories.creation_myths.push(story),
            PromptCategory::Legend => stories.legends.push(story),
            PromptCategory::SacredPlace => stories.sacred_places.push(story),
            PromptCategory::ForbiddenZone => stories.forbidden_zones.push(story),
            PromptCategory::LostCivilization => stories.lost_civilizations.push(story),
        }
    }

    stories
}

/// Async function to generate stories in parallel batches
async fn generate_stories_parallel(
    prompts: Vec<TaggedPrompt>,
    config: LlmConfig,
    completed: Arc<std::sync::atomic::AtomicUsize>,
    results: Arc<Mutex<Vec<(PromptCategory, String)>>>,
) {
    use tokio::sync::Semaphore;

    let total = prompts.len();

    // Semaphore to limit concurrent requests
    let semaphore = Arc::new(Semaphore::new(config.parallel_requests));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .expect("Failed to create async HTTP client");

    let mut handles = Vec::new();

    for tagged in prompts {
        let sem = semaphore.clone();
        let client = client.clone();
        let config = config.clone();
        let completed = completed.clone();
        let results = results.clone();
        let category = tagged.category;
        let prompt = tagged.prompt;

        let handle = tokio::spawn(async move {
            // Acquire semaphore permit (limits concurrent requests)
            let _permit = sem.acquire().await.expect("Semaphore closed");

            let result = generate_story_async(&client, &config, &prompt).await;

            // Update progress counter
            let count = completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

            // Print progress inline
            eprint!("\r  Generating stories... {}/{} ", count, total);

            if let Ok(story) = result {
                let mut res = results.lock().unwrap();
                res.push((category, story));
            }
        });

        handles.push(handle);
    }

    // Wait for all requests to complete
    for handle in handles {
        let _ = handle.await;
    }

    // Clear the progress line
    eprintln!();
}

/// Async version of story generation
async fn generate_story_async(
    client: &reqwest::Client,
    config: &LlmConfig,
    prompt: &LlmPrompt,
) -> Result<String, LlmError> {
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: prompt.system_context.clone(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: prompt.user_prompt.clone(),
        },
    ];

    #[derive(Serialize)]
    struct AsyncChatRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        temperature: f32,
    }

    let request = AsyncChatRequest {
        model: config.model.clone(),
        messages,
        max_tokens: config.max_tokens,
        temperature: config.temperature,
    };

    let url = format!("{}/v1/chat/completions", config.base_url);

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| LlmError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(LlmError::ApiError(format!("Status {}: {}", status, body)));
    }

    let completion: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| LlmError::ParseError(e.to_string()))?;

    completion
        .choices
        .first()
        .and_then(|c| c.message.get_content())
        .ok_or(LlmError::EmptyResponse)
}

/// Collection of LLM-generated rich stories
#[derive(Default, Serialize)]
pub struct RichStories {
    pub world_overview: Option<String>,
    pub creation_myths: Vec<String>,
    pub legends: Vec<String>,
    pub sacred_places: Vec<String>,
    pub forbidden_zones: Vec<String>,
    pub lost_civilizations: Vec<String>,
}

impl RichStories {
    /// Export stories to a markdown file
    pub fn export_markdown(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::{BufWriter, Write};

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writeln!(writer, "# The Chronicles of the World\n")?;
        writeln!(writer, "*Tales woven by the ancient storytellers, preserved through the ages*\n")?;
        writeln!(writer, "---\n")?;

        // World Overview
        if let Some(overview) = &self.world_overview {
            writeln!(writer, "# World Overview\n")?;
            writeln!(writer, "{}\n", overview)?;
            writeln!(writer, "---\n")?;
        }

        // Creation Myths
        if !self.creation_myths.is_empty() {
            writeln!(writer, "# Creation Myths\n")?;
            writeln!(writer, "*How the world came to be, as told by those who remember the beginning*\n")?;
            for (i, myth) in self.creation_myths.iter().enumerate() {
                writeln!(writer, "## The {} Creation\n", ordinal(i + 1))?;
                writeln!(writer, "{}\n", myth)?;
                writeln!(writer, "---\n")?;
            }
        }

        // Legends
        if !self.legends.is_empty() {
            writeln!(writer, "# Legends of Heroes\n")?;
            writeln!(writer, "*Great deeds performed by those who walked before*\n")?;
            for (i, legend) in self.legends.iter().enumerate() {
                writeln!(writer, "## Legend {}\n", i + 1)?;
                writeln!(writer, "{}\n", legend)?;
                writeln!(writer, "---\n")?;
            }
        }

        // Sacred Places
        if !self.sacred_places.is_empty() {
            writeln!(writer, "# Sacred Places\n")?;
            writeln!(writer, "*Where the veil between worlds grows thin*\n")?;
            for (i, place) in self.sacred_places.iter().enumerate() {
                writeln!(writer, "## Sacred Site {}\n", i + 1)?;
                writeln!(writer, "{}\n", place)?;
                writeln!(writer, "---\n")?;
            }
        }

        // Forbidden Zones
        if !self.forbidden_zones.is_empty() {
            writeln!(writer, "# Forbidden Zones\n")?;
            writeln!(writer, "*Places where none should tread*\n")?;
            for (i, zone) in self.forbidden_zones.iter().enumerate() {
                writeln!(writer, "## The {} Forbidden Place\n", ordinal(i + 1))?;
                writeln!(writer, "{}\n", zone)?;
                writeln!(writer, "---\n")?;
            }
        }

        // Lost Civilizations
        if !self.lost_civilizations.is_empty() {
            writeln!(writer, "# Lost Civilizations\n")?;
            writeln!(writer, "*Echoes of glory long faded*\n")?;
            for (i, civ) in self.lost_civilizations.iter().enumerate() {
                writeln!(writer, "## The {} Lost Empire\n", ordinal(i + 1))?;
                writeln!(writer, "{}\n", civ)?;
                writeln!(writer, "---\n")?;
            }
        }

        Ok(())
    }

    /// Check if any stories were generated
    pub fn is_empty(&self) -> bool {
        self.world_overview.is_none()
            && self.creation_myths.is_empty()
            && self.legends.is_empty()
            && self.sacred_places.is_empty()
            && self.forbidden_zones.is_empty()
            && self.lost_civilizations.is_empty()
    }

    /// Count total stories generated
    pub fn count(&self) -> usize {
        (if self.world_overview.is_some() { 1 } else { 0 })
            + self.creation_myths.len()
            + self.legends.len()
            + self.sacred_places.len()
            + self.forbidden_zones.len()
            + self.lost_civilizations.len()
    }
}

fn ordinal(n: usize) -> &'static str {
    match n {
        1 => "First",
        2 => "Second",
        3 => "Third",
        4 => "Fourth",
        5 => "Fifth",
        6 => "Sixth",
        7 => "Seventh",
        8 => "Eighth",
        9 => "Ninth",
        10 => "Tenth",
        _ => "Next",
    }
}

/// Generate a single unified creation poem from all lore discoveries
/// This creates one cohesive, short poetic narrative instead of many separate stories
pub fn generate_creation_poem(
    lore_result: &super::LoreResult,
    config: &LlmConfig,
) -> Result<String, LlmError> {
    // Gather key elements from wanderer discoveries
    let mut landmarks: Vec<String> = Vec::new();
    let mut cultures: Vec<String> = Vec::new();
    let mut features: Vec<String> = Vec::new();

    // Collect unique landmark names (up to 10)
    for landmark in lore_result.landmarks.iter().take(10) {
        landmarks.push(format!("{} ({})", landmark.name, landmark.feature_type.description()));
    }

    // Collect unique cultures from wanderers
    for wanderer in &lore_result.wanderers {
        let culture = wanderer.cultural_lens.culture_name();
        if !cultures.contains(&culture.to_string()) {
            cultures.push(culture.to_string());
        }
    }

    // Collect notable features from story seeds
    for seed in lore_result.story_seeds.iter().take(5) {
        let desc = format!("{:?}", seed.seed_type).split('{').next().unwrap_or("").to_string();
        if !features.contains(&desc) {
            features.push(desc);
        }
    }

    let landmarks_str = if landmarks.is_empty() {
        "ancient mountains and forgotten valleys".to_string()
    } else {
        landmarks.join(", ")
    };

    let cultures_str = if cultures.is_empty() {
        "wandering peoples".to_string()
    } else {
        cultures.join(", ")
    };

    let system_prompt = r#"You are a mythic poet creating creation stories for fantasy worlds.
Write in a style inspired by ancient creation myths - Kalevala, Eddas, Genesis, Enuma Elish.
Be concise, evocative, and rhythmic. Use imagery over explanation."#;

    let user_prompt = format!(
        r#"Write a short creation poem (12-20 lines) for a world with these elements:

Sacred Places: {}

Peoples: {}

The poem should:
- Explain how the world was born from primordial chaos
- Name the sacred mountains, waters, or lands
- Hint at the peoples who would come to inhabit it
- End with a sense of mystery and wonder
- Use poetic rhythm and imagery, not prose

Write only the poem, no title or explanation."#,
        landmarks_str,
        cultures_str
    );

    // Make synchronous request for single poem
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| LlmError::NetworkError(e.to_string()))?;

    #[derive(Serialize)]
    struct ChatRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        temperature: f32,
    }

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        max_tokens: 512, // Short poem doesn't need many tokens
        temperature: config.temperature,
    };

    let url = format!("{}/v1/chat/completions", config.base_url);

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .map_err(|e| LlmError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(LlmError::ApiError(format!("Status {}: {}", status, body)));
    }

    let completion: ChatCompletionResponse = response
        .json()
        .map_err(|e| LlmError::ParseError(e.to_string()))?;

    completion
        .choices
        .first()
        .and_then(|c| c.message.get_content())
        .ok_or(LlmError::EmptyResponse)
}

/// Export a creation poem to a text file
pub fn export_creation_poem(poem: &str, path: &str) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::{BufWriter, Write};

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    writeln!(writer, "═══════════════════════════════════════")?;
    writeln!(writer, "         THE CREATION OF THE WORLD")?;
    writeln!(writer, "═══════════════════════════════════════")?;
    writeln!(writer)?;
    writeln!(writer, "{}", poem)?;
    writeln!(writer)?;
    writeln!(writer, "═══════════════════════════════════════")?;

    Ok(())
}
