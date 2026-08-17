use anyhow::Result;
use candle_core::{Device, Module, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{Repo, RepoType, api::sync::ApiBuilder};
use std::sync::{OnceLock, RwLock};
use tokenizers::Tokenizer;

pub(crate) struct CrossEncoder {
    model: BertModel,
    classifier: candle_nn::Linear,
    tokenizer: Tokenizer,
    device: Device,
}

impl CrossEncoder {
    fn load() -> Result<Self> {
        let device = Device::Cpu;
        let repo = Repo::new(
            "cross-encoder/ms-marco-MiniLM-L-6-v2".to_string(),
            RepoType::Model,
        );
        let api = ApiBuilder::new().with_progress(false).build()?;
        let r = api.repo(repo);

        let config_path = r.get("config.json")?;
        let tokenizer_path = r.get("tokenizer.json")?;
        let weights_path = r.get("model.safetensors")?;

        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = serde_json::from_str(&config_str)?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer load error: {}", e))?;

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)?
        };

        let model = BertModel::load(vb.pp("bert"), &config)?;

        let num_labels = 1usize;
        let hidden_size = config.hidden_size;
        let classifier_w = vb
            .pp("classifier")
            .get((num_labels, hidden_size), "weight")?;
        let classifier_b = vb.pp("classifier").get(num_labels, "bias")?;
        let classifier = candle_nn::Linear::new(classifier_w, Some(classifier_b));

        Ok(Self {
            model,
            classifier,
            tokenizer,
            device,
        })
    }

    pub(crate) fn score(&self, query: &str, candidate: &str) -> Result<f32> {
        let text = format!("{} [SEP] {}", query, candidate);
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Encoding error: {}", e))?;

        let token_ids = Tensor::new(encoding.get_ids(), &self.device)?.unsqueeze(0)?;
        let attention_mask =
            Tensor::new(encoding.get_attention_mask(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = token_ids.zeros_like()?;

        let hidden = self
            .model
            .forward(&token_ids, &token_type_ids, Some(&attention_mask))?;
        let cls = hidden.narrow(1, 0, 1)?.squeeze(1)?;
        let logits = self.classifier.forward(&cls)?;
        let score = logits.narrow(1, 0, 1)?.squeeze(1)?.to_scalar::<f32>()?;
        Ok(score)
    }
}

pub fn cached_cross_encoder() -> Result<std::sync::RwLockReadGuard<'static, CrossEncoder>, String> {
    static CE: OnceLock<Result<RwLock<CrossEncoder>, String>> = OnceLock::new();
    match CE.get_or_init(|| {
        CrossEncoder::load()
            .map(RwLock::new)
            .map_err(|error| error.to_string())
    }) {
        Ok(encoder) => encoder.read().map_err(|_| "RwLock poisoned".to_string()),
        Err(error) => Err(error.clone()),
    }
}
