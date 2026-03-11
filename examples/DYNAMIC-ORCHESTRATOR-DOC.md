# Dynamic Orchestrator: Main Flow Points

The `dynamic_orchestrator`'s execution is defined by four main points within its `Flow`. Here is a step-by-step breakdown of each one and its purpose:

### 1. The Planner Node (The "Brain")
This is the starting point of the automated logic.

*   **Input**: It reads the high-level `"goal"` from the `SharedStore` (e.g., "write a Rust function that fetches a URL and handles errors").
*   **Action**: It calls an LLM with a carefully constructed prompt that includes the list of all available agents loaded from `agents.toml`. It instructs the LLM to act as a planner and create a step-by-step plan.
*   **Output**:
    1.  It produces a JSON array of agent tasks, like `[{"name": "researcher", "prompt": "..."}, {"name": "coder", "prompt": "..."}]`.
    2.  This plan is written to the `SharedStore` under the key `"agent_plan"`.
    3.  It sets **`"action" = "dispatch"`** in the store, which tells the `Flow` engine to proceed to the next node.

**Purpose**: To translate a high-level human goal into a concrete, machine-executable plan.

---

### 2. The Dispatcher Node (The "Engine")
This node is a loop that executes the plan created by the Planner.

*   **Input**: It reads the `"agent_plan"` from the `SharedStore`.
*   **Action (Loop Cycle)**:
    1.  It **pops the first task** off the `agent_plan` list.
    2.  If the plan is now empty, it changes the flow's direction by setting **`"action" = "aggregate"`** and the loop ends.
    3.  If tasks remain, it uses the task's `"name"` to look up the corresponding agent "factory" in the registry.
    4.  It **executes that specific agent** with the given `"prompt"`. The agent runs and writes its result to its unique output key (e.g., `"code_result"`).
    5.  It saves the remaining tasks back to `"agent_plan"` and sets **`"action" = "dispatch"`**, which makes the `Flow` route back to this same Dispatcher node for the next cycle.
*   **Output**: It progressively executes the plan, collecting each agent's output in an `"agent_results"` list in the `SharedStore`.

**Purpose**: To execute the plan step-by-step, invoking the correct agent for each task and managing the state of the plan.

---

### 3. The Aggregator Node (The "Synthesizer")
This is the final active step, triggered after the Dispatcher finishes the plan.

*   **Input**: It reads the original `"goal"` and the complete list of `"agent_results"`.
*   **Action**: It calls an LLM one last time, providing all the intermediate results and asking it to synthesize them into a single, final, well-structured report.
*   **Output**: It writes this comprehensive final answer to the `SharedStore` under the key `"final_report"`.

**Purpose**: To combine the work of all the specialized agents into a single, coherent, user-facing response.

---

### 4. Flow Termination (The "Stop Signal")
This isn't a node, but a crucial mechanism.

*   **How it works**: After the Aggregator node runs, it **does not set an `"action"` key** in the `SharedStore`.
*   **Result**: The `Flow` engine finishes the node, checks the store for the next action, and finds nothing. With no action to take, the flow concludes successfully.

**Purpose**: To provide a clean and definitive end to the execution loop.
