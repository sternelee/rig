use rig::Embed;
use rig::embeddings::EmbeddingModel;
use rig_libsql::{Column, ColumnValue, LibsqlVectorStore, LibsqlVectorStoreTable};
use serde::Deserialize;

#[derive(Embed, Clone, Debug, Deserialize)]
struct TestDocument {
    id: String,
    #[embed]
    content: String,
}

impl LibsqlVectorStoreTable for TestDocument {
    fn name() -> &'static str {
        "test_documents"
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

// Mock embedding model for testing
struct MockEmbeddingModel;

impl EmbeddingModel for MockEmbeddingModel {
    const MAX_DOCUMENTS: usize = 100;

    async fn embed_texts(
        &self,
        documents: Vec<String>,
    ) -> Result<Vec<rig::embeddings::Embedding>, rig::embeddings::EmbeddingError> {
        Ok(documents
            .into_iter()
            .map(|_| rig::embeddings::Embedding {
                document: "test".to_string(),
                vec: vec![0.1, 0.2, 0.3, 0.4],
            })
            .collect())
    }

    fn ndims(&self) -> usize {
        4
    }
}

#[tokio::test]
async fn test_vector_store_creation() {
    let db = libsql::Builder::new_local_replica(":memory:")
        .build()
        .await
        .expect("Failed to create database");
    
    let conn = db.connect().expect("Failed to connect");

    let model = MockEmbeddingModel;
    let result = LibsqlVectorStore::<MockEmbeddingModel, TestDocument>::new(conn, &model).await;
    
    assert!(result.is_ok(), "Failed to create vector store");
}
