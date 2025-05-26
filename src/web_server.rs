use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::convert::Infallible;
use warp::{Filter, Reply};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;

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

#[derive(Serialize, Clone)]
pub struct ClientInfo {
    pub id: String,
    pub address: String,
    pub connected_time: String,
}

#[derive(Serialize, Clone)]
pub struct LogMessage {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub struct WebServer {
    media_server: Arc<Mutex<Option<MediaServer>>>,
    media_client: Arc<Mutex<Option<MediaClient>>>,
    loaded_files: Arc<Mutex<Vec<FileInfo>>>,
    available_files: Arc<Mutex<Vec<String>>>,
    log_messages: Arc<Mutex<Vec<LogMessage>>>,
}

impl WebServer {
    pub fn new() -> Self {
        Self {
            media_server: Arc::new(Mutex::new(None)),
            media_client: Arc::new(Mutex::new(None)),
            loaded_files: Arc::new(Mutex::new(Vec::new())),
            available_files: Arc::new(Mutex::new(Vec::new())),
            log_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_log_message(&self, level: &str, message: &str) {
        let timestamp = Utc::now().format("%H:%M:%S").to_string();
        let log_message = LogMessage {
            timestamp: timestamp.clone(),
            level: level.to_string(),
            message: message.to_string(),
        };
        
        let mut logs = self.log_messages.lock().unwrap();
        logs.push(log_message);
        
        // Keep only the last 100 log messages to prevent memory buildup
        if logs.len() > 100 {
            logs.remove(0);
        }
          // Also print to console for debugging
        println!("[{}] {}: {}", timestamp, level, message);
    }

    pub async fn start_web_server(self) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<impl Reply, warp::Rejection> {    let response = match request.command.as_str() {
        "start-server" => handle_start_server(request.params, &web_server).await,
        "stop-server" => handle_stop_server(&web_server).await,
        "get-connected-clients" => handle_get_connected_clients(&web_server).await,
        "get-logs" => handle_get_logs(&web_server).await,
        "disconnect-specific-client" => handle_disconnect_specific_client(request.params, &web_server).await,
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

    // Add log messages to the web interface
    web_server.add_log_message("INFO", &format!("Raw params: {}", params));
    web_server.add_log_message("INFO", &format!("Extracted directory: '{}'", directory));
    web_server.add_log_message("INFO", &format!("Directory length: {}", directory.len()));

    if directory.is_empty() {
        web_server.add_log_message("ERROR", "Directory path is required");
        return WebResponse {
            success: false,
            error: Some("Directory path is required".to_string()),
            data: None,
        };
    }

    // Remove any surrounding quotes if they exist
    let cleaned_directory = directory.trim_matches('"').trim();
    web_server.add_log_message("INFO", &format!("Cleaned directory: '{}'", cleaned_directory));

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
            
            // Get connected clients info (initially empty since server just started)
            let clients: Vec<ClientInfo> = Vec::new();
            
            *web_server.loaded_files.lock().unwrap() = files.clone();
            
            // Clone server for background task before storing
            let server_for_task = server.clone();
            
            // Store server reference
            *web_server.media_server.lock().unwrap() = Some(server);
            
            // Add success log messages
            web_server.add_log_message("INFO", &format!("Media server started on port {}", port));
            web_server.add_log_message("INFO", "Waiting for clients to connect...");
            web_server.add_log_message("INFO", &format!("Loaded {} media file(s)", files.len()));
            
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
                    "files": files,
                    "clients": clients
                })),
            }
        }
        Err(e) => {
            web_server.add_log_message("ERROR", &format!("Failed to load media files: {}", e));
            WebResponse {
                success: false,
                error: Some(format!("Failed to load media files: {}", e)),
                data: None,
            }
        }
    }
}

async fn handle_stop_server(web_server: &Arc<WebServer>) -> WebResponse {
    web_server.add_log_message("INFO", "Stopping media server...");
    *web_server.media_server.lock().unwrap() = None;
    web_server.loaded_files.lock().unwrap().clear();
    web_server.add_log_message("INFO", "Media server stopped successfully");

    WebResponse {
        success: true,
        error: None,
        data: None,
    }
}

async fn handle_get_connected_clients(web_server: &Arc<WebServer>) -> WebResponse {
    if let Some(server) = web_server.media_server.lock().unwrap().as_ref() {
        let connected_clients_data = server.get_connected_clients();
        let clients: Vec<ClientInfo> = connected_clients_data
            .iter()
            .map(|(id, address)| ClientInfo {
                id: id.clone(),
                address: address.clone(),
                connected_time: Utc::now().format("%H:%M:%S").to_string(),
            })
            .collect();

        WebResponse {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "clients": clients
            })),
        }
    } else {
        WebResponse {
            success: false,
            error: Some("Server is not running".to_string()),
            data: None,
        }
    }
}

async fn handle_get_logs(web_server: &Arc<WebServer>) -> WebResponse {
    let logs = web_server.log_messages.lock().unwrap().clone();
    WebResponse {
        success: true,
        error: None,
        data: Some(serde_json::json!({
            "logs": logs
        })),
    }
}

async fn handle_disconnect_specific_client(
    params: serde_json::Value,
    web_server: &Arc<WebServer>,
) -> WebResponse {
    let client_id = params["clientId"].as_str().unwrap_or("").to_string();

    if client_id.is_empty() {
        return WebResponse {
            success: false,
            error: Some("Client ID is required".to_string()),
            data: None,
        };
    }

    if let Some(server) = web_server.media_server.lock().unwrap().as_ref() {
        let disconnected = server.disconnect_client(&client_id);
        
        if disconnected {
            web_server.add_log_message("INFO", &format!("Client {} disconnected", client_id));
            WebResponse {
                success: true,
                error: None,
                data: Some(serde_json::json!({
                    "message": format!("Client {} disconnected", client_id)
                })),
            }
        } else {
            WebResponse {
                success: false,
                error: Some(format!("Client {} not found", client_id)),
                data: None,
            }
        }
    } else {
        WebResponse {
            success: false,
            error: Some("Server is not running".to_string()),
            data: None,
        }
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
