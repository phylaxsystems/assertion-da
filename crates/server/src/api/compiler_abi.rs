use std::sync::Arc;

use alloy::{
    json_abi::JsonAbi,
    primitives::B256,
};
use anyhow::Result;
use bollard::Docker;
use serde::{
    Deserialize,
    Serialize,
};
use tracing::{
    info,
    warn,
};

use crate::api::{
    db,
    source_compilation::{
        compile_solidity,
        is_supported_compiler_version,
        MAX_COMPILER_ABI_BYTES,
    },
    types::DbRequestSender,
};

const STORAGE_KEY_PREFIX: &[u8] = b"assertion-compiler-abi:";
const SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct StoredCompilerAbi {
    schema_version: u8,
    abi: JsonAbi,
}

enum CompilerAbiLookup {
    Missing,
    Available(JsonAbi),
    Unsupported,
}

/// Producer data required to recover compiler metadata for a legacy assertion.
pub struct LegacyCompilerArtifact<'a> {
    pub contract_name: &'a str,
    pub compiler_version: &'a str,
    pub source: &'a str,
    pub deployment_bytecode: &'a [u8],
}

/// Stores the compiler ABI in an additive sidecar, leaving the legacy assertion
/// record unchanged for old binaries and rollback deployments.
pub async fn store(db: &DbRequestSender, id: B256, abi: JsonAbi) -> Result<()> {
    db::insert_metadata(db, storage_key(id), serialize(abi)?).await
}

/// Resolves a stored ABI or lazily reconstructs a missing legacy sidecar.
/// Unknown future sidecar versions are never overwritten.
pub async fn resolve(
    db: &DbRequestSender,
    id: B256,
    legacy: LegacyCompilerArtifact<'_>,
    docker: Arc<Docker>,
) -> Result<Option<JsonAbi>> {
    match lookup(db, id).await? {
        CompilerAbiLookup::Available(abi) => Ok(Some(abi)),
        CompilerAbiLookup::Unsupported => Ok(None),
        CompilerAbiLookup::Missing => Ok(backfill(db, id, legacy, docker).await),
    }
}

async fn lookup(db: &DbRequestSender, id: B256) -> Result<CompilerAbiLookup> {
    let Some(value) = db::get_value(db, storage_key(id)).await? else {
        return Ok(CompilerAbiLookup::Missing);
    };

    let stored: StoredCompilerAbi = match serde_json::from_slice(&value) {
        Ok(stored) => stored,
        Err(error) => {
            warn!(?id, %error, "Ignoring invalid compiler ABI sidecar");
            return Ok(CompilerAbiLookup::Unsupported);
        }
    };
    if stored.schema_version != SCHEMA_VERSION {
        warn!(
            ?id,
            schema_version = stored.schema_version,
            "Ignoring unsupported compiler ABI sidecar"
        );
        return Ok(CompilerAbiLookup::Unsupported);
    }
    if serde_json::to_vec(&stored.abi)?.len() > MAX_COMPILER_ABI_BYTES {
        warn!(?id, "Ignoring oversized compiler ABI sidecar");
        return Ok(CompilerAbiLookup::Unsupported);
    }
    Ok(CompilerAbiLookup::Available(stored.abi))
}

async fn backfill(
    db: &DbRequestSender,
    id: B256,
    legacy: LegacyCompilerArtifact<'_>,
    docker: Arc<Docker>,
) -> Option<JsonAbi> {
    if legacy.source.is_empty() || !is_supported_compiler_version(legacy.compiler_version) {
        return None;
    }

    let artifact = match compile_solidity(
        legacy.contract_name,
        legacy.source,
        legacy.compiler_version,
        docker,
    )
    .await
    {
        Ok(artifact) => artifact,
        Err(error) => {
            warn!(?id, %error, "Could not reconstruct compiler ABI for legacy assertion");
            return None;
        }
    };

    if !deployment_matches_compilation(legacy.deployment_bytecode, &artifact.bytecode) {
        warn!(
            ?id,
            "Refusing compiler ABI backfill because bytecode does not match stored assertion"
        );
        return None;
    }

    let serialized = match serialize(artifact.abi.clone()) {
        Ok(serialized) => serialized,
        Err(error) => {
            warn!(?id, %error, "Could not serialize compiler ABI backfill");
            return None;
        }
    };
    if let Err(error) = db::insert_metadata(db, storage_key(id), serialized).await {
        warn!(?id, %error, "Could not persist compiler ABI backfill");
    } else {
        info!(?id, "Persisted compiler ABI sidecar for legacy assertion");
    }

    Some(artifact.abi)
}

fn storage_key(id: B256) -> Vec<u8> {
    let mut key = Vec::with_capacity(STORAGE_KEY_PREFIX.len() + id.len());
    key.extend_from_slice(STORAGE_KEY_PREFIX);
    key.extend_from_slice(id.as_slice());
    key
}

fn serialize(abi: JsonAbi) -> Result<Vec<u8>> {
    let abi_size = serde_json::to_vec(&abi)?.len();
    anyhow::ensure!(
        abi_size <= MAX_COMPILER_ABI_BYTES,
        "compiler ABI exceeds {MAX_COMPILER_ABI_BYTES} bytes"
    );
    Ok(serde_json::to_vec(&StoredCompilerAbi {
        schema_version: SCHEMA_VERSION,
        abi,
    })?)
}

fn deployment_matches_compilation(deployment: &[u8], compiled_bytecode: &[u8]) -> bool {
    !compiled_bytecode.is_empty() && deployment.starts_with(compiled_bytecode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn test_abi() -> JsonAbi {
        serde_json::from_value(json!([
            {
                "type": "function",
                "name": "check",
                "inputs": [],
                "outputs": [],
                "stateMutability": "nonpayable"
            }
        ]))
        .unwrap()
    }

    #[test]
    fn sidecar_is_versioned_and_uses_alloy_json_abi() {
        let serialized = serialize(test_abi()).unwrap();
        let stored: StoredCompilerAbi = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(stored.schema_version, SCHEMA_VERSION);
        assert!(stored.abi.functions.contains_key("check"));
    }

    #[test]
    fn rejects_oversized_abi() {
        let oversized: JsonAbi = serde_json::from_value(json!([
            {
                "type": "function",
                "name": "x".repeat(MAX_COMPILER_ABI_BYTES),
                "inputs": [],
                "outputs": [],
                "stateMutability": "nonpayable"
            }
        ]))
        .unwrap();

        assert!(serialize(oversized).is_err());
    }

    #[test]
    fn backfill_requires_matching_nonempty_bytecode_prefix() {
        assert!(deployment_matches_compilation(
            &[0x60, 0x01, 0xaa, 0xbb],
            &[0x60, 0x01]
        ));
        assert!(deployment_matches_compilation(&[0x60, 0x01], &[0x60, 0x01]));
        assert!(!deployment_matches_compilation(
            &[0x60, 0x02],
            &[0x60, 0x01]
        ));
        assert!(!deployment_matches_compilation(&[0x60, 0x01], &[]));
    }

    #[tokio::test]
    async fn stores_and_loads_sidecar_through_db_port() {
        let database = sled::Config::tmp().unwrap().open().unwrap();
        let (sender, receiver) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let listener = tokio::spawn(db::listen_for_db(receiver, database, cancel.clone()));
        let id = B256::repeat_byte(0x11);

        store(&sender, id, test_abi()).await.unwrap();
        let result = lookup(&sender, id).await.unwrap();

        assert!(matches!(
            result,
            CompilerAbiLookup::Available(abi) if abi.functions.contains_key("check")
        ));
        cancel.cancel();
        listener.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn preserves_unknown_future_sidecar_version() {
        let database = sled::Config::tmp().unwrap().open().unwrap();
        let (sender, receiver) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let listener = tokio::spawn(db::listen_for_db(receiver, database, cancel.clone()));
        let id = B256::repeat_byte(0x22);
        let future_sidecar = serde_json::to_vec(&json!({
            "schema_version": SCHEMA_VERSION + 1,
            "abi": []
        }))
        .unwrap();
        db::insert_metadata(&sender, storage_key(id), future_sidecar)
            .await
            .unwrap();

        assert!(matches!(
            lookup(&sender, id).await.unwrap(),
            CompilerAbiLookup::Unsupported
        ));
        cancel.cancel();
        listener.await.unwrap().unwrap();
    }
}
