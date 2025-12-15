use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{prelude::*};
use std::path::PathBuf;
use std::fs;
use std::mem;
use tracing::info;

mod helper;
mod api_server;
mod engine_pool;

use helper::{
    load_text_to_speech, load_voice_style, timer, write_wav_file, sanitize_filename,
};
use api_server::{start_server, ServerConfig};

#[derive(Parser, Debug)]
#[command(name = "TTS ONNX Inference")]
#[command(about = "TTS Inference with ONNX Runtime (Rust)", long_about = None)]
struct Args {
    /// Start OpenAI-compatible API server
    #[arg(long, default_value = "false")]
    openai: bool,

    /// API server host (only used with --openai)
    #[arg(long)]
    host: Option<String>,

    /// API server port (only used with --openai)
    #[arg(long)]
    port: Option<u16>,

    /// Configuration file for API server (only used with --openai)
    #[arg(long, default_value = "config.json")]
    config: String,

    /// Use GPU for inference (default: CPU)
    #[arg(long)]
    use_gpu: Option<bool>,

    /// Path to ONNX model directory
    #[arg(long)]
    onnx_dir: Option<String>,

    /// Number of denoising steps
    #[arg(long)]
    total_step: Option<usize>,

    /// Speech speed factor (higher = faster)
    #[arg(long)]
    speed: Option<f32>,

    /// Number of times to generate
    #[arg(long, default_value = "4")]
    n_test: usize,

    /// Voice style file path(s)
    #[arg(long, value_delimiter = ',', default_values_t = vec!["assets/voice_styles/M1.json".to_string()])]
    voice_style: Vec<String>,

    /// Text(s) to synthesize
    #[arg(long, value_delimiter = '|', default_values_t = vec!["This morning, I took a walk in the park, and the sound of the birds and the breeze was so pleasant that I stopped for a long time just to listen.".to_string()])]
    text: Vec<String>,

    /// Output directory
    #[arg(long, default_value = "results")]
    save_dir: String,

    /// Enable batch mode (multiple text-style pairs)
    #[arg(long, default_value = "false")]
    batch: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== TTS Inference with ONNX Runtime (Rust) ===\n");
    let args = Args::parse();

    if args.openai {
        println!("Starting OpenAI-compatible API server mode...");

        // Load configuration
        let mut server_config = ServerConfig::load_or_default(&args.config);

        // Override config with command line arguments only if explicitly provided
        if let Some(host) = args.host {
            server_config.server.host = host;
        }
        if let Some(port) = args.port {
            server_config.server.port = port;
        }
        if let Some(onnx_dir) = args.onnx_dir {
            server_config.tts.onnx_dir = onnx_dir;
        }
        if let Some(use_gpu) = args.use_gpu {
            server_config.tts.use_gpu = use_gpu;
        }
        if let Some(total_step) = args.total_step {
            server_config.tts.total_step = total_step;
        }
        if let Some(speed) = args.speed {
            server_config.tts.speed = speed;
        }

        let log_filter = format!("{},ort={}", server_config.logging.level, server_config.logging.ort_level);

        tracing_subscriber::registry()
		.with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| log_filter.into()))
		.with(tracing_subscriber::fmt::layer())
		.init();

        info!("Server configuration loaded: {:?}", server_config);

        // Start the server
        start_server(server_config).await?;

        return Ok(());
    }

    // --- CLI Mode (original functionality) --- //
    let total_step = args.total_step.unwrap_or(5);
    let speed = args.speed.unwrap_or(1.05);
    let n_test = args.n_test;
    let voice_style_paths = &args.voice_style;
    let text_list = &args.text;
    let save_dir = &args.save_dir;
    let batch = args.batch;

    if batch {
        if voice_style_paths.len() != text_list.len() {
            anyhow::bail!(
                "Number of voice styles ({}) must match number of texts ({})",
                voice_style_paths.len(),
                text_list.len()
            );
        }
    }

    let bsz = voice_style_paths.len();

    let mut text_to_speech = load_text_to_speech(&args.onnx_dir.as_deref().unwrap_or("assets/onnx"), args.use_gpu.unwrap_or(false))?;

    let style = load_voice_style(voice_style_paths, true)?;

    fs::create_dir_all(save_dir)?;

    for n in 0..n_test {
        println!("\n[{}/{}] Starting synthesis...", n + 1, n_test);

        let (wav, duration) = if batch {
            timer("Generating speech from text", || {
                text_to_speech.batch(text_list, &style, total_step, speed)
            })?
        } else {
            let (w, d) = timer("Generating speech from text", || {
                text_to_speech.call(&text_list[0], &style, total_step, speed, 0.3)
            })?;
            (w, vec![d])
        };

        // Save outputs
        for i in 0..bsz {
            let fname = format!("{}_{}.wav", sanitize_filename(&text_list[i], 20), n + 1);
            let wav_slice = if batch {
                let wav_len = wav.len() / bsz;
                let actual_len = (text_to_speech.sample_rate as f32 * duration[i]) as usize;
                let wav_start = i * wav_len;
                let wav_end = wav_start + actual_len.min(wav_len);
                &wav[wav_start..wav_end]
            } else {
                // For non-batch mode, wav is a single concatenated audio
                let actual_len = (text_to_speech.sample_rate as f32 * duration[0]) as usize;
                &wav[..actual_len.min(wav.len())]
            };

            let output_path = PathBuf::from(save_dir).join(&fname);
            write_wav_file(&output_path, wav_slice, text_to_speech.sample_rate)?;
            println!("Saved: {}", output_path.display());
        }
    }

    println!("\n=== Synthesis completed successfully! ===");
    mem::forget(text_to_speech);

    // Use _exit to bypass all cleanup handlers and avoid ONNX Runtime mutex issues on macOS
    unsafe {
        libc::_exit(0);
    }
}
