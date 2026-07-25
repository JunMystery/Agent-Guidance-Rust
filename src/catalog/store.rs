use rust_embed::Embed;

#[derive(Embed)]
#[folder = "skills/"]
pub struct SkillAssets;

pub fn list_embedded_skills() -> Vec<String> {
    SkillAssets::iter()
        .map(|path| path.as_ref().to_string())
        .collect()
}

pub fn get_embedded_skill(path: &str) -> Option<String> {
    SkillAssets::get(path).and_then(|file| {
        std::str::from_utf8(file.data.as_ref())
            .ok()
            .map(|s| s.to_string())
    })
}
