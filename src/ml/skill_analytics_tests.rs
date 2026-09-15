use std::path::PathBuf;
use crate::catalog::store::{SkillItem, SkillSource};
use crate::ml::skill_analytics::{
    apply_analytics_boost, format_skill_analytics_report, get_project_skill_frequencies,
    record_skill_usage,
};

fn temp_proj_dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "ag_test_sa_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn mock_skill(name: &str) -> SkillItem {
    SkillItem {
        name: name.to_string(),
        relative_path: format!("{}/SKILL.md", name),
        content: format!("# {}", name),
        source: SkillSource::Embedded,
    }
}

#[test]
fn test_record_and_get_frequencies() {
    let proj = temp_proj_dir("freq_test");
    record_skill_usage(&proj, "rust-expert");
    record_skill_usage(&proj, "rust-expert");
    record_skill_usage(&proj, "clean-code");

    let freqs = get_project_skill_frequencies(&proj);
    assert_eq!(*freqs.get("rust-expert").unwrap_or(&0), 2);
    assert_eq!(*freqs.get("clean-code").unwrap_or(&0), 1);
    assert_eq!(*freqs.get("python-expert").unwrap_or(&0), 0);

    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn test_apply_analytics_boost() {
    let proj = temp_proj_dir("boost_test");
    record_skill_usage(&proj, "skill-a");
    record_skill_usage(&proj, "skill-a");
    record_skill_usage(&proj, "skill-a"); // 3 * 0.02 = +0.06

    let cands = vec![
        (0.70, mock_skill("skill-a")),
        (0.72, mock_skill("skill-b")),
    ];

    let boosted = apply_analytics_boost(cands, &proj);
    assert_eq!(boosted.len(), 2);
    // skill-a: 0.70 + 0.06 = 0.76 > skill-b: 0.72
    assert_eq!(boosted[0].1.name, "skill-a");
    assert!((boosted[0].0 - 0.76).abs() < 0.001);
    assert!((boosted[1].0 - 0.72).abs() < 0.001);

    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn test_format_skill_analytics_report() {
    let proj = temp_proj_dir("report_test");
    record_skill_usage(&proj, "domain-skill");

    let report = format_skill_analytics_report(&proj);
    assert!(report.contains("Cross-Session Skill Analytics"));
    assert!(report.contains("domain-skill"));
    assert!(report.contains("+0.02"));

    let _ = std::fs::remove_dir_all(&proj);
}
