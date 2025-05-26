use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::convert::Infallible;
use warp::{Filter, Reply};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};

use crate::{MediaServer, MediaClient};

#[derive(Serialize, Deserialize, Debug)]
pub struct WebRequest {
    pub command: String,
    pub params: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WebResponse {
    pub success: bool,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Serialize, Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: usize,
    pub media_type: String,
}

pub struct WebServer {
    media_server: Arc<Mutex<Option<MediaServer>>>,
    media_client: Arc<Mutex<Option<MediaClient>>>,
    loaded_files: Arc<Mutex<Vec<FileInfo>>>,
    available_files: Arc<Mutex<Vec<String>>>,
}

impl WebServer {
    pub fn new() -> Self {
        Self {
            media_server: Arc::new(Mutex::new(None)),
            media_client: Arc::new(Mutex::new(None)),
            loaded_files: Arc::new(Mutex::new(Vec::new())),
            available_files: Arc::new(Mutex::new(Vec::new())),
        }
    }    pub async fn start_web_server(self) -> Result<(), Box<dyn std::error::Error>> {
        let web_server = Arc::new(self);
        let port = 3000; // Default port
        
        // Serve static files
        let static_files = warp::path::end()
            .and(warp::get())
            .and_then(serve_index);

        let css_route = warp::path("style.css")
            .and(warp::get())
            .and_then(serve_css);

        let js_route = warp::path("script.js")
            .and(warp::get())
            .and_then(serve_js);

        // API routes
        let api = warp::path("api")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_web_server(web_server))
            .and_then(handle_api_request);

        let routes = static_files
            .or(css_route)
            .or(js_route)
            .or(api)
            .with(warp::cors().allow_any_origin());

        println!("Web server starting on http://0.0.0.0:{}", port);
        warp::serve(routes)
            .run(([0, 0, 0, 0], port))
            .await;

        Ok(())
    }
}

fn with_web_server(web_server: Arc<WebServer>) -> impl Filter<Extract = (Arc<WebServer>,), Error = Infallible> + Clone {
    warp::any().map(move || web_server.clone())
}

async fn serve_index() -> Result<impl Reply, warp::Rejection> {
    match fs::read_to_string("index.html") {
        Ok(content) => Ok(warp::reply::with_header(
            content,
            "content-type",
            "text/html; charset=utf-8",
        )),
        Err(_) => {
            // Fallback to a basic HTML template if file doesn't exist
            let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Media Sync - File not found</title>
</head>
<body>
    <h1>Media Sync</h1>
    <p>Please make sure index.html, style.css, and script.js are in the same directory as the executable.</p>
</body>
</html>"#;
            Ok(warp::reply::with_header(
                html.to_string(),
                "content-type",
                "text/html; charset=utf-8",
            ))
        },
    }
}

async fn serve_css() -> Result<impl Reply, warp::Rejection> {
    match fs::read_to_string("style.css") {
        Ok(content) => Ok(warp::reply::with_header(
            content,
            "content-type",
            "text/css; charset=utf-8",
        )),
        Err(_) => Ok(warp::reply::with_header(
            "/* CSS file not found */".to_string(),
            "content-type",
            "text/css; charset=utf-8",
        )),
    }
}

async fn serve_js() -> Result<impl Reply, warp::Rejection> {
    match fs::read_to_string("script.js") {
        Ok(content) => Ok(warp::reply::with_header(
            content,
            "content-type",
            "application/javascript; charset=utf-8",
        )),
        Err(_) => Ok(warp::reply::with_header(
            "// JavaScript file not found".to_string(),
            "content-type",
            "application/javascript; charset=utf-8",
        )),
    }
}

async fn handle_api_request(
    request: WebRequest,
    web_server: Arc<WebServer>,
) -> Result<impl Reply, warp::Rejection> {
    let response = match request.command.as_str() {
        "start-server" => handle_start_server(request.params, &web_server).await,
        "stop-server" => handle_stop_server(&web_server).await,
        "connect-client" => handle_connect_client(request.params, &web_server).await,
        "disconnect-client" => handle_disconnect_client(&web_server).await,
        "request-media" => handle_request_media(request.params, &web_server).await,
        "stream-media" => handle_stream_media(request.params, &web_server).await,
        _ => WebResponse {
            success: false,
            error: Some("Unknown command".to_string()),
            data: None,
        },
    };

    Ok(warp::reply::json(&response))
}

async fn handle_start_server(
    params: serde_json::Value,
    web_server: &Arc<WebServer>,
) -> WebResponse {
    let port = params["port"].as_u64().unwrap_or(8080) as u16;
    let directory = params["directory"].as_str().unwrap_or("").to_string();

    // Debug logging
    println!("Raw params: {}", params);
    println!("Extracted directory: '{}'", directory);
    println!("Directory length: {}", directory.len());

    if directory.is_empty() {
        return WebResponse {
            success: false,
            error: Some("Directory path is required".to_string()),
            data: None,
        };
    }

    // Remove any surrounding quotes if they exist
    let cleaned_directory = directory.trim_matches('"').trim();
    println!("Cleaned directory: '{}'", cleaned_directory);

    let server = MediaServer::new();
    match server.load_media_path(cleaned_directory) {
        Ok(_) => {
            // Get loaded files info
            let files: Vec<FileInfo> = {
                let media_files = server.media_files.lock().unwrap();
                media_files
                    .iter()
                    .map(|(_, file)| FileInfo {
                        name: file.filename.clone(),
                        size: file.data.len(),
                        media_type: file.media_type.clone(),
                    })
                    .collect()
            };
            
            *web_server.loaded_files.lock().unwrap() = files.clone();
            
            // Clone server for background task before storing
            let server_for_task = server.clone();
            
            // Store server reference
            *web_server.media_server.lock().unwrap() = Some(server);
            
            // Start server in background
            tokio::spawn(async move {
                if let Err(e) = server_for_task.start_server(port) {
                    eprintln!("Server error: {}", e);
                }
            });

            WebResponse {
                success: true,
                error: None,
                data: Some(serde_json::json!({
                    "files": files
                })),
            }
        }
        Err(e) => WebResponse {
            success: false,
            error: Some(format!("Failed to load media files: {}", e)),
            data: None,
        },
    }
}

async fn handle_stop_server(web_server: &Arc<WebServer>) -> WebResponse {
    *web_server.media_server.lock().unwrap() = None;
    web_server.loaded_files.lock().unwrap().clear();

    WebResponse {
        success: true,
        error: None,
        data: None,
    }
}

async fn handle_connect_client(
    params: serde_json::Value,
    web_server: &Arc<WebServer>,
) -> WebResponse {
    let server_address = params["serverAddress"].as_str().unwrap_or("").to_string();
    let client_id = params["clientId"].as_str().unwrap_or("").to_string();

    if server_address.is_empty() || client_id.is_empty() {
        return WebResponse {
            success: false,
            error: Some("Server address and client ID are required".to_string()),
            data: None,
        };
    }

    let client = MediaClient::new(server_address, client_id);
    
    // In a real implementation, you'd set up proper callbacks here
    // For now, we'll simulate a successful connection
    match client.connect() {
        Ok(_) => {
            // Simulate receiving file list
            let files = vec![
                "sample_video.mp4".to_string(),
                "sample_audio.mp3".to_string(),
                "sample_image.jpg".to_string(),
            ];
            
            *web_server.available_files.lock().unwrap() = files.clone();
            *web_server.media_client.lock().unwrap() = Some(client);

            WebResponse {
                success: true,
                error: None,
                data: Some(serde_json::json!({
                    "files": files
                })),
            }
        }
        Err(e) => WebResponse {
            success: false,
            error: Some(format!("Failed to connect: {}", e)),
            data: None,
        },
    }
}

async fn handle_disconnect_client(web_server: &Arc<WebServer>) -> WebResponse {
    *web_server.media_client.lock().unwrap() = None;
    web_server.available_files.lock().unwrap().clear();

    WebResponse {
        success: true,
        error: None,
        data: None,
    }
}

async fn handle_request_media(
    params: serde_json::Value,
    _web_server: &Arc<WebServer>,
) -> WebResponse {
    let filename = params["filename"].as_str().unwrap_or("").to_string();

    if filename.is_empty() {
        return WebResponse {
            success: false,
            error: Some("Filename is required".to_string()),
            data: None,
        };
    }

    // In a real implementation, this would request the media from the server
    // For demonstration, we'll create dummy media data
    let dummy_data = vec![0u8; 1000]; // Dummy binary data
    let media_type = get_media_type_from_filename(&filename);
    
    let encoded_data = general_purpose::STANDARD.encode(&dummy_data);

    WebResponse {
        success: true,
        error: None,
        data: Some(serde_json::json!({
            "mediaData": {
                "filename": filename,
                "data": encoded_data,
                "mediaType": media_type
            }
        })),
    }
}

async fn handle_stream_media(
    params: serde_json::Value,
    _web_server: &Arc<WebServer>,
) -> WebResponse {
    let filename = params["filename"].as_str().unwrap_or("").to_string();

    if filename.is_empty() {
        return WebResponse {
            success: false,
            error: Some("Filename is required".to_string()),
            data: None,
        };
    }

    // In a real implementation, this would stream the media to all connected clients
    // For now, we'll just return success
    WebResponse {
        success: true,
        error: None,
        data: None,
    }
}

fn get_media_type_from_filename(filename: &str) -> String {
    if let Some(extension) = Path::new(filename).extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        match ext.as_str() {
            "mp4" | "avi" | "mkv" | "mov" | "webm" => "video".to_string(),
            "mp3" | "wav" | "flac" | "ogg" | "aac" => "audio".to_string(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => "image".to_string(),
            _ => "unknown".to_string(),
        }
    } else {
        "unknown".to_string()
    }
}
