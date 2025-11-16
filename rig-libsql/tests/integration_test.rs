use rig::Embed;
use rig::embeddings::EmbeddingModel;
use rig_libsql::{Column, ColumnValue, LibsqlVectorStoreTable};
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
#[derive(Clone)]
struct MockEmbeddingModel;

impl EmbeddingModel for MockEmbeddingModel {
    const MAX_DOCUMENTS: usize = 100;

    fn ndims(&self) -> usize {
        4
    }

    fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> impl std::future::Future<Output = Result<Vec<rig::embeddings::Embedding>, rig::embeddings::EmbeddingError>> + Send {
        async {
            Ok(texts
                .into_iter()
                .map(|_| rig::embeddings::Embedding {
                    document: "test".to_string(),
                    vec: vec![0.1, 0.2, 0.3, 0.4],
                })
                .collect())
        }
    }
}

#[tokio::test]
async fn test_libsql_connection() {
    // Test that we can create a libsql database connection
    let db = libsql::Builder::new_local(":memory:")
        .build()
        .await
        .expect("Failed to create database");
    
    let conn = db.connect().expect("Failed to connect");
    
    // Test basic query
    let mut rows = conn.query("SELECT 1", ()).await.expect("Failed to execute query");
    let row = rows.next().await.expect("Failed to get row").expect("No row returned");
    let val: i64 = row.get(0).expect("Failed to get value");
    assert_eq!(val, 1);
}

// Note: This test requires the vec0 extension to be available in libsql
// For Turso cloud databases, vec0 may need to be enabled on the database instance
// For local testing, the extension needs to be compiled and loaded
#[tokio::test]
#[ignore] // Ignore by default since vec0 extension may not be available in test environment
async fn test_vector_store_creation() {
    let db = libsql::Builder::new_local(":memory:")
        .build()
        .await
        .expect("Failed to create database");
    
    let conn = db.connect().expect("Failed to connect");

    let model = MockEmbeddingModel;
    let result = rig_libsql::LibsqlVectorStore::<MockEmbeddingModel, TestDocument>::new(conn, &model).await;
    
    match &result {
        Ok(_) => println!("Vector store created successfully"),
        Err(e) => println!("Error creating vector store: {:?}", e),
    }
    
    assert!(result.is_ok(), "Failed to create vector store: {:?}", result.err());
}

