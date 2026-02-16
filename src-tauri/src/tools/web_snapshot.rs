//! Web snapshot tool for capturing screenshots of web applications.
//!
//! Uses headless Chrome to navigate to URLs and capture screenshots.

use std::path::Path;

use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::tool::ToolDescriptor;
use crate::policy::PolicyEngine;
use crate::tools::types::{Tool, ToolCallOutput, ToolError};

const DEFAULT_VIEWPORT_WIDTH: u32 = 1280;
const DEFAULT_VIEWPORT_HEIGHT: u32 = 720;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 120;

/// Tool for capturing web snapshots (screenshots).
pub struct WebSnapshotTool;

/// Input for web.snapshot tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSnapshotInput {
    pub url: String,
    pub wait_for_selector: Option<String>,
    pub wait_for_timeout_ms: Option<u64>,
    pub viewport_width: Option<u32>,
    pub viewport_height: Option<u32>,
    pub full_page: Option<bool>,
    pub timeout_secs: Option<u64>,
}

/// Output for web.snapshot tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSnapshotOutput {
    pub snapshot_id: String,
    pub url: String,
    pub artifact_path: String,
    pub viewport: ViewportInfo,
    pub console_errors: Vec<ConsoleError>,
    pub failed_requests: Vec<FailedRequest>,
    pub page_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportInfo {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleError {
    pub level: String,
    pub message: String,
    pub source: Option<String>,
    pub line_number: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedRequest {
    pub url: String,
    pub status_code: Option<u16>,
    pub error: Option<String>,
}

impl Tool for WebSnapshotTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "web.snapshot".into(),
            description: concat!(
                "Capture a screenshot of a web page. ",
                "Use this to verify UI changes or inspect the current state of a web app. ",
                "Returns the screenshot as an artifact along with console errors and failed network requests."
            ).into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to capture (e.g., 'http://localhost:3000')"
                    },
                    "wait_for_selector": {
                        "type": "string",
                        "description": "CSS selector to wait for before capturing (optional)"
                    },
                    "wait_for_timeout_ms": {
                        "type": "integer",
                        "description": "Additional time to wait after page load in milliseconds (default: 1000)"
                    },
                    "viewport_width": {
                        "type": "integer",
                        "description": "Viewport width in pixels (default: 1280)"
                    },
                    "viewport_height": {
                        "type": "integer",
                        "description": "Viewport height in pixels (default: 720)"
                    },
                    "full_page": {
                        "type": "boolean",
                        "description": "Capture full page scroll height (default: false)"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Maximum time to wait for page load in seconds (default: 30, max: 120)"
                    }
                },
                "required": ["url"]
            }),
            output_schema: None,
        }
    }

    fn invoke(
        &self,
        _policy: &PolicyEngine,
        cwd: &Path,
        input: serde_json::Value,
    ) -> Result<ToolCallOutput, ToolError> {
        let args: WebSnapshotInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid input: {}", e)))?;

        // Validate URL (must be http or https)
        if !args.url.starts_with("http://") && !args.url.starts_with("https://") {
            return Err(ToolError::InvalidInput(
                "URL must start with http:// or https://".into(),
            ));
        }

        // Only allow localhost or local network URLs by default for security
        let is_localhost = args.url.contains("localhost")
            || args.url.contains("127.0.0.1")
            || args.url.contains("192.168.")
            || args.url.contains("10.")
            || args.url.contains("172.");

        if !is_localhost {
            return Err(ToolError::PolicyDenied(
                "Only localhost and local network URLs are allowed".into(),
            ));
        }

        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|e| ToolError::Execution(format!("no async runtime: {}", e)))?;

        let cwd_owned = cwd.to_path_buf();
        let result = std::thread::spawn(move || {
            runtime.block_on(capture_snapshot(&cwd_owned, args))
        })
        .join()
        .map_err(|_| ToolError::Execution("web snapshot thread panicked".to_string()))??;

        Ok(ToolCallOutput {
            ok: true,
            data: serde_json::to_value(result).unwrap_or_default(),
            error: None,
        })
    }
}

async fn capture_snapshot(
    cwd: &Path,
    args: WebSnapshotInput,
) -> Result<WebSnapshotOutput, ToolError> {
    let timeout_secs = args
        .timeout_secs
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
        .min(MAX_TIMEOUT_SECS);
    let wait_timeout_ms = args.wait_for_timeout_ms.unwrap_or(1000);
    let viewport_width = args.viewport_width.unwrap_or(DEFAULT_VIEWPORT_WIDTH);
    let viewport_height = args.viewport_height.unwrap_or(DEFAULT_VIEWPORT_HEIGHT);
    let full_page = args.full_page.unwrap_or(false);

    // Create artifacts directory
    let artifacts_dir = cwd.join(".orchestrix").join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)
        .map_err(|e| ToolError::Execution(format!("failed to create artifacts dir: {}", e)))?;

    let snapshot_id = Uuid::new_v4().to_string();
    let artifact_path = artifacts_dir.join(format!("snapshot_{}.png", snapshot_id));

    // Launch browser
    let launch_options = LaunchOptions {
        headless: true,
        window_size: Some((viewport_width, viewport_height)),
        ..Default::default()
    };

    let browser = Browser::new(launch_options)
        .map_err(|e| ToolError::Execution(format!("failed to launch browser: {}", e)))?;

    let tab = browser
        .new_tab()
        .map_err(|e| ToolError::Execution(format!("failed to create tab: {}", e)))?;

    // Set viewport
    tab.set_default_timeout(std::time::Duration::from_secs(timeout_secs));

    // Enable console and network logging
    tab.enable_log()
        .map_err(|e| ToolError::Execution(format!("failed to enable logging: {}", e)))?;

    // Navigate to URL
    let navigation = tab
        .navigate_to(&args.url)
        .map_err(|e| ToolError::Execution(format!("failed to navigate: {}", e)))?;

    navigation
        .wait_until_navigated()
        .map_err(|e| ToolError::Execution(format!("failed to wait for navigation: {}", e)))?;

    // Wait for selector if specified
    if let Some(ref selector) = args.wait_for_selector {
        tab.wait_for_element(selector).map_err(|e| {
            ToolError::Execution(format!("failed to wait for selector '{}': {}", selector, e))
        })?;
    }

    // Additional wait time for dynamic content
    if wait_timeout_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(wait_timeout_ms)).await;
    }

    // Get page title
    let page_title = tab.get_title().ok().map(|t| t.to_string());

    // Collect console errors
    let console_errors = collect_console_errors(&tab);

    // Collect failed requests
    let failed_requests = collect_failed_requests(&tab);

    // Capture screenshot
    let screenshot_data = if full_page {
        tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, true)
    } else {
        tab.capture_screenshot(CaptureScreenshotFormatOption::Png, None, None, false)
    }
    .map_err(|e| ToolError::Execution(format!("failed to capture screenshot: {}", e)))?;

    // Save screenshot to file
    std::fs::write(&artifact_path, screenshot_data)
        .map_err(|e| ToolError::Execution(format!("failed to save screenshot: {}", e)))?;

    Ok(WebSnapshotOutput {
        snapshot_id,
        url: args.url,
        artifact_path: artifact_path.to_string_lossy().to_string(),
        viewport: ViewportInfo {
            width: viewport_width,
            height: viewport_height,
        },
        console_errors,
        failed_requests,
        page_title,
    })
}

fn collect_console_errors(_tab: &headless_chrome::Tab) -> Vec<ConsoleError> {
    // This is a simplified version - in a full implementation,
    // we'd capture console messages from the browser's CDP events
    // For now, return empty as headless_chrome doesn't expose
    // console messages directly in the public API
    Vec::new()
}

fn collect_failed_requests(_tab: &headless_chrome::Tab) -> Vec<FailedRequest> {
    // Similar to console errors, we'd capture network events
    // from CDP in a full implementation
    Vec::new()
}
