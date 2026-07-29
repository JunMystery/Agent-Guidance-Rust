pub mod embeddings;
pub mod llm_selector;

pub fn download_models() -> anyhow::Result<()> {
    use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};

    println!("  Downloading embedding model (118MB)...");
    let emb = ApiBuilder::new()
        .with_progress(true)
        .build()?
        .repo(Repo::new(
            "intfloat/multilingual-e5-small".into(),
            RepoType::Model,
        ));
    emb.get("model.safetensors")?;

    println!("  Downloading cross-encoder model (80MB)...");
    let ce = ApiBuilder::new()
        .with_progress(true)
        .build()?
        .repo(Repo::new(
            "cross-encoder/ms-marco-MiniLM-L-6-v2".into(),
            RepoType::Model,
        ));
    ce.get("model.safetensors")?;

    println!("  ✓ ML models cached at ~/.cache/huggingface/hub/");
    Ok(())
}
