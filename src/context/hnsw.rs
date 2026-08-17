use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm1 = 0.0f32;
    let mut norm2 = 0.0f32;
    for i in 0..v1.len() {
        dot += v1[i] * v2[i];
        norm1 += v1[i] * v1[i];
        norm2 += v2[i] * v2[i];
    }
    let denom = (norm1 * norm2).sqrt();
    if denom == 0.0 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

#[derive(Debug, Clone)]
struct Neighbor {
    id: usize,
    similarity: f32,
}

impl PartialEq for Neighbor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Neighbor {}

impl PartialOrd for Neighbor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.similarity.partial_cmp(&other.similarity)
    }
}

impl Ord for Neighbor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// A reverse-ordered neighbor for min-heap priority queues.
#[derive(Debug, Clone)]
struct MinNeighbor(Neighbor);

impl PartialEq for MinNeighbor {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for MinNeighbor {}

impl PartialOrd for MinNeighbor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.0.partial_cmp(&self.0)
    }
}

impl Ord for MinNeighbor {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0)
    }
}

/// Hierarchical Navigable Small World (HNSW) Vector Index for sub-millisecond similarity search.
#[derive(Debug, Clone)]
pub struct HnswIndex<T> {
    pub m: usize,
    pub m_max: usize,
    pub m_max0: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub ml: f64,
    pub entry_point: Option<usize>,
    pub max_level: usize,
    pub vectors: Vec<Vec<f32>>,
    pub payloads: Vec<T>,
    // node_id -> [layer -> Vec<neighbor_node_id>]
    pub graph: Vec<Vec<Vec<usize>>>,
}

impl<T> Default for HnswIndex<T> {
    fn default() -> Self {
        Self::new(16, 64, 32)
    }
}

impl<T> HnswIndex<T> {
    /// Creates a new HnswIndex with configurable hyperparameters.
    pub fn new(m: usize, ef_construction: usize, ef_search: usize) -> Self {
        let m_max = m;
        let m_max0 = m * 2;
        let ml = 1.0 / (m as f64).ln().max(1e-9);
        Self {
            m,
            m_max,
            m_max0,
            ef_construction: ef_construction.max(m),
            ef_search: ef_search.max(1),
            ml,
            entry_point: None,
            max_level: 0,
            vectors: Vec::new(),
            payloads: Vec::new(),
            graph: Vec::new(),
        }
    }

    /// Number of vectors indexed.
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Checks if index is empty.
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    fn sample_level(&self, seed: usize) -> usize {
        // Deterministic pseudo-random generation to avoid external rand dependency
        let mut x = (seed as u64).wrapping_mul(0x517cc1b727220a95).wrapping_add(1);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        let unif = ((x as f64) / (u64::MAX as f64)).clamp(1e-7, 1.0 - 1e-7);
        ((-unif.ln()) * self.ml).floor() as usize
    }

    fn dist_similarity(&self, v1: &[f32], v2: &[f32]) -> f32 {
        cosine_similarity(v1, v2)
    }

    /// Inserts a vector and associated payload into the HNSW graph.
    pub fn insert(&mut self, vector: Vec<f32>, payload: T) {
        let node_id = self.vectors.len();
        let target_level = self.sample_level(node_id);

        self.vectors.push(vector);
        self.payloads.push(payload);
        self.graph.push(vec![Vec::new(); target_level + 1]);

        let Some(mut curr_ep) = self.entry_point else {
            self.entry_point = Some(node_id);
            self.max_level = target_level;
            return;
        };

        let node_vec = &self.vectors[node_id];
        let mut curr_dist = self.dist_similarity(node_vec, &self.vectors[curr_ep]);

        // 1. Greedy search from max_level down to target_level + 1
        if self.max_level > target_level {
            for lc in (target_level + 1..=self.max_level).rev() {
                let mut changed = true;
                while changed {
                    changed = false;
                    if lc < self.graph[curr_ep].len() {
                        for &neighbor in &self.graph[curr_ep][lc] {
                            let d = self.dist_similarity(node_vec, &self.vectors[neighbor]);
                            if d > curr_dist {
                                curr_dist = d;
                                curr_ep = neighbor;
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        // 2. Search and connect from min(max_level, target_level) down to 0
        let top_level = self.max_level.min(target_level);
        for lc in (0..=top_level).rev() {
            let candidates = self.search_layer(node_vec, curr_ep, self.ef_construction, lc);
            let neighbors = self.select_neighbors(&candidates, if lc == 0 { self.m_max0 } else { self.m_max });

            for &n in &neighbors {
                self.graph[node_id][lc].push(n);
                self.graph[n][lc].push(node_id);

                // Prune if neighbor node exceeded m_max
                let max_edges = if lc == 0 { self.m_max0 } else { self.m_max };
                if self.graph[n][lc].len() > max_edges {
                    let mut n_cand = Vec::new();
                    let n_vec = &self.vectors[n];
                    for &target in &self.graph[n][lc] {
                        let sim = self.dist_similarity(n_vec, &self.vectors[target]);
                        n_cand.push(Neighbor { id: target, similarity: sim });
                    }
                    self.graph[n][lc] = self.select_neighbors(&n_cand, max_edges);
                }
            }

            if let Some(best) = candidates.first() {
                curr_ep = best.id;
            }
        }

        if target_level > self.max_level {
            self.max_level = target_level;
            self.entry_point = Some(node_id);
        }
    }

    fn search_layer(&self, query: &[f32], ep: usize, ef: usize, lc: usize) -> Vec<Neighbor> {
        let mut visited = HashSet::new();
        visited.insert(ep);

        let initial_sim = self.dist_similarity(query, &self.vectors[ep]);
        let mut candidates = BinaryHeap::new();
        candidates.push(MinNeighbor(Neighbor { id: ep, similarity: initial_sim }));

        let mut w = BinaryHeap::new();
        w.push(Neighbor { id: ep, similarity: initial_sim });

        while let Some(MinNeighbor(c)) = candidates.pop() {
            if let Some(worst_w) = w.peek() {
                if c.similarity < worst_w.similarity && w.len() >= ef {
                    break;
                }
            }

            if lc < self.graph[c.id].len() {
                for &neighbor in &self.graph[c.id][lc] {
                    if visited.insert(neighbor) {
                        let sim = self.dist_similarity(query, &self.vectors[neighbor]);
                        let worst_sim = w.peek().map(|n| n.similarity).unwrap_or(f32::NEG_INFINITY);

                        if sim > worst_sim || w.len() < ef {
                            candidates.push(MinNeighbor(Neighbor { id: neighbor, similarity: sim }));
                            w.push(Neighbor { id: neighbor, similarity: sim });

                            if w.len() > ef {
                                w.pop();
                            }
                        }
                    }
                }
            }
        }

        let mut results: Vec<Neighbor> = w.into_sorted_vec();
        results.reverse();
        results
    }

    fn select_neighbors(&self, candidates: &[Neighbor], m: usize) -> Vec<usize> {
        let mut sorted = candidates.to_vec();
        sorted.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(Ordering::Equal));
        sorted.into_iter().take(m).map(|n| n.id).collect()
    }

    /// Performs sub-millisecond approximate nearest neighbor search across the HNSW graph.
    pub fn search(&self, query: &[f32], top_k: usize, threshold: f32) -> Vec<(f32, &T)> {
        let Some(mut curr_ep) = self.entry_point else {
            return Vec::new();
        };

        if self.vectors.is_empty() || top_k == 0 {
            return Vec::new();
        }

        let mut curr_dist = self.dist_similarity(query, &self.vectors[curr_ep]);

        // 1. Greedy search from max_level down to 1
        for lc in (1..=self.max_level).rev() {
            let mut changed = true;
            while changed {
                changed = false;
                if lc < self.graph[curr_ep].len() {
                    for &neighbor in &self.graph[curr_ep][lc] {
                        let d = self.dist_similarity(query, &self.vectors[neighbor]);
                        if d > curr_dist {
                            curr_dist = d;
                            curr_ep = neighbor;
                            changed = true;
                        }
                    }
                }
            }
        }

        // 2. Comprehensive search on layer 0 with ef_search
        let ef = self.ef_search.max(top_k);
        let candidates = self.search_layer(query, curr_ep, ef, 0);

        candidates
            .into_iter()
            .filter(|n| n.similarity >= threshold)
            .take(top_k)
            .map(|n| (n.similarity, &self.payloads[n.id]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_graph_insertion_and_search() {
        let mut index = HnswIndex::new(16, 64, 32);

        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let v3 = vec![0.0, 0.0, 1.0];
        let v4 = vec![0.707, 0.707, 0.0];

        index.insert(v1, "vector_x");
        index.insert(v2, "vector_y");
        index.insert(v3, "vector_z");
        index.insert(v4, "vector_xy");

        assert_eq!(index.len(), 4);

        // Search near vector_x
        let query = vec![0.9, 0.1, 0.0];
        let hits = index.search(&query, 2, 0.5);

        assert!(!hits.is_empty());
        assert_eq!(*hits[0].1, "vector_x");
        assert!(hits[0].0 > 0.9);
    }

    #[test]
    fn test_hnsw_empty_search() {
        let index: HnswIndex<String> = HnswIndex::default();
        let hits = index.search(&[1.0, 2.0], 5, 0.0);
        assert!(hits.is_empty());
    }
}
