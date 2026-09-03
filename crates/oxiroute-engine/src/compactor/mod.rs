use oxiroute_core::{ChatRequest, ContentPart, MessageContent};

pub struct PromptCompactor;

impl PromptCompactor {
    pub fn compact_request(mut request: ChatRequest) -> ChatRequest {
        for msg in &mut request.messages {
            match msg.content {
                MessageContent::Text(ref mut text) => {
                    *text = Self::compress_text(text);
                }
                MessageContent::Parts(ref mut parts) => {
                    for part in parts {
                        if let ContentPart::Text { ref mut text } = part {
                            *text = Self::compress_text(text);
                        } else if let ContentPart::ToolResult { ref mut content, is_error, .. } = part {
                            // If not error, prune large redundant terminal/log lines
                            if !*is_error && content.len() > 1000 {
                                *content = Self::truncate_middle(content, 400, 400);
                            }
                        }
                    }
                }
            }
        }

        request
    }

    fn compress_text(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut consecutive_newlines = 0;

        for line in text.lines() {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                consecutive_newlines += 1;
                if consecutive_newlines <= 1 {
                    result.push('\n');
                }
            } else {
                consecutive_newlines = 0;
                result.push_str(trimmed);
                result.push('\n');
            }
        }

        if !text.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }

        result
    }

    fn truncate_middle(text: &str, head_chars: usize, tail_chars: usize) -> String {
        if text.len() <= head_chars + tail_chars + 50 {
            return text.to_string();
        }

        let head = &text[..head_chars];
        let tail = &text[text.len() - tail_chars..];
        let omitted = text.len() - head_chars - tail_chars;

        format!("{}\n\n... [RTK: {} chars omitted] ...\n\n{}", head, omitted, tail)
    }
}
