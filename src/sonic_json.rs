extern crate alloc;
use alloc::borrow::Cow;
use rmcp::{
    ErrorData,
    handler::server::tool::IntoCallToolResult,
    model::{CallToolResult, Content, IntoContents},
};
use schemars::JsonSchema;
use sonic_rs::Serialize;
pub(crate) struct SonicJson<T>(pub(crate) T);
pub(crate) struct SonicToolResult<T, E>(pub(crate) Result<T, E>);
#[expect(
    clippy::missing_trait_methods,
    reason = "JsonSchema 为未覆盖的方法提供了默认实现，这里仅代理当前类型需要的入口"
)]
impl<T: JsonSchema> JsonSchema for SonicJson<T> {
    fn schema_name() -> Cow<'static, str> {
        T::schema_name()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        T::json_schema(generator)
    }
}
impl<T: Serialize + JsonSchema + 'static> IntoCallToolResult for SonicJson<T> {
    fn into_call_tool_result(self) -> Result<CallToolResult, ErrorData> {
        let text = sonic_rs::to_string(&self.0).map_err(|error| {
            ErrorData::internal_error(
                format!("Failed to serialize structured content with sonic-rs: {error}"),
                None,
            )
        })?;
        let structured_content =
            sonic_rs::from_str::<rmcp::serde_json::Value>(&text).map_err(|error| {
                ErrorData::internal_error(
                    format!("Failed to parse structured content with sonic-rs: {error}"),
                    None,
                )
            })?;
        let mut result = CallToolResult::structured(structured_content);
        result.content = vec![Content::text(text)];
        Ok(result)
    }
}
impl<T: Serialize + JsonSchema + 'static, E: IntoContents> IntoCallToolResult
    for SonicToolResult<T, E>
{
    fn into_call_tool_result(self) -> Result<CallToolResult, ErrorData> {
        match self.0 {
            Ok(value) => SonicJson(value).into_call_tool_result(),
            Err(error) => Ok(CallToolResult::error(error.into_contents())),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::SonicJson;
    use crate::tool_types::{SearchHwndResponse, WindowEntry, WindowRect};
    use core::fmt::{Debug, Display};
    fn must_debug<T, E: Debug>(result: Result<T, E>, message: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error:?}"),
        }
    }
    fn must_display<T, E: Display>(result: Result<T, E>, message: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error}"),
        }
    }
    #[test]
    fn sonic_json_keeps_text_and_structured_content_in_sync() {
        let result = must_debug(
            rmcp::handler::server::tool::IntoCallToolResult::into_call_tool_result(SonicJson(
                SearchHwndResponse {
                    process_name: String::from("explorer"),
                    windows: vec![WindowEntry {
                        hwnd: String::from("0x1234"),
                        pid: 42,
                        title: String::from("Explorer"),
                        class_name: String::from("CabinetWClass"),
                        visible: true,
                        minimized: false,
                        rect: WindowRect {
                            left: 1,
                            top: 2,
                            right: 3,
                            bottom: 4,
                        },
                    }],
                },
            )),
            "SonicJson 应能转为工具结果",
        );
        let Some(text) = result
            .content
            .first()
            .and_then(|content| content.as_text())
            .map(|content| content.text.as_str())
        else {
            panic!("应包含文本内容");
        };
        let parsed = must_display(
            sonic_rs::from_str::<rmcp::serde_json::Value>(text),
            "文本内容应能解析为 JSON",
        );
        assert_eq!(result.structured_content, Some(parsed));
        assert_eq!(result.is_error, Some(false));
    }
}
