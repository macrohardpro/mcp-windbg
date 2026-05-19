use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
};
use pulldown_cmark::{html, Options, Parser};
use tracing::{debug, info};

use crate::http::error::HttpError;

use super::AppState;

/// Handle report rendering (HTML)
pub async fn handle_report(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, HttpError> {
    info!("Report requested for session {}", session_id);
    
    // Get workspace path
    let workspace = state
        .session_manager
        .get_workspace(&session_id)
        .await
        .ok_or_else(|| HttpError::SessionNotFound(session_id.clone()))?;
    
    // Read analysis output
    let report_path = workspace.join("analysis_output.txt");
    
    if !report_path.exists() {
        return Err(HttpError::SessionNotFound(format!(
            "Report not found for session {}",
            session_id
        )));
    }
    
    let markdown = tokio::fs::read_to_string(&report_path)
        .await
        .map_err(|e| HttpError::Internal(format!("Failed to read report: {}", e)))?;
    
    debug!("Read report ({} bytes)", markdown.len());
    
    // Render markdown to HTML
    let html = render_markdown_to_html(&markdown)?;
    
    Ok(Html(html))
}

/// Handle report download (raw Markdown)
pub async fn handle_download(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, HttpError> {
    info!("Report download requested for session {}", session_id);
    
    // Get workspace path
    let workspace = state
        .session_manager
        .get_workspace(&session_id)
        .await
        .ok_or_else(|| HttpError::SessionNotFound(session_id.clone()))?;
    
    // Read analysis output
    let report_path = workspace.join("analysis_output.txt");
    
    if !report_path.exists() {
        return Err(HttpError::SessionNotFound(format!(
            "Report not found for session {}",
            session_id
        )));
    }
    
    let markdown = tokio::fs::read_to_string(&report_path)
        .await
        .map_err(|e| HttpError::Internal(format!("Failed to read report: {}", e)))?;
    
    debug!("Serving report download ({} bytes)", markdown.len());
    
    // Return as downloadable file
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"analysis_{}.md\"", session_id),
        )
        .body(markdown)
        .map_err(|e| HttpError::Internal(format!("Failed to build response: {}", e)))?;
    
    Ok(response)
}

/// Render Markdown to HTML with syntax highlighting
fn render_markdown_to_html(markdown: &str) -> Result<String, HttpError> {
    // Parse markdown
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    
    let parser = Parser::new_ext(markdown, options);
    
    // Convert to HTML
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    
    // Wrap in HTML template with GitHub-style CSS and tab navigation
    let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>崩溃转储分析报告</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", "微软雅黑", Helvetica, Arial, sans-serif;
            line-height: 1.6;
            color: #24292e;
            background-color: #ffffff;
            max-width: 980px;
            margin: 0 auto;
            padding: 20px;
        }}
        .header {{
            background: linear-gradient(135deg, #24292e 0%, #3a3f44 100%);
            color: #ffffff;
            padding: 24px 28px;
            margin: -20px -20px 0 -20px;
            border-radius: 6px 6px 0 0;
        }}
        .header h1 {{
            margin: 0;
            border: none;
            color: #ffffff;
            font-size: 1.75em;
        }}
        .header p {{
            margin: 6px 0 0 0;
            opacity: 0.75;
            font-size: 0.9em;
        }}
        /* Tab navigation */
        .tab-bar {{
            display: flex;
            gap: 0;
            margin: 0 -20px;
            padding: 0 20px;
            background: #f6f8fa;
            border-bottom: 1px solid #e1e4e8;
            position: sticky;
            top: 0;
            z-index: 10;
        }}
        .tab-btn {{
            padding: 12px 24px;
            border: none;
            background: transparent;
            cursor: pointer;
            font-size: 0.95em;
            font-family: inherit;
            color: #586069;
            border-bottom: 3px solid transparent;
            transition: all 0.2s ease;
            outline: none;
        }}
        .tab-btn:hover {{
            color: #24292e;
            background: rgba(0,0,0,0.03);
        }}
        .tab-btn.active {{
            color: #0366d6;
            border-bottom-color: #0366d6;
            font-weight: 600;
        }}
        /* View containers */
        .report-view {{
            display: none;
        }}
        .report-view.active {{
            display: block;
        }}
        /* Content styles */
        .content {{
            padding-top: 20px;
        }}
        h1, h2, h3, h4, h5, h6 {{
            margin-top: 24px;
            margin-bottom: 16px;
            font-weight: 600;
            line-height: 1.25;
        }}
        h1 {{
            font-size: 2em;
            border-bottom: 1px solid #eaecef;
            padding-bottom: 0.3em;
        }}
        h2 {{
            font-size: 1.5em;
            border-bottom: 1px solid #eaecef;
            padding-bottom: 0.3em;
        }}
        h3 {{
            font-size: 1.25em;
        }}
        code {{
            background-color: rgba(27, 31, 35, 0.05);
            border-radius: 3px;
            font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
            font-size: 85%;
            margin: 0;
            padding: 0.2em 0.4em;
        }}
        pre {{
            background-color: #f6f8fa;
            border-radius: 3px;
            font-size: 85%;
            line-height: 1.45;
            overflow: auto;
            padding: 16px;
        }}
        pre code {{
            background-color: transparent;
            border: 0;
            display: inline;
            line-height: inherit;
            margin: 0;
            overflow: visible;
            padding: 0;
            word-wrap: normal;
        }}
        table {{
            border-collapse: collapse;
            border-spacing: 0;
            width: 100%;
            margin-top: 0;
            margin-bottom: 16px;
        }}
        table th {{
            font-weight: 600;
            padding: 6px 13px;
            border: 1px solid #dfe2e5;
            background-color: #f6f8fa;
        }}
        table td {{
            padding: 6px 13px;
            border: 1px solid #dfe2e5;
        }}
        table tr {{
            background-color: #ffffff;
            border-top: 1px solid #c6cbd1;
        }}
        table tr:nth-child(2n) {{
            background-color: #f6f8fa;
        }}
        blockquote {{
            border-left: 4px solid #dfe2e5;
            color: #6a737d;
            padding: 0 1em;
            margin: 0;
        }}
        a {{
            color: #0366d6;
            text-decoration: none;
        }}
        a:hover {{
            text-decoration: underline;
        }}
        ul, ol {{
            padding-left: 2em;
            margin-top: 0;
            margin-bottom: 16px;
        }}
        li + li {{
            margin-top: 0.25em;
        }}
        hr {{
            height: 0.25em;
            padding: 0;
            margin: 24px 0;
            background-color: #e1e4e8;
            border: 0;
        }}
        .footer {{
            margin-top: 40px;
            padding-top: 20px;
            border-top: 1px solid #eaecef;
            color: #6a737d;
            font-size: 0.9em;
            text-align: center;
        }}
        .badge {{
            display: inline-block;
            padding: 2px 8px;
            border-radius: 12px;
            font-size: 0.75em;
            font-weight: 600;
            margin-left: 8px;
        }}
        .badge-dev {{
            background: #e3f2fd;
            color: #1565c0;
        }}
        .badge-support {{
            background: #e8f5e9;
            color: #2e7d32;
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>崩溃转储分析报告</h1>
        <p>由 mcp-windbg-rs 自动生成</p>
    </div>
    <div class="tab-bar">
        <button class="tab-btn active" onclick="switchTab('dev')">研发视图 <span class="badge badge-dev">技术</span></button>
        <button class="tab-btn" onclick="switchTab('support')">售后视图 <span class="badge badge-support">支持</span></button>
    </div>
    <div class="content">
        <div class="report-view active" id="dev-view">
            {}
        </div>
        <div class="report-view" id="support-view">
            <div id="support-content"></div>
        </div>
    </div>
    <div class="footer">
        <p>mcp-windbg-rs &mdash; Windows 崩溃转储智能分析平台</p>
    </div>
    <script>
        // Store the full HTML for tab switching
        var fullHtml = document.getElementById('dev-view').innerHTML;

        function extractSupportContent() {{
            var tmp = document.createElement('div');
            tmp.innerHTML = fullHtml;

            // Find the h2 that starts the support section
            var h2s = tmp.querySelectorAll('h2');
            var supportH2 = null;
            for (var i = 0; i < h2s.length; i++) {{
                if (h2s[i].textContent.includes('售后支持报告')) {{
                    supportH2 = h2s[i];
                    break;
                }}
            }}

            if (!supportH2) {{
                // No support section found — show a message
                var p = document.createElement('p');
                p.textContent = '报告尚未生成售后支持部分的內容。请等待分析完成后刷新页面。';
                p.style.cssText = 'padding: 40px; text-align: center; color: #666;';
                return p.outerHTML;
            }}

            // Collect supportH2 and all elements after it
            var parts = [];
            var el = supportH2;
            while (el) {{
                parts.push(el.outerHTML);
                el = el.nextElementSibling;
            }}
            return parts.join('');
        }}

        function switchTab(view) {{
            var devView = document.getElementById('dev-view');
            var supportView = document.getElementById('support-view');
            var buttons = document.querySelectorAll('.tab-btn');

            buttons.forEach(function(btn) {{ btn.classList.remove('active'); }});

            if (view === 'dev') {{
                devView.classList.add('active');
                supportView.classList.remove('active');
                buttons[0].classList.add('active');
            }} else {{
                devView.classList.remove('active');
                supportView.classList.add('active');
                buttons[1].classList.add('active');
                // Extract support content on first switch
                if (!document.getElementById('support-content').innerHTML) {{
                    document.getElementById('support-content').innerHTML = extractSupportContent();
                }}
            }}
        }}

        // Pre-extract support content
        document.getElementById('support-content').innerHTML = extractSupportContent();
    </script>
</body>
</html>"#,
        html_output
    );
    
    Ok(full_html)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_render_simple_markdown() {
        let markdown = "# Hello\n\nThis is a **test**.";
        let html = render_markdown_to_html(markdown).unwrap();
        
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>test</strong>"));
        assert!(html.contains("<!DOCTYPE html>"));
    }
    
    #[test]
    fn test_render_code_block() {
        let markdown = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let html = render_markdown_to_html(markdown).unwrap();
        
        assert!(html.contains("<pre>"));
        assert!(html.contains("fn main()"));
    }
    
    #[test]
    fn test_render_table() {
        let markdown = "| Column 1 | Column 2 |\n|----------|----------|\n| Value 1  | Value 2  |";
        let html = render_markdown_to_html(markdown).unwrap();
        
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>"));
        assert!(html.contains("<td>"));
    }
    
    #[test]
    fn test_render_empty_markdown() {
        let markdown = "";
        let html = render_markdown_to_html(markdown).unwrap();

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("崩溃转储分析报告"));
    }
}
