use anyhow::Error;
pub(crate) type AppResult<T> = anyhow::Result<T>;
pub(crate) fn to_tool_error(error: &Error) -> String {
    tracing::error!("{error:#}");
    error.to_string()
}
