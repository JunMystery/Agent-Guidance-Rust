pub mod cross_encoder;
pub mod embeddings;
pub mod llm_selector;
pub mod onnx_engine;

use rayon::ThreadPool;
use std::sync::{Condvar, Mutex, OnceLock};

const ML_WORKER_THREADS: usize = 2;

pub struct SyncMlQueue {
    permits: Mutex<usize>,
    cvar: Condvar,
}

impl SyncMlQueue {
    pub fn new(max_permits: usize) -> Self {
        Self {
            permits: Mutex::new(max_permits),
            cvar: Condvar::new(),
        }
    }

    pub fn acquire(&self) -> SyncMlPermit<'_> {
        let mut count = self.permits.lock().unwrap_or_else(|p| p.into_inner());
        while *count == 0 {
            count = self.cvar.wait(count).unwrap_or_else(|p| p.into_inner());
        }
        *count -= 1;
        SyncMlPermit { queue: self }
    }
}

pub struct SyncMlPermit<'a> {
    queue: &'a SyncMlQueue,
}

impl<'a> Drop for SyncMlPermit<'a> {
    fn drop(&mut self) {
        let mut count = self.queue.permits.lock().unwrap_or_else(|p| p.into_inner());
        *count += 1;
        self.queue.cvar.notify_one();
    }
}

pub fn ml_queue() -> &'static SyncMlQueue {
    static QUEUE: OnceLock<SyncMlQueue> = OnceLock::new();
    QUEUE.get_or_init(|| SyncMlQueue::new(2))
}

pub fn inference_pool() -> &'static ThreadPool {
    static POOL: OnceLock<ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(ML_WORKER_THREADS)
            .thread_name(|index| format!("agent-guidance-ml-{index}"))
            .build()
            .expect("failed to create ML worker pool")
    })
}

pub fn download_models() -> anyhow::Result<()> {
    use hf_hub::{Repo, RepoType, api::sync::ApiBuilder};

    println!("  Downloading embedding model (118MB)...");
    let emb = ApiBuilder::new()
        .with_progress(true)
        .build()?
        .repo(Repo::new(
            "intfloat/multilingual-e5-small".into(),
            RepoType::Model,
        ));
    let _ = emb.get("config.json");
    let _ = emb.get("tokenizer.json");
    let _ = emb.get("model.safetensors");

    println!("  Downloading cross-encoder model (80MB)...");
    let ce = ApiBuilder::new()
        .with_progress(true)
        .build()?
        .repo(Repo::new(
            "cross-encoder/ms-marco-MiniLM-L-6-v2".into(),
            RepoType::Model,
        ));
    let _ = ce.get("config.json");
    let _ = ce.get("tokenizer.json");
    let _ = ce.get("model.safetensors");

    println!("  [OK] ML models cached at ~/.cache/huggingface/hub/");
    Ok(())
}
