// Cargo.toml dependencies you'll need:
/*
[dependencies]
eframe = "0.27"
egui = "0.27"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
*/

use eframe::egui;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

// Reuse the existing protocol messages
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Join { client_id: String },
    RequestMediaList,
    RequestMedia { filename: String },
    Welcome { client_id: String },
    MediaList { files: Vec<String> },
    MediaData { 
        filename: String, 
        data: Vec<u8>, 
        media_type: String,
        timestamp: u64 
    },
    PlayCommand { 
        filename: String, 
        timestamp: u64 
    },
    PauseCommand,
    Error { message: String },
}

#[derive(Clone)]
pub struct MediaFile {
    pub filename: String,
    pub data: Vec<u8>,
    pub media_type: String,
}

// GUI State Management
#[derive(Default)]
pub struct AppState {
    // Server state
    server_running: bool,
    server_port: String,
    media_directory: String,
    loaded_media_files: Vec<String>,
    
    // Client state
    client_connected: bool,
    server_address: String,
    client_id: String,
    available_files: Vec<String>,
    
    // UI state
    current_tab: Tab,
    status_messages: Vec<String>,
    selected_media_file: Option<String>,
    
    // Background handles
    server_handle: Option<Arc<Mutex<MediaServer>>>,
    client_handle: Option<Arc<Mutex<MediaClient>>>,
}

#[derive(Default, PartialEq)]
enum Tab {
    #[default]
    Server,
    Client,
}

// Modified MediaServer for GUI integration
pub struct MediaServer {
    media_files: Arc<Mutex<HashMap<String, MediaFile>>>,
    clients: Arc<Mutex<HashMap<String, Arc<Mutex<TcpStream>>>>>,
    current_media: Arc<Mutex<Option<String>>>,
    is_playing: Arc<Mutex<bool>>,
    status_callback: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
}

impl MediaServer {
    pub fn new() -> Self {
        Self {
            media_files: Arc::new(Mutex::new(HashMap::new())),
            clients: Arc::new(Mutex::new(HashMap::new())),
            current_media: Arc::new(Mutex::new(None)),
            is_playing: Arc::new(Mutex::new(false)),
            status_callback: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_status_callback<F>(&self, callback: F) 
    where F: Fn(String) + Send + Sync + 'static {
        *self.status_callback.lock().unwrap() = Some(Box::new(callback));
    }

    fn log_status(&self, message: String) {
        println!("{}", message);
        if let Some(callback) = self.status_callback.lock().unwrap().as_ref() {
            callback(message);
        }
    }

    pub fn load_media_path(&self, path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut media_files = self.media_files.lock().unwrap();
        let mut loaded_files = Vec::new();
        let path = Path::new(path);
        
        if path.is_file() {
            if let Ok(filename) = self.load_single_file(&mut media_files, path) {
                loaded_files.push(filename);
            }
        } else if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let file_path = entry.path();
                
                if file_path.is_file() {
                    if let Ok(filename) = self.load_single_file(&mut media_files, &file_path) {
                        loaded_files.push(filename);
                    }
                }
            }
        } else {
            return Err(format!("Path '{}' is not a valid file or directory", path.display()).into());
        }
        
        if loaded_files.is_empty() {
            self.log_status("No supported media files found in the specified path.".to_string());
        } else {
            self.log_status(format!("Loaded {} media file(s)", loaded_files.len()));
        }
        
        Ok(loaded_files)
    }

    fn load_single_file(&self, media_files: &mut HashMap<String, MediaFile>, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            let media_type = match ext.as_str() {
                "mp4" | "avi" | "mkv" | "mov" | "webm" => "video",
                "mp3" | "wav" | "flac" | "ogg" | "aac" => "audio",
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => "image",
                _ => return Err("Unsupported file type".into()),
            };

            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let data = fs::read(path)?;
            
            media_files.insert(filename.clone(), MediaFile {
                filename: filename.clone(),
                data,
                media_type: media_type.to_string(),
            });
            
            self.log_status(format!("Loaded media file: {} ({} bytes)", filename, media_files.get(&filename).unwrap().data.len()));
            return Ok(filename);
        }
        
        Err("No file extension".into())
    }

    pub fn start_server(&self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
        self.log_status(format!("Media server started on port {}", port));

        let media_files = Arc::clone(&self.media_files);
        let clients = Arc::clone(&self.clients);
        let current_media = Arc::clone(&self.current_media);
        let is_playing = Arc::clone(&self.is_playing);
        let status_callback = Arc::clone(&self.status_callback);

        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let media_files = Arc::clone(&media_files);
                        let clients = Arc::clone(&clients);
                        let current_media = Arc::clone(&current_media);
                        let is_playing = Arc::clone(&is_playing);
                        let status_callback = Arc::clone(&status_callback);
                        
                        thread::spawn(move || {
                            Self::handle_client(stream, media_files, clients, current_media, is_playing, status_callback);
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }

    fn handle_client(
        stream: TcpStream,
        media_files: Arc<Mutex<HashMap<String, MediaFile>>>,
        clients: Arc<Mutex<HashMap<String, Arc<Mutex<TcpStream>>>>>,
        current_media: Arc<Mutex<Option<String>>>,
        is_playing: Arc<Mutex<bool>>,
        status_callback: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
    ) {
        let peer_addr = stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
        
        // Log status through callback
        if let Some(callback) = status_callback.lock().unwrap().as_ref() {
            callback(format!("New client connected: {}", peer_addr));
        }
        
        let stream = Arc::new(Mutex::new(stream));
        let mut reader = BufReader::new(stream.lock().unwrap().try_clone().unwrap());
        let mut client_id = String::new();

        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                        callback(format!("Client {} disconnected", peer_addr));
                    }
                    if !client_id.is_empty() {
                        clients.lock().unwrap().remove(&client_id);
                    }
                    break;
                }
                Ok(_) => {
                    line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    
                    if let Ok(message) = serde_json::from_str::<Message>(&line) {
                        if let Message::Join { client_id: id } = &message {
                            client_id = id.clone();
                            clients.lock().unwrap().insert(client_id.clone(), Arc::clone(&stream));
                        }
                        
                        Self::process_message(
                            message,
                            &stream,
                            &media_files,
                            &clients,
                            &current_media,
                            &is_playing,
                            &status_callback,
                        );
                    }
                }
                Err(_) => {
                    if !client_id.is_empty() {
                        clients.lock().unwrap().remove(&client_id);
                    }
                    break;
                }
            }
        }
    }

    fn process_message(
        message: Message,
        stream: &Arc<Mutex<TcpStream>>,
        media_files: &Arc<Mutex<HashMap<String, MediaFile>>>,
        clients: &Arc<Mutex<HashMap<String, Arc<Mutex<TcpStream>>>>>,
        current_media: &Arc<Mutex<Option<String>>>,
        is_playing: &Arc<Mutex<bool>>,
        status_callback: &Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
    ) {
        match message {
            Message::Join { client_id } => {
                let response = Message::Welcome { 
                    client_id: client_id.clone() 
                };
                Self::send_message(stream, &response);
                
                if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                    callback(format!("Client {} joined", client_id));
                }
            }
            
            Message::RequestMediaList => {
                let media_files = media_files.lock().unwrap();
                let files: Vec<String> = media_files.keys().cloned().collect();
                let response = Message::MediaList { files };
                Self::send_message(stream, &response);
            }
            
            Message::RequestMedia { filename } => {
                let media_files = media_files.lock().unwrap();
                if let Some(media_file) = media_files.get(&filename) {
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    
                    let response = Message::MediaData {
                        filename: media_file.filename.clone(),
                        data: media_file.data.clone(),
                        media_type: media_file.media_type.clone(),
                        timestamp,
                    };
                    
                    if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                        callback(format!("Sending media data for: {} ({} bytes)", filename, media_file.data.len()));
                    }
                    
                    Self::send_message(stream, &response);
                    
                    *current_media.lock().unwrap() = Some(filename.clone());
                    *is_playing.lock().unwrap() = true;
                    
                    // Play media on host
                    if let Err(e) = Self::play_media_on_host(&media_file.filename, &media_file.data, &media_file.media_type) {
                        if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                            callback(format!("Error playing media on host: {}", e));
                        }
                    }
                    
                    let play_command = Message::PlayCommand {
                        filename: filename.clone(),
                        timestamp,
                    };
                    Self::broadcast_to_others(clients, stream, &play_command);
                } else {
                    let response = Message::Error {
                        message: format!("Media file '{}' not found", filename),
                    };
                    Self::send_message(stream, &response);
                }
            }
            
            _ => {}
        }
    }

    fn play_media_on_host(filename: &str, data: &[u8], media_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = format!("host_temp_{}", filename);
        fs::write(&temp_file, data)?;
        
        match media_type {
            "video" | "audio" | "image" => {
                #[cfg(target_os = "windows")]
                {
                    std::process::Command::new("cmd")
                        .args(&["/C", "start", "", &temp_file])
                        .spawn()?;
                }
                
                #[cfg(target_os = "macos")]
                {
                    std::process::Command::new("open")
                        .arg(&temp_file)
                        .spawn()?;
                }
                
                #[cfg(target_os = "linux")]
                {
                    std::process::Command::new("xdg-open")
                        .arg(&temp_file)
                        .spawn()?;
                }
            }
            _ => {}
        }
        
        Ok(())
    }

    fn send_message(stream: &Arc<Mutex<TcpStream>>, message: &Message) {
        if let Ok(data) = serde_json::to_string(message) {
            if let Ok(mut stream) = stream.lock() {
                let line = format!("{}\n", data);
                let _ = stream.write_all(line.as_bytes());
                let _ = stream.flush();
            }
        }
    }

    fn broadcast_to_others(
        clients: &Arc<Mutex<HashMap<String, Arc<Mutex<TcpStream>>>>>,
        sender_stream: &Arc<Mutex<TcpStream>>,
        message: &Message,
    ) {
        let clients = clients.lock().unwrap();
        let sender_addr = sender_stream.lock().unwrap().peer_addr().ok();
        
        for (_client_id, client_stream) in clients.iter() {
            if let Ok(client_addr) = client_stream.lock().unwrap().peer_addr() {
                if Some(client_addr) != sender_addr {
                    Self::send_message(client_stream, message);
                }
            }
        }
    }
}

// Modified MediaClient for GUI integration
pub struct MediaClient {
    server_addr: String,
    client_id: String,
    status_callback: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
    files_callback: Arc<Mutex<Option<Box<dyn Fn(Vec<String>) + Send + Sync>>>>,
}

impl MediaClient {
    pub fn new(server_addr: String, client_id: String) -> Self {
        Self {
            server_addr,
            client_id,
            status_callback: Arc::new(Mutex::new(None)),
            files_callback: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_status_callback<F>(&self, callback: F) 
    where F: Fn(String) + Send + Sync + 'static {
        *self.status_callback.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn set_files_callback<F>(&self, callback: F) 
    where F: Fn(Vec<String>) + Send + Sync + 'static {
        *self.files_callback.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn connect(&self) -> Result<(), Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(&self.server_addr)?;
        
        if let Some(callback) = self.status_callback.lock().unwrap().as_ref() {
            callback(format!("Connected to media server at {}", self.server_addr));
        }

        let join_msg = Message::Join {
            client_id: self.client_id.clone(),
        };
        self.send_message(&stream, &join_msg)?;

        let status_callback = Arc::clone(&self.status_callback);
        let files_callback = Arc::clone(&self.files_callback);
        let client_id = self.client_id.clone();

        thread::spawn(move || {
            let _ = Self::handle_server_messages(stream, status_callback, files_callback, client_id);
        });
        
        Ok(())
    }

    fn handle_server_messages(
        stream: TcpStream, 
        status_callback: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
        files_callback: Arc<Mutex<Option<Box<dyn Fn(Vec<String>) + Send + Sync>>>>,
        client_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let stream = Arc::new(Mutex::new(stream));
        
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                        callback("Server disconnected".to_string());
                    }
                    break;
                }
                Ok(_) => {
                    line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    
                    if let Ok(message) = serde_json::from_str::<Message>(&line) {
                        Self::process_server_message(message, &stream, &status_callback, &files_callback, &client_id)?;
                    }
                }
                Err(_) => break,
            }
        }
        
        Ok(())
    }

    fn process_server_message(
        message: Message, 
        stream: &Arc<Mutex<TcpStream>>,
        status_callback: &Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
        files_callback: &Arc<Mutex<Option<Box<dyn Fn(Vec<String>) + Send + Sync>>>>,
        client_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match message {
            Message::Welcome { client_id } => {
                if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                    callback(format!("Welcome! Client ID: {}", client_id));
                }
                
                let request = Message::RequestMediaList;
                Self::send_message_arc(stream, &request)?;
            }
            
            Message::MediaList { files } => {
                if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                    callback(format!("Received {} media files from server", files.len()));
                }
                
                if let Some(callback) = files_callback.lock().unwrap().as_ref() {
                    callback(files);
                }
            }
            
            Message::MediaData { filename, data, media_type, timestamp } => {
                if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                    callback(format!("Received media: {} ({} bytes)", filename, data.len()));
                }
                
                let _ = Self::handle_media_playback(&filename, &data, &media_type, timestamp, client_id);
            }
            
            Message::PlayCommand { filename, timestamp } => {
                if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                    callback(format!("Play command received for: {} at timestamp {}", filename, timestamp));
                }
            }
            
            Message::PauseCommand => {
                if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                    callback("Pause command received".to_string());
                }
            }
            
            Message::Error { message } => {
                if let Some(callback) = status_callback.lock().unwrap().as_ref() {
                    callback(format!("Server error: {}", message));
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }

    fn handle_media_playback(filename: &str, data: &[u8], media_type: &str, _timestamp: u64, client_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = format!("temp_{}_{}", client_id, filename);
        fs::write(&temp_file, data)?;
        
        match media_type {
            "video" | "audio" | "image" => {
                #[cfg(target_os = "windows")]
                {
                    std::process::Command::new("cmd")
                        .args(&["/C", "start", "", &temp_file])
                        .spawn()?;
                }
                
                #[cfg(target_os = "macos")]
                {
                    std::process::Command::new("open")
                        .arg(&temp_file)
                        .spawn()?;
                }
                
                #[cfg(target_os = "linux")]
                {
                    std::process::Command::new("xdg-open")
                        .arg(&temp_file)
                        .spawn()?;
                }
            }
            _ => {}
        }
        
        Ok(())
    }

    fn send_message(stream: &TcpStream, message: &Message) -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = stream.try_clone()?;
        let data = serde_json::to_string(message)?;
        let line = format!("{}\n", data);
        stream.write_all(line.as_bytes())?;
        stream.flush()?;
        Ok(())
    }

    fn send_message_arc(stream: &Arc<Mutex<TcpStream>>, message: &Message) -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::to_string(message)?;
        let line = format!("{}\n", data);
        let mut stream = stream.lock().unwrap();
        stream.write_all(line.as_bytes())?;
        stream.flush()?;
        Ok(())
    }

    pub fn request_media(&self, stream: &Arc<Mutex<TcpStream>>, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let request = Message::RequestMedia {
            filename: filename.to_string(),
        };
        Self::send_message_arc(stream, &request)
    }
}

// Main GUI Application
impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Media Streaming Server & Client");
            
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Server, "Server");
                ui.selectable_value(&mut self.current_tab, Tab::Client, "Client");
            });
            
            ui.separator();
            
            match self.current_tab {
                Tab::Server => self.server_ui(ui),
                Tab::Client => self.client_ui(ui),
            }
            
            ui.separator();
            
            // Status messages
            ui.label("Status Messages:");
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    for message in &self.status_messages {
                        ui.label(message);
                    }
                });
        });
        
        // Request repaint to keep UI responsive
        ctx.request_repaint();
    }
}

impl AppState {
    fn server_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Media Server");
        
        ui.horizontal(|ui| {
            ui.label("Port:");
            ui.text_edit_singleline(&mut self.server_port);
        });
        
        ui.horizontal(|ui| {
            ui.label("Media Directory:");
            ui.text_edit_singleline(&mut self.media_directory);
            if ui.button("Browse").clicked() {
                // You can implement file dialog here
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.media_directory = path.display().to_string();
                }
            }
        });
        
        if ui.button("Start Server").clicked() && !self.server_running {
            self.start_server();
        }
        
        if ui.button("Stop Server").clicked() && self.server_running {
            self.stop_server();
        }
        
        ui.label(format!("Server Status: {}", 
            if self.server_running { "Running" } else { "Stopped" }));
        
        if !self.loaded_media_files.is_empty() {
            ui.label("Loaded Media Files:");
            for file in &self.loaded_media_files {
                ui.label(format!("• {}", file));
            }
        }
    }
    
    fn client_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Media Client");
        
        ui.horizontal(|ui| {
            ui.label("Server Address:");
            ui.text_edit_singleline(&mut self.server_address);
        });
        
        ui.horizontal(|ui| {
            ui.label("Client ID:");
            ui.text_edit_singleline(&mut self.client_id);
        });
        
        if ui.button("Connect").clicked() && !self.client_connected {
            self.connect_client();
        }
        
        if ui.button("Disconnect").clicked() && self.client_connected {
            self.disconnect_client();
        }
        
        ui.label(format!("Client Status: {}", 
            if self.client_connected { "Connected" } else { "Disconnected" }));
        
        if !self.available_files.is_empty() {
            ui.label("Available Media Files:");
            for file in &self.available_files.clone() {
                ui.horizontal(|ui| {
                    ui.label(&file);
                    if ui.button("Request").clicked() {
                        self.request_media_file(file);
                    }
                });
            }
        }
    }
    
    fn start_server(&mut self) {
        if let Ok(port) = self.server_port.parse::<u16>() {
            let server = Arc::new(Mutex::new(MediaServer::new()));
            
            // Set up status callback
            {
                let server_lock = server.lock().unwrap();
                let status_messages = self.status_messages.clone();
                server_lock.set_status_callback(move |msg| {
                    // In a real app, you'd use a channel to communicate with the main thread
                    println!("Server: {}", msg);
                });
            }
            
            // Load media files
            if let Ok(files) = server.lock().unwrap().load_media_path(&self.media_directory) {
                self.loaded_media_files = files;
            }
            
            // Start server
            let server_clone = Arc::clone(&server);
            thread::spawn(move || {
                if let Err(e) = server_clone.lock().unwrap().start_server(port) {
                    eprintln!("Server error: {}", e);
                }
            });
            
            self.server_handle = Some(server);
            self.server_running = true;
            self.status_messages.push("Server started successfully".to_string());
        } else {
            self.status_messages.push("Invalid port number".to_string());
        }
    }
    
    fn stop_server(&mut self) {
        self.server_handle = None;
        self.server_running = false;
        self.status_messages.push("Server stopped".to_string());
    }
    
    fn connect_client(&mut self) {
        if !self.server_address.is_empty() && !self.client_id.is_empty() {
            let client = Arc::new(Mutex::new(
                MediaClient::new(self.server_address.clone(), self.client_id.clone())
            ));
            
            // Set up callbacks
            {
                let client_lock = client.lock().unwrap();
                client_lock.set_status_callback(move |msg| {
                    println!("Client: {}", msg);
                });
                
                let available_files = self.available_files.clone();
                client_lock.set_files_callback(move |files| {
                    // In a real app, you'd use a channel to update the UI
                    println!("Received files: {:?}", files);
                });
            }
            
            // Connect
            let client_clone = Arc::clone(&client);
            thread::spawn(move || {
                if let Err(e) = client_clone.lock().unwrap().connect() {
                    eprintln!("Client error: {}", e);
                }
            });
            
            self.client_handle = Some(client);
            self.client_connected = true;
            self.status_messages.push("Connected to server".to_string());
        } else {
            self.status_messages.push("Please fill in server address and client ID".to_string());
        }
    }
    
    fn disconnect_client(&mut self) {
        self.client_handle = None;
        self.client_connected = false;
        self.available_files.clear();
        self.status_messages.push("Disconnected from server".to_string());
    }
    
    fn request_media_file(&mut self, filename: &str) {
        if let Some(client_handle) = &self.client_handle {
            // In a real implementation, you'd need to maintain the stream connection
            // This is a simplified version for demonstration
            self.status_messages.push(format!("Requesting media file: {}", filename));
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        ..Default::default()
    };
    
    let mut app_state = AppState::default();
    app_state.server_port = "8080".to_string();
    app_state.server_address = "127.0.0.1:8080".to_string();
    app_state.client_id = "client1".to_string();
    
    eframe::run_native(
        "Media Streaming App",
        options,
        Box::new(|_cc| Box::new(app_state)),
    )
}