mod output;
mod mcp;
mod adapter_gen;

use clap::Parser;
use anyhow::Result;
use aion_router::SkillRouter;
use aion_router::message_bus::MessageBus;
use aion_router::agent_runtime::AgentRuntime;
use aion_router::coordinator::MultiAgentCoordinator;
use aion_types::types::RouterPaths;
use aion_types::agent_message::AgentRole;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

use output::{OutputMode, ProgressReporter};

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

    /// Output as machine-readable JSON (suppresses stderr)
    #[arg(long)]
    json: bool,

    /// Quiet mode: only output the result, no status messages
    #[arg(long, short)]
    quiet: bool,
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
    /// Multi-agent commands
    Agent {
        #[command(subcommand)]
        subcommand: AgentCommands,
    },
    /// Start MCP (Model Context Protocol) stdio server for Claude integration
    McpServer,
    /// Generate adapter configurations for AI platforms (Claude MCP, OpenAI, HTTP)
    Adapter {
        #[command(subcommand)]
        subcommand: AdapterCommands,
    },
}

#[derive(clap::Subcommand)]
enum AdapterCommands {
    /// Generate all adapter configs (MCP, OpenAI, OpenAPI, aionui)
    Generate {
        #[arg(short, long, default_value = "./adapters", help = "Output directory")]
        output: String,
        #[arg(short, long, help = "Format: all|mcp|openai|openapi|aionui")]
        format: Option<String>,
    },
}

#[derive(clap::Subcommand)]
enum AgentCommands {
    /// Start an agent with specified role and capabilities
    Start {
        #[arg(short, long, default_value = "executor", help = "Agent role: orchestrator|executor|specialist|reviewer|memory_keeper")]
        role: String,
        #[arg(short, long, default_value = "", help = "Comma-separated list of capabilities (empty = all)")]
        capabilities: String,
        #[arg(short, long, help = "Agent ID (auto-generated if not set)")]
        id: Option<String>,
    },
    /// Run a task using multi-agent coordination (Orchestrator dispatches to Executors)
    Run {
        #[arg(help = "The task to execute")]
        task: String,
        #[arg(short, long, default_value = "2", help = "Number of executor agents to spawn")]
        agents: usize,
    },
    /// List registered agents info
    Info,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Try loading .env from current dir, then from binary's parent dir
    dotenvy::dotenv().ok();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let env_path = dir.join(".env");
            if env_path.exists() {
                dotenvy::from_path(&env_path).ok();
            }
            // Also try parent of bin/ directory
            if let Some(parent) = dir.parent() {
                let env_path = parent.join(".env");
                if env_path.exists() {
                    dotenvy::from_path(&env_path).ok();
                }
            }
        }
    }

    let cli = Cli::parse();

    let output_mode = if cli.json {
        OutputMode::Json
    } else if cli.quiet {
        OutputMode::Quiet
    } else {
        OutputMode::Pretty
    };

    // MCP 模式：tracing 必须输出到 stderr（stdout 是 JSON-RPC 通道）
    // 其他模式：仅 Pretty 模式初始化日志
    let is_mcp = matches!(&cli.command, Some(Commands::McpServer));
    if is_mcp {
        // 标记 MCP 模式，orchestrator 会据此限制等待时间不超过 MCP 协议超时
        std::env::set_var("AION_MCP_MODE", "1");
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("aion_router=info,aion_intel=info"))
            )
            .init();
    } else if matches!(output_mode, OutputMode::Pretty) {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("aion_router=info,aion_intel=info,aion_memory=info"))
            )
            .init();
    }

    let workdir = cli.workdir.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let paths = RouterPaths::for_workspace(&workdir);
    let reporter = ProgressReporter::new(output_mode);

    match cli.command {
        Some(Commands::ExportManifest { output }) => {
            reporter.info(&format!("Exporting manifest to {}...", output));
            let router = SkillRouter::new(paths)?;
            let definitions: Vec<_> = router.registry().definitions().cloned().collect();
            let json = serde_json::to_string_pretty(&definitions)?;
            std::fs::write(&output, json)?;
            reporter.info("Manifest exported successfully!");
        }
        Some(Commands::Run { task }) => {
            run_task(&task, cli.context, paths, &reporter).await?;
        }
        Some(Commands::Agent { subcommand }) => {
            run_agent_command(subcommand, paths, &reporter).await?;
        }
        Some(Commands::McpServer) => {
            mcp::run_mcp_server(paths).await?;
        }
        Some(Commands::Adapter { subcommand }) => {
            run_adapter_command(subcommand, paths, &reporter)?;
        }
        None => {
            let task = cli.task.ok_or_else(|| anyhow::anyhow!("No task provided. Use 'run <task>' or specify a task directly."))?;
            run_task(&task, cli.context, paths, &reporter).await?;
        }
    }

    Ok(())
}

async fn run_agent_command(cmd: AgentCommands, paths: RouterPaths, reporter: &ProgressReporter) -> Result<()> {
    match cmd {
        AgentCommands::Info => {
            reporter.info("Multi-Agent System Info");
            reporter.info("  MessageBus: in-process (tokio broadcast)");
            reporter.info("  NATS backend: not configured (set NATS_URL to enable)");
            reporter.info("");
            reporter.info("Available roles:");
            reporter.info("  orchestrator  - Coordinates tasks, dispatches to executors");
            reporter.info("  executor      - General-purpose task executor");
            reporter.info("  specialist    - Focused on specific capabilities");
            reporter.info("  reviewer      - Security review and result verification");
            reporter.info("  memory_keeper - Memory store management");
        }

        AgentCommands::Start { role, capabilities, id } => {
            let agent_id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let caps: Vec<String> = capabilities
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let agent_role = match role.as_str() {
                "orchestrator" => AgentRole::Orchestrator,
                "planner"      => AgentRole::Planner,
                "executor"     => AgentRole::Executor,
                "specialist"   => AgentRole::Specialist { capabilities: caps.clone() },
                "reviewer"     => AgentRole::Reviewer,
                "memory_keeper"=> AgentRole::MemoryKeeper,
                other => {
                    return Err(anyhow::anyhow!("Unknown role: '{}'. Valid roles: orchestrator, planner, executor, specialist, reviewer, memory_keeper", other));
                }
            };

            reporter.info(&format!("Starting agent [{}] as {:?}", agent_id, agent_role));
            reporter.info(&format!("  Capabilities: {}", if caps.is_empty() { "all".to_string() } else { caps.join(", ") }));

            let bus = Arc::new(MessageBus::new(64));
            let runtime = AgentRuntime::new(&agent_id, agent_role, caps, paths, Arc::clone(&bus))?;
            let handle = runtime.spawn();

            reporter.info(&format!("Agent [{}] running. Press Ctrl+C to stop.", agent_id));
            tokio::signal::ctrl_c().await?;
            reporter.info(&format!("Shutting down agent [{}]...", agent_id));
            handle.abort();
        }

        AgentCommands::Run { task, agents } => {
            reporter.info(&format!("Multi-Agent execution: {} executor agent(s)", agents));
            reporter.info(&format!("Task: {}", task));

            let bus = Arc::new(MessageBus::new(128));
            let progress = reporter.pipeline_progress(agents as u64 + 1); // +1 for result collection

            let mut handles = Vec::new();
            for i in 0..agents {
                let agent_id = format!("executor-{}", i);
                let runtime = AgentRuntime::new(
                    &agent_id,
                    AgentRole::Executor,
                    vec![],
                    paths.clone(),
                    Arc::clone(&bus),
                )?;
                handles.push(runtime.spawn());
                if let Some(ref pb) = progress {
                    pb.set_message(format!("Spawned agent [{}]", agent_id));
                    pb.inc(1);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            let mut coordinator = MultiAgentCoordinator::new(Arc::clone(&bus));
            for i in 0..agents {
                let agent_id = format!("executor-{}", i);
                coordinator.register_agent(aion_types::agent_message::AgentRef::local(
                    &agent_id,
                    AgentRole::Executor,
                ));
            }

            let target = format!("executor-{}", 0);
            let task_id = uuid::Uuid::new_v4().to_string();
            let msg = aion_types::agent_message::AgentMessage::new(
                "orchestrator-0",
                &target,
                aion_types::agent_message::AgentMessageType::TaskAssignment {
                    task_id: task_id.clone(),
                    task: task.clone(),
                    capability: "echo".to_string(),
                },
            );

            let mut rx = bus.subscribe();
            bus.publish(msg);

            if let Some(ref pb) = progress {
                pb.set_message("Waiting for result...");
            }

            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(60);
            loop {
                match tokio::time::timeout_at(deadline, rx.recv()).await {
                    Ok(Ok(response_msg)) => {
                        if let aion_types::agent_message::AgentMessageType::TaskResult { task_id: tid, success, result, error } = &response_msg.message_type {
                            if *tid == task_id {
                                if let Some(ref pb) = progress {
                                    pb.inc(1);
                                    pb.finish_and_clear();
                                }
                                if *success {
                                    reporter.info(&format!("Task completed by agent [{}]", response_msg.from_agent));
                                    if !result.is_null() {
                                        println!("{}", serde_json::to_string_pretty(result)?);
                                    }
                                } else {
                                    reporter.finish_error(None, &format!("Task failed: {}", error.as_deref().unwrap_or("unknown error")));
                                    std::process::exit(1);
                                }
                                break;
                            }
                        }
                    }
                    _ => {
                        if let Some(ref pb) = progress {
                            pb.finish_and_clear();
                        }
                        reporter.finish_error(None, "Task timed out");
                        std::process::exit(1);
                    }
                }
            }

            for h in handles { h.abort(); }
        }
    }
    Ok(())
}

fn run_adapter_command(cmd: AdapterCommands, paths: RouterPaths, reporter: &ProgressReporter) -> Result<()> {
    match cmd {
        AdapterCommands::Generate { output, format } => {
            let output_dir = std::path::PathBuf::from(&output);
            let fmt = format.as_deref().unwrap_or("all");

            reporter.info(&format!("Generating adapter configs → {}", output));

            let router = SkillRouter::new(paths)?;
            let caps: Vec<_> = router.registry().definitions().cloned().collect();

            std::fs::create_dir_all(&output_dir)?;

            match fmt {
                "all" => {
                    std::fs::create_dir_all(output_dir.join("claude-mcp"))?;
                    std::fs::create_dir_all(output_dir.join("openai"))?;
                    std::fs::create_dir_all(output_dir.join("http"))?;
                    std::fs::create_dir_all(output_dir.join("aionui"))?;
                    adapter_gen::generate_mcp_config(&output_dir)?;
                    adapter_gen::generate_openai_functions(&caps, &output_dir)?;
                    adapter_gen::generate_openapi_spec(&caps, &output_dir)?;
                    adapter_gen::generate_aionui_config(&caps, &output_dir)?;
                    reporter.info("Generated: claude-mcp, openai, openapi, aionui");
                }
                "mcp" => {
                    std::fs::create_dir_all(output_dir.join("claude-mcp"))?;
                    adapter_gen::generate_mcp_config(&output_dir)?;
                    reporter.info("Generated: claude-mcp/claude_desktop_config.json");
                }
                "openai" => {
                    std::fs::create_dir_all(output_dir.join("openai"))?;
                    adapter_gen::generate_openai_functions(&caps, &output_dir)?;
                    reporter.info("Generated: openai/functions.json");
                }
                "openapi" => {
                    std::fs::create_dir_all(output_dir.join("http"))?;
                    adapter_gen::generate_openapi_spec(&caps, &output_dir)?;
                    reporter.info("Generated: http/openapi.json");
                }
                "aionui" => {
                    std::fs::create_dir_all(output_dir.join("aionui"))?;
                    adapter_gen::generate_aionui_config(&caps, &output_dir)?;
                    reporter.info("Generated: aionui/skill.json");
                }
                other => {
                    return Err(anyhow::anyhow!("Unknown format: '{}'. Use: all|mcp|openai|openapi|aionui", other));
                }
            }
        }
    }
    Ok(())
}

async fn run_task(task: &str, context: Option<String>, paths: RouterPaths, reporter: &ProgressReporter) -> Result<()> {
    reporter.info(&format!("Aion CLI - workspace: {:?}", paths.workspace_root));
    reporter.info(&format!("Task: {}", task));

    let router = SkillRouter::new(paths)?;

    let extra_context = if let Some(ctx_str) = context {
        Some(serde_json::from_str(&ctx_str)?)
    } else {
        None
    };

    let spinner = reporter.routing_spinner();

    match router.route_with_context(task, extra_context).await {
        Ok(result) => {
            reporter.finish_success(
                spinner,
                &result.skill.metadata.name,
                &result.execution.status,
                &format!("{:?}", result.lifecycle),
                &result.execution.result,
            );
        }
        Err(e) => {
            reporter.finish_error(spinner, &e.to_string());
            std::process::exit(1);
        }
    }
    Ok(())
}
