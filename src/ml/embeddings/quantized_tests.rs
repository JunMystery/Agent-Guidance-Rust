use super::backend::EmbeddingBackend;
use super::providers::{ExecutionProvider, detect_optimal_provider};
use super::quantized::OnnxQuantizedModel;

#[test]
fn test_detect_optimal_provider_env_overrides() {
    unsafe {
        std::env::set_var("AGENT_GUIDANCE_DEVICE", "cpu");
    }
    let (prov, name) = detect_optimal_provider();
    assert_eq!(prov, ExecutionProvider::Cpu);
    assert_eq!(name, "CPU (AVX2/SIMD Baseline)");
    assert!(!prov.is_gpu());

    unsafe {
        std::env::set_var("AGENT_GUIDANCE_DEVICE", "directml");
    }
    let (prov_dml, name_dml) = detect_optimal_provider();
    assert_eq!(prov_dml, ExecutionProvider::DirectML { device_id: 0 });
    assert!(name_dml.contains("DirectML"));
    assert!(prov_dml.is_gpu());

    unsafe {
        std::env::set_var("AGENT_GUIDANCE_DEVICE", "cuda");
    }
    let (prov_cuda, name_cuda) = detect_optimal_provider();
    assert_eq!(prov_cuda, ExecutionProvider::Cuda { device_id: 0 });
    assert!(name_cuda.contains("CUDA"));
    assert!(prov_cuda.is_gpu());

    unsafe {
        std::env::set_var("AGENT_GUIDANCE_DEVICE", "auto");
    }
    let (prov_auto, _) = detect_optimal_provider();
    assert!(matches!(
        prov_auto,
        ExecutionProvider::Cpu | ExecutionProvider::DirectML { .. } | ExecutionProvider::Cuda { .. }
    ));
}

#[test]
fn test_normalize_vector() {
    let vec = vec![3.0f32, 4.0f32];
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 5.0).abs() < 1e-5);

    let mut v = vec.clone();
    let norm_calc: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    for x in v.iter_mut() {
        *x /= norm_calc;
    }
    let unit_norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((unit_norm - 1.0).abs() < 1e-5);
    assert!((v[0] - 0.6).abs() < 1e-5);
    assert!((v[1] - 0.8).abs() < 1e-5);
}

#[test]
fn test_pool_and_normalize_3d_tensor() {
    // 1 item in batch, 2 tokens, hidden_dim = 2
    let shape = [1i64, 2, 2];
    let data = [1.0f32, 2.0, 3.0, 4.0]; // token 1: [1, 2], token 2: [3, 4]
    let mask = [1i64, 1]; // both tokens active

    // Mean should be [(1+3)/2, (2+4)/2] = [2.0, 3.0]
    // Norm is sqrt(4 + 9) = sqrt(13) ~ 3.60555
    let res = OnnxQuantizedModel::pool_and_normalize(&shape, &data, &mask, 1, 2).unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].len(), 2);

    let norm: f32 = res[0].iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5);
    let expected_ratio = 2.0 / 3.0;
    let actual_ratio = res[0][0] / res[0][1];
    assert!((actual_ratio - expected_ratio).abs() < 1e-5);
}

#[test]
fn test_pool_and_normalize_3d_tensor_with_padding_mask() {
    // 1 item in batch, 2 tokens, hidden_dim = 2, second token is PADDING (mask = 0)
    let shape = [1i64, 2, 2];
    let data = [1.0f32, 2.0, 999.0, 999.0]; // token 1: [1, 2], token 2 (padded): [999, 999]
    let mask = [1i64, 0]; // second token masked out!

    // Mean should be ONLY token 1: [1.0, 2.0]
    let res = OnnxQuantizedModel::pool_and_normalize(&shape, &data, &mask, 1, 2).unwrap();
    assert_eq!(res.len(), 1);

    let norm: f32 = res[0].iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5);
    let expected_ratio = 1.0 / 2.0;
    let actual_ratio = res[0][0] / res[0][1];
    assert!((actual_ratio - expected_ratio).abs() < 1e-5);
}

#[test]
fn test_pool_and_normalize_2d_tensor() {
    let shape = [2i64, 2]; // 2 items in batch, hidden_dim = 2
    let data = [3.0f32, 4.0, 0.0, 5.0];
    let mask = [1i64, 1];

    let res = OnnxQuantizedModel::pool_and_normalize(&shape, &data, &mask, 2, 1).unwrap();
    assert_eq!(res.len(), 2);
    let norm0: f32 = res[0].iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm1: f32 = res[1].iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm0 - 1.0).abs() < 1e-5);
    assert!((norm1 - 1.0).abs() < 1e-5);
    assert!((res[0][0] - 0.6).abs() < 1e-5);
    assert!((res[0][1] - 0.8).abs() < 1e-5);
}

#[test]
fn test_model_discovery_precedence() {
    let temp_dir = std::env::temp_dir().join("ag_test_onnx_discovery");
    let _ = std::fs::create_dir_all(&temp_dir);

    // Write empty files
    let standard_onnx = temp_dir.join("model.onnx");
    let q8_onnx = temp_dir.join("bge-small-en-v1.5-q8.onnx");
    let _ = std::fs::write(&standard_onnx, b"dummy");
    let _ = std::fs::write(&q8_onnx, b"dummy");

    let (discovered, is_quantized) = OnnxQuantizedModel::discover_model_file(&temp_dir).unwrap();
    // Should prioritize q8 model over standard model.onnx
    assert!(is_quantized);
    assert!(discovered.to_string_lossy().contains("bge-small-en-v1.5-q8.onnx"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
