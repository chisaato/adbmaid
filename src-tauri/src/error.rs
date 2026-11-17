use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("ADB 协议错误: {0}")]
    Adb(String),

    #[error("工具链未就绪: {0}")]
    Toolchain(String),

    #[error("网络/下载错误: {0}")]
    Network(String),

    #[error("校验失败: {0}")]
    Verify(String),

    #[error("无效参数: {0}")]
    InvalidArg(String),

    #[error("未找到目标: {0}")]
    NotFound(String),

    #[error("内部错误: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
