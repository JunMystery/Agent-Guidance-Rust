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
