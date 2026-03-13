# Government of Alberta (GoA) - [**DRAFT**]: AgentFlow Enterprise Usage Policy

## 1. Purpose
This policy establishes the guidelines for the responsible, secure, ethical, and human-centered use of the **AgentFlow** framework within the Government of Alberta (GoA) enterprise. It operationalizes the provincial *Artificial Intelligence Usage Policy* specifically for developers and business areas leveraging AgentFlow to build AI agents, orchestrators, and automated workflows.

## 2. Scope
This policy applies to all GoA departments, agencies, boards, and commissions utilizing AgentFlow for developing, procuring, adopting, or implementing AI-enabled systems. Contract managers must also incorporate these requirements when managing relationships with third-party vendors utilizing AgentFlow on behalf of the GoA.

## 3. Core Principles (Aligned with GoA AI Principles)

### 3.1 Maintain Security and Privacy
AgentFlow implementations must protect information in the custody or control of the GoA from unauthorized access or disclosure.
* **Tool Security:** Developers MUST use AgentFlow's `ToolRegistry` to strictly allowlist executable commands. The execution of arbitrary, LLM-generated tool names or binaries is strictly prohibited.
* **Data Sovereignty:** When utilizing external LLM providers (e.g., OpenAI) via AgentFlow's `rig` integration, no personal, sensitive, or highly classified GoA data may be transmitted unless approved sovereign compute capabilities or secure enterprise agreements are in place.

### 3.2 Ensure Strong Ethics and Mitigate Bias
* AgentFlow pipelines must be designed to mitigate bias. Where appropriate, workflows should incorporate critic or reviewer nodes (e.g., the `Reflection` or `RpiWorkflow` patterns) to evaluate and correct potentially biased or harmful outputs before they reach the public.

### 3.3 Enhance Trust and Explainability
* The development and implementation of AgentFlow systems must be transparent and explainable.
* Developers should leverage AgentFlow's state management (e.g., `StateDiff`, `TypedFlow`) to maintain a clear, auditable trail of how inputs are transformed into outputs. All AI-generated outputs must be clearly labelled as such.

### 3.4 Maintain Human Control
* AgentFlow is a tool to support GoA operations, not a replacement for human decision-making. 
* **Bounded Execution:** Automated loops (e.g., ReAct, Plan-and-Execute) MUST enforce safety limits using `Flow::with_max_steps` to prevent infinite, unbounded LLM execution cycles.
* **Human-in-the-loop:** Critical workflows must integrate human-approval nodes before executing sensitive actions or finalizing decisions.

### 3.5 Empower Staff
* Staff are encouraged to explore AgentFlow in secure, controlled testing environments (sandboxes) to build AI capacity, utilizing features like YAML-defined `skills` for rapid prototyping without compromising production systems.

## 4. Implementation Specifications

1. **Production Approval:** Integration of any AgentFlow solution into GoA processes or service delivery is subject to formal review and approval by Cybersecurity Services and Technology and Innovation (TI).
2. **Concurrency Safety:** To ensure system stability and prevent deadlocks, async LLM operations reading from shared state must utilize `create_diff_node` or equivalent snapshot-and-diff patterns. Locks must never be held across `.await` points during external API calls.
3. **Robust Error Handling:** Workflows must utilize type-safe error handling (`AgentFlowError`) and features like `create_corrective_retry_node` to ensure systems gracefully handle failures, timeouts, or hallucinations without silent degradation.
4. **Unreviewed Models:** Unless specifically prohibited, GoA staff may test AgentFlow with unreviewed LLMs *only* if publicly accessible content is used as inputs.

## 5. Responsibilities

* **Business Areas / Deputy Heads:** Accountable for all AgentFlow outputs within their department. They must ensure AI outputs are accurate, align with privacy obligations, and are transparently communicated to Albertans.
* **Developers / Engineers:** Responsible for adhering to secure coding conventions within AgentFlow, enforcing `ToolRegistry` allowlists, bounding execution loops, and reporting potential AI security issues.
* **Technology and Innovation (TI):** Responsible for evaluating AgentFlow deployments through a coordinated approach (security, privacy, legal) to ensure they operate as intended and minimize data collection.

## 6. Compliance
Non-compliance with this policy could result in security breaches, loss of public trust, or damage to GoA's reputation. Depending on severity, non-compliance may lead to disciplinary action. Exceptions must be documented via a Statement of Acceptable Risk and approved by the Deputy Minister of TI.

---
*Reference: Government of Alberta Artificial Intelligence Usage Policy (May 1, 2025)*
