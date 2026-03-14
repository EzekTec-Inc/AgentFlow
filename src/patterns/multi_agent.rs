use crate::core::node::{Node, SharedStore};
use futures::future::join_all;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info, instrument};

/// Controls how results from parallel agents are merged into the final store.
///
/// Pass one of these variants to [`MultiAgent::with_strategy`].
#[derive(Clone)]
pub enum MergeStrategy {
    /// All agents share the **same** `Arc<RwLock<…>>`. Each agent's writes
    /// are immediately visible to other agents. Use distinct output keys to
    /// avoid overwrites.
    ///
    /// This is the default strategy.
    SharedStore,
    /// Each agent receives a snapshot of the store and runs against its own
    /// private copy. Results are merged back with a `"agent_N."` prefix, e.g.
    /// `"agent_0.result"`, `"agent_1.result"`.
    ///
    /// Use this when agents would otherwise overwrite each other's keys.
    Namespaced,
    /// Each agent runs against its own snapshot. The user-supplied closure
    /// receives all per-agent result stores and returns the merged store.
    ///
    /// Wrap your closure in [`Arc::new`] when constructing this variant:
    ///
    /// ```rust,ignore
    /// MergeStrategy::Custom(Arc::new(|stores| { /* ... */ }))
    /// ```
    Custom(Arc<dyn Fn(Vec<SharedStore>) -> SharedStore + Send + Sync>),
}

/// Runs multiple agents concurrently and merges their results.
///
/// `MultiAgent` is the right choice when you need several specialised agents
/// to work in parallel on the same task (e.g. researcher + coder + reviewer).
///
/// # Merge strategies
///
/// See [`MergeStrategy`] for the full explanation. Quick reference:
///
/// | Strategy | Isolation | Output keys |
/// |---|---|---|
/// | `SharedStore` | none — shared `Arc` | as written by each agent |
/// | `Namespaced` | snapshot per agent | `"agent_0.key"`, `"agent_1.key"`, … |
/// | `Custom(Arc<dyn Fn>)` | snapshot per agent | determined by your closure |
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
/// use std::collections::HashMap;
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
///
/// #[tokio::main]
/// async fn main() {
///     let researcher = create_node(|store: SharedStore| async move {
///         store.write().await.insert("research".into(), serde_json::json!("findings"));
///         store
///     });
///     let coder = create_node(|store: SharedStore| async move {
///         store.write().await.insert("code".into(), serde_json::json!("fn main() {}"));
///         store
///     });
///
///     let mut multi = MultiAgent::new();
///     multi.add_agent(researcher);
///     multi.add_agent(coder);
///
///     let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
///     let result = multi.run(store).await;
/// }
/// ```
#[derive(Clone)]
pub struct MultiAgent {
    /// The registered agents, executed concurrently.
    pub agents: Vec<Box<dyn Node<SharedStore, SharedStore>>>,
    /// The active merge strategy.
    pub strategy: MergeStrategy,
}

impl MultiAgent {
    /// Create a `MultiAgent` with the default [`MergeStrategy::SharedStore`].
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            strategy: MergeStrategy::SharedStore,
        }
    }

    /// Create a `MultiAgent` with an explicit merge strategy.
    pub fn with_strategy(strategy: MergeStrategy) -> Self {
        Self {
            agents: Vec::new(),
            strategy,
        }
    }

    /// Register an agent node. Agents are executed in the order they are added.
    pub fn add_agent(&mut self, agent: Box<dyn Node<SharedStore, SharedStore>>) {
        self.agents.push(agent);
    }

    /// Run all agents concurrently and merge the results using the active strategy.
    #[instrument(name = "multi_agent.run", skip(self, store), fields(agent_count = self.agents.len()))]
    pub async fn run(&self, store: SharedStore) -> SharedStore {
        info!(agent_count = self.agents.len(), "MultiAgent::run starting");
        match &self.strategy {
            MergeStrategy::SharedStore => self.run_shared(store).await,
            MergeStrategy::Namespaced => self.run_namespaced(store).await,
            MergeStrategy::Custom(merge_fn) => self.run_custom(store, merge_fn.clone()).await,
        }
    }

    /// SharedStore strategy — all agents share one `Arc`.
    #[instrument(name = "multi_agent.run_shared", skip(self, store), fields(agent_count = self.agents.len()))]
    async fn run_shared(&self, store: SharedStore) -> SharedStore {
        debug!(
            agent_count = self.agents.len(),
            "MultiAgent::run_shared spawning agents"
        );
        let futures = self.agents.iter().map(|agent| agent.call(store.clone()));
        join_all(futures).await;
        info!("MultiAgent::run_shared complete");
        store
    }

    /// Namespaced strategy — snapshot per agent, merge with prefix.
    #[instrument(name = "multi_agent.run_namespaced", skip(self, store), fields(agent_count = self.agents.len()))]
    async fn run_namespaced(&self, store: SharedStore) -> SharedStore {
        debug!(
            agent_count = self.agents.len(),
            "MultiAgent::run_namespaced starting"
        );

        // Snapshot the store once, then fan out to all agents concurrently
        let snapshot = store.read().await.clone();
        let futures = self.agents.iter().enumerate().map(|(idx, agent)| {
            let agent_store = std::sync::Arc::new(tokio::sync::RwLock::new(snapshot.clone()));
            async move { (idx, agent.call(agent_store).await) }
        });
        let agent_stores = join_all(futures).await;

        // Merge results back with "agent_N." prefix
        for (idx, agent_store) in agent_stores {
            let agent_data = agent_store.read().await;
            let mut merged_store = store.write().await;
            for (key, value) in agent_data.iter() {
                merged_store.insert(format!("agent_{}.{}", idx, key), value.clone());
            }
        }
        info!("MultiAgent::run_namespaced complete");
        store
    }

    /// Custom strategy — snapshot per agent, user-supplied merge closure.
    #[instrument(name = "multi_agent.run_custom", skip(self, store, merge_fn), fields(agent_count = self.agents.len()))]
    async fn run_custom(
        &self,
        store: SharedStore,
        merge_fn: Arc<dyn Fn(Vec<SharedStore>) -> SharedStore + Send + Sync>,
    ) -> SharedStore {
        debug!(
            agent_count = self.agents.len(),
            "MultiAgent::run_custom starting"
        );

        // Snapshot the store once, then fan out to all agents concurrently
        let snapshot = store.read().await.clone();
        let futures = self.agents.iter().map(|agent| {
            let agent_store = std::sync::Arc::new(tokio::sync::RwLock::new(snapshot.clone()));
            agent.call(agent_store)
        });
        let results = join_all(futures).await;

        info!("MultiAgent::run_custom complete, calling merge_fn");
        merge_fn(results)
    }
}

impl Node<SharedStore, SharedStore> for MultiAgent {
    fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
        Box::pin(self.run(input))
    }
}

impl Default for MultiAgent {
    fn default() -> Self {
        Self::new()
    }
}
