
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::helper::{load_text_to_speech, load_voice_style, TextToSpeech, Style};


#[derive(Debug, Clone)]
pub struct EnginePoolConfig {
    /// Number of TTS engines to keep in the pool
    pub engine_pool_size: usize,
    /// Whether to preload engines on startup
    pub warmup_on_startup: bool,
    /// Timeout for engine checkout in milliseconds
    pub engine_checkout_timeout_ms: u64,
    /// Maximum number of voice styles to cache
    pub voice_style_cache_size: usize,
    /// ONNX model directory
    pub onnx_dir: String,
    /// Whether to use GPU (not currently supported)
    pub use_gpu: bool,
}

impl Default for EnginePoolConfig {
    fn default() -> Self {
        Self {
            engine_pool_size: 1,
            warmup_on_startup: false,
            engine_checkout_timeout_ms: 5000,
            voice_style_cache_size: 10,
            onnx_dir: "assets/onnx".to_string(),
            use_gpu: false,
        }
    }
}


// Voice Style Cache Entry
#[derive(Debug, Clone)]
struct CacheEntry {
    voice_style: Style,
    last_accessed: SystemTime,
    file_path: PathBuf,
    file_modified: Option<SystemTime>,
}

impl CacheEntry {
    fn new(voice_style: Style, file_path: PathBuf) -> Result<Self> {
        let metadata = std::fs::metadata(&file_path)?;
        let file_modified = metadata.modified().ok();

        Ok(Self {
            voice_style,
            last_accessed: SystemTime::now(),
            file_path,
            file_modified,
        })
    }

    fn is_valid(&self) -> bool {
        // Check if file still exists
        if !self.file_path.exists() {
            return false;
        }

        // Check if file has been modified since caching
        if let Some(cached_modified) = self.file_modified {
            if let Ok(metadata) = std::fs::metadata(&self.file_path) {
                if let Ok(current_modified) = metadata.modified() {
                    if current_modified > cached_modified {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn _touch(&mut self) {
        self.last_accessed = SystemTime::now();
    }
}


// Engine Checkout Handle
pub struct EngineHandle {
    engine_id: String,
    pool: Arc<TTSEnginePool>,
}

impl EngineHandle {
    /// Get the TTS engine
    pub async fn engine(&self) -> Result<Arc<Mutex<TextToSpeech>>> {
        let engines = self.pool.engines.read().await;
        // Find engine by ID
        for (id, engine) in engines.iter() {
            if id == &self.engine_id {
                return Ok(Arc::clone(engine));
            }
        }
        drop(engines);
        Err(anyhow!("Engine {} not found", self.engine_id))
    }

    /// Get cached voice style or load if not cached
    pub async fn get_voice_style(&self, voice_path: &str) -> Result<Style> {
        self.pool.get_voice_style(voice_path).await
    }
}

pub struct TTSEnginePool {
    config: EnginePoolConfig,
    engines: Arc<RwLock<HashMap<String, Arc<Mutex<TextToSpeech>>>>>,
    semaphore: Arc<Semaphore>,
    voice_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    stats: Arc<RwLock<PoolStats>>,
}

#[derive(Debug, Default)]
struct PoolStats {
    total_checkouts: u64,
    cache_hits: u64,
    cache_misses: u64,
    engine_replacements: u64,
}

impl TTSEnginePool {
    /// Create a new engine pool
    pub async fn new(config: EnginePoolConfig) -> Result<Self> {
        let pool_size = config.engine_pool_size;

        info!("Creating TTS engine pool with size {}", pool_size);

        // Validate pool size
        if pool_size == 0 || pool_size > 10 {
            return Err(anyhow!("Engine pool size must be between 1 and 10"));
        }

        let pool = Self {
            engines: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(pool_size)),
            voice_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(PoolStats::default())),
            config,
        };

        // Warm up engines if requested
        if pool.config.warmup_on_startup {
            info!("Warming up engine pool...");
            pool.warmup().await?;
            info!("Engine pool warmup completed");
        }

        Ok(pool)
    }

    /// Warm up the pool by preloading all engines
    async fn warmup(&self) -> Result<()> {
        let pool_size = self.config.engine_pool_size;
        let mut engines = self.engines.write().await;

        for i in 0..pool_size {
            info!("Loading TTS engine {}/{}", i + 1, pool_size);

            match load_text_to_speech(&self.config.onnx_dir, self.config.use_gpu) {
                Ok(engine) => {
                    let engine_id = Uuid::new_v4().to_string();
                    engines.insert(engine_id.clone(), Arc::new(Mutex::new(engine)));
                    debug!("Engine {} loaded successfully", engine_id);
                }
                Err(e) => {
                    error!("Failed to load engine {}: {}", i + 1, e);
                    return Err(anyhow!("Engine warmup failed: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Check out an engine from the pool
    pub async fn checkout(&self) -> Result<EngineHandle> {
        let checkout_timeout = Duration::from_millis(self.config.engine_checkout_timeout_ms);

        // Acquire semaphore permit with timeout
        let _permit = timeout(
            checkout_timeout,
            self.semaphore.acquire()
        ).await
            .map_err(|_| anyhow!("Engine checkout timeout after {}ms", self.config.engine_checkout_timeout_ms))?
            .map_err(|_| anyhow!("Semaphore closed"))?;

        {
            let mut stats = self.stats.write().await;
            stats.total_checkouts += 1;
        }

        {
            let engines = self.engines.read().await;
            if engines.is_empty() {
                drop(engines);
                self.create_engine().await?;
            }
        }

        let engine_id = {
            let engines = self.engines.read().await;
            engines.keys().next().unwrap().clone()
        };

        debug!("Checked out engine {}", engine_id);

        Ok(EngineHandle {
            engine_id,
            pool: Arc::new(self.clone()),
        })
    }

    /// Create a new engine (lazy loading)
    async fn create_engine(&self) -> Result<()> {
        info!("Creating new TTS engine (lazy load)");

        let engine = load_text_to_speech(&self.config.onnx_dir, self.config.use_gpu)?;
        let engine_id = Uuid::new_v4().to_string();

        let mut engines = self.engines.write().await;
        engines.insert(engine_id.clone(), Arc::new(Mutex::new(engine)));

        info!("Created engine {}", engine_id);
        Ok(())
    }

    /// Get voice style from cache or load it
    pub async fn get_voice_style(&self, voice_path: &str) -> Result<Style> {
        let absolute_path = Path::new(voice_path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(voice_path));
        let cache_key = absolute_path.to_string_lossy().to_string();

        {
            let cache = self.voice_cache.read().await;
            if let Some(entry) = cache.get(&cache_key) {
                if entry.is_valid() {
                    {
                        let mut stats = self.stats.write().await;
                        stats.cache_hits += 1;
                    }
                    debug!("Voice style cache hit: {}", voice_path);
                    return Ok(entry.voice_style.clone());
                }
            }
        }

        // Cache miss - load voice style
        {
            let mut stats = self.stats.write().await;
            stats.cache_misses += 1;
        }

        debug!("Voice style cache miss: {}", voice_path);

        let voice_style = load_voice_style(&[voice_path.to_string()], false)?;

        self.add_to_cache(cache_key, voice_style.clone()).await?;

        Ok(voice_style)
    }

    /// Add voice style to cache with LRU eviction
    async fn add_to_cache(&self, cache_key: String, voice_style: Style) -> Result<()> {
        let path = PathBuf::from(&cache_key);
        let entry = CacheEntry::new(voice_style, path)?;

        let mut cache = self.voice_cache.write().await;

        if cache.len() >= self.config.voice_style_cache_size {
            // Find LRU entry
            if let Some(lru_key) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed)
                .map(|(k, _)| k.clone())
            {
                debug!("Evicting voice style from cache: {}", lru_key);
                cache.remove(&lru_key);
            }
        }

        cache.insert(cache_key, entry);
        Ok(())
    }

    pub async fn get_stats(&self) -> PoolStatsResponse {
        let engines = self.engines.read().await;
        let stats = self.stats.read().await;
        let cache = self.voice_cache.read().await;

        let cache_hit_rate = if stats.cache_hits + stats.cache_misses > 0 {
            (stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64) * 100.0
        } else {
            0.0
        };

        PoolStatsResponse {
            total_engines: engines.len(),
            available_permits: self.semaphore.available_permits(),
            cached_voice_styles: cache.len(),
            total_checkouts: stats.total_checkouts,
            cache_hits: stats.cache_hits,
            cache_misses: stats.cache_misses,
            cache_hit_rate,
            engine_replacements: stats.engine_replacements,
        }
    }

    pub async fn _shutdown(&self) -> Result<()> {
        info!("Shutting down TTS engine pool...");

        // Clear all engines
        let mut engines = self.engines.write().await;
        let engine_count = engines.len();
        engines.clear();

        // Clear cache
        let mut cache = self.voice_cache.write().await;
        cache.clear();

        info!("Engine pool shutdown complete (cleared {} engines)", engine_count);
        Ok(())
    }
}

impl Clone for TTSEnginePool {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            engines: Arc::clone(&self.engines),
            semaphore: Arc::clone(&self.semaphore),
            voice_cache: Arc::clone(&self.voice_cache),
            stats: Arc::clone(&self.stats),
        }
    }
}

// ============================================================================
// Public Statistics Response
// ============================================================================

#[derive(Debug, Serialize)]
pub struct PoolStatsResponse {
    pub total_engines: usize,
    pub available_permits: usize,
    pub cached_voice_styles: usize,
    pub total_checkouts: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,
    pub engine_replacements: u64,
}