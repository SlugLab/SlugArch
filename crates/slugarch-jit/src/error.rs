use thiserror::Error;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitErrorCode {
    Null = 1,
    StructSize = 2,
    AbiVersion = 3,
    Parse = 4,
    PolicyVersion = 5,
    TooManyInstructions = 6,
    TooManyRanges = 7,
    InvalidRange = 8,
    InvalidStride = 9,
    BudgetExceeded = 10,
    InvalidControlFlow = 11,
    Unsupported = 12,
    DigestMismatch = 13,
    Rejected = 14,
    Drop = 15,
    Timeout = 16,
    Backend = 17,
    Io = 18,
    Poisoned = 19,
    Panic = 20,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct JitError {
    code: JitErrorCode,
    message: String,
}

impl JitError {
    pub fn new(code: JitErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> JitErrorCode {
        self.code
    }
}
