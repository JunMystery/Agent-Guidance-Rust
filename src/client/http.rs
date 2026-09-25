use anyhow::{Context, Result, anyhow};
use std::time::{Duration, Instant};
use tracing::debug;
use ureq::{Agent, AgentBuilder};

use super::cache::{get_cached_remote_stats, get_cached_slice, save_cached_remote_stats, save_cached_slice};
use super::types::{
    ServerHealthResponse, SkillSearchRequest, SkillSearchResponse,
    SkillSliceRequest, SkillSliceResponse, SkillStatsResponse,
};
use crate::config::ServerConfig;
use crate::optimizer::compressor::estimate_tokens;

#[derive(Clone)]
pub struct RemoteMlClient {
    pub config: ServerConfig,
    agent: Agent,
}

impl RemoteMlClient {
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    pub fn new(config: &ServerConfig) -> Self {
        let timeout = Duration::from_millis(config.timeout_ms);
        let agent = AgentBuilder::new().timeout(timeout).build();

        Self {
            config: config.clone(),
            agent,
        }
    }

    fn prepare_request(&self, req: ureq::Request) -> ureq::Request {
        if !self.config.api_key.is_empty() {
            req.set("Authorization", &format!("Bearer {}", self.config.api_key))
        } else {
            req
        }
    }

    pub fn check_health(&self) -> Result<(ServerHealthResponse, Duration)> {
        let url = self.config.endpoint_url("/health");
        let start = Instant::now();
        let req = self.prepare_request(self.agent.get(&url));
        let resp = req
            .call()
            .with_context(|| format!("Health check request failed to {}", url))?;
        let latency = start.elapsed();

        let health: ServerHealthResponse = resp
            .into_json()
            .with_context(|| "Failed to parse /health response JSON")?;

        debug!("Server health verified in {:.2?}", latency);
        Ok((health, latency))
    }

    pub fn get_skill_stats(&self) -> Result<SkillStatsResponse> {
        let url = self.config.endpoint_url("/api/skills/stats");
        let cached = get_cached_remote_stats();
        let mut req = self.prepare_request(self.agent.get(&url));
        if let Some(ref c) = cached {
            if !c.catalog_hash.is_empty() {
                req = req.set("If-None-Match", &format!("\"{}\"", c.catalog_hash));
            }
        }
        let resp = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(304, _)) => {
                if let Some(c) = cached {
                    return Ok(c);
                }
                return Err(anyhow!("Received 304 Not Modified but local cache is empty"));
            }
            Err(e) => {
                if let Some(c) = cached {
                    return Ok(c);
                }
                return Err(anyhow!("Failed to fetch stats from {}: {}", url, e));
            }
        };

        if resp.status() == 304 {
            if let Some(c) = cached {
                return Ok(c);
            }
        }

        let stats: SkillStatsResponse = resp
            .into_json()
            .with_context(|| "Failed to parse /api/skills/stats response JSON")?;

        let _ = save_cached_remote_stats(&stats);
        Ok(stats)
    }

    pub fn search_skills(&self, query: &str, max_results: usize) -> Result<SkillSearchResponse> {
        let url = self.config.endpoint_url("/api/skills/search");
        let payload = SkillSearchRequest {
            query: query.to_string(),
            max_results: Some(max_results),
            threshold: Some(0.20),
        };

        let req = self.prepare_request(self.agent.post(&url));
        let resp = req
            .send_json(payload)
            .with_context(|| format!("Skill search request failed to {}", url))?;

        let etag = resp
            .header("ETag")
            .map(|s| s.trim_matches('"').to_string())
            .unwrap_or_default();

        let mut search_res: SkillSearchResponse = resp
            .into_json()
            .with_context(|| "Failed to parse /api/skills/search response JSON")?;

        if search_res.catalog_hash.is_empty() && !etag.is_empty() {
            search_res.catalog_hash = etag;
        }

        Ok(search_res)
    }

    pub fn slice_skill(&self, skill_name: &str, prompt: &str) -> Result<SkillSliceResponse> {
        let cached_stats = get_cached_remote_stats();
        let cat_hash = cached_stats
            .as_ref()
            .map(|s| s.catalog_hash.as_str())
            .unwrap_or("");

        if !cat_hash.is_empty() {
            if let Some(cached_content) = get_cached_slice(skill_name, prompt, cat_hash) {
                let token_count = estimate_tokens(&cached_content, false);
                return Ok(SkillSliceResponse {
                    skill: skill_name.to_string(),
                    content: cached_content,
                    token_count,
                    original_token_count: token_count * 4,
                    catalog_hash: cat_hash.to_string(),
                });
            }
        }

        let url = self.config.endpoint_url("/api/skills/slice");
        let payload = SkillSliceRequest {
            skill: skill_name.to_string(),
            task: prompt.to_string(),
            max_sections: Some(3),
        };

        let req = self.prepare_request(self.agent.post(&url));
        let resp = req
            .send_json(payload)
            .with_context(|| format!("Skill slice request failed to {}", url))?;

        let etag = resp
            .header("ETag")
            .map(|s| s.trim_matches('"').to_string())
            .unwrap_or_default();

        let mut slice_res: SkillSliceResponse = resp
            .into_json()
            .with_context(|| "Failed to parse /api/skills/slice response JSON")?;

        if slice_res.catalog_hash.is_empty() && !etag.is_empty() {
            slice_res.catalog_hash = etag;
        }

        let hash_for_cache = if !slice_res.catalog_hash.is_empty() {
            slice_res.catalog_hash.as_str()
        } else {
            cat_hash
        };

        if !hash_for_cache.is_empty() {
            let _ = save_cached_slice(
                skill_name,
                prompt,
                hash_for_cache,
                &slice_res.content,
                slice_res.token_count,
            );
        }

        Ok(slice_res)
    }

    pub async fn check_health_async(&self) -> Result<(ServerHealthResponse, Duration)> {
        let client = self.clone();
        tokio::task::spawn_blocking(move || client.check_health())
            .await
            .map_err(|e| anyhow!("Task execution error: {}", e))?
    }

    pub async fn get_skill_stats_async(&self) -> Result<SkillStatsResponse> {
        let client = self.clone();
        tokio::task::spawn_blocking(move || client.get_skill_stats())
            .await
            .map_err(|e| anyhow!("Task execution error: {}", e))?
    }

    pub async fn search_skills_async(&self, query: String, max_results: usize) -> Result<SkillSearchResponse> {
        let client = self.clone();
        tokio::task::spawn_blocking(move || client.search_skills(&query, max_results))
            .await
            .map_err(|e| anyhow!("Task execution error: {}", e))?
    }

    pub async fn slice_skill_async(&self, skill_name: String, prompt: String) -> Result<SkillSliceResponse> {
        let client = self.clone();
        tokio::task::spawn_blocking(move || client.slice_skill(&skill_name, &prompt))
            .await
            .map_err(|e| anyhow!("Task execution error: {}", e))?
    }
}
