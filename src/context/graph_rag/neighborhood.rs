use anyhow::Result;
use rusqlite::params;
use crate::context::db::CodeGraphDb;
use super::community::CommunityHierarchy;

#[derive(Debug, Clone)]
pub struct NeighborNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
    pub edge_type: String,
    pub weight: f64,
}

#[derive(Debug, Clone)]
pub struct SymbolNeighborhood {
    pub target_id: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
    pub code_excerpt: Option<String>,
    pub callers: Vec<NeighborNode>,
    pub callees: Vec<NeighborNode>,
    pub transitive_chains: Vec<(String, String, String)>,
    pub community_title: Option<String>,
    pub community_layer: Option<String>,
    pub mermaid_dag: String,
}

pub fn fetch_symbol_neighborhood(
    db: &CodeGraphDb,
    hierarchy: &CommunityHierarchy,
    symbol_query: &str,
) -> Result<Option<SymbolNeighborhood>> {
    let q = symbol_query.trim();
    if q.is_empty() {
        return Ok(None);
    }

    // 1. Locate primary symbol entity
    let mut stmt = db.conn.prepare(
        "SELECT id, name, kind, file_path, start_line, end_line, signature
         FROM symbols WHERE name = ?1 OR id = ?1
         ORDER BY (end_line - start_line) DESC LIMIT 1"
    )?;

    let target_opt = stmt.query_row(params![q], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, usize>(4)?,
            row.get::<_, usize>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    }).ok().or_else(|| {
        let pattern = format!("%{}%", q);
        let mut fallback_stmt = db.conn.prepare(
            "SELECT id, name, kind, file_path, start_line, end_line, signature
             FROM symbols WHERE name LIKE ?1
             ORDER BY (end_line - start_line) DESC LIMIT 1"
        ).ok()?;
        fallback_stmt.query_row(params![pattern], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, usize>(4)?,
                row.get::<_, usize>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        }).ok()
    });

    let (target_id, name, kind, file_path, start_line, end_line, signature) = match target_opt {
        Some(t) => t,
        None => return Ok(None),
    };

    // 2. Fetch code excerpt
    let code_excerpt = db.conn.query_row(
        "SELECT chunk_text FROM content_chunks WHERE file_path = ?1 AND start_line <= ?2 AND end_line >= ?2 LIMIT 1",
        params![file_path, start_line],
        |row| row.get::<_, String>(0),
    ).ok();

    // 3. Fetch 1-hop callers
    let mut c_stmt = db.conn.prepare(
        "SELECT s.id, s.name, s.kind, s.file_path, s.start_line, e.edge_type, e.weight
         FROM symbol_edges e JOIN symbols s ON e.source_id = s.id
         WHERE e.target_id = ?1 LIMIT 12"
    )?;
    let callers = c_stmt.query_map(params![target_id], |row| {
        Ok(NeighborNode {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            file_path: row.get(3)?,
            start_line: row.get(4)?,
            edge_type: row.get(5)?,
            weight: row.get(6)?,
        })
    })?.filter_map(|r| r.ok()).collect::<Vec<_>>();

    // 4. Fetch 1-hop callees
    let mut out_stmt = db.conn.prepare(
        "SELECT s.id, s.name, s.kind, s.file_path, s.start_line, e.edge_type, e.weight
         FROM symbol_edges e JOIN symbols s ON e.target_id = s.id
         WHERE e.source_id = ?1 LIMIT 12"
    )?;
    let callees = out_stmt.query_map(params![target_id], |row| {
        Ok(NeighborNode {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            file_path: row.get(3)?,
            start_line: row.get(4)?,
            edge_type: row.get(5)?,
            weight: row.get(6)?,
        })
    })?.filter_map(|r| r.ok()).collect::<Vec<_>>();

    // 5. Fetch 2-hop transitive chains
    let mut t_stmt = db.conn.prepare(
        "SELECT s1.name, s2.name, s3.name
         FROM symbol_edges e1
         JOIN symbol_edges e2 ON e1.target_id = e2.source_id
         JOIN symbols s1 ON e1.source_id = s1.id
         JOIN symbols s2 ON e1.target_id = s2.id
         JOIN symbols s3 ON e2.target_id = s3.id
         WHERE e1.target_id = ?1 OR e2.source_id = ?1 LIMIT 6"
    )?;
    let transitive_chains = t_stmt.query_map(params![target_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?.filter_map(|r| r.ok()).collect::<Vec<_>>();

    // 6. Community Context
    let comm = hierarchy.find_community_for_file(&file_path);
    let community_title = comm.map(|c| c.summary.title.clone());
    let community_layer = comm.map(|c| c.summary.layer.clone());

    // 7. Mini Mermaid DAG
    let mut dag = String::from("graph LR\n");
    dag.push_str(&format!("    tgt[\"{}\"]:::targetNode\n", name));
    for c in &callers {
        dag.push_str(&format!("    {}[\"{}\"] -->|{}| tgt\n", c.name.replace(':', "_"), c.name, c.edge_type));
    }
    for c in &callees {
        dag.push_str(&format!("    tgt -->|{}| {}[\"{}\"]\n", c.edge_type, c.name.replace(':', "_"), c.name));
    }
    dag.push_str("    classDef targetNode fill:#00e5ff,stroke:#ffffff,stroke-width:2px,color:#000;\n");

    Ok(Some(SymbolNeighborhood {
        target_id,
        name,
        kind,
        file_path,
        start_line,
        end_line,
        signature,
        code_excerpt,
        callers,
        callees,
        transitive_chains,
        community_title,
        community_layer,
        mermaid_dag: dag,
    }))
}
