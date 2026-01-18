//! Image generation integration for visual storytelling
//!
//! Connects to an image generation server to create illustrations
//! for the generated stories and landmarks.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use super::types::{GeographicFeature, Landmark, StorySeed, StorySeedType, CulturalLens};

/// Configuration for the image generation server
#[derive(Clone, Debug)]
pub struct ImageGenConfig {
    /// Base URL of the image generation server (e.g., "http://192.168.8.59:8001")
    pub base_url: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Output directory for generated images
    pub output_dir: String,
}

impl Default for ImageGenConfig {
    fn default() -> Self {
        Self {
            base_url: "http://192.168.8.59:8001".to_string(),
            timeout_secs: 300,
            width: 1024,
            height: 1024,
            output_dir: ".".to_string(),
        }
    }
}

/// Image generation request
#[derive(Serialize, Debug)]
pub struct ImageGenRequest {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cfg_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
}

/// Image generation response - flexible to handle different API formats
#[derive(Deserialize, Debug)]
pub struct ImageGenResponse {
    /// Base64 encoded image data (if returned as JSON)
    #[serde(default)]
    pub image: Option<String>,
    /// Alternative field name for image data
    #[serde(default)]
    pub images: Option<Vec<String>>,
    /// URL to the generated image (if server returns URL)
    #[serde(default)]
    pub url: Option<String>,
    /// Alternative: data field
    #[serde(default)]
    pub data: Option<Vec<ImageData>>,
}

#[derive(Deserialize, Debug)]
pub struct ImageData {
    #[serde(default)]
    pub b64_json: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// Image generation client
pub struct ImageGenClient {
    config: ImageGenConfig,
    client: reqwest::blocking::Client,
}

impl ImageGenClient {
    /// Create a new image generation client
    pub fn new(config: ImageGenConfig) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Generate an image from a prompt and save it
    pub fn generate_and_save(&self, prompt: &str, filename: &str) -> Result<String, ImageGenError> {
        let request = ImageGenRequest {
            prompt: prompt.to_string(),
            negative_prompt: Some("blurry, low quality, distorted, text, watermark".to_string()),
            width: Some(self.config.width),
            height: Some(self.config.height),
            steps: Some(30),
            cfg_scale: Some(7.5),
            seed: None,
        };

        let url = format!("{}/generate", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| ImageGenError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(ImageGenError::ApiError(format!(
                "Status {}: {}",
                status, body
            )));
        }

        // Check content type to determine how to handle response
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let output_path = Path::new(&self.config.output_dir).join(filename);
        let output_path_str = output_path.to_string_lossy().to_string();

        if content_type.contains("image/") {
            // Direct image response
            let bytes = response.bytes().map_err(|e| ImageGenError::ParseError(e.to_string()))?;
            let mut file = File::create(&output_path)
                .map_err(|e| ImageGenError::IoError(e.to_string()))?;
            file.write_all(&bytes)
                .map_err(|e| ImageGenError::IoError(e.to_string()))?;
        } else {
            // JSON response with base64 image
            let gen_response: ImageGenResponse = response
                .json()
                .map_err(|e| ImageGenError::ParseError(e.to_string()))?;

            let image_data = self.extract_image_data(&gen_response)?;
            use base64::Engine;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(&image_data)
            .map_err(|e| ImageGenError::ParseError(format!("Base64 decode error: {}", e)))?;

            let mut file = File::create(&output_path)
                .map_err(|e| ImageGenError::IoError(e.to_string()))?;
            file.write_all(&decoded)
                .map_err(|e| ImageGenError::IoError(e.to_string()))?;
        }

        Ok(output_path_str)
    }

    /// Extract image data from various response formats
    fn extract_image_data(&self, response: &ImageGenResponse) -> Result<String, ImageGenError> {
        // Try different response formats
        if let Some(ref image) = response.image {
            return Ok(image.clone());
        }
        if let Some(ref images) = response.images {
            if let Some(first) = images.first() {
                return Ok(first.clone());
            }
        }
        if let Some(ref data) = response.data {
            if let Some(first) = data.first() {
                if let Some(ref b64) = first.b64_json {
                    return Ok(b64.clone());
                }
            }
        }
        Err(ImageGenError::EmptyResponse)
    }

    /// Check if the image generation server is available
    pub fn health_check(&self) -> bool {
        let url = format!("{}/health", self.config.base_url);
        // Try health endpoint, fall back to just checking if server responds
        if self.client.get(&url).send().map(|r| r.status().is_success()).unwrap_or(false) {
            return true;
        }
        // Try base URL
        self.client.get(&self.config.base_url).send().is_ok()
    }
}

/// Errors that can occur during image generation
#[derive(Debug)]
pub enum ImageGenError {
    NetworkError(String),
    ApiError(String),
    ParseError(String),
    IoError(String),
    EmptyResponse,
}

impl std::fmt::Display for ImageGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageGenError::NetworkError(e) => write!(f, "Network error: {}", e),
            ImageGenError::ApiError(e) => write!(f, "API error: {}", e),
            ImageGenError::ParseError(e) => write!(f, "Parse error: {}", e),
            ImageGenError::IoError(e) => write!(f, "IO error: {}", e),
            ImageGenError::EmptyResponse => write!(f, "Empty response from image generator"),
        }
    }
}

impl std::error::Error for ImageGenError {}

/// Generate a visual prompt for a landmark
pub fn landmark_to_prompt(landmark: &Landmark) -> String {
    let feature_desc = match &landmark.feature_type {
        GeographicFeature::MountainPeak { height, is_volcanic } => {
            if *is_volcanic {
                format!("majestic volcanic mountain peak, {:.0}m elevation, smoke rising from crater, dramatic lighting", height)
            } else {
                format!("towering snow-capped mountain peak, {:.0}m elevation, jagged rocks, alpine landscape", height)
            }
        }
        GeographicFeature::MountainRange { peak_count, highest } => {
            format!("vast mountain range with {} peaks, highest at {:.0}m, epic vista, dramatic clouds", peak_count, highest)
        }
        GeographicFeature::Volcano { active } => {
            if *active {
                "active volcano with flowing lava, fire and smoke, dramatic night sky, apocalyptic beauty".to_string()
            } else {
                "dormant volcano, lush vegetation reclaiming ancient crater, peaceful power".to_string()
            }
        }
        GeographicFeature::Lake { depth, area } => {
            format!("serene mystical lake spanning {} tiles, {:.0}m deep, mist rising, surrounded by ancient trees", area, depth)
        }
        GeographicFeature::RiverSource { flow_strength } => {
            format!("sacred river source, pristine spring waters, flow strength {:.1}, moss-covered rocks, magical atmosphere", flow_strength)
        }
        GeographicFeature::Valley { depth, river_carved } => {
            if *river_carved {
                format!("river-carved valley {:.0}m deep, flowing waters, lush vegetation, epic fantasy landscape", depth.abs())
            } else {
                "sweeping valley vista, morning mist, ancient ruins visible in distance, epic fantasy landscape".to_string()
            }
        }
        GeographicFeature::Plateau { elevation, .. } => {
            format!("vast elevated plateau at {:.0}m, windswept grasses, distant horizons, dramatic sky", elevation)
        }
        GeographicFeature::Cliff { drop } => {
            format!("dramatic cliff face, {:.0}m sheer drop, crashing waves or mist below, golden hour lighting", drop)
        }
        GeographicFeature::Waterfall { height } => {
            format!("majestic waterfall {:.0}m tall, rainbow mist, lush surroundings, hidden paradise", height)
        }
        GeographicFeature::GlacialField => {
            "massive ancient glacier, blue ice formations, crevasses, stark beauty, arctic wilderness".to_string()
        }
        GeographicFeature::DesertHeart => {
            "endless golden sand dunes, clear blue sky, mysterious oasis in distance, ancient secrets".to_string()
        }
        GeographicFeature::AncientSite { biome } => {
            format!("ancient sacred site in {} biome, mysterious ruins, overgrown with vegetation, magical aura", biome)
        }
        GeographicFeature::MysticalAnomaly { biome } => {
            format!("mystical anomaly in {} biome, magical energy visible, ethereal glow, otherworldly", biome)
        }
        GeographicFeature::PrimordialRemnant { biome } => {
            format!("primordial remnant in {} biome, ancient power, untouched by time, sacred ground", biome)
        }
        GeographicFeature::Coast => {
            "dramatic coastline, waves crashing on rocks, sea spray, golden hour lighting".to_string()
        }
        GeographicFeature::Peninsula => {
            "windswept peninsula jutting into the sea, lighthouse on the point, dramatic cliffs".to_string()
        }
        GeographicFeature::Island { area } => {
            format!("mysterious island spanning {} tiles, hidden coves, ancient secrets, tropical paradise", area)
        }
        _ => "epic fantasy landscape, dramatic lighting, mysterious atmosphere".to_string(),
    };

    format!(
        "{}, fantasy art style, highly detailed, cinematic composition, concept art, digital painting",
        feature_desc
    )
}

/// Generate a visual prompt for a story seed
pub fn story_seed_to_prompt(seed: &StorySeed) -> String {
    let base_prompt = match &seed.seed_type {
        StorySeedType::CreationMyth { origin_feature, cosmic_scale } => {
            format!(
                "creation myth scene, {:?} as origin, {:?} cosmic scale, birth of world, divine beings, epic atmosphere",
                origin_feature, cosmic_scale
            )
        }
        StorySeedType::HeroLegend { journey_type, .. } => {
            format!(
                "heroic fantasy scene, legendary warrior on {:?} journey, dramatic action, mythological atmosphere",
                journey_type
            )
        }
        StorySeedType::Parable { moral_theme, setting_feature } => {
            format!(
                "wisdom tale scene, {:?} moral lesson, {} setting, contemplative atmosphere",
                moral_theme, setting_feature
            )
        }
        StorySeedType::OriginStory { people_or_creature, birthplace_feature } => {
            format!(
                "origin myth, {} emerging from {}, primordial scene, magical birth",
                people_or_creature, birthplace_feature
            )
        }
        StorySeedType::CataclysmMyth { disaster_type, .. } => {
            format!(
                "cataclysmic {:?} scene, world-ending disaster, dramatic sky, apocalyptic fantasy",
                disaster_type
            )
        }
        StorySeedType::SacredPlace { sanctity_source, .. } => {
            format!(
                "sacred temple scene, {:?} as source of holiness, pilgrims approaching, divine light, spiritual atmosphere",
                sanctity_source
            )
        }
        StorySeedType::ForbiddenZone { danger_type, .. } => {
            format!(
                "forbidden dark zone, {:?} lurking in shadows, warning signs, ominous atmosphere, dark fantasy",
                danger_type
            )
        }
        StorySeedType::LostCivilization { ruin_biome, fall_cause } => {
            format!(
                "lost civilization ruins in {}, {:?} caused their fall, crumbling grandeur, nature reclaiming",
                ruin_biome, fall_cause
            )
        }
    };

    format!(
        "{}, fantasy art style, highly detailed, cinematic composition, concept art, digital painting",
        base_prompt
    )
}

/// Generate a cultural-themed prompt
pub fn culture_to_prompt(culture: &CulturalLens) -> String {
    let culture_desc = match culture {
        CulturalLens::Highland { .. } => "highland mountain people, stone fortresses, hardy warriors, alpine culture",
        CulturalLens::Maritime { .. } => "seafaring culture, tall ships, coastal settlements, ocean explorers",
        CulturalLens::Desert { .. } => "desert nomads, oasis camps, flowing robes, sand-swept tents",
        CulturalLens::Sylvan { .. } => "forest dwelling people, treehouse villages, nature magic, elven aesthetic",
        CulturalLens::Steppe { .. } => "nomadic steppe riders, vast grasslands, horse culture, yurt camps",
        CulturalLens::Subterranean { .. } => "underground civilization, cavern cities, crystal lighting, deep dwellers",
    };

    format!(
        "{}, fantasy culture, detailed architecture, daily life scene, concept art, digital painting",
        culture_desc
    )
}

/// Generate images for stories
pub struct StoryImageGenerator {
    client: ImageGenClient,
}

impl StoryImageGenerator {
    pub fn new(config: ImageGenConfig) -> Self {
        Self {
            client: ImageGenClient::new(config),
        }
    }

    /// Generate images for key landmarks
    pub fn generate_landmark_images(
        &self,
        landmarks: &[Landmark],
        max_images: usize,
        progress_callback: Option<&dyn Fn(usize, usize, &str)>,
    ) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let count = landmarks.len().min(max_images);

        for (i, landmark) in landmarks.iter().take(count).enumerate() {
            if let Some(cb) = progress_callback {
                cb(i, count, &format!("Generating image for {}...", landmark.name));
            }

            let prompt = landmark_to_prompt(landmark);
            let filename = format!("landmark_{:03}_{}.png", i, sanitize_filename(&landmark.name));

            match self.client.generate_and_save(&prompt, &filename) {
                Ok(path) => {
                    results.push((landmark.name.clone(), path));
                }
                Err(e) => {
                    eprintln!("Failed to generate image for {}: {}", landmark.name, e);
                }
            }
        }

        results
    }

    /// Generate images for story seeds
    pub fn generate_story_images(
        &self,
        stories: &[StorySeed],
        max_images: usize,
        progress_callback: Option<&dyn Fn(usize, usize, &str)>,
    ) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let count = stories.len().min(max_images);

        for (i, story) in stories.iter().take(count).enumerate() {
            if let Some(cb) = progress_callback {
                cb(i, count, &format!("Generating story image {}...", i + 1));
            }

            let prompt = story_seed_to_prompt(story);
            let type_name = format!("{:?}", story.seed_type).split('{').next().unwrap_or("story").to_string();
            let filename = format!("story_{:03}_{}.png", i, sanitize_filename(&type_name));

            match self.client.generate_and_save(&prompt, &filename) {
                Ok(path) => {
                    results.push((type_name, path));
                }
                Err(e) => {
                    eprintln!("Failed to generate story image: {}", e);
                }
            }
        }

        results
    }

    /// Check if server is available
    pub fn is_available(&self) -> bool {
        self.client.health_check()
    }
}

/// Sanitize a string for use as a filename
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}
