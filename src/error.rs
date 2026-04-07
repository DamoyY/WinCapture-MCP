use anyhow::Error;
use rmcp::ErrorData;
pub(crate) type AppResult<T> = anyhow::Result<T>;
pub(crate) fn to_mcp_error(error: &Error) -> ErrorData {
    tracing::error!("{error:#}");
    ErrorData::internal_error(error.to_string(), None)
}
