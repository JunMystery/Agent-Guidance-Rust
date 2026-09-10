use anyhow::Result;
use rusqlite::Connection;

mod content;
mod core;
mod graph;

pub(crate) fn init_schema(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    core::create_core_tables(&tx)?;
    graph::create_graph_tables(&tx)?;
    content::create_content_tables(&tx)?;
    tx.commit()?;
    Ok(())
}
