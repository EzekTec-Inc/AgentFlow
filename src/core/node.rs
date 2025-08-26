use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dyn_clone::DynClone;

/// Shared store for communication between nodes
pub type SharedStore = Arc<Mutex<HashMap<String, Value>>>;

/// Core Node trait - handles simple (LLM) tasks
pub trait Node<I, O>: Send + Sync + DynClone {
    fn call(&self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + '_>>;
}
dyn_clone::clone_trait_object!(<I, O> Node<I, O>);

/// Simple node that works with SharedStore
pub type SimpleNode = Box<dyn Node<SharedStore, SharedStore>>;

/// Helper function to create a simple node
pub fn create_node<F, Fut>(func: F) -> SimpleNode
where
    F: Fn(SharedStore) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = SharedStore> + Send + 'static,
{
    #[derive(Clone)]
    struct FuncNode<F>(F);

    impl<F, Fut> Node<SharedStore, SharedStore> for FuncNode<F>
    where
        F: Fn(SharedStore) -> Fut + Send + Sync + Clone,
        Fut: Future<Output = SharedStore> + Send + 'static,
    {
        fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
            Box::pin(self.0(input))
        }
    }

    Box::new(FuncNode(func))
}

/// Helper to create a node with built-in retry/fallback logic and prep/exec/post pipeline.
/// This is the closest Rust equivalent to the Python Node with retries and pipeline.
pub fn create_retry_node<PrepF, PrepFut, ExecF, ExecFut, PostF, PostFut>(
    prep: PrepF,
    exec: ExecF,
    post: PostF,
    max_retries: usize,
    wait_millis: u64,
    fallback: Option<fn(&SharedStore, &Value, &anyhow::Error) -> SharedStore>,
) -> SimpleNode
where
    PrepF: Fn(SharedStore) -> PrepFut + Send + Sync + Clone + 'static,
    PrepFut: Future<Output = Value> + Send + 'static,
    ExecF: Fn(&SharedStore, &Value) -> ExecFut + Send + Sync + Clone + 'static,
    ExecFut: Future<Output = Result<Value, anyhow::Error>> + Send + 'static,
    PostF: Fn(SharedStore, &Value, &Value) -> PostFut + Send + Sync + Clone + 'static,
    PostFut: Future<Output = SharedStore> + Send + 'static,
{
    #[derive(Clone)]
    struct RetryNode<PrepF, ExecF, PostF> {
        prep: PrepF,
        exec: ExecF,
        post: PostF,
        max_retries: usize,
        wait_millis: u64,
        fallback: Option<fn(&SharedStore, &Value, &anyhow::Error) -> SharedStore>,
    }

    impl<PrepF, PrepFut, ExecF, ExecFut, PostF, PostFut> Node<SharedStore, SharedStore>
        for RetryNode<PrepF, ExecF, PostF>
    where
        PrepF: Fn(SharedStore) -> PrepFut + Send + Sync + Clone + 'static,
        PrepFut: Future<Output = Value> + Send + 'static,
        ExecF: Fn(&SharedStore, &Value) -> ExecFut + Send + Sync + Clone + 'static,
        ExecFut: Future<Output = Result<Value, anyhow::Error>> + Send + 'static,
        PostF: Fn(SharedStore, &Value, &Value) -> PostFut + Send + Sync + Clone + 'static,
        PostFut: Future<Output = SharedStore> + Send + 'static,
    {
        fn call(&self, input: SharedStore) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
            let prep = self.prep.clone();
            let exec = self.exec.clone();
            let post = self.post.clone();
            let max_retries = self.max_retries;
            let wait_millis = self.wait_millis;
            let fallback = self.fallback;
            Box::pin(async move {
                let prep_res = prep(input.clone()).await;
                let mut last_err: Option<anyhow::Error> = None;
                let mut exec_res: Option<Value> = None;
                for attempt in 0..max_retries {
                    match exec(&input, &prep_res).await {
                        Ok(val) => {
                            exec_res = Some(val);
                            break;
                        }
                        Err(e) => {
                            last_err = Some(e);
                            if attempt + 1 < max_retries && wait_millis > 0 {
                                tokio::time::sleep(Duration::from_millis(wait_millis)).await;
                            }
                        }
                    }
                }
                let exec_val = if let Some(val) = exec_res {
                    val
                } else if let Some(fallback_fn) = fallback {
                    // fallback returns a SharedStore, but we want a Value for post
                    // We'll just insert an error string for now
                    let _fallback_store = fallback_fn(&input, &prep_res, &last_err.unwrap());
                    let fallback_val = serde_json::json!({"error": "fallback triggered"});
                    // Optionally, merge fallback_store into input here
                    fallback_val
                } else {
                    serde_json::json!({"error": format!("Node failed after {} retries: {:?}", max_retries, last_err)})
                };
                post(input, &prep_res, &exec_val).await
            })
        }
    }

    Box::new(RetryNode {
        prep,
        exec,
        post,
        max_retries,
        wait_millis,
        fallback,
    })
}

/// Helper function to create a batch node that processes Vec<SharedStore>
pub fn create_batch_node<F, Fut>(func: F) -> Box<dyn Node<Vec<SharedStore>, SharedStore>>
where
    F: Fn(Vec<SharedStore>) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = SharedStore> + Send + 'static,
{
    #[derive(Clone)]
    struct BatchFuncNode<F>(F);

    impl<F, Fut> Node<Vec<SharedStore>, SharedStore> for BatchFuncNode<F>
    where
        F: Fn(Vec<SharedStore>) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = SharedStore> + Send + 'static,
    {
        fn call(&self, input: Vec<SharedStore>) -> Pin<Box<dyn Future<Output = SharedStore> + Send + '_>> {
            Box::pin(self.0(input))
        }
    }

    Box::new(BatchFuncNode(func))
}

/// Blanket implementation for boxed nodes
impl<I, O> Node<I, O> for Box<dyn Node<I, O>>
where
    I: Send + 'static,
    O: Send + 'static,
{
    fn call(&self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + '_>> {
        (**self).call(input)
    }
}

