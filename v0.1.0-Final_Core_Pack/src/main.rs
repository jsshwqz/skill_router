use clap::Parser;
use anyhow::Result;
use std::path::Path;
use skill_router::models::Config;
use skill_router::planner::Planner;
use skill_router::loader::Loader;
use skill_router::registry::RegistryManager;
use skill_router::matcher::Matcher;
use skill_router::executor::Executor;
use serde_json::json;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(help = "The task to be executed")]
    task: String,

    #[arg(short, long, default_value = "config.json")]
    config: String,

    #[arg(short, long, help = "Output in JSON format")]
    json: bool,
}

fn init_default_config(path: &Path) -> Result<Config> {
    let config = Config {
        enable_auto_install: false,
        skills_dir: "skills".to_string(),
        registry_file: "registry.json".to_string(),
        logs_dir: "logs".to_string(),
        trusted_sources: vec!["https://github.com/trusted-source".to_string()],
        llm_enabled: Some(false),
        llm_command: None,
    };
    let content = serde_json::to_string_pretty(&config)?;
    std::fs::write(path, content)?;
    Ok(config)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = Path::new(&cli.config);
    
    let config = if config_path.exists() {
        skill_router::load_config(config_path)?
    } else {
        if !cli.json { println!("Config file not found. Creating default config."); }
        init_default_config(config_path)?
    };

    let mut registry = RegistryManager::load_registry(&config.registry_file)?;
    let local_skills = Loader::load_skills(&config.skills_dir)?;
    for skill in local_skills {
        RegistryManager::update_skill(&mut registry, skill);
    }
    RegistryManager::save_registry(&config.registry_file, &registry)?;

    if !cli.json { println!("Task: {}", cli.task); }

    let required_caps = Planner::infer_capabilities(&cli.task);
    if !cli.json { println!("Required Capabilities: {:?}", required_caps); }

    if required_caps.is_empty() {
        if cli.json {
            println!("{}", json!({"status": "error", "message": "No capabilities inferred"}));
        } else {
            println!("No capabilities inferred for task.");
        }
        return Ok(());
    }

    let skill_to_execute = if let Some(skill) = Matcher::find_best_match(&registry, &required_caps) {
        if !cli.json { println!("Matched Skill: {}", skill.name); }
        Some(skill)
    } else {
        if !cli.json { 
            println!("No matching skill found for capabilities: {:?}", required_caps);
            println!("Phase 3: Triggering Search/Synthesis...");
        }
        
        let mut new_skill = None;
        for cap in &required_caps {
            if let Some(candidate) = skill_router::online_search::OnlineSearch::search(&config, cap) {
                if !cli.json { println!("Found candidate skill: {} for cap: {}", candidate.name, cap); }
                new_skill = Some(candidate);
                break;
            }
        }

        if new_skill.is_none() {
            if let Some(cap) = required_caps.first() {
                match skill_router::synth::Synth::synthesize(&config, cap, &cli.task) {
                    Ok(synth_skill) => {
                        if !cli.json { println!("Synthesized skill: {}", synth_skill.name); }
                        new_skill = Some(synth_skill);
                    }
                    Err(e) => if !cli.json { eprintln!("Failed to synthesize skill: {}", e) },
                }
            }
        }

        if let Some(skill) = new_skill {
            RegistryManager::update_skill(&mut registry, skill.clone());
            RegistryManager::save_registry(&config.registry_file, &registry)?;
            Some(skill)
        } else {
            None
        }
    };

    if let Some(skill) = skill_to_execute {
        let start_time = std::time::Instant::now();
        let result = Executor::execute(&config, &skill, cli.json);
        let duration = start_time.elapsed().as_millis() as f64;
        
        let mut lifecycle_decision = None;
        {
            let skill_entry = registry.skills.get_mut(&skill.name).unwrap();
            let mut usage = skill_entry.usage.clone().unwrap_or_default();
            usage.total_calls += 1;
            usage.last_used = chrono::Utc::now().to_rfc3339();
            
            let total_time = (usage.avg_latency_ms * (usage.total_calls as f64 - 1.0)) + duration;
            usage.avg_latency_ms = total_time / (usage.total_calls as f64);

            match &result {
                Ok(_) => {
                    usage.success_calls += 1;
                    if !cli.json { println!("Execution finished successfully."); }
                }
                Err(e) => {
                    usage.failed_calls += 1;
                    if !cli.json { eprintln!("Execution failed: {}", e); }
                }
            };
            
            skill_entry.usage = Some(usage);

            if let Some(decision) = skill_router::lifecycle::Lifecycle::decide(skill_entry) {
                if !cli.json { println!("Lifecycle Recommendation for '{}': {}", skill.name, decision); }
                skill_entry.lifecycle = Some(skill_router::models::Lifecycle { decision: decision.clone() });
                lifecycle_decision = Some(decision);
            }
        }
        
        let status = if result.is_ok() { "success" } else { "failed" };
        RegistryManager::save_registry(&config.registry_file, &registry)?;

        if cli.json {
            println!("{}", json!({
                "status": status,
                "skill": skill.name,
                "duration_ms": duration,
                "lifecycle": lifecycle_decision
            }));
        }
    } else {
        if cli.json {
            println!("{}", json!({"status": "error", "message": "No skill matched or synthesized"}));
        } else {
            println!("Final: No skill could be matched or synthesized for task.");
        }
    }

    Ok(())
}
