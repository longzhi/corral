use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod audit;
mod broker;
mod manifest;
mod platform;
mod policy;
mod watchdog;

#[derive(Parser)]
#[command(name = "corral")]
#[command(about = "Capability-based sandbox for Agent Skills", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a skill in the sandbox
    Run {
        /// Path to skill directory
        #[arg(short, long)]
        skill: PathBuf,

        /// Additional arguments to pass to the skill
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Inspect skill permissions without running
    Inspect {
        /// Path to skill directory
        #[arg(short, long)]
        skill: PathBuf,
    },

    /// Approve skill permissions
    Approve {
        /// Path to skill directory
        #[arg(short, long)]
        skill: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run { skill, args } => {
            run_skill(skill, args).await?;
        }
        Commands::Inspect { skill } => {
            inspect_skill(skill)?;
        }
        Commands::Approve { skill } => {
            approve_skill(skill)?;
        }
    }

    Ok(())
}

async fn run_skill(skill_path: PathBuf, _args: Vec<String>) -> Result<()> {
    use crate::manifest::Manifest;
    use crate::policy::PolicyEngine;

    // Load and parse manifest
    let manifest = Manifest::load(&skill_path)?;
    tracing::info!("Loaded skill: {} v{}", manifest.name, manifest.version);

    // Create policy engine
    let policy = PolicyEngine::new(manifest.clone());

    // Setup sandbox environment
    let runtime = platform::create_runtime(&manifest, &skill_path)?;

    // Start broker
    let broker_handle = broker::start_broker(policy.clone()).await?;

    // Start watchdog
    let watchdog = watchdog::Watchdog::new(manifest.clone());

    // Execute skill
    tracing::info!("Executing skill...");
    let result = runtime.execute(&broker_handle).await?;

    // Stop watchdog
    watchdog.stop()?;

    // Generate audit log
    audit::log_execution(&manifest, &broker_handle, result.exit_code).await?;

    if result.exit_code == 0 {
        tracing::info!("Skill completed successfully");
        println!("{}", result.stdout);
    } else {
        tracing::error!("Skill failed with exit code: {}", result.exit_code);
        eprintln!("{}", result.stderr);
        std::process::exit(result.exit_code);
    }

    Ok(())
}

fn inspect_skill(skill_path: PathBuf) -> Result<()> {
    use crate::manifest::Manifest;

    let manifest = Manifest::load(&skill_path)?;

    println!("📦 Skill: {} v{}", manifest.name, manifest.version);
    println!("   Author: {}", manifest.author);
    println!("   Description: {}", manifest.description);
    println!();
    println!("Permissions requested:");
    println!();

    // File system permissions
    if let Some(fs) = &manifest.permissions.fs {
        println!("📁 File Access:");
        if let Some(read) = &fs.read {
            println!("   Read:");
            for path in read {
                println!("     - {}", path);
            }
        }
        if let Some(write) = &fs.write {
            println!("   Write:");
            for path in write {
                println!("     - {}", path);
            }
        }
        println!();
    }

    // Network permissions
    if let Some(network) = &manifest.permissions.network {
        if let Some(allow) = &network.allow {
            println!("🌐 Network:");
            for host in allow {
                println!("   - {}", host);
            }
            println!();
        }
    }

    // Service permissions
    if let Some(services) = &manifest.permissions.services {
        println!("🔧 Services:");
        if services.reminders.is_some() {
            println!("   - Reminders");
        }
        if services.calendar.is_some() {
            println!("   - Calendar");
        }
        if services.browser.is_some() {
            println!("   - Browser");
        }
        if services.notifications.is_some() {
            println!("   - Notifications");
        }
        if services.clipboard.is_some() {
            println!("   - Clipboard");
        }
        println!();
    }

    // Exec permissions
    if let Some(exec) = &manifest.permissions.exec {
        println!("⚙️  Executables:");
        for cmd in exec {
            println!("   - {}", cmd);
        }
        println!();
    }

    // Env permissions
    if let Some(env) = &manifest.permissions.env {
        println!("🌍 Environment Variables:");
        for var in env {
            println!("   - {}", var);
        }
        println!();
    }

    Ok(())
}

fn approve_skill(skill_path: PathBuf) -> Result<()> {
    use crate::manifest::Manifest;
    use std::io::{self, Write};

    let manifest = Manifest::load(&skill_path)?;

    inspect_skill(skill_path.clone())?;

    print!("Approve these permissions? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" {
        // In a real implementation, this would store approval in a permissions database
        // For now, just confirm
        println!("✅ Permissions approved for {}", manifest.name);
        Ok(())
    } else {
        println!("❌ Permissions denied");
        std::process::exit(1);
    }
}
