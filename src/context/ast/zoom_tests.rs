use super::super::types::AstLanguage;
use super::*;

#[test]
fn test_rust_zoom_slice() {
    let code = r#"
pub struct Config {
    pub timeout: u32,
}

pub fn add(a: i32, b: i32) -> i32 {
    let res = a + b;
    println!("adding numbers");
    res
}

pub fn multiply(a: i32, b: i32) -> i32 {
    let mut prod = 0;
    for _ in 0..b {
        prod += a;
    }
    prod
}

pub fn divide(a: i32, b: i32) -> i32 {
    if b == 0 {
        panic!("zero division");
    }
    a / b
}
"#;

    let res = generate_zoom_slice(code, AstLanguage::Rust, "math.rs", "multiply")
        .expect("should generate zoom slice");

    assert!(res.target_found);
    assert_eq!(res.folded_functions_count, 2);
    assert!(res.sliced_content.contains("pub struct Config"));
    assert!(res.sliced_content.contains("ZOOM FOCUS: multiply"));
    assert!(res.sliced_content.contains("for _ in 0..b"));
    assert!(res.sliced_content.contains("add(a: i32, b: i32) -> i32 { /*"));
    assert!(res.sliced_content.contains("lines folded"));
    assert!(res.sliced_content.contains("divide(a: i32, b: i32) -> i32 { /*"));
    assert!(!res.sliced_content.contains("println!(\"adding numbers\")"));
    assert!(!res.sliced_content.contains("panic!(\"zero division\")"));
    assert!(res.savings_percent > 0);
}

#[test]
fn test_python_zoom_slice() {
    let py_code = r#"
class Service:
    name: str

def helper_one():
    step1 = 10
    step2 = 20
    return step1 + step2

def main_worker():
    msg = "working"
    print(msg)
    return True
"#;

    let res = generate_zoom_slice(py_code, AstLanguage::Python, "service.py", "main_worker")
        .expect("should generate python zoom slice");

    assert!(res.target_found);
    assert_eq!(res.folded_functions_count, 1);
    assert!(res.sliced_content.contains("class Service:"));
    assert!(res.sliced_content.contains("ZOOM FOCUS: main_worker"));
    assert!(res.sliced_content.contains("print(msg)"));
    assert!(res.sliced_content.contains("lines folded"));
    assert!(!res.sliced_content.contains("step1 = 10"));
}

#[test]
fn test_target_not_found() {
    let code = "pub fn foo() {}";
    let res = generate_zoom_slice(code, AstLanguage::Rust, "test.rs", "non_existent");
    assert!(res.is_none());
}

#[test]
fn test_optimizer_skeleton_zoom_integration() {
    let code = r#"
pub struct User { id: u64 }
pub fn alpha() {
    let a = 1;
    let _ = a;
}
pub fn beta() {
    let b = 2;
    println!("focus");
}
pub fn gamma() {
    let c = 3;
    let _ = c;
}
"#;
    let res = crate::optimizer::skeleton::generate_zoom_slice(code, "test.rs", "beta")
        .expect("should generate zoom slice via skeleton entrypoint");
    assert!(res.target_found);
    assert_eq!(res.folded_functions_count, 2);
    assert!(res.sliced_content.contains("pub struct User"));
    assert!(res.sliced_content.contains("ZOOM FOCUS: beta"));
    assert!(res.sliced_content.contains("println!(\"focus\")"));
    assert!(res.sliced_content.contains("alpha() { /*"));
    assert!(res.sliced_content.contains("gamma() { /*"));
}
