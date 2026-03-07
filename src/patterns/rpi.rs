use crate::core::flow::Flow;
use crate::core::node::{SharedStore, SimpleNode};

/// An RpiWorkflow orchestrates a standard Research -> Plan -> Implement -> Verify pipeline
/// as prescribed by the rust-agentic-skills methodology.
pub struct RpiWorkflow {
    flow: Flow,
}

impl Default for RpiWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

impl RpiWorkflow {
    pub fn new() -> Self {
        Self { flow: Flow::new() }
    }

    /// Set the Research node that investigates context and documentation.
    pub fn with_research(mut self, node: SimpleNode) -> Self {
        self.flow.add_node("research", node);
        // By default, transition to plan unless an error occurs
        self.flow.add_edge("research", "default", "plan");
        self
    }

    /// Set the Plan node that decides the execution steps.
    pub fn with_plan(mut self, node: SimpleNode) -> Self {
        self.flow.add_node("plan", node);
        // By default, transition to implement
        self.flow.add_edge("plan", "default", "implement");
        self
    }

    /// Set the Implement node that executes the skill/tool.
    pub fn with_implement(mut self, node: SimpleNode) -> Self {
        self.flow.add_node("implement", node);
        // By default, transition to verify
        self.flow.add_edge("implement", "default", "verify");
        self
    }

    /// Set the Verify node that checks the output of the implementation.
    pub fn with_verify(mut self, node: SimpleNode) -> Self {
        self.flow.add_node("verify", node);
        // Allows iteration: if verify says 'retry', go back to plan or implement
        self.flow.add_edge("verify", "replan", "plan");
        self.flow.add_edge("verify", "reimplement", "implement");
        // Verify 'default' ends the flow (no further edge defined)
        self
    }

    /// Explicitly override or set a custom edge transition between RPI phases.
    pub fn add_custom_edge(mut self, from_phase: &str, action: &str, to_phase: &str) -> Self {
        self.flow.add_edge(from_phase, action, to_phase);
        self
    }

    /// Build and execute the RPI workflow.
    pub async fn run(&self, store: SharedStore) -> SharedStore {
        // Because start_node is implicitly the first node added to Flow,
        // we guarantee `research` runs first assuming standard builder order.
        self.flow.run(store).await
    }
}

/// A simpler helper function to create a basic RpiWorkflow graph if all nodes are provided upfront
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
