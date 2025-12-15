
use anyhow::{anyhow, Result};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::helper::{load_text_to_speech, load_voice_style, timer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: ServerSettings,
    pub tts: TtsSettings,
    pub auth: AuthSettings,
    pub logging: LoggingSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSettings {
    pub onnx_dir: String,
    pub use_gpu: bool,
    pub total_step: usize,
    pub speed: f32,
    pub default_voice_style: String,
    #[serde(default = "default_engine_pool_size")]
    pub engine_pool_size: usize,
    #[serde(default = "default_warmup_on_startup")]
    pub warmup_on_startup: bool,
    #[serde(default = "default_engine_checkout_timeout_ms")]
    pub engine_checkout_timeout_ms: u64,
    #[serde(default = "default_voice_style_cache_size")]
    pub voice_style_cache_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSettings {
    pub require_api_key: bool,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub ort_level: String,
}

// Default value functions for serde
fn default_engine_pool_size() -> usize { 1 }
fn default_warmup_on_startup() -> bool { false }
fn default_engine_checkout_timeout_ms() -> u64 { 5000 }
fn default_voice_style_cache_size() -> usize { 10 }

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                host: "0.0.0.0".to_string(),
                port: 8080,
            },
            tts: TtsSettings {
                onnx_dir: "assets/onnx".to_string(),
                use_gpu: false,
                total_step: 5,
                speed: 1.05,
                default_voice_style: "assets/voice_styles/M1.json".to_string(),
                engine_pool_size: 1,
                warmup_on_startup: false,
                engine_checkout_timeout_ms: 5000,
                voice_style_cache_size: 10,
            },
            auth: AuthSettings {
                require_api_key: false,
                api_key: None,
            },
            logging: LoggingSettings {
                level: "info".to_string(),
                ort_level: "warn".to_string(),
            },
        }
    }
}

impl ServerConfig {
    pub fn load_from_file(path: &str) -> Result<Self> {
        let file = std::fs::File::open(path)
            .map_err(|e| anyhow!("Failed to open config file {}: {}", path, e))?;
        let reader = std::io::BufReader::new(file);
        let config: ServerConfig = serde_json::from_reader(reader)
            .map_err(|e| anyhow!("Failed to parse config file {}: {}", path, e))?;
        Ok(config)
    }

    pub fn load_or_default(path: &str) -> Self {
        match Self::load_from_file(path) {
            Ok(config) => config,
            Err(e) => {
                warn!("Failed to load config from {}: {}, using defaults", path, e);
                Self::default()
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TtsRequest {
    /// Text to synthesize
    pub input: String,
    /// Voice model to use (default: "supertts")
    pub model: Option<String>,
    /// Voice style (OpenAI uses "voice" parameter)
    pub voice: Option<String>,
    /// Speech speed (0.25 to 4.0)
    pub speed: Option<f32>,
    /// Response format (default: "wav", only "wav" supported)
    pub response_format: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TtsError {
    pub error: TtsErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct TtsErrorDetail {
    pub message: String,
    pub type_: String,
    pub code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub version: String,
    pub model_loaded: bool,
    pub pool_stats: Option<crate::engine_pool::PoolStatsResponse>,
}

#[derive(Clone)]
pub struct AppState {
    pub config: ServerConfig,
    pub text_to_speech: Arc<Mutex<Option<crate::helper::TextToSpeech>>>, // Kept for backward compatibility
    pub default_voice_style: String,
    pub engine_pool: Option<Arc<crate::engine_pool::TTSEnginePool>>,
}

// Voice Style Resolution Helper
fn resolve_voice_style_path(voice_name: Option<&str>, default_path: &str) -> Result<String> {
    // If no voice name provided, use default
    let voice_name = match voice_name {
        Some(name) => name,
        None => {
            if Path::new(default_path).exists() {
                return Ok(default_path.to_string());
            } else {
                return Err(anyhow!("Default voice style file not found: {}", default_path));
            }
        }
    };

    // Direct file path support (if voice name contains .json or /)
    if voice_name.contains(".json") || voice_name.contains("/") || voice_name.contains("\\") {
        let direct_path = if voice_name.ends_with(".json") {
            voice_name.to_string()
        } else {
            format!("{}.json", voice_name)
        };

        if Path::new(&direct_path).exists() {
            return Ok(direct_path);
        } else {
            return Err(anyhow!("Voice style file not found: {}", direct_path));
        }
    }

    // Standard voice name mappings
    let voice_mappings = [
        ("m1", "assets/voice_styles/M1.json"),
        ("male1", "assets/voice_styles/M1.json"),
        ("f1", "assets/voice_styles/F1.json"),
        ("female1", "assets/voice_styles/F1.json"),
        ("m2", "assets/voice_styles/M1.json"),
        ("male2", "assets/voice_styles/M2.json"),
        ("f2", "assets/voice_styles/F1.json"),
        ("female2", "assets/voice_styles/F2.json"),
    ];

    let normalized_name = voice_name.to_lowercase();

    // Try exact matches first
    for (name, path) in &voice_mappings {
        if normalized_name == *name {
            if Path::new(path).exists() {
                return Ok(path.to_string());
            } else {
                return Err(anyhow!("Voice style file not found for voice '{}': {}", name, path));
            }
        }
    }

    // Try partial matches
    for (name, path) in &voice_mappings {
        if normalized_name.contains(name) {
            if Path::new(path).exists() {
                return Ok(path.to_string());
            } else {
                return Err(anyhow!("Voice style file not found for voice '{}': {}", name, path));
            }
        }
    }

    // Try to find a file that matches the voice name in voice_styles directory
    let voice_styles_dir = "assets/voice_styles";
    if Path::new(voice_styles_dir).exists() {
        if let Ok(entries) = std::fs::read_dir(voice_styles_dir) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".json") {
                        let file_stem = file_name.trim_end_matches(".json").to_lowercase();
                        if file_stem.contains(&normalized_name) || normalized_name.contains(&file_stem) {
                            let full_path = format!("{}/{}", voice_styles_dir, file_name);
                            return Ok(full_path);
                        }
                    }
                }
            }
        }
    }

    // Default fallback - try M1.json first, then F1.json
    let fallback_options = ["assets/voice_styles/F1.json", "assets/voice_styles/M1.json"];
    for fallback in &fallback_options {
        if Path::new(fallback).exists() {
            warn!("Unknown voice '{}', falling back to: {}", voice_name, fallback);
            return Ok(fallback.to_string());
        }
    }

    // No voice style files found
    let available_voices = if Path::new(voice_styles_dir).exists() {
        if let Ok(entries) = std::fs::read_dir(voice_styles_dir) {
            let voices: Vec<String> = entries
                .flatten()
                .filter_map(|entry| {
                    entry.file_name().to_str().and_then(|name| {
                        if name.ends_with(".json") {
                            Some(name.trim_end_matches(".json").to_string())
                        } else {
                            None
                        }
                    })
                })
                .collect();

            if !voices.is_empty() {
                format!("Available voice styles: {}", voices.join(", "))
            } else {
                "No voice style files found in assets/voice_styles/ directory".to_string()
            }
        } else {
            "Unable to read voice styles directory".to_string()
        }
    } else {
        "Voice styles directory 'assets/voice_styles/' does not exist".to_string()
    };

    Err(anyhow!(
        "Unknown voice '{}' and no fallback available. {}",
        voice_name,
        available_voices
    ))
}
// Authentication Middleware
fn check_api_key(headers: &HeaderMap, config: &AuthSettings) -> Result<(), StatusCode> {
    if !config.require_api_key {
        return Ok(());
    }

    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    match (auth_header, &config.api_key) {
        (Some(token), Some(expected_token)) if token == expected_token => Ok(()),
        (Some(_), Some(_)) => Err(StatusCode::UNAUTHORIZED),
        (None, Some(_)) => Err(StatusCode::UNAUTHORIZED),
        _ => Ok(()),
    }
}
pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    // Get pool stats if pool is available
    let pool_stats = if let Some(pool) = &state.engine_pool {
        Some(pool.get_stats().await)
    } else {
        None
    };

    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        model_loaded: true, // We'll assume model is loaded if server is running
        pool_stats,
    };

    Json(response)
}

#[derive(Debug, Serialize)]
pub struct VoicesResponse {
    pub voices: Vec<VoiceInfo>,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct VoiceInfo {
    pub name: String,
    pub path: String,
    pub exists: bool,
}

pub async fn list_voices() -> impl IntoResponse {
    let voice_styles_dir = "assets/voice_styles";
    let mut voices = Vec::new();

    // Add standard voice mappings
    let standard_voices = [
        ("m1", "assets/voice_styles/M1.json"),
        ("male1", "assets/voice_styles/M1.json"),
        ("f1", "assets/voice_styles/F1.json"),
        ("female1", "assets/voice_styles/F1.json"),
        ("m2", "assets/voice_styles/M1.json"),
        ("male2", "assets/voice_styles/M2.json"),
        ("f2", "assets/voice_styles/F1.json"),
        ("female2", "assets/voice_styles/F2.json"),
    ];

    for (name, path) in &standard_voices {
        voices.push(VoiceInfo {
            name: name.to_string(),
            path: path.to_string(),
            exists: Path::new(path).exists(),
        });
    }

    // Add any additional voice files found in the directory
    if Path::new(voice_styles_dir).exists() {
        if let Ok(entries) = std::fs::read_dir(voice_styles_dir) {
            let mut seen_files = std::collections::HashSet::new();

            // Mark standard files as seen
            for (_, path) in &standard_voices {
                if let Some(file_name) = Path::new(path).file_name() {
                    if let Some(name_str) = file_name.to_str() {
                        seen_files.insert(name_str.to_lowercase());
                    }
                }
            }

            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".json") && !seen_files.contains(&file_name.to_lowercase()) {
                        let name = file_name.trim_end_matches(".json").to_string();
                        let path = format!("{}/{}", voice_styles_dir, file_name);
                        voices.push(VoiceInfo {
                            name,
                            path: path.clone(),
                            exists: Path::new(&path).exists(),
                        });
                        seen_files.insert(file_name.to_lowercase());
                    }
                }
            }
        }
    }

    let response = VoicesResponse {
        voices,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Json(response)
}

pub async fn tts_speech(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TtsRequest>,
) -> Result<Response, StatusCode> {
    let request_id = Uuid::new_v4().to_string();
    let start_time = Instant::now();

      // Log model and response_format for debugging
    let model = request.model.as_deref().unwrap_or("supertts");
    let response_format = request.response_format.as_deref().unwrap_or("wav");

    info!("[{}] TTS request: model='{}' input='{}' voice={:?} format={:?}",
          request_id, model, request.input, request.voice, response_format);

    // Check authentication
    if let Err(status) = check_api_key(&headers, &state.config.auth) {
        warn!("[{}] Authentication failed", request_id);
        return Err(status);
    }

    // Validate input
    if request.input.trim().is_empty() {
        let error = TtsError {
            error: TtsErrorDetail {
                message: "Input text cannot be empty".to_string(),
                type_: "invalid_request_error".to_string(),
                code: Some("empty_input".to_string()),
            },
        };
        return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
    }

    // Validate model (we accept any model name but log it)
    if model != "supertts" && model != "tts-1" && model != "tts-1-hd" {
        warn!("[{}] Using unsupported model '{}', will use supertts engine", request_id, model);
    }

    // Validate response format (only wav is supported)
    if response_format != "wav" {
        let error = TtsError {
            error: TtsErrorDetail {
                message: format!("Response format '{}' is not supported. Only 'wav' is supported.", response_format),
                type_: "invalid_request_error".to_string(),
                code: Some("unsupported_format".to_string()),
            },
        };
        return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
    }

    // Validate speed
    if let Some(speed) = request.speed {
        if speed < 0.25 || speed > 4.0 {
            let error = TtsError {
                error: TtsErrorDetail {
                    message: "Speed must be between 0.25 and 4.0".to_string(),
                    type_: "invalid_request_error".to_string(),
                    code: Some("invalid_speed".to_string()),
                },
            };
            return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
        }
    }

    // Map voice parameter to voice style file with validation
    let voice_style_path = match resolve_voice_style_path(request.voice.as_deref(), &state.default_voice_style) {
        Ok(path) => path,
        Err(e) => {
            error!("[{}] Voice style resolution failed: {}", request_id, e);
            let error = TtsError {
                error: TtsErrorDetail {
                    message: format!("Voice style not found: {}", e),
                    type_: "invalid_request_error".to_string(),
                    code: Some("voice_not_found".to_string()),
                },
            };
            return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
        }
    };

    // Use engine pool if available, otherwise fallback to single engine
    let (wav_data, sample_rate) = if let Some(pool) = &state.engine_pool {
        // Use engine pool
        info!("[{}] Using engine pool for TTS generation", request_id);

        let engine_handle = match pool.checkout().await {
            Ok(handle) => handle,
            Err(e) => {
                error!("[{}] Failed to checkout engine: {}", request_id, e);
                let error = TtsError {
                    error: TtsErrorDetail {
                        message: format!("Engine pool exhausted: {}", e),
                        type_: "service_unavailable".to_string(),
                        code: Some("pool_exhausted".to_string()),
                    },
                };
                return Ok((StatusCode::SERVICE_UNAVAILABLE, Json(error)).into_response());
            }
        };

        // Load voice style using pool cache
        let style = match engine_handle.get_voice_style(&voice_style_path).await {
            Ok(style) => style,
            Err(e) => {
                error!("[{}] Failed to load voice style {}: {}", request_id, voice_style_path, e);
                let error = TtsError {
                    error: TtsErrorDetail {
                        message: format!("Failed to load voice style: {}", e),
                        type_: "invalid_request_error".to_string(),
                        code: Some("voice_style_load_failed".to_string()),
                    },
                };
                return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
            }
        };

        // Get the engine and generate speech
        let speed = request.speed.unwrap_or(state.config.tts.speed);
        let total_step = state.config.tts.total_step;

        let result = match engine_handle.engine().await {
            Ok(text_to_speech_mutex) => {
                let mut text_to_speech = text_to_speech_mutex.lock().await;
                let sample_rate = text_to_speech.sample_rate;

                match timer("TTS Generation", || {
                    text_to_speech.call(&request.input, &style, total_step, speed, 0.3)
                }) {
                    Ok(result) => (result.0, sample_rate as f32),
                    Err(e) => {
                        error!("[{}] TTS generation failed: {}", request_id, e);
                        let error = TtsError {
                            error: TtsErrorDetail {
                                message: format!("TTS generation failed: {}", e),
                                type_: "internal_server_error".to_string(),
                                code: Some("tts_generation_failed".to_string()),
                            },
                        };
                        return Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response());
                    }
                }
            }
            Err(e) => {
                error!("[{}] Failed to get engine: {}", request_id, e);
                let error = TtsError {
                    error: TtsErrorDetail {
                        message: format!("Failed to get engine: {}", e),
                        type_: "internal_server_error".to_string(),
                        code: Some("engine_access_failed".to_string()),
                    },
                };
                return Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response());
            }
        };

        // Engine handle is automatically dropped and returned to pool
        result
    } else {
        // Fallback to single engine (backward compatibility)
        info!("[{}] Using single engine (fallback)", request_id);

        let mut tts_guard = state.text_to_speech.lock().unwrap();
        let text_to_speech = match tts_guard.as_mut() {
            Some(tts) => tts,
            None => {
                info!("[{}] Loading TTS engine...", request_id);
                match load_text_to_speech(&state.config.tts.onnx_dir, state.config.tts.use_gpu) {
                    Ok(tts) => {
                        *tts_guard = Some(tts);
                        tts_guard.as_mut().unwrap()
                    }
                    Err(e) => {
                        error!("[{}] Failed to load TTS engine: {}", request_id, e);
                        let error = TtsError {
                            error: TtsErrorDetail {
                                message: format!("Failed to load TTS engine: {}", e),
                                type_: "internal_server_error".to_string(),
                                code: Some("tts_load_failed".to_string()),
                            },
                        };
                        return Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response());
                    }
                }
            }
        };

        // Load voice style (simplified approach - load on demand without caching)
        let style = match load_voice_style(&[voice_style_path.to_string()], false) {
            Ok(style) => style,
            Err(e) => {
                error!("[{}] Failed to load voice style {}: {}", request_id, voice_style_path, e);
                let error = TtsError {
                    error: TtsErrorDetail {
                        message: format!("Failed to load voice style: {}", e),
                        type_: "invalid_request_error".to_string(),
                        code: Some("voice_style_load_failed".to_string()),
                    },
                };
                return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
            }
        };

        // Generate speech
        let speed = request.speed.unwrap_or(state.config.tts.speed);
        let total_step = state.config.tts.total_step;

        let sample_rate = text_to_speech.sample_rate;
        match timer("TTS Generation", || {
            text_to_speech.call(&request.input, &style, total_step, speed, 0.3)
        }) {
            Ok(result) => (result.0, sample_rate as f32),
            Err(e) => {
                error!("[{}] TTS generation failed: {}", request_id, e);
                let error = TtsError {
                    error: TtsErrorDetail {
                        message: format!("TTS generation failed: {}", e),
                        type_: "internal_server_error".to_string(),
                        code: Some("tts_generation_failed".to_string()),
                    },
                };
                return Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response());
            }
        }
    };

    // Convert WAV data to bytes
    let mut wav_buffer = Vec::new();
    if let Err(e) = crate::helper::write_wav_to_buffer(&mut wav_buffer, &wav_data, sample_rate as i32) {
        error!("[{}] Failed to encode WAV: {}", request_id, e);
        let error = TtsError {
            error: TtsErrorDetail {
                message: format!("Failed to encode WAV: {}", e),
                type_: "internal_server_error".to_string(),
                code: Some("wav_encoding_failed".to_string()),
            },
        };
        return Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response());
    }

    let duration = start_time.elapsed();
    info!("[{}] TTS request completed in {:?} ({} bytes)", request_id, duration, wav_buffer.len());

    // Return WAV audio response with detailed headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/wav")
        .header(header::CONTENT_LENGTH, wav_buffer.len())
        .header("X-Request-ID", request_id)
        .header("X-Model-Used", model)
        .header("X-Voice-Used", request.voice.unwrap_or_else(|| "default".to_string()))
        .header("X-Response-Format", response_format)
        .header("X-Processing-Time", format!("{:.3}ms", duration.as_millis()))
        .header("Cache-Control", "no-cache")
        .body(axum::body::Body::from(wav_buffer))
        .unwrap();

    Ok(response)
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/voices", get(list_voices))
        .route("/v1/audio/speech", post(tts_speech))
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive())
        )
        .with_state(state)
}

pub async fn start_server(config: ServerConfig) -> Result<()> {
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&bind_addr).await
        .map_err(|e| anyhow!("Failed to bind to {}: {}", bind_addr, e))?;

    // Initialize engine pool if configured
    let engine_pool = if config.tts.engine_pool_size > 1 {
        info!("Initializing TTS engine pool with size {}", config.tts.engine_pool_size);

        let pool_config = crate::engine_pool::EnginePoolConfig {
            engine_pool_size: config.tts.engine_pool_size,
            warmup_on_startup: config.tts.warmup_on_startup,
            engine_checkout_timeout_ms: config.tts.engine_checkout_timeout_ms,
            voice_style_cache_size: config.tts.voice_style_cache_size,
            onnx_dir: config.tts.onnx_dir.clone(),
            use_gpu: config.tts.use_gpu,
        };

        match crate::engine_pool::TTSEnginePool::new(pool_config).await {
            Ok(pool) => {
                info!("Engine pool initialized successfully");
                Some(Arc::new(pool))
            }
            Err(e) => {
                warn!("Failed to initialize engine pool: {}. Using fallback single engine.", e);
                None
            }
        }
    } else {
        info!("Engine pool disabled (size <= 1), using single engine mode");
        None
    };

    // Initialize application state
    let state = AppState {
        default_voice_style: config.tts.default_voice_style.clone(),
        config: config.clone(),
        text_to_speech: Arc::new(Mutex::new(None)), // Kept for backward compatibility
        engine_pool,
    };

    let router = create_router(state);

    info!("Starting superTTS API server on {}", bind_addr);
    info!("Available endpoints:");
    info!("  GET  /health - Health check (includes pool stats if pool is enabled)");
    info!("  GET  /voices - List available voice styles");
    info!("  POST /v1/audio/speech - OpenAI compatible TTS endpoint");

    axum::serve(listener, router).await
        .map_err(|e| anyhow!("Server error: {}", e))?;

    Ok(())
}