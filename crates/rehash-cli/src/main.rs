use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use rehash_core::{CacheEngine, CacheKey, Hasher};

#[derive(Parser)]
#[command(name = "rehash", about = "Build cache daemon")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    Build(BuildArgs),
    Clear,
    Stats,
    Info,
}

#[derive(clap::Args)]
struct BuildArgs {
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<String>,

    #[arg(short, long)]
    watch: Option<PathBuf>,

    #[arg(short, long)]
    force: bool,

    #[arg(long, default_value = "1024")]
    max_cache_mb: u64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let rt = tokio::runtime::Runtime::new()?;
    match cli.command {
        Command::Build(args) => cmd_build(args, &rt),
        Command::Clear => cmd_clear(),
        Command::Stats => cmd_stats(),
        Command::Info => cmd_info(),
    }
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rehash")
}

fn discover_build_files(root: &Path) -> Vec<PathBuf> {
    let candidates = [
        "Cargo.toml", "CMakeLists.txt", "package.json", "Makefile",
        "meson.build", "build.gradle", "pom.xml", "go.mod",
    ];
    let mut files = Vec::new();
    for name in &candidates {
        let p = root.join(name);
        if p.exists() { files.push(p); }
    }
    files
}

// ponytail: O(n) dir walk, ok for <100k files; switch to walkdir if profiling says so
fn discover_source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() { return files; }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "node_modules" && name != "target" && name != "build" {
                    stack.push(path);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext, "rs" | "c" | "h" | "cpp" | "hpp" | "js" | "ts" | "py" | "go" | "java") {
                    files.push(path);
                }
            }
        }
    }
    files
}

fn dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    for entry in entries.flatten() {
        let path = entry.path();
        total += if path.is_dir() { dir_size(&path) } else { path.metadata().map(|m| m.len()).unwrap_or(0) };
    }
    total
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB { format!("{:.2} GB", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.2} MB", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.2} KB", bytes as f64 / KB as f64) }
    else { format!("{} B", bytes) }
}

fn cmd_build(args: BuildArgs, rt: &tokio::runtime::Runtime) -> Result<()> {
    let root = args.watch.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
    let cache = cache_dir();
    let mut engine = CacheEngine::new(cache.clone(), args.max_cache_mb);

    let mut hasher = Hasher::new();
    for f in discover_build_files(&root) { let _ = hasher.update(&f); }
    for f in discover_source_files(&root) { let _ = hasher.update(&f); }
    hasher.update_str(&args.command.join(" "));
    hasher.update_str(&std::env::var("PATH").unwrap_or_default());
    if let Ok(tc) = std::env::var("RUST_TOOLCHAIN") { hasher.update_str(&tc); }

    let key = CacheKey::new(hasher.finalize());

    let cached = rt.block_on(engine.contains(&key))?; // ponytail: sync wrapper, async core avoids dual API

    if cached && !args.force {
        println!("cache hit");
        rt.block_on(engine.restore(&key, &root))?;
        return Ok(());
    }

    println!("miss, running: {}", args.command.join(" "));
    let start = Instant::now();
    // ponytail: no SIGINT forwarding to child; add when builds are long
    let status = std::process::Command::new(&args.command[0])
        .args(&args.command[1..])
        .current_dir(&root)
        .status()?;
    let elapsed = start.elapsed();
    if !status.success() {
        anyhow::bail!("build failed with exit code: {:?}", status.code());
    }

    // ponytail: hardcoded output dirs for v0.1.0; make configurable when multi-project
    let out_dirs = [root.join("target").join("debug"), root.join("build"), root.join("dist")];
    let mut outputs = Vec::new();
    for d in &out_dirs {
        if d.exists() {
            if let Ok(entries) = std::fs::read_dir(d) {
                for e in entries.flatten() { outputs.push(e.path()); }
            }
        }
    }

    rt.block_on(engine.store(&key, &outputs, "default", elapsed.as_millis() as u64))?;
    println!("cached {} outputs in {:.2}s", outputs.len(), elapsed.as_secs_f64());
    Ok(())
}

fn cmd_clear() -> Result<()> {
    let cache = cache_dir();
    if cache.exists() { std::fs::remove_dir_all(&cache)?; }
    println!("cleared");
    Ok(())
}

fn cmd_stats() -> Result<()> {
    let cache = cache_dir();
    if !cache.exists() { return Ok(println!("cache empty")); }
    let objects = cache.join("objects");
    let count = if objects.exists() { std::fs::read_dir(objects)?.count() } else { 0 };
    println!("entries: {}\nsize:    {}\npath:    {}", count, format_size(dir_size(&cache)), cache.display());
    Ok(())
}

fn cmd_info() -> Result<()> {
    println!("rehash {}\ncache:  {}\nhash:   blake3", env!("CARGO_PKG_VERSION"), cache_dir().display());
    Ok(())
}
