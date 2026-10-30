use device_flow::DeviceCodePrompt;

/// Text telling the user where to go and which code to enter.
pub fn format_device_prompt(prompt: &DeviceCodePrompt) -> String {
    let url = prompt
        .verification_uri_complete
        .as_ref()
        .unwrap_or(&prompt.verification_uri);
    format!(
        "To authorize this application, visit:\n  {url}\nThen enter the following code: {}\n",
        prompt.user_code
    )
}

/// Text shown when the browser could not be opened automatically.
pub fn format_manual_open(url: &str) -> String {
    format!("Opening browser failed!\n\nOpen this URL in your browser:\n  {url}")
}

#[cfg(test)]
#[path = "prompt.tests.rs"]
mod tests;
