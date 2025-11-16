<div style="display: flex; align-items: center; justify-content: center;">
    <picture>
        <source media="(prefers-color-scheme: dark)" srcset="../img/rig_logo_dark.svg">
        <source media="(prefers-color-scheme: light)" srcset="../img/rig_logo.svg">
        <img src="../img/rig_logo.svg" width="200" alt="Rig logo">
    </picture>
    <span style="font-size: 48px; margin: 0 20px; font-weight: regular; font-family: Open Sans, sans-serif;"> + </span>
    <picture>
        <img src="https://turso.tech/turso-symbol.svg" width="200" alt="Turso logo">
    </picture>
</div>

<br><br>

## Rig-libSQL

This companion crate implements a Rig vector store based on Turso's libSQL.

libSQL is an open contribution fork of SQLite that's designed for the edge and cloud. It's developed and maintained by Turso and provides additional features like:
- Built-in replication
- Remote connectivity 
- Edge deployment support
- Embedded replicas with sync capabilities

## Usage

Add the companion crate to your `Cargo.toml`, along with the rig-core crate:

```toml
[dependencies]
rig-libsql = "0.1.0"
rig-core = "0.24.0"
```

You can also run `cargo add rig-libsql rig-core` to add the most recent versions of the dependencies to your project.

See the [`/examples`](./examples) folder for usage examples.

## Connection Options

libSQL supports multiple connection modes:

### Local File-based Database

```rust
let db = libsql::Builder::new_local("vector_store.db").build().await?;
let conn = db.connect()?;
```

### Remote Turso Database

```rust
let db = libsql::Builder::new_remote(
    "libsql://your-db.turso.io".to_string(),
    "your-auth-token".to_string()
).build().await?;
let conn = db.connect()?;
```

### Embedded Replica with Sync

```rust
let db = libsql::Builder::new_remote_replica(
    "vector_store.db",
    "libsql://your-db.turso.io".to_string(),
    "your-auth-token".to_string()
).build().await?;
let conn = db.connect()?;
```

## Vector Extension

This implementation uses the `sqlite-vec` extension for vector similarity search. The extension needs to be loaded:

```rust
// Attempt to load the vec0 extension
conn.execute("SELECT load_extension('vec0')", ()).await.ok();
```

Note: For Turso cloud databases, the vec0 extension may need to be enabled on your database instance.

## Features

- **Vector Search**: Perform similarity searches using embeddings
- **Flexible Storage**: Store documents with custom schemas
- **Multiple Deployment Options**: Local, remote, or embedded replica
- **Edge-Ready**: Deploy to the edge with Turso's infrastructure
- **Sync Capabilities**: Keep local replicas in sync with remote databases

## Example

```rust
use rig::{
    embeddings::EmbeddingsBuilder,
    providers::openai::{Client, TEXT_EMBEDDING_ADA_002},
    vector_store::VectorStoreIndex,
    Embed,
};
use rig_libsql::{Column, ColumnValue, LibsqlVectorStore, LibsqlVectorStoreTable};
use serde::Deserialize;

#[derive(Embed, Clone, Debug, Deserialize)]
struct Document {
    id: String,
    #[embed]
    content: String,
}

impl LibsqlVectorStoreTable for Document {
    fn name() -> &'static str {
        "documents"
    }

    fn schema() -> Vec<Column> {
        vec![
            Column::new("id", "TEXT PRIMARY KEY"),
            Column::new("content", "TEXT"),
        ]
    }

    fn id(&self) -> String {
        self.id.clone()
    }

    fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
        vec![
            ("id", Box::new(self.id.clone())),
            ("content", Box::new(self.content.clone())),
        ]
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = libsql::Builder::new_local("vector_store.db").build().await?;
    let conn = db.connect()?;
    
    let openai_client = Client::new("YOUR_API_KEY");
    let model = openai_client.embedding_model(TEXT_EMBEDDING_ADA_002);

    let vector_store = LibsqlVectorStore::new(conn, &model).await?;
    
    // ... use vector store
    Ok(())
}
```

## Learn More

- [Turso Documentation](https://docs.turso.tech/)
- [libSQL GitHub](https://github.com/tursodatabase/libsql)
- [sqlite-vec Extension](https://github.com/asg017/sqlite-vec)
