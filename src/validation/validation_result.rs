pub struct ValidationResult {
    pub is_valid: bool,
    pub message: Option<String>,
    pub error: Option<Box<dyn std::error::Error>>,
}

impl ValidationResult {
    pub fn new(is_valid: bool) -> Self {
        Self {
            is_valid,
            message: None,
            error: None,
        }
    }

    pub fn with_message(is_valid: bool, message: impl Into<String>) -> Self {
        Self {
            is_valid,
            message: Some(message.into()),
            error: None,
        }
    }

    pub fn with_error(is_valid: bool, message: impl Into<String>, error: impl std::error::Error + 'static) -> Self {
        Self {
            is_valid,
            message: Some(message.into()),
            error: Some(Box::new(error)),
        }
    }
}