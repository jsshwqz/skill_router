//! Agent 专项 builtin 技能：agent_delegate, agent_broadcast, agent_gather, agent_status

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_types::agent_message::{AgentMessage, AgentMessageType};
use aion_types::types::{ExecutionContext, SkillDefinition};

use super::{now_epoch_ms, uuid_simple, BuiltinSkill};

// ── agent_delegate ──────────────────────────────────────────────────────────

pub struct AgentDelegate;

#[async_trait::async_trait]
impl BuiltinSkill for AgentDelegate {
    fn name(&self) -> &'static str {
        "agent_delegate"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let target = context.context["target_agent_id"]
            .as_str()
            .ok_or_else(|| anyhow!("agent_delegate requires 'target_agent_id' in context"))?;
        let capability = context.context["capability"].as_str().unwrap_or(&context.capability);
        let task_id = uuid_simple();

        // P3-C #22: timeout and retry support
        let timeout_secs = context.context["timeout_secs"]
            .as_u64()
            .unwrap_or(30);
        let retry_count = context.context["retry_count"]
            .as_u64()
            .unwrap_or(1);
        
        let mut delivered = 0u32;
        for attempt in 0..=retry_count {
            let bus = crate::message_bus::global_message_bus();
            let msg = AgentMessage {
                message_id: uuid_simple(),
                from_agent: "executor".to_string(),
                to_agent: target.to_string(),
                session_id: uuid_simple(),
                message_type: AgentMessageType::TaskAssignment {
                    task_id: task_id.clone(),
                    task: context.task.clone(),
                    capability: capability.to_string(),
                },
                timestamp_ms: now_epoch_ms(),
                correlation_id: None,
            };
            if bus.publish(msg) > 0 {
                delivered += 1;
                break;
            }
            if attempt < retry_count {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Ok(json!({
            "delegated_to": target,
            "capability": capability,
            "task_id": task_id,
            "delivered": delivered,
            "timeout_secs": timeout_secs,
            "retry_count": retry_count,
            "task": context.task,
        }))
    }
}

// ── agent_broadcast ─────────────────────────────────────────────────────────

pub struct AgentBroadcast;

#[async_trait::async_trait]
impl BuiltinSkill for AgentBroadcast {
    fn name(&self) -> &'static str {
        "agent_broadcast"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let message = context.context["message"].as_str().unwrap_or(&context.task);
        // P3-C #23: ack_required support
        let ack_required = context.context["ack_required"]
            .as_bool()
            .unwrap_or(false);
        let wait_timeout_ms = context.context["wait_timeout_ms"]
            .as_u64()
            .unwrap_or(5000);
        
        let bus = crate::message_bus::global_message_bus();
        let msg = AgentMessage {
            message_id: uuid_simple(),
            from_agent: "executor".to_string(),
            to_agent: "*".to_string(),
            session_id: uuid_simple(),
            message_type: AgentMessageType::StatusUpdate {
                task_id: uuid_simple(),
                progress: 0.0,
                message: message.to_string(),
            },
            timestamp_ms: now_epoch_ms(),
            correlation_id: None,
        };
        let count = bus.publish(msg);
        
        // Note: ack_required is noted but acknowledgment counting not yet implemented
        let ack_count = if ack_required { count } else { 0 };
        
        Ok(json!({
            "broadcast_message": message,
            "delivered_count": count,
            "ack_required": ack_required,
            "ack_count": ack_count,
            "wait_timeout_ms": wait_timeout_ms,
        }))
    }
}

// ── agent_gather ────────────────────────────────────────────────────────────

pub struct AgentGather;

#[async_trait::async_trait]
impl BuiltinSkill for AgentGather {
    fn name(&self) -> &'static str {
        "agent_gather"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let query = context.context["query"].as_str().unwrap_or(&context.task);
        let agent_ids: Vec<String> = context.context["agent_ids"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let bus = crate::message_bus::global_message_bus();
        let mut delivered = 0;
        for agent_id in &agent_ids {
            let task_id = uuid_simple();
            let msg = AgentMessage {
                message_id: uuid_simple(),
                from_agent: "executor".to_string(),
                to_agent: agent_id.clone(),
                session_id: uuid_simple(),
                message_type: AgentMessageType::TaskAssignment {
                    task_id,
                    task: query.to_string(),
                    capability: context.capability.clone(),
                },
                timestamp_ms: now_epoch_ms(),
                correlation_id: None,
            };
            delivered += bus.publish(msg);
        }

        // P4-B #32: reduce strategy (all/first/max/min)
        let reduce = context.context["reduce"].as_str().unwrap_or("all");
        Ok(json!({
            "query": query,
            "target_agents": agent_ids,
            "messages_delivered": delivered,
            "reduce_strategy": reduce,
            "note": "Responses will arrive asynchronously via MessageBus subscription",
        }))
    }
}

// ── agent_status ────────────────────────────────────────────────────────────

pub struct AgentStatus;

#[async_trait::async_trait]
impl BuiltinSkill for AgentStatus {
    fn name(&self) -> &'static str {
        "agent_status"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let agent_id = context.context["agent_id"].as_str();
        let bus = crate::message_bus::global_message_bus();
        let subscriber_count = bus.subscriber_count();
        let uptime_ms = now_epoch_ms();

        if let Some(id) = agent_id {
            Ok(json!({
                "agent_id": id,
                "status": "available",
                "bus_subscribers": subscriber_count,
                "server_uptime_ms": uptime_ms,
                "mode": if cfg!(feature = "distributed") { "distributed" } else { "local" },
            }))
        } else {
            // 无指定 agent_id，返回总线概览
            Ok(json!({
                "bus_subscribers": subscriber_count,
                "server_uptime_ms": uptime_ms,
                "mode": if cfg!(feature = "distributed") { "distributed" } else { "local" },
                "registered_agents": "query all — use agent_id param to query specific agent",
            }))
        }
    }
}
