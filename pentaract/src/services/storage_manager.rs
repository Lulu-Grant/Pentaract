use futures::future::join_all;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{
        channels::{DownloadFileData, UploadFileData},
        encryption::{EncryptionKey, FileCipher},
        telegram_api::bot_api::TelegramBotApi,
    },
    errors::{PentaractError, PentaractResult},
    models::{
        file_chunks::{FileChunk, FileChunkReplica, FileChunkWithReplicas},
        storages::Storage,
    },
    repositories::{files::FilesRepository, storages::StoragesRepository},
    schemas::files::DownloadedChunkSchema,
};

use super::storage_workers_scheduler::StorageWorkersScheduler;

pub struct StorageManagerService<'d> {
    storages_repo: StoragesRepository<'d>,
    files_repo: FilesRepository<'d>,
    telegram_baseurl: &'d str,
    db: &'d PgPool,
    chunk_size: usize,
    rate_limit: u8,
    cipher: FileCipher,
}

impl<'d> StorageManagerService<'d> {
    pub fn new(
        db: &'d PgPool,
        telegram_baseurl: &'d str,
        rate_limit: u8,
        encryption_key: EncryptionKey,
    ) -> Self {
        let files_repo = FilesRepository::new(db);
        let storages_repo = StoragesRepository::new(db);
        let chunk_size = 20 * 1024 * 1024;
        let cipher = FileCipher::new(encryption_key);
        Self {
            storages_repo,
            files_repo,
            chunk_size,
            telegram_baseurl,
            db,
            rate_limit,
            cipher,
        }
    }

    pub async fn upload(&self, data: UploadFileData) -> PentaractResult<()> {
        // 1. getting storage
        let storage = self.storages_repo.get_by_file_id(data.file_id).await?;
        let mut target_storages = vec![storage];
        target_storages.extend(
            self.storages_repo
                .list_replicas(target_storages[0].id)
                .await?,
        );

        // 2. dividing file into chunks
        let bytes_chunks = data.file_data.chunks(self.chunk_size);

        // 3. uploading by chunks
        let futures_: Vec<_> = bytes_chunks
            .enumerate()
            .map(|(position, bytes_chunk)| {
                self.upload_chunk(&target_storages, data.file_id, position, bytes_chunk)
            })
            .collect();

        let chunks = join_all(futures_)
            .await
            .into_iter()
            .collect::<PentaractResult<Vec<_>>>()?;

        // 4. saving chunks to db
        self.files_repo.create_chunks_batch(chunks).await
    }

    async fn upload_chunk(
        &self,
        target_storages: &[Storage],
        file_id: Uuid,
        position: usize,
        bytes_chunk: &[u8],
    ) -> PentaractResult<FileChunkWithReplicas> {
        let encrypted_chunk = self.cipher.encrypt_chunk(bytes_chunk)?;
        let chunk_id = Uuid::new_v4();
        let mut replicas = Vec::with_capacity(target_storages.len());

        for storage in target_storages {
            let scheduler = StorageWorkersScheduler::new(self.db, self.rate_limit);
            let document = TelegramBotApi::new(self.telegram_baseurl, scheduler)
                .upload(&encrypted_chunk, storage.chat_id, storage.id)
                .await?;

            tracing::debug!(
                "[TELEGRAM API] uploaded chunk with file_id \"{}\" and position \"{}\" to storage \"{}\"",
                document.file_id,
                position,
                storage.id
            );

            replicas.push(FileChunkReplica::new(
                Uuid::new_v4(),
                chunk_id,
                storage.id,
                document.file_id,
            ));
        }

        Ok(FileChunkWithReplicas {
            chunk: FileChunk::new(chunk_id, file_id, position as i16),
            replicas,
        })
    }

    pub async fn download(&self, data: DownloadFileData) -> PentaractResult<Vec<u8>> {
        // 1. getting chunks
        let chunks = self.files_repo.list_chunks_of_file(data.file_id).await?;

        // 2. downloading by chunks
        let futures_: Vec<_> = chunks
            .into_iter()
            .map(|chunk| self.download_chunk(data.storage_id, chunk))
            .collect();
        let mut chunks = join_all(futures_)
            .await
            .into_iter()
            .collect::<PentaractResult<Vec<_>>>()?;

        // 3. sorting in a right positions and merging into single bytes slice
        chunks.sort_by_key(|chunk| chunk.position);
        let file = chunks.into_iter().flat_map(|chunk| chunk.data).collect();
        Ok(file)
    }

    async fn download_chunk(
        &self,
        storage_id: Uuid,
        chunk: FileChunkWithReplicas,
    ) -> PentaractResult<DownloadedChunkSchema> {
        let mut replicas = chunk.replicas;
        replicas.sort_by_key(|replica| replica.storage_id != storage_id);
        let mut last_error = None;

        for replica in replicas {
            let scheduler = StorageWorkersScheduler::new(self.db, self.rate_limit);
            let result = TelegramBotApi::new(self.telegram_baseurl, scheduler)
                .download(&replica.telegram_file_id, replica.storage_id)
                .await
                .and_then(|data| {
                    self.cipher.decrypt_chunk(&data).map(|decrypted| {
                        DownloadedChunkSchema::new(chunk.chunk.position, decrypted)
                    })
                });

            match result {
                Ok(file) => {
                    tracing::debug!(
                        "[TELEGRAM API] downloaded chunk with file_id \"{}\" and position \"{}\" from storage \"{}\"",
                        chunk.chunk.file_id,
                        chunk.chunk.position,
                        replica.storage_id
                    );
                    return Ok(file);
                }
                Err(e) => {
                    tracing::warn!(
                        "failed to download chunk \"{}\" from storage \"{}\": {e}",
                        chunk.chunk.id,
                        replica.storage_id
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| PentaractError::DoesNotExist("chunk replicas".to_owned())))
    }
}
