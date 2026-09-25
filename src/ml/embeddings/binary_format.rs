use anyhow::{Context, Result, bail};

pub const AGV1_MAGIC: &[u8; 4] = b"AGV1";
pub const AGS1_MAGIC: &[u8; 4] = b"AGS1";
pub const BINARY_VERSION: u32 = 1;
pub const VECTOR_DATA_OFFSET: usize = 64; // 64-byte aligned for AVX-512 / GPU

#[derive(Debug, Clone)]
pub struct VectorBinary {
    pub count: u32,
    pub dim: u32,
    pub vectors: Vec<Vec<f32>>,
}

impl VectorBinary {
    pub fn new(vectors: Vec<Vec<f32>>) -> Self {
        let count = vectors.len() as u32;
        let dim = vectors.first().map(|v| v.len()).unwrap_or(0) as u32;
        Self { count, dim, vectors }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_floats = (self.count as usize) * (self.dim as usize);
        let mut data = Vec::with_capacity(VECTOR_DATA_OFFSET + total_floats * 4);
        data.extend_from_slice(AGV1_MAGIC);
        data.extend_from_slice(&BINARY_VERSION.to_le_bytes());
        data.extend_from_slice(&self.count.to_le_bytes());
        data.extend_from_slice(&self.dim.to_le_bytes());
        data.resize(VECTOR_DATA_OFFSET, 0); // 48 bytes padding to hit 64-byte alignment
        for vec in &self.vectors {
            for val in vec {
                data.extend_from_slice(&val.to_le_bytes());
            }
        }
        data
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            bail!("Vector binary data too short: {} bytes", data.len());
        }
        let (count, dim, offset) = if data.starts_with(AGV1_MAGIC) {
            if data.len() < VECTOR_DATA_OFFSET {
                bail!("Truncated AGV1 header");
            }
            let count = u32::from_le_bytes(data[8..12].try_into()?);
            let dim = u32::from_le_bytes(data[12..16].try_into()?);
            (count, dim, VECTOR_DATA_OFFSET)
        } else {
            // Legacy 8-byte header fallback: [count: u32][dim: u32]
            let count = u32::from_le_bytes(data[0..4].try_into()?);
            let dim = u32::from_le_bytes(data[4..8].try_into()?);
            (count, dim, 8)
        };

        let expected_len = offset + (count as usize) * (dim as usize) * 4;
        if data.len() < expected_len {
            bail!("Vector binary data payload truncated: expected {} got {}", expected_len, data.len());
        }

        let mut vectors = Vec::with_capacity(count as usize);
        let mut cur = offset;
        for _ in 0..count {
            let mut vec = Vec::with_capacity(dim as usize);
            for _ in 0..dim {
                vec.push(f32::from_le_bytes(data[cur..cur + 4].try_into()?));
                cur += 4;
            }
            vectors.push(vec);
        }
        Ok(Self { count, dim, vectors })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRecord {
    pub name: String,
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct SkillsBinary {
    pub skills: Vec<SkillRecord>,
}

impl SkillsBinary {
    pub fn new(skills: Vec<SkillRecord>) -> Self {
        Self { skills }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let count = self.skills.len() as u32;
        let table_offset = 16u32;
        let entry_size = 24usize;
        let data_start = 16 + (count as usize) * entry_size;

        let mut header_and_table = Vec::with_capacity(data_start);
        header_and_table.extend_from_slice(AGS1_MAGIC);
        header_and_table.extend_from_slice(&BINARY_VERSION.to_le_bytes());
        header_and_table.extend_from_slice(&count.to_le_bytes());
        header_and_table.extend_from_slice(&table_offset.to_le_bytes());

        let mut payload = Vec::new();
        for s in &self.skills {
            let name_off = (data_start + payload.len()) as u32;
            let name_len = s.name.len() as u32;
            payload.extend_from_slice(s.name.as_bytes());

            let rel_off = (data_start + payload.len()) as u32;
            let rel_len = s.relative_path.len() as u32;
            payload.extend_from_slice(s.relative_path.as_bytes());

            let cont_off = (data_start + payload.len()) as u32;
            let cont_len = s.content.len() as u32;
            payload.extend_from_slice(s.content.as_bytes());

            header_and_table.extend_from_slice(&name_off.to_le_bytes());
            header_and_table.extend_from_slice(&name_len.to_le_bytes());
            header_and_table.extend_from_slice(&rel_off.to_le_bytes());
            header_and_table.extend_from_slice(&rel_len.to_le_bytes());
            header_and_table.extend_from_slice(&cont_off.to_le_bytes());
            header_and_table.extend_from_slice(&cont_len.to_le_bytes());
        }

        header_and_table.extend_from_slice(&payload);
        header_and_table
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 16 || !data.starts_with(AGS1_MAGIC) {
            bail!("Invalid AGS1 binary header");
        }
        let count = u32::from_le_bytes(data[8..12].try_into()?) as usize;
        let table_offset = u32::from_le_bytes(data[12..16].try_into()?) as usize;
        let entry_size = 24usize;

        let mut skills = Vec::with_capacity(count);
        for i in 0..count {
            let off = table_offset + i * entry_size;
            if off + entry_size > data.len() {
                bail!("Corrupt AGS1 table at index {}", i);
            }
            let name_off = u32::from_le_bytes(data[off..off + 4].try_into()?) as usize;
            let name_len = u32::from_le_bytes(data[off + 4..off + 8].try_into()?) as usize;
            let rel_off = u32::from_le_bytes(data[off + 8..off + 12].try_into()?) as usize;
            let rel_len = u32::from_le_bytes(data[off + 12..off + 16].try_into()?) as usize;
            let cont_off = u32::from_le_bytes(data[off + 16..off + 20].try_into()?) as usize;
            let cont_len = u32::from_le_bytes(data[off + 20..off + 24].try_into()?) as usize;

            let name = std::str::from_utf8(&data[name_off..name_off + name_len])
                .context("Invalid UTF-8 in skill name")?
                .to_string();
            let relative_path = std::str::from_utf8(&data[rel_off..rel_off + rel_len])
                .context("Invalid UTF-8 in relative_path")?
                .to_string();
            let content = std::str::from_utf8(&data[cont_off..cont_off + cont_len])
                .context("Invalid UTF-8 in content")?
                .to_string();

            skills.push(SkillRecord { name, relative_path, content });
        }

        Ok(Self { skills })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_binary_roundtrip() {
        let v1 = vec![1.0f32, 2.0, 3.0];
        let v2 = vec![4.0f32, 5.0, 6.0];
        let vb = VectorBinary::new(vec![v1.clone(), v2.clone()]);
        let bytes = vb.serialize();
        assert_eq!(&bytes[0..4], AGV1_MAGIC);
        assert_eq!(bytes.len(), VECTOR_DATA_OFFSET + 2 * 3 * 4);

        let decoded = VectorBinary::deserialize(&bytes).unwrap();
        assert_eq!(decoded.count, 2);
        assert_eq!(decoded.dim, 3);
        assert_eq!(decoded.vectors, vec![v1, v2]);
    }

    #[test]
    fn test_skills_binary_roundtrip() {
        let skills = vec![
            SkillRecord {
                name: "skill-one".to_string(),
                relative_path: "skills/one/SKILL.md".to_string(),
                content: "# Skill One\nDo something".to_string(),
            },
            SkillRecord {
                name: "skill-two".to_string(),
                relative_path: "skills/two/SKILL.md".to_string(),
                content: "# Skill Two\nDo other".to_string(),
            },
        ];
        let sb = SkillsBinary::new(skills.clone());
        let bytes = sb.serialize();
        assert_eq!(&bytes[0..4], AGS1_MAGIC);

        let decoded = SkillsBinary::deserialize(&bytes).unwrap();
        assert_eq!(decoded.skills, skills);
    }
}
