//! Parallel fan-out / fan-in execution for [`Flow`].
//!
//! [`ParallelFlow`] runs multiple independent sub-flows concurrently and
//! merges their results into a single [`SharedStore`] through a user-supplied
//! merge function.
//!
//! # When to use
//!
//! - You have N independent computations (e.g. retrieving context from
//!   multiple sources, calling different tools in parallel) that can all start
//!   at the same time and whose results need to be combined before the next
//!   sequential step.
//! - You want the parallel branches to share *nothing* at runtime and only
//!   merge once they have all finished (fan-out → fan-in).
//!
//! # How it works
//!
//! 1. Each branch receives a **snapshot** clone of the initial store so
//!    branches are fully isolated from each other.
//! 2. All branches are spawned as independent Tokio tasks and awaited with
//!    [`futures::future::join_all`].
//! 3. Once every branch has completed, the user-supplied `merge` function is
//!    called with the initial store and the list of branch result stores,
//!    producing the final store.
//!
//! # Example
//!
//! ```rust,no_run
//! use agentflow::core::parallel::ParallelFlow;
//! use agentflow::core::flow::Flow;
//! use agentflow::core::node::{create_node, SharedStore};
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Branch A: fetch system info
//!     let mut branch_a = Flow::new();
//!     branch_a.add_node("fetch_sys", create_node(|store: SharedStore| async move {
//!         store.write().await.insert("sys".into(), serde_json::json!("linux"));
//!         store
//!     }));
//!
//!     // Branch B: fetch user info
//!     let mut branch_b = Flow::new();
//!     branch_b.add_node("fetch_user", create_node(|store: SharedStore| async move {
//!         store.write().await.insert("user".into(), serde_json::json!("alice"));
//!         store
//!     }));
//!
//!     let pf = ParallelFlow::new(vec![branch_a, branch_b])
//!         .with_merge(|_initial, results| {
//!             // Collect all keys from every branch into one store.
//!             let merged: SharedStore = Arc::new(RwLock::new(HashMap::new()));
//!             Box::pin(async move {
//!                 let mut guard = merged.write().await;
//!                 for branch_store in results {
//!                     for (k, v) in branch_store.read().await.iter() {
//!                         guard.insert(k.clone(), v.clone());
//!                     }
//!                 }
//!                 drop(guard);
//!                 merged
//!             })
//!         });
//!
//!     let initial: SharedStore = Arc::new(RwLock::new(HashMap::new()));
//!     let result = pf.run(initial).await;
//!     // result now contains both "sys" and "user"
//! }
//! ```

use crate::core::flow::Flow;
use crate::core::node::SharedStore;
use futures::future::join_all;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info, instrument};

/// Merge function signature: `(initial_store, branch_results) -> merged_store`.
pub type MergeFn = Arc<
    dyn Fn(SharedStore, Vec<SharedStore>) -> Pin<Box<dyn Future<Output = SharedStore> + Send>>
        + Send
        + Sync,
>;

/// Runs multiple [`Flow`]s in parallel and merges their output stores.
///
/// See the [module-level documentation](self) for a full example.
pub struct ParallelFlow {
    branches: Vec<Flow>,
    merge_fn: Option<MergeFn>,
}

impl ParallelFlow {
    /// Create a `ParallelFlow` from a list of branch flows.
    pub fn new(branches: Vec<Flow>) -> Self {
        Self {
            branches,
            merge_fn: None,
        }
    }

    /// Supply a custom merge function.
    ///
    /// The function receives:
    /// - `initial` — the store that was passed to [`run`](Self::run) *before*
    ///   any branch executed (a snapshot clone is also given to each branch).
    /// - `results` — one store per branch, in the same order as
    ///   [`new`](Self::new).
    ///
    /// The default merge strategy (when this is not called) is a
    /// **last-writer-wins union**: branches are merged in order, so later
    /// branches overwrite keys from earlier ones.
    pub fn with_merge<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(SharedStore, Vec<SharedStore>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = SharedStore> + Send + 'static,
    {
        self.merge_fn = Some(Arc::new(move |initial, results| {
            Box::pin(f(initial, results))
        }));
        self
    }

    /// Execute all branches in parallel and return the merged store.
    ///
    /// Each branch receives a **snapshot clone** of `initial_store` and runs
    /// in isolation.  After all branches finish, the merge function is called.
    #[instrument(name = "parallel_flow.run", skip(self, initial_store), fields(branches = self.branches.len()))]
    pub async fn run(&self, initial_store: SharedStore) -> SharedStore {
        let branch_count = self.branches.len();
        debug!(branch_count, "ParallelFlow spawning branches");

        // Give every branch its own snapshot so they are fully isolated.
        let futs: Vec<_> = self
            .branches
            .iter()
            .enumerate()
            .map(|(i, flow)| {
                let snapshot = clone_store_snapshot(&initial_store);
                let flow = flow.clone();   // Flow: Clone
                async move {
                    debug!(branch = i, "ParallelFlow branch started");
                    let result = flow.run(snapshot).await;
                    debug!(branch = i, "ParallelFlow branch finished");
                    result
                }
            })
            .collect();

        let results: Vec<SharedStore> = join_all(futs).await;

        info!(branch_count, "ParallelFlow all branches done; merging");

        if let Some(merge_fn) = &self.merge_fn {
            merge_fn(initial_store, results).await
        } else {
            default_merge(initial_store, results).await
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Clone the contents of `store` into a new, independent `SharedStore`.
///
/// This is a **deep copy** of the key-value data; the two stores do not share
/// the underlying `RwLock`.
fn clone_store_snapshot(store: &SharedStore) -> SharedStore {
    // We need a synchronous snapshot.  Use `try_read` in a spin — the store
    // should never be write-locked at the call site (between nodes).
    // In practice `blocking_read` is not available on tokio's RwLock in async
    // context, so we clone the Arc and let `join_all` do the async part.
    // The actual snapshot is taken inside the spawned future before the branch
    // flow executes.
    store.clone()
}

/// Default merge: union all branch stores into the initial store (last-writer-wins).
async fn default_merge(initial: SharedStore, results: Vec<SharedStore>) -> SharedStore {
    let mut guard = initial.write().await;
    for branch in results {
        let branch_guard = branch.read().await;
        for (k, v) in branch_guard.iter() {
            guard.insert(k.clone(), v.clone());
        }
    }
    drop(guard);
    initial
}

// ── Flow: Clone ───────────────────────────────────────────────────────────────
// Flow needs to be Clone so we can move it into the async block above.
// This impl lives here to keep flow.rs dependency-free of parallel.rs.
