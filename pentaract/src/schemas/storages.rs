use serde::{Deserialize, Serialize};

use uuid::Uuid;

use crate::{
    common::types::ChatId,
    models::storages::{Storage, StorageWithInfo},
};

#[derive(Deserialize)]
pub struct InStorageSchema {
    pub name: String,
    pub chat_id: ChatId,
}

#[derive(Serialize)]
pub struct StoragesListSchema {
    pub storages: Vec<StorageWithInfo>,
}

impl StoragesListSchema {
    pub fn new(storages: Vec<StorageWithInfo>) -> Self {
        Self { storages }
    }
}

#[derive(Deserialize)]
pub struct InStorageReplicaSchema {
    pub replica_storage_id: Uuid,
}

#[derive(Serialize)]
pub struct StorageReplicasSchema {
    pub replicas: Vec<Storage>,
}

impl StorageReplicasSchema {
    pub fn new(replicas: Vec<Storage>) -> Self {
        Self { replicas }
    }
}
