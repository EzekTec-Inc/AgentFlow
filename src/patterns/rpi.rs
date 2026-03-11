use crate::core::flow::Flow;
use crate::core::node::{SharedStore, SimpleNode};
use std::time::Instant;
use tracing::{debug, info, instrument};

/// Orchestrates a **Research → Plan → Implement → Verify** agentic loop.
///
/// `RpiWorkflow` is a specialised [`Flow`] that enforces the four-phase
/// problem-solving pattern from the
/// [rust-agentic-skills](https://github.com/rust-agentic-skills) methodology:
///
/// | Phase | Purpose |
/// |---|---|
/// | **Research** | Investigate context, gather documentation, understand requirements |
/// | **Plan** | Decide the approach and list concrete execution steps |
/// | **Implement** | Execute the plan (call tools, write code, invoke APIs) |
/// | **Verify** | Validate the output; optionally loop back to Plan or Implement |
///
/// # Routing
///
/// Default edges are wired automatically:
///
/// ```text
/// research ──default──► plan ──default──► implement ──default──► verify
///                          ▲                   ▲
///                          │                   │
///                       replan             reimplement
///                          └─────────────────────── verify
/// ```
///
/// - `verify` + action `"replan"` → back to `plan`
/// - `verify` + action `"reimplement"` → back to `implement`
/// - `verify` + action `"default"` (or none) → flow ends
///
/// # Example
///
/// ```rust,no_run
/// use agentflow::prelude::*;
/// use agentflow::patterns::rpi::RpiWorkflow;
/// use std::collections::HashMap;
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
///
/// #[tokio::main]
/// async fn main() {
///     let rpi = RpiWorkflow::new()
///         .with_research(create_node(|store: SharedStore| async move { store }))
///         .with_plan(create_node(|store: SharedStore| async move { store }))
///         .with_implement(create_node(|store: SharedStore| async move { store }))
///         .with_verify(create_node(|store: SharedStore| async move { store }));
///
///     let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
///     let result = rpi.run(store).await;
/// }
/// ```
///
/// [`Flow`]: crate::core::flow::Flow
pub struct RpiWorkflow {
    flow: Flow,
}

impl Default for RpiWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

impl RpiWorkflow {
    /// Create an empty `RpiWorkflow`. Register nodes with the `with_*` builder
    /// methods before calling [`run`](Self::run).
    pub fn new() -> Self {
        Self { flow: Flow::new() }
    }

    /// Register the **Research** node.
    ///
    /// The research node should explore context, read documentation, and write
    /// findings into the store (e.g. `store["research"]`).
    ///
    /// Default outgoing edge: `"default"` → `"plan"`.
    pub fn with_research(mut self, node: SimpleNode) -> Self {
        self.flow.add_node("research", node);
        self.flow.add_edge("research", "default", "plan");
        self
    }

    /// Register the **Plan** node.
    ///
    /// The plan node should read research findings and write a structured
    /// action plan (e.g. `store["plan"]`).
    ///
    /// Default outgoing edge: `"default"` → `"implement"`.
    pub fn with_plan(mut self, node: SimpleNode) -> Self {
        self.flow.add_node("plan", node);
        self.flow.add_edge("plan", "default", "implement");
        self
    }

    /// Register the **Implement** node.
    ///
    /// The implement node should execute the plan — call tools, write code,
    /// invoke APIs — and write the result (e.g. `store["output"]`).
    ///
    /// Default outgoing edge: `"default"` → `"verify"`.
    pub fn with_implement(mut self, node: SimpleNode) -> Self {
        self.flow.add_node("implement", node);
        self.flow.add_edge("implement", "default", "verify");
        self
    }

    /// Register the **Verify** node.
    ///
    /// The verify node should inspect the implementation output and either:
    ///
    /// - Set `store["action"] = "default"` (or omit `"action"`) to accept the
    ///   result and end the flow.
    /// - Set `store["action"] = "replan"` to loop back to the Plan phase.
    /// - Set `store["action"] = "reimplement"` to loop back to the Implement
    ///   phase.
    ///
    /// Outgoing edges:
    /// - `"replan"` → `"plan"`
    /// - `"reimplement"` → `"implement"`
    pub fn with_verify(mut self, node: SimpleNode) -> Self {
        self.flow.add_node("verify", node);
        self.flow.add_edge("verify", "replan", "plan");
        self.flow.add_edge("verify", "reimplement", "implement");
        self
    }

    /// Override or add a custom edge between any two RPI phases.
    ///
    /// Useful for adding non-standard loops or error-recovery paths beyond the
    /// built-in `replan` / `reimplement` edges.
    pub fn add_custom_edge(mut self, from_phase: &str, action: &str, to_phase: &str) -> Self {
        self.flow.add_edge(from_phase, action, to_phase);
        self
    }

    /// Execute the RPI workflow starting from the `"research"` phase.
    ///
    /// The start node is always the first node registered — as long as you use
    /// the standard builder order (`with_research` first), `"research"` will
    /// always run first.
    #[instrument(name = "rpi.run", skip(self, store))]
    pub async fn run(&self, store: SharedStore) -> SharedStore {
        let t = Instant::now();
        debug!("RpiWorkflow: starting research phase");
        let result = self.flow.run(store).await;
        info!(elapsed_ms = t.elapsed().as_millis(), "RpiWorkflow: complete");
        result
    }
}

/// Convenience function to build a complete `RpiWorkflow` in one call.
///
/// Equivalent to:
/// ```rust,ignore
/// RpiWorkflow::new()
///     .with_research(research)
///     .with_plan(plan)
///     .with_implement(implement)
///     .with_verify(verify)
/// ```
pub fn create_rpi_workflow(
    research: SimpleNode,
    plan: SimpleNode,
    implement: SimpleNode,
    verify: SimpleNode,
) -> RpiWorkflow {
    RpiWorkflow::new()
        .with_research(research)
        .with_plan(plan)
        .with_implement(implement)
        .with_verify(verify)
}
