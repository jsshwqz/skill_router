//! 分布式 CapabilityRegistry 后端
//!
//! 提供基于 NATS JetStream KV 的远程能力注册表，以及
//! 混合模式（本地缓存 + NATS 远程）的注册表实现。
//!
//! # Feature Gate
//! 仅在 `distributed` feature 启用时编译。

#[cfg(feature = "distributed")]
mod nats_registry {
    use anyhow::Result;
    use tracing::{debug, info, warn};

    use aion_types::capability_registry::{CapabilityDefinition, CapabilityRegistry, RegistryBackend};

    /// NATS JetStream KV 后端的能力注册表
    ///
    /// 使用 NATS JetStream KeyValue 存储能力定义，
    /// 支持跨节点的能力发现与注册。
    ///
    /// KV bucket: `aion.capabilities`
    /// Key 格式: `{capability_name}`
    /// Value 格式: JSON 序列化的 `CapabilityDefinition`
    pub struct NatsRegistryBackend {
        /// 本地缓存（从 NATS 同步到内存）
        cache: std::collections::BTreeMap<String, CapabilityDefinition>,
        /// NATS 客户端（用于读写 KV）
        client: async_nats::Client,
        /// JetStream KV bucket 名称
        bucket_name: String,
    }

    impl NatsRegistryBackend {
        /// 连接 NATS 并初始化 KV bucket
        pub async fn connect(nats_url: &str, bucket_name: &str) -> Result<Self> {
            info!("NatsRegistry: connecting to NATS at {}", nats_url);
            let client = async_nats::connect(nats_url).await?;
            info!("NatsRegistry: connected, bucket='{}'", bucket_name);

            Ok(Self {
                cache: std::collections::BTreeMap::new(),
                client,
                bucket_name: bucket_name.to_string(),
            })
        }

        /// 从 NATS KV 同步所有能力定义到本地缓存
        ///
        /// 应在启动时调用一次，之后通过 Watch 增量更新
        pub async fn sync_from_remote(&mut self) -> Result<usize> {
            let js = async_nats::jetstream::new(self.client.clone());

            // 尝试获取或创建 KV bucket
            let kv = match js.get_key_value(&self.bucket_name).await {
                Ok(kv) => kv,
                Err(_) => {
                    info!("NatsRegistry: creating KV bucket '{}'", self.bucket_name);
                    js.create_key_value(async_nats::jetstream::kv::Config {
                        bucket: self.bucket_name.clone(),
                        description: "Aion capability definitions".to_string(),
                        ..Default::default()
                    })
                    .await?
                }
            };

            // 列出所有 key
            let mut keys = kv.keys().await?;
            let mut count = 0;
            use futures_util::StreamExt;
            while let Some(key) = keys.next().await {
                if let Ok(key) = key {
                    match kv.get(&key).await {
                        Ok(Some(bytes)) => {
                            if let Ok(def) = serde_json::from_slice::<CapabilityDefinition>(&bytes) {
                                debug!("NatsRegistry: synced capability '{}'", def.name);
                                self.cache.insert(def.name.clone(), def);
                                count += 1;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => warn!("NatsRegistry: failed to get key '{}': {}", key, e),
                    }
                }
            }

            info!("NatsRegistry: synced {} capabilities from NATS", count);
            Ok(count)
        }

        /// 将能力定义发布到 NATS KV
        pub async fn publish(&self, def: &CapabilityDefinition) -> Result<()> {
            let js = async_nats::jetstream::new(self.client.clone());
            let kv = js.get_key_value(&self.bucket_name).await?;
            let payload = serde_json::to_vec(def)?;
            kv.put(&def.name, payload.into()).await?;
            debug!("NatsRegistry: published capability '{}' to NATS", def.name);
            Ok(())
        }

        /// 获取 NATS 客户端引用
        pub fn client(&self) -> &async_nats::Client {
            &self.client
        }
    }

    impl RegistryBackend for NatsRegistryBackend {
        fn get(&self, name: &str) -> Option<CapabilityDefinition> {
            self.cache.get(name).cloned()
        }

        fn put(&mut self, def: CapabilityDefinition) -> Result<()> {
            self.cache.insert(def.name.clone(), def);
            // 注意：同步版本只更新本地缓存
            // 调用方应使用 publish() 异步推送到 NATS
            Ok(())
        }

        fn list(&self) -> Vec<CapabilityDefinition> {
            self.cache.values().cloned().collect()
        }

        fn contains(&self, name: &str) -> bool {
            self.cache.contains_key(name)
        }

        fn len(&self) -> usize {
            self.cache.len()
        }
    }

    /// 混合模式注册表：本地 builtin + NATS 远程
    ///
    /// 优先查本地（builtin + 本地发现），miss 时查 NATS 缓存。
    /// 注册新能力时同时写入本地和 NATS。
    pub struct HybridRegistryBackend {
        /// 本地注册表（builtin + 本地发现的能力）
        local: CapabilityRegistry,
        /// NATS 远程注册表（其他节点注册的能力）
        remote: NatsRegistryBackend,
    }

    impl HybridRegistryBackend {
        /// 创建混合注册表
        pub fn new(local: CapabilityRegistry, remote: NatsRegistryBackend) -> Self {
            Self { local, remote }
        }

        /// 获取本地注册表的可变引用
        pub fn local_mut(&mut self) -> &mut CapabilityRegistry {
            &mut self.local
        }

        /// 获取远程注册表的可变引用
        pub fn remote_mut(&mut self) -> &mut NatsRegistryBackend {
            &mut self.remote
        }

        /// 异步发布能力到 NATS
        pub async fn publish_to_remote(&self, def: &CapabilityDefinition) -> Result<()> {
            self.remote.publish(def).await
        }

        /// 从 NATS 同步远程能力
        pub async fn sync_remote(&mut self) -> Result<usize> {
            self.remote.sync_from_remote().await
        }
    }

    impl RegistryBackend for HybridRegistryBackend {
        fn get(&self, name: &str) -> Option<CapabilityDefinition> {
            // 优先查本地
            if let Some(def) = RegistryBackend::get(&self.local, name) {
                return Some(def);
            }
            // miss 时查远程缓存
            self.remote.get(name)
        }

        fn put(&mut self, def: CapabilityDefinition) -> Result<()> {
            // 同时写入本地和远程缓存
            let name = def.name.clone();
            self.local.register(def.clone());
            self.remote.put(def)?;
            debug!("HybridRegistry: registered '{}' locally + remote cache", name);
            Ok(())
        }

        fn list(&self) -> Vec<CapabilityDefinition> {
            let mut all = std::collections::BTreeMap::new();
            // 先加远程
            for def in self.remote.list() {
                all.insert(def.name.clone(), def);
            }
            // 本地覆盖远程（本地优先）
            for def in RegistryBackend::list(&self.local) {
                all.insert(def.name.clone(), def);
            }
            all.into_values().collect()
        }

        fn contains(&self, name: &str) -> bool {
            RegistryBackend::contains(&self.local, name) || self.remote.contains(name)
        }

        fn len(&self) -> usize {
            self.list().len()
        }
    }
}

#[cfg(feature = "distributed")]
pub use nats_registry::{HybridRegistryBackend, NatsRegistryBackend};
