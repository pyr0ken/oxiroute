use oxiroute_core::{Capabilities, ChatRequest, ContentPart, MessageContent};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequiredCapabilities {
    pub vision: bool,
    pub pdf: bool,
    pub tools: bool,
    pub thinking: bool,
}

pub struct CapabilityDetector;

impl CapabilityDetector {
    pub fn detect(request: &ChatRequest) -> RequiredCapabilities {
        let mut caps = RequiredCapabilities::default();

        if let Some(ref tools) = request.tools {
            if !tools.is_empty() {
                caps.tools = true;
            }
        }

        if request.thinking.is_some() {
            caps.thinking = true;
        }

        for msg in &request.messages {
            match msg.content {
                MessageContent::Parts(ref parts) => {
                    for part in parts {
                        match part {
                            ContentPart::ImageUrl { .. } => {
                                caps.vision = true;
                            }
                            ContentPart::ToolUse { .. } | ContentPart::ToolResult { .. } => {
                                caps.tools = true;
                            }
                            ContentPart::Thinking { .. } => {
                                caps.thinking = true;
                            }
                            _ => {}
                        }
                    }
                }
                MessageContent::Text(ref text) => {
                    if text.contains("data:image/") || text.contains(".png") || text.contains(".jpg") {
                        // Soft hint
                    }
                }
            }
        }

        caps
    }

    pub fn matches(required: &RequiredCapabilities, model_caps: &Capabilities) -> bool {
        if required.vision && !model_caps.vision {
            return false;
        }
        if required.tools && !model_caps.tools {
            return false;
        }
        if required.thinking && !model_caps.thinking {
            return false;
        }
        true
    }
}
