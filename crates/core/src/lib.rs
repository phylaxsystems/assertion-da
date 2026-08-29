use alloy::json_abi::JsonAbi;
use alloy::primitives::{
    B256,
    Bytes,
};
use serde::{
    Deserialize,
    Serialize,
};

///The submission payload for the DA layer.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DaSubmission {
    pub solidity_source: String,
    pub compiler_version: String,
    pub assertion_contract_name: String,
    pub constructor_args: Vec<String>,
    pub constructor_abi_signature: String,
}

///The response from the DA layer when submitting an assertion
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DaSubmissionResponse {
    pub id: B256,
    pub prover_signature: Bytes,
}

///The response from the DA layer when fetching an assertion
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DaFetchResponse {
    pub solidity_source: String,
    pub bytecode: Bytes,
    pub prover_signature: Bytes,
    pub encoded_constructor_args: Bytes,
    pub constructor_abi_signature: String,
    /// Producer-selected assertion contract. Absent on older servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion_contract_name: Option<String>,
    /// Compiler used for the stored artifact. Absent on older servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiler_version: Option<String>,
    /// Exact compiler-produced ABI. Absent only for raw or unrecoverable legacy artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<JsonAbi>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_legacy_fetch_response_without_compiler_metadata() {
        let response: DaFetchResponse = serde_json::from_value(serde_json::json!({
            "solidity_source": "contract Assertion {}",
            "bytecode": "0x6000",
            "prover_signature": "0x",
            "encoded_constructor_args": "0x",
            "constructor_abi_signature": "constructor()"
        }))
        .unwrap();

        assert!(response.assertion_contract_name.is_none());
        assert!(response.compiler_version.is_none());
        assert!(response.abi.is_none());
    }
}
