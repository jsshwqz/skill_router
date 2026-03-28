use clap::Parser;
use anyhow::Result;
use aion_router::SkillRouter;
use aion_types::types::RouterPaths;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "aion-cli", about = "Standard CLI for Skill Router Workspace")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(help = "The natural language task to execute (shorthand for 'run')")]
    task: Option<String>,

    #[arg(short, long, help = "Path to workspace root (defaults to current dir)")]
    workdir: Option<PathBuf>,

    #[arg(short, long, help = "Optional JSON context for the task")]
    context: Option<String>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Execute a task (default)
    Run {
        #[arg(help = "The natural language task to execute")]
        task: String,
    },
    /// Export all registered capabilities to a manifest file for AI agents
    ExportManifest {
        #[arg(short, long, default_value = "capabilities_manifest.json")]
        output: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workdir = cli.workdir.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let paths = RouterPaths::for_workspace(&workdir);

    match cli.command {
        Some(Commands::ExportManifest { output }) => {
            println!("📂 Exporting manifest to {}...", output);
            let router = SkillRouter::new(paths)?;
            let definitions: Vec<_> = router.registry().definitions().cloned().collect();
            let json = serde_json::to_string_pretty(&definitions)?;
            std::fs::write(&output, json)?;
            println!("✅ Manifest exported successfully!");
        }
        Some(Commands::Run { task }) => {
            run_task(&task, cli.context, paths)?;
        }
        None => {
            let task = cli.task.ok_or_else(|| anyhow::anyhow!("No task provided. Use 'run <task>' or specify a task directly."))?;
            run_task(&task, cli.context, paths)?;
        }
    }

    Ok(())
}

fn run_task(task: &str, context: Option<String>, paths: RouterPaths) -> Result<()> {
    println!("🚀 Aion-Core CLI Starting...");
    println!("📂 Workspace: {:?}", paths.workspace_root);
    println!("📝 Task: {}", task);

    let router = SkillRouter::new(paths)?;

    let extra_context = if let Some(ctx_str) = context {
        Some(serde_json::from_str(&ctx_str)?)
    } else {
        None
    };

    println!("🔍 Routing task...");
    match router.route_with_context(task, extra_context) {
        Ok(result) => {
            println!("✅ Execution Success!");
            println!("🛠️  Skill: {}", result.skill.metadata.name);
            println!("📊 Status: {}", result.execution.status);
            println!("💡 Lifecycle: {:?}", result.lifecycle);
            if !result.execution.result.is_null() {
                println!("📦 Output: {}", serde_json::to_string_pretty(&result.execution.result)?);
            }
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            std::process::exit(1);
        }
    }
    Ok(())
}
