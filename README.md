# SuperTTS

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.84+-orange.svg)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/supertonic-tts.svg)](https://crates.io/crates/supertts)

**High Performance · Rust-based Text-to-Speech Service**

Fast ONNX inference with OpenAI-compatible API, engine pooling, and multiple voice styles.

[Features](#-update-news) • [Quick Start](#-installation) • [CLI Tool](#-basic-usage) • [API Service](#-api-server-mode) • [Performance](#-performance-report)

</div>

## 📖 Overview

This guide provides an API server for running TTS inference using Rust with exceptional performance and OpenAI API compatibility.

## ✨ Update News

**2025.12.13** - Implemented TTS engine pool for improved performance and concurrent request handling. Added configurable pool size, engine warmup, and voice style caching.[complete performance report](docs/performance_report.md)

**2025.11.23** - Added OpenAI-compatible REST API server mode with `--openai` flag. Now you can run superTTS as a web service!

**2025.11.19** - Added `--speed` parameter to control speech synthesis speed (default: 1.05, recommended range: 0.9-1.5).

**2025.11.19** - Added automatic text chunking for long-form inference. Long texts are split into chunks and synthesized with natural pauses.

---

## 🚀 Installation

This project uses [Cargo](https://doc.rust-lang.org/cargo/) for package management.

### Install Rust (if not already installed)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build the project
```bash
cargo build --release
# or 
make build 
```

###  Download ONNX models (NOTE: Make sure git-lfs is installed)
```bash
git clone https://huggingface.co/Supertone/supertonic assets

# or 
make download
```

---

## Basic Usage

You can run the inference in two ways:
1. **Using cargo run** (builds if needed, then runs)
2. **Direct binary execution** (faster if already built)

### Example 1: Default Inference
Run inference with default settings:
```bash
# Using cargo run
cargo run --release --bin supertts

# Or directly execute the built binary (faster)
./target/release/supertts

# or openai http api
make run 
```

This will use:
- Voice style: `assets/voice_styles/M1.json`
- Text: "This morning, I took a walk in the park, and the sound of the birds and the breeze was so pleasant that I stopped for a long time just to listen."
- Output directory: `results/`
- Total steps: 5
- Number of generations: 4

### Example 2: Batch Inference
Process multiple voice styles and texts at once:
```bash
# Using cargo run
cargo run --release --bin supertts -- \
  --batch \
  --voice-style assets/voice_styles/M1.json,assets/voice_styles/F1.json \
  --text "The sun sets behind the mountains, painting the sky in shades of pink and orange.|The weather is beautiful and sunny outside. A gentle breeze makes the air feel fresh and pleasant."

# Or using the binary directly
./target/release/supertts \
  --batch \
  --voice-style assets/voice_styles/M1.json,assets/voice_styles/F1.json \
  --text "The sun sets behind the mountains, painting the sky in shades of pink and orange.|The weather is beautiful and sunny outside. A gentle breeze makes the air feel fresh and pleasant."
```

This will:
- Generate speech for 2 different voice-text pairs
- Use male voice (M1.json) for the first text
- Use female voice (F1.json) for the second text
- Process both samples in a single batch

### Example 3: High Quality Inference
Increase denoising steps for better quality:
```bash
# Using cargo run
cargo run --release --bin supertts -- \
  --total-step 10 \
  --voice-style assets/voice_styles/M1.json \
  --text "Increasing the number of denoising steps improves the output's fidelity and overall quality."

# Or using the binary directly
./target/release/example_onnx \
  --total-step 10 \
  --voice-style assets/voice_styles/M1.json \
  --text "Increasing the number of denoising steps improves the output's fidelity and overall quality."
```

This will:
- Use 10 denoising steps instead of the default 5
- Produce higher quality output at the cost of slower inference

### Example 4: Long-Form Inference
The system automatically chunks long texts into manageable segments, synthesizes each segment separately, and concatenates them with natural pauses (0.3 seconds by default) into a single audio file. This happens by default when you don't use the `--batch` flag:

```bash
# Using cargo run
cargo run --release --bin supertts -- \
  --voice-style assets/voice_styles/M1.json \
  --text "This is a very long text that will be automatically split into multiple chunks. The system will process each chunk separately and then concatenate them together with natural pauses between segments. This ensures that even very long texts can be processed efficiently while maintaining natural speech flow and avoiding memory issues."

# Or using the binary directly
./target/release/supertts \
  --voice-style assets/voice_styles/M1.json \
  --text "This is a very long text that will be automatically split into multiple chunks. The system will process each chunk separately and then concatenate them together with natural pauses between segments. This ensures that even very long texts can be processed efficiently while maintaining natural speech flow and avoiding memory issues."
```

This will:
- Automatically split the text into chunks based on paragraph and sentence boundaries
- Synthesize each chunk separately
- Add 0.3 seconds of silence between chunks for natural pauses
- Concatenate all chunks into a single audio file

**Note**: Automatic text chunking is disabled when using `--batch` mode. In batch mode, each text is processed as-is without chunking.

---

## 🌐 API Server Mode

You can now run superTTS as an OpenAI-compatible REST API server! This enables integration with existing OpenAI TTS clients and web applications.

### Example 5: Start API Server
```bash
make run 
# Start the API server with default settings
cargo run --release --bin supertts -- --openai

# Start with custom host and port
cargo run --release --bin supertts -- --openai --host 127.0.0.1 --port 8080

# Start with custom configuration file
cargo run --release --bin supertts -- --openai --config my-config.json
```

This will start an HTTP server that provides:
- **Health Check**: `GET /health` - Server health status and engine pool statistics
- **Voice List**: `GET /voices` - List available voice styles and their status
- **TTS Endpoint**: `POST /v1/audio/speech` - OpenAI-compatible text-to-speech

---

### API Usage Examples

#### Health Check
```bash
curl http://localhost:8080/health
```
This returns a JSON response with server status, version, model loaded status, and engine pool statistics (if engine pool is enabled):
```json
{
  "status": "healthy",
  "timestamp": "2025-12-13T12:00:00Z",
  "version": "1.0.0",
  "model_loaded": true,
  "pool_stats": {
    "total_engines": 2,
    "available_permits": 2,
    "cached_voice_styles": 3,
    "total_checkouts": 150,
    "cache_hits": 120,
    "cache_misses": 30,
    "cache_hit_rate": 80.0,
    "engine_replacements": 0
  }
}
```

#### List Available Voices
```bash
curl http://localhost:8080/voices
```
This returns a JSON response showing all available voice styles, their paths, and availability status.

#### Text-to-Speech Request
```bash
curl -X POST "http://localhost:8080/v1/audio/speech" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "supertts",
    "input": "Hello, this is a test of the superTTS API server!",
    "voice": "F2"
  }' \
  --output speech.wav
```

#### Advanced TTS Request with Custom Speed
```bash
curl -X POST "http://localhost:8080/v1/audio/speech" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "supertts",
    "input": "This is an example with custom speech speed.",
    "voice": "f1",
    "response_format": "wav",
    "speed": 1.2
  }' \
  --output custom_speed.wav
```

#### API Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `model` | string | No | `"supertts"` | Model name. Supports `"supertts"`, `"tts-1"`, `"tts-1-hd"` (all use same engine) |
| `input` | string | Yes | - | Text to synthesize (max ~4000 characters recommended) |
| `voice` | string | No | `"f1"` | Voice style. See voice mapping section for options |
| `response_format` | string | No | `"wav"` | Output format. Only `"wav"` is currently supported |
| `speed` | float | No | `1.0` | Speech speed (0.9 to 1.5) |

#### Using Different Voices
The API has enhanced voice style support with intelligent file resolution:

**Standard Voice Names:**
- `m1`, `male1` → Maps to `assets/voice_styles/M1.json` (default male voice)
- `f1`, `female1` → Maps to `assets/voice_styles/F1.json` (default female voice)
- `m2`, `male2` → Maps to `assets/voice_styles/M2.json`
- `f2`, `female2` → Maps to `assets/voice_styles/F2.json`

**Advanced Features:**
- **Direct File Path**: Use absolute or relative paths: `"voice": "custom_voices/my_voice.json"`
- **Auto-detection**: System automatically finds JSON files in `assets/voice_styles/` directory
- **Fallback Support**: If requested voice isn't found, system falls back to available voices
- **Error Messages**: Detailed error messages include list of available voices

```bash
# Use standard voice names
curl -X POST "http://localhost:8080/v1/audio/speech" \
  -H "Content-Type: application/json" \
  -d '{
    "input": "This is synthesized using a female voice.",
    "voice": "female"
  }' \
  --output female_voice.wav

# Use direct file path
curl -X POST "http://localhost:8080/v1/audio/speech" \
  -H "Content-Type: application/json" \
  -d '{
    "input": "This uses a custom voice file.",
    "voice": "custom_voices/narrator.json"
  }' \
  --output custom_voice.wav
```

---

### Configuration File

Create a `config.json` file to customize the API server:

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 8080
  },
  "tts": {
    "onnx_dir": "assets/onnx",
    "use_gpu": false,
    "total_step": 5,
    "speed": 1.05,
    "default_voice_style": "assets/voice_styles/M1.json",
    "engine_pool_size": 2,
    "warmup_on_startup": true,
    "engine_checkout_timeout_ms": 5000,
    "voice_style_cache_size": 10
  },
  "auth": {
    "require_api_key": false,
    "api_key": null
  },
  "logging": {
    "level": "info"
  }
}
```

#### Engine Pool Configuration

The TTS engine pool improves performance by maintaining multiple preloaded TTS engines and caching voice styles. This eliminates model loading latency and enables concurrent request processing.

**Engine Pool Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `engine_pool_size` | int | 1 | Number of TTS engines to keep in the pool (1-10) |
| `warmup_on_startup` | bool | false | Preload all engines on server startup |
| `engine_checkout_timeout_ms` | int | 5000 | Timeout for engine checkout in milliseconds |
| `voice_style_cache_size` | int | 10 | Maximum number of voice styles to cache in memory |

**Performance Benefits:**

- **Eliminates Loading Latency**: Models are preloaded and reused (saves 1-3 seconds per request)
- **Enables Concurrency**: Multiple requests can be processed simultaneously
- **Voice Style Caching**: Frequently used voice styles are cached for faster access
- **Configurable Pool Size**: Adjust based on your server's memory and concurrent needs

**Recommended Settings:**

- **Low Traffic**: `engine_pool_size: 1`, `warmup_on_startup: true`
- **Medium Traffic**: `engine_pool_size: 2-3`, `warmup_on_startup: true`
- **High Traffic**: `engine_pool_size: 4-6`, `warmup_on_startup: true`

**Memory Usage:**
Each engine in the pool consumes approximately 300m of RAM. Plan your pool size accordingly.

---

## 📊 Performance Report

The TTS engine pool has been extensively tested for performance improvements. Here are the key results:

### RTF (Real-Time Factor) Performance

| Metric | Value | Description |
|--------|-------|-------------|
| **Average RTF** | **0.033** | 32.9x faster than real-time |
| Median RTF | 0.033 | Consistent performance |
| Min RTF | 0.021 | Fastest: 47.4x real-time |
| Max RTF | 0.050 | Slowest: 20.1x real-time |
| P95 RTF | 0.048 | 95% of requests < 0.048 RTF |

### Response Time Improvements

| Metric | Without Pool | With Engine Pool | Improvement |
|--------|--------------|------------------|-------------|
| Average Response | 1-3 seconds | **0.431s** | **85-95%** |
| Fastest Response | 1-2 seconds | **0.18s** | **90%** |
| 95th Percentile | 3-4 seconds | **0.48s** | **88%** |

### Concurrency Performance

| Test | Concurrency | Avg Response | RPS | Success Rate |
|------|-------------|--------------|-----|--------------|
| 5 concurrent | 5 | 0.431s | 2.32 | 100% |
| 10 concurrent | 10 | 0.851s | 1.17 | 100% |

### Cache Performance

| Test | Cache Hit Rate | Total Checkouts | Cache Hits |
|------|----------------|-----------------|------------|
| 5 concurrent | **86.7%** | 30 | 26 |
| 10 concurrent | **93.3%** | 60 | 56 |

For detailed performance analysis, test methodology, and additional benchmarks, see the [complete performance report](docs/performance_report.md).

#### Authentication (Optional)
To enable API key authentication, update your config.json:

```json
{
  "auth": {
    "require_api_key": true,
    "api_key": "your-secret-api-key-here"
  }
}
```

Then include the API key in requests:
```bash
curl -X POST "http://localhost:8080/v1/audio/speech" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-api-key-here" \
  -d '{
    "input": "This request uses authentication.",
    "voice": "F2"
  }' \
  --output authenticated.wav
```

---
## Available Arguments

| Argument | Type | Default | Description |
|----------|------|---------|-------------|
| **API Server** | | | |
| `--openai` | flag | False | Start OpenAI-compatible API server mode |
| `--host` | str | `0.0.0.0` | API server host (only used with `--openai`) |
| `--port` | int | 8080 | API server port (only used with `--openai`) |
| `--config` | str | `config.json` | Configuration file for API server |
| **CLI Mode** | | | |
| `--use-gpu` | flag | False | Use GPU for inference (default: CPU) |
| `--onnx-dir` | str | `assets/onnx` | Path to ONNX model directory |
| `--total-step` | int | 5 | Number of denoising steps (higher = better quality, slower) |
| `--n-test` | int | 4 | Number of times to generate each sample |
| `--voice-style` | str+ | `assets/voice_styles/M1.json` | Voice style file path(s) |
| `--text` | str+ | (long default text) | Text(s) to synthesize |
| `--save-dir` | str | `results` | Output directory |
| `--batch` | flag | False | Enable batch mode (multiple text-style pairs, disables automatic chunking) |

## Notes

- **Batch Processing**: When using `--batch`, the number of `--voice-style` files must match the number of `--text` entries
- **Automatic Chunking**: Without `--batch`, long texts are automatically split and concatenated with 0.3s pauses
- **Quality vs Speed**: Higher `--total-step` values produce better quality but take longer
- **GPU Support**: GPU mode is not supported yet
- **Known Issues**: On some platforms (especially macOS), there might be a mutex cleanup warning during exit. This is a known ONNX Runtime issue and doesn't affect functionality. The implementation uses `libc::_exit()` and `mem::forget()` to bypass this issue.

--- 

## Official Project

This implementation is based on the official supertonic project: https://github.com/supertone-inc/supertonic


