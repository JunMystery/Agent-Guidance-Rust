use std::path::Path;
use crate::catalog::store::get_embedded_skill;
use super::validate_path;
use super::skills_gate::clean_skill_identifier;

pub(crate) fn resolve_skill(
    raw_req: &str,
    proposals: &[(String, String, f32)],
    all_skills: &[crate::catalog::store::SkillItem],
    proj_path: &Path,
) -> Option<(String, String, &'static str)> {
    let clean_name = clean_skill_identifier(raw_req);

    // 1. Check proposals
    if let Some((prop_name, rel_path, _)) = proposals.iter().find(|(n, rel, _)| {
        n.eq_ignore_ascii_case(&clean_name)
            || rel.ends_with(&clean_name)
            || rel.contains(&format!("/{}", clean_name))
    }) {
        let content = get_embedded_skill(prop_name)
            .or_else(|| get_embedded_skill(rel_path))
            .or_else(|| validate_path(proj_path, rel_path).ok().and_then(|p| std::fs::read_to_string(p).ok()))
            .or_else(|| validate_path(proj_path, prop_name).ok().and_then(|p| std::fs::read_to_string(p).ok()));
        if let Some(c) = content {
            let tag = if get_embedded_skill(prop_name).is_some() || get_embedded_skill(rel_path).is_some() {
                " [Embedded Catalog]"
            } else {
                " [Local Workspace]"
            };
            return Some((prop_name.clone(), c, tag));
        }
    }

    // 2. Check all catalog & workspace skills
    if let Some(item) = all_skills.iter().find(|s| {
        s.name.eq_ignore_ascii_case(&clean_name)
            || s.relative_path.eq_ignore_ascii_case(&clean_name)
            || s.relative_path.ends_with(&clean_name)
            || clean_name.ends_with(&s.relative_path)
            || clean_name.contains(&format!("/{}/", s.name))
            || clean_name.contains(&format!("\\{}\\", s.name))
    }) {
        let tag = match item.source {
            crate::catalog::store::SkillSource::Embedded => " [Embedded Catalog]",
            crate::catalog::store::SkillSource::LocalWorkspace(_) => " [Local Workspace]",
        };
        return Some((item.name.clone(), item.content.clone(), tag));
    }

    // 3. Fallback to embedded skill direct lookup
    if let Some(c) = get_embedded_skill(&clean_name) {
        return Some((clean_name, c, " [Embedded Catalog]"));
    }

    // 4. Fallback to direct file path lookup
    if let Ok(full_path) = validate_path(proj_path, &clean_name) {
        if let Ok(c) = std::fs::read_to_string(&full_path) {
            let name = crate::catalog::store::scanner::extract_frontmatter_name(&c)
                .unwrap_or_else(|| {
                    full_path
                        .parent()
                        .and_then(|p| p.file_name())
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| clean_name.clone())
                });
            return Some((name, c, " [Local Workspace]"));
        }
    }

    None
}
