//! aion-forge-cli — Aion Forge 命令行接口
//!
//! 用法：
//!   aion-forge-cli <tool> '<json_params>'    执行工具
//!   aion-forge-cli --list                    列出工具
//!   aion-forge-cli acp                       ACP 协议模式（供 AionUi Agents）
//!   aion-forge-cli setup                     一键注册 Agent + 部署扩展

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use glitch_filter::{GlitchFilter, Sanitizer};
use serde_json::{json, Value};

use aion_types::types::ExecutionContext;

mod acp;
mod setup;

#[derive(Parser)]
#[command(
    name = "aion-forge-cli",
    version = "0.7.0",
    about = "Aion Forge — 72 个内置工具的 AI 智能体引擎"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long)]
    tool: Option<String>,

    #[arg(short, long)]
    params: Option<String>,

    #[arg(short, long)]
    list: bool,

    #[arg(short, long)]
    quiet: bool,
}

#[derive(Parser)]
enum Commands {
    /// ACP 协议服务器模式（供 AionUi Agents 页面使用）
    Acp,

    /// 一键注册 Agent + 部署扩展文件到 aioncore
    Setup {
        /// 仅探查，不写入
        #[arg(short, long)]
        dry_run: bool,
        /// 显示内置 Agent 的模型配置参考
        #[arg(short, long)]
        models: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // ACP 模式下日志必须走 stderr（避免污染 stdout 的 ACP 协议流），
    // 非 ACP 模式也统一走 stderr，让 stdout 只输出命令结果
    let is_acp = matches!(&cli.command, Some(Commands::Acp));
    if is_acp {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_writer(std::io::stderr)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_target(cli.quiet)
            .init();
    }

    dotenvy::dotenv().ok();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = dotenvy::from_path(dir.join(".env"));
        }
    }

    // 设置 AI_PROVIDERS_DISABLED 默认值（如果未设置或为空）
    // 只禁真的不能用的：host-anthropic-proxy（本地代理未运行）、ollama-local（未安装）
    // opencode-zen/openrouter/openai-compatible/zhipu-compatible 都有API Key，不禁用
    match std::env::var("AI_PROVIDERS_DISABLED") {
        Ok(v) if !v.trim().is_empty() => {} // 已有有效值，跳过
        _ => {
            std::env::set_var(
                "AI_PROVIDERS_DISABLED",
                "host-anthropic-proxy,ollama-local",
            );
        }
    }

    aion_router::learner::init_learner(&std::env::current_dir().unwrap_or_default());

    // 处理子命令
    match &cli.command {
        Some(Commands::Acp) => {
            acp::run_acp_server().await?;
            return Ok(());
        }
        Some(Commands::Setup { dry_run, models }) => {
            setup::run(*dry_run, *models)?;
            return Ok(());
        }
        None => {}
    }

    if cli.list {
        let caps = aion_types::capability_registry::CapabilityRegistry::builtin();
        let tools: Vec<Value> = caps
            .definitions()
            .map(|d| json!({"name": d.name, "description": d.description}))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"tools": tools, "total": tools.len()}))?
        );
        return Ok(());
    }

    let tool_name = cli.tool.unwrap_or_default();
    let mut params: Value = match &cli.params {
        Some(p) => serde_json::from_str(p).unwrap_or_else(|_| json!({"text": p})),
        None => json!({}),
    };

    // ── 输入安全：Glitch 检测 + 控制字符清理 ──────────────────
    if let Some(text) = params.get("text").and_then(|v| v.as_str()) {
        let filter = GlitchFilter::new();
        let alerts = filter.check(text);
        if !alerts.is_empty() {
            eprintln!("⚠️  Glitch 检测：发现 {} 个异常 token", alerts.len());
            for token in &alerts {
                eprintln!("  - {} ({:?}): {}", token.token, token.behavior, token.category);
            }
        }

        let mut sanitizer = Sanitizer::new();
        let clean_text = sanitizer.sanitize(text);
        if sanitizer.sanitized_count() > 0 {
            eprintln!("🧹 输入已清理：移除了 {} 个危险字符", sanitizer.sanitized_count());
            if !cli.quiet {
                eprintln!("{}", sanitizer.sanitized_report());
            }
            params["text"] = json!(clean_text);
        }
    }

    let ctx = ExecutionContext::new(&tool_name, &tool_name).with_context(params);

    let registry = aion_router::builtins::BuiltinRegistry::default_registry();
    if let Some(builtin) = registry.get(&tool_name) {
        let dummy_skill = aion_types::types::SkillDefinition {
            metadata: aion_types::types::SkillMetadata {
                name: tool_name.clone(),
                version: "0.1.0".to_string(),
                capabilities: vec![tool_name.clone()],
                entrypoint: format!("builtin:{}", tool_name),
                engine_capable: false,
                permissions: aion_types::types::PermissionSet::default_deny().with_network(true),
                instruction: None,
            },
            root_dir: PathBuf::new(),
            source: aion_types::types::SkillSource::Local,
        };
        let result = builtin.execute(&dummy_skill, &ctx).await?;

        if cli.quiet {
            println!("{}", serde_json::to_string(&result)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    } else {
        anyhow::bail!("工具 '{}' 未找到。用 --list 查看所有可用工具。", tool_name);
    }

    Ok(())
}
