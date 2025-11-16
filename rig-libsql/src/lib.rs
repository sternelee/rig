use rig::OneOrMany;
use rig::embeddings::{Embedding, EmbeddingModel};
use rig::vector_store::request::{FilterError, SearchFilter, VectorSearchRequest};
use rig::vector_store::{VectorStoreError, VectorStoreIndex};
use serde::Deserialize;
use std::marker::PhantomData;
use std::ops::RangeInclusive;
use tracing::{debug, info};
use zerocopy::IntoBytes;

#[derive(Debug)]
pub enum LibsqlError {
    DatabaseError(Box<dyn std::error::Error + Send + Sync>),
    SerializationError(Box<dyn std::error::Error + Send + Sync>),
    InvalidColumnType(String),
}

pub trait ColumnValue: Send + Sync {
    fn to_sql_string(&self) -> String;
    fn column_type(&self) -> &'static str;
}

pub struct Column {
    name: &'static str,
    col_type: &'static str,
    indexed: bool,
}

impl Column {
    pub fn new(name: &'static str, col_type: &'static str) -> Self {
        Self {
            name,
            col_type,
            indexed: false,
        }
    }

    pub fn indexed(mut self) -> Self {
        self.indexed = true;
        self
    }
}

/// Example of a document type that can be used with LibsqlVectorStore
/// ```rust
/// use rig::Embed;
/// use serde::Deserialize;
/// use rig_libsql::{Column, ColumnValue, LibsqlVectorStoreTable};
///
/// #[derive(Embed, Clone, Debug, Deserialize)]
/// struct Document {
///     id: String,
///     #[embed]
///     content: String,
/// }
///
/// impl LibsqlVectorStoreTable for Document {
///     fn name() -> &'static str {
///         "documents"
///     }
///
///     fn schema() -> Vec<Column> {
///         vec![
///             Column::new("id", "TEXT PRIMARY KEY"),
///             Column::new("content", "TEXT"),
///         ]
///     }
///
///     fn id(&self) -> String {
///         self.id.clone()
///     }
///
///     fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
///         vec![
///             ("id", Box::new(self.id.clone())),
///             ("content", Box::new(self.content.clone())),
///         ]
///     }
/// }
/// ```
pub trait LibsqlVectorStoreTable: Send + Sync + Clone {
    fn name() -> &'static str;
    fn schema() -> Vec<Column>;
    fn id(&self) -> String;
    fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)>;
}

#[derive(Clone)]
pub struct LibsqlVectorStore<E, T>
where
    E: EmbeddingModel + 'static,
    T: LibsqlVectorStoreTable + 'static,
{
    conn: libsql::Connection,
    _phantom: PhantomData<(E, T)>,
}

impl<E, T> LibsqlVectorStore<E, T>
where
    E: EmbeddingModel + 'static,
    T: LibsqlVectorStoreTable + 'static,
{
    pub async fn new(conn: libsql::Connection, embedding_model: &E) -> Result<Self, VectorStoreError> {
        let dims = embedding_model.ndims();
        let table_name = T::name();
        let schema = T::schema();

        // Build the table schema
        let mut create_table = format!("CREATE TABLE IF NOT EXISTS {table_name} (");

        // Add columns
        let mut first = true;
        for column in &schema {
            if !first {
                create_table.push(',');
            }
            create_table.push_str(&format!("\n    {} {}", column.name, column.col_type));
            first = false;
        }

        create_table.push_str("\n)");

        // Build index creation statements
        let mut create_indexes = vec![format!(
            "CREATE INDEX IF NOT EXISTS idx_{}_id ON {}(id)",
            table_name, table_name
        )];

        // Add indexes for marked columns
        for column in schema {
            if column.indexed {
                create_indexes.push(format!(
                    "CREATE INDEX IF NOT EXISTS idx_{}_{} ON {}({})",
                    table_name, column.name, table_name, column.name
                ));
            }
        }

        // Execute table and index creation
        conn.execute("BEGIN", ())
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        // Create document table
        conn.execute(&create_table, ())
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        // Create indexes
        for index_stmt in create_indexes {
            conn.execute(&index_stmt, ())
                .await
                .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;
        }

        // Create embeddings table
        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS {table_name}_embeddings USING vec0(embedding float[{dims}])"
            ),
            (),
        )
        .await
        .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        conn.execute("COMMIT", ())
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        Ok(Self {
            conn,
            _phantom: PhantomData,
        })
    }

    pub fn index(self, model: E) -> LibsqlVectorIndex<E, T> {
        LibsqlVectorIndex::new(model, self)
    }

    pub async fn add_rows(
        &self,
        documents: Vec<(T, OneOrMany<Embedding>)>,
    ) -> Result<i64, VectorStoreError> {
        info!("Adding {} documents to store", documents.len());
        let table_name = T::name();
        let mut last_id = 0i64;

        // Start transaction
        self.conn
            .execute("BEGIN", ())
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        for (doc, embeddings) in &documents {
            debug!("Storing document with id {}", doc.id());

            let values = doc.column_values();
            let columns = values.iter().map(|(col, _)| *col).collect::<Vec<_>>();

            let placeholders = (1..=values.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>();

            let insert_sql = format!(
                "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
                table_name,
                columns.join(", "),
                placeholders.join(", ")
            );

            // libsql uses params! macro or a tuple for parameters
            let param_values: Vec<String> = values.iter().map(|(_, val)| val.to_sql_string()).collect();
            
            // Execute the insert using libsql's parameter binding
            self.conn
                .execute(&insert_sql, libsql::params_from_iter(param_values))
                .await
                .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

            // Get last insert rowid
            let mut rowid_result = self.conn
                .query("SELECT last_insert_rowid()", ())
                .await
                .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

            if let Some(row) = rowid_result.next().await.map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))? {
                last_id = row.get::<i64>(0).map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;
            }

            let embeddings_sql =
                format!("INSERT INTO {table_name}_embeddings (rowid, embedding) VALUES (?1, ?2)");

            for (i, embedding) in embeddings.iter().enumerate() {
                let vec = serialize_embedding(embedding);
                debug!(
                    "Storing embedding {} of {} (size: {} bytes)",
                    i + 1,
                    embeddings.len(),
                    vec.len() * 4
                );
                let blob = vec.as_bytes().to_vec();
                self.conn
                    .execute(&embeddings_sql, libsql::params![last_id, blob])
                    .await
                    .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;
            }
        }

        // Commit transaction
        self.conn
            .execute("COMMIT", ())
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        Ok(last_id)
    }
}

#[derive(Clone, Default)]
pub struct LibsqlSearchFilter {
    condition: String,
    params: Vec<serde_json::Value>,
}

impl SearchFilter for LibsqlSearchFilter {
    type Value = serde_json::Value;

    fn eq(key: String, value: Self::Value) -> Self {
        Self {
            condition: format!("{key} = ?"),
            params: vec![value],
        }
    }

    fn gt(key: String, value: Self::Value) -> Self {
        Self {
            condition: format!("{key} > ?"),
            params: vec![value],
        }
    }

    fn lt(key: String, value: Self::Value) -> Self {
        Self {
            condition: format!("{key} < ?"),
            params: vec![value],
        }
    }

    fn and(self, rhs: Self) -> Self {
        Self {
            condition: format!("({}) AND ({})", self.condition, rhs.condition),
            params: self.params.into_iter().chain(rhs.params).collect(),
        }
    }

    fn or(self, rhs: Self) -> Self {
        Self {
            condition: format!("({}) OR ({})", self.condition, rhs.condition),
            params: self.params.into_iter().chain(rhs.params).collect(),
        }
    }
}

impl LibsqlSearchFilter {
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Self {
        Self {
            condition: format!("NOT ({})", self.condition),
            ..self
        }
    }

    /// Tests whether the value at `key` is contained in the range
    pub fn between<N>(key: String, range: RangeInclusive<N>) -> Self
    where
        N: Ord + std::fmt::Display,
    {
        let lo = range.start();
        let hi = range.end();

        Self {
            condition: format!("{key} between {lo} and {hi}"),
            ..Default::default()
        }
    }

    // Null checks
    pub fn is_null(key: String) -> Self {
        Self {
            condition: format!("{key} is null"),
            ..Default::default()
        }
    }

    pub fn is_not_null(key: String) -> Self {
        Self {
            condition: format!("{key} is not null"),
            ..Default::default()
        }
    }

    // String ops
    /// Tests whether the value at `key` satisfies the glob pattern
    /// `pattern` should be a valid SQLite glob pattern
    pub fn glob<'a, S>(key: String, pattern: S) -> Self
    where
        S: AsRef<&'a str>,
    {
        Self {
            condition: format!("{key} glob {}", pattern.as_ref()),
            ..Default::default()
        }
    }

    /// Tests whether the value at `key` satisfies the "like" pattern
    /// `pattern` should be a valid SQLite like pattern
    pub fn like<'a, S>(key: String, pattern: S) -> Self
    where
        S: AsRef<&'a str>,
    {
        Self {
            condition: format!("{key} like {}", pattern.as_ref()),
            ..Default::default()
        }
    }
}

impl LibsqlSearchFilter {
    fn compile_params(self) -> Result<Vec<libsql::Value>, FilterError> {
        let mut params = Vec::with_capacity(self.params.len());

        fn convert(value: serde_json::Value) -> Result<libsql::Value, FilterError> {
            use serde_json::Value::*;

            match value {
                Null => Ok(libsql::Value::Null),
                Bool(b) => Ok(libsql::Value::Integer(b as i64)),
                String(s) => Ok(libsql::Value::Text(s)),
                Number(n) => Ok(if let Some(float) = n.as_f64() {
                    libsql::Value::Real(float)
                } else if let Some(int) = n.as_i64() {
                    libsql::Value::Integer(int)
                } else {
                    unreachable!()
                }),
                Array(arr) => {
                    let blob = serde_json::to_vec(&arr)
                        .map_err(|e| FilterError::Serialization(e.to_string()))?;

                    Ok(libsql::Value::Blob(blob))
                }
                Object(obj) => {
                    let blob = serde_json::to_vec(&obj)
                        .map_err(|e| FilterError::Serialization(e.to_string()))?;

                    Ok(libsql::Value::Blob(blob))
                }
            }
        }

        for param in self.params.into_iter() {
            params.push(convert(param)?)
        }

        Ok(params)
    }
}

/// Turso libSQL vector store implementation for Rig.
///
/// This crate provides a libSQL-based vector store implementation that can be used with Rig.
/// It uses the `sqlite-vec` extension to enable vector similarity search capabilities.
/// libSQL is a fork of SQLite that's designed for the edge and cloud, developed by Turso.
///
/// # Example
/// ```rust,no_run
/// use rig::{
///     embeddings::EmbeddingsBuilder,
///     providers::openai::{Client, TEXT_EMBEDDING_ADA_002},
///     vector_store::VectorStoreIndex,
///     Embed,
/// };
/// use rig_libsql::{Column, ColumnValue, LibsqlVectorStore, LibsqlVectorStoreTable};
/// use serde::Deserialize;
///
/// #[derive(Embed, Clone, Debug, Deserialize)]
/// struct Document {
///     id: String,
///     #[embed]
///     content: String,
/// }
///
/// impl LibsqlVectorStoreTable for Document {
///     fn name() -> &'static str {
///         "documents"
///     }
///
///     fn schema() -> Vec<Column> {
///         vec![
///             Column::new("id", "TEXT PRIMARY KEY"),
///             Column::new("content", "TEXT"),
///         ]
///     }
///
///     fn id(&self) -> String {
///         self.id.clone()
///     }
///
///     fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
///         vec![
///             ("id", Box::new(self.id.clone())),
///             ("content", Box::new(self.content.clone())),
///         ]
///     }
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let db = libsql::Builder::new_local("vector_store.db").build().await?;
/// let conn = db.connect()?;
/// 
/// let openai_client = Client::new("YOUR_API_KEY");
/// let model = openai_client.embedding_model(TEXT_EMBEDDING_ADA_002);
///
/// // Initialize vector store
/// let vector_store = LibsqlVectorStore::new(conn, &model).await?;
///
/// // Create documents
/// let documents = vec![
///     Document {
///         id: "doc1".to_string(),
///         content: "Example document 1".to_string(),
///     },
///     Document {
///         id: "doc2".to_string(),
///         content: "Example document 2".to_string(),
///     },
/// ];
///
/// // Generate embeddings
/// let embeddings = EmbeddingsBuilder::new(model.clone())
///     .documents(documents)?
///     .build()
///     .await?;
///
/// // Add to vector store
/// vector_store.add_rows(embeddings).await?;
///
/// // Create index and search
/// let index = vector_store.index(model);
/// let results = index
///     .top_n::<Document>("Example query", 2)
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct LibsqlVectorIndex<E, T>
where
    E: EmbeddingModel + 'static,
    T: LibsqlVectorStoreTable + 'static,
{
    store: LibsqlVectorStore<E, T>,
    embedding_model: E,
}

impl<E, T> LibsqlVectorIndex<E, T>
where
    E: EmbeddingModel + 'static,
    T: LibsqlVectorStoreTable,
{
    pub fn new(embedding_model: E, store: LibsqlVectorStore<E, T>) -> Self {
        Self {
            store,
            embedding_model,
        }
    }
}

fn build_where_clause(
    req: &VectorSearchRequest<LibsqlSearchFilter>,
    query_vec: Vec<f32>,
) -> Result<(String, Vec<libsql::Value>), FilterError> {
    let thresh = req.threshold().unwrap_or(0.);
    let thresh = LibsqlSearchFilter::gt("e.distance".into(), thresh.into());

    let filter = req
        .filter()
        .as_ref()
        .cloned()
        .map(|filter| thresh.clone().and(filter))
        .unwrap_or(thresh);

    let where_clause = format!(
        "WHERE e.embedding MATCH ? AND k = ? AND {}",
        filter.condition
    );

    let query_vec = query_vec.into_iter().flat_map(f32::to_le_bytes).collect();
    let query_vec = libsql::Value::Blob(query_vec);
    let samples = req.samples() as u32;

    let mut params = vec![query_vec, libsql::Value::Integer(samples as i64)];
    let filter_params = filter.clone().compile_params()?;
    params.extend(filter_params);

    Ok((where_clause, params))
}

impl<E: EmbeddingModel + std::marker::Sync, T: LibsqlVectorStoreTable> VectorStoreIndex
    for LibsqlVectorIndex<E, T>
{
    type Filter = LibsqlSearchFilter;

    async fn top_n<D>(
        &self,
        req: VectorSearchRequest<LibsqlSearchFilter>,
    ) -> Result<Vec<(f64, String, D)>, VectorStoreError>
    where
        D: for<'de> Deserialize<'de>,
    {
        tracing::debug!("Finding top {} matches for query", req.samples() as usize);
        let embedding = self.embedding_model.embed_text(req.query()).await?;
        let query_vec: Vec<f32> = serialize_embedding(&embedding);
        let table_name = T::name();

        // Get all column names from LibsqlVectorStoreTable
        let columns = T::schema();
        let column_names: Vec<&str> = columns.iter().map(|column| column.name).collect();

        // Build SELECT statement with all columns
        let select_cols = column_names.join(", ");

        let (where_clause, params) = build_where_clause(&req, query_vec)?;

        let query_sql = format!(
            "SELECT d.{select_cols}, e.distance
            FROM {table_name}_embeddings e
            JOIN {table_name} d ON e.rowid = d.rowid
            {where_clause}
            ORDER BY e.distance"
        );

        let mut rows_result = self
            .store
            .conn
            .query(&query_sql, libsql::params_from_iter(params))
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        let mut top_n = Vec::new();
        
        while let Some(row) = rows_result
            .next()
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?
        {
            // Create a map of column names to values
            let mut map = serde_json::Map::new();
            for (i, col_name) in column_names.iter().enumerate() {
                let value: String = row.get(i as i32).map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;
                map.insert(col_name.to_string(), serde_json::Value::String(value));
            }
            let distance: f64 = row.get(column_names.len() as i32).map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;
            let id: String = row.get(0).map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

            match serde_json::from_value::<D>(serde_json::Value::Object(map)) {
                Ok(doc) => {
                    top_n.push((distance, id, doc));
                }
                Err(e) => {
                    debug!("Failed to deserialize document {}: {}", id, e);
                    continue;
                }
            }
        }

        debug!("Found {} potential matches", top_n.len());
        debug!("Returning {} matches", top_n.len());
        Ok(top_n)
    }

    async fn top_n_ids(
        &self,
        req: VectorSearchRequest<LibsqlSearchFilter>,
    ) -> Result<Vec<(f64, String)>, VectorStoreError> {
        tracing::debug!(
            "Finding top {} document IDs for query",
            req.samples() as usize
        );
        let embedding = self.embedding_model.embed_text(req.query()).await?;
        let query_vec = serialize_embedding(&embedding);
        let table_name = T::name();

        let (where_clause, params) = build_where_clause(&req, query_vec)?;

        let query_sql = format!(
            "SELECT d.id, e.distance
             FROM {table_name}_embeddings e
             JOIN {table_name} d ON e.rowid = d.rowid
             {where_clause}
             ORDER BY e.distance"
        );

        let mut rows_result = self
            .store
            .conn
            .query(&query_sql, libsql::params_from_iter(params))
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;

        let mut results = Vec::new();

        while let Some(row) = rows_result
            .next()
            .await
            .map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?
        {
            let distance: f64 = row.get(1).map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;
            let id: String = row.get(0).map_err(|e| VectorStoreError::DatastoreError(Box::new(e)))?;
            results.push((distance, id));
        }

        debug!("Found {} matching document IDs", results.len());
        Ok(results)
    }
}

fn serialize_embedding(embedding: &Embedding) -> Vec<f32> {
    embedding.vec.iter().map(|x| *x as f32).collect()
}

impl ColumnValue for String {
    fn to_sql_string(&self) -> String {
        self.clone()
    }

    fn column_type(&self) -> &'static str {
        "TEXT"
    }
}
