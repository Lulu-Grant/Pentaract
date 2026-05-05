use crate::common::types::Position;

#[derive(Debug, sqlx::FromRow)]
pub struct FileChunk {
    pub id: uuid::Uuid,
    pub file_id: uuid::Uuid,
    pub position: Position,
}

impl FileChunk {
    pub fn new(id: uuid::Uuid, file_id: uuid::Uuid, position: Position) -> Self {
        Self {
            id,
            file_id,
            position,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct FileChunkReplica {
    pub id: uuid::Uuid,
    pub chunk_id: uuid::Uuid,
    pub storage_id: uuid::Uuid,
    pub telegram_file_id: String,
}

impl FileChunkReplica {
    pub fn new(
        id: uuid::Uuid,
        chunk_id: uuid::Uuid,
        storage_id: uuid::Uuid,
        telegram_file_id: String,
    ) -> Self {
        Self {
            id,
            chunk_id,
            storage_id,
            telegram_file_id,
        }
    }
}

#[derive(Debug)]
pub struct FileChunkWithReplicas {
    pub chunk: FileChunk,
    pub replicas: Vec<FileChunkReplica>,
}
