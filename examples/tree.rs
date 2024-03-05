use php_ls::indexer::reindex_project;

fn main() -> anyhow::Result<()> {
    let root_path = "/home/eksandral/projects/php-template";
    reindex_project(&root_path)?;
    Ok(())
}
