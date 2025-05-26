use std::collections::HashMap;
use std::fs;
use std::io::{Write, BufReader, BufRead};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

mod web_server;

// Protocol messages
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    // Client to Server
    Join { client_id: String },
    RequestMediaList,
    RequestMedia { filename: String },
    
    // Server to Client
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

#[derive(Clone)]
pub struct MediaServer {
    pub media_files: Arc<Mutex<HashMap<String, MediaFile>>>,
    clients: Arc<Mutex<HashMap<String, Arc<Mutex<TcpStream>>>>>,
    current_media: Arc<Mutex<Option<String>>>,
    is_playing: Arc<Mutex<bool>>,
}

impl MediaServer {
    pub fn new() -> Self {
        Self {
            media_files: Arc::new(Mutex::new(HashMap::new())),
            clients: Arc::new(Mutex::new(HashMap::new())),
            current_media: Arc::new(Mutex::new(None)),
            is_playing: Arc::new(Mutex::new(false)),
        }
    }

    pub fn load_media_path(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut media_files = self.media_files.lock().unwrap();
        
        // Debug logging
        println!("Loading media from path: '{}'", path);
        println!("Path length: {}", path.len());
        
        let path_obj = Path::new(path);
        
        // Debug path information
        println!("Path exists: {}", path_obj.exists());
        println!("Path is file: {}", path_obj.is_file());
        println!("Path is dir: {}", path_obj.is_dir());
        
        if path_obj.is_file() {
            // Handle single file
            println!("Loading single file: {}", path);
            self.load_single_file(&mut media_files, path_obj)?;
        } else if path_obj.is_dir() {
            // Handle directory
            println!("Loading directory: {}", path);
            for entry in fs::read_dir(path_obj)? {
                let entry = entry?;
                let file_path = entry.path();
                
                if file_path.is_file() {
                    self.load_single_file(&mut media_files, &file_path)?;
                }
            }
        } else {
            let error_msg = format!("Path '{}' is not a valid file or directory", path);
            println!("Error: {}", error_msg);
            return Err(error_msg.into());
        }
        
        if media_files.is_empty() {
            println!("No supported media files found in the specified path.");
        } else {
            println!("Loaded {} media file(s)", media_files.len());
        }
        
        Ok(())
    }

    fn load_single_file(&self, media_files: &mut HashMap<String, MediaFile>, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            let media_type = match ext.as_str() {
                "mp4" | "avi" | "mkv" | "mov" | "webm" => "video",
                "mp3" | "wav" | "flac" | "ogg" | "aac" => "audio",
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => "image",
                _ => return Ok(()), // Skip unsupported files
            };

            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let data = fs::read(path)?;
            
            media_files.insert(filename.clone(), MediaFile {
                filename: filename.clone(),
                data,
                media_type: media_type.to_string(),
            });
            
            println!("Loaded media file: {} ({} bytes)", filename, media_files.get(&filename).unwrap().data.len());
        }
        
        Ok(())
    }

    // This function now only plays on the HOST/SERVER machine
    fn play_media_on_host(filename: &str, data: &[u8], media_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary file for the media on the host
        let temp_file = format!("host_temp_{}", filename);
        fs::write(&temp_file, data)?;
        
        println!("Playing media on HOST: {} ({} bytes, type: {})", filename, data.len(), media_type);
        
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
            
            _ => {
                println!("Unknown media type: {}", media_type);
            }
        }
        
        Ok(())
    }

    pub fn start_server(&self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
        println!("Media server started on port {}", port);
        println!("Waiting for clients to connect...");

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let media_files = Arc::clone(&self.media_files);
                    let clients = Arc::clone(&self.clients);
                    let current_media = Arc::clone(&self.current_media);
                    let is_playing = Arc::clone(&self.is_playing);
                    
                    thread::spawn(move || {
                        Self::handle_client(stream, media_files, clients, current_media, is_playing);
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                }
            }
        }
        
        Ok(())
    }

    fn handle_client(
        stream: TcpStream,
        media_files: Arc<Mutex<HashMap<String, MediaFile>>>,
        clients: Arc<Mutex<HashMap<String, Arc<Mutex<TcpStream>>>>>,
        current_media: Arc<Mutex<Option<String>>>,
        is_playing: Arc<Mutex<bool>>,
    ) {
        let peer_addr = stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
        println!("New client connected: {}", peer_addr);
        
        let stream = Arc::new(Mutex::new(stream));
        let mut reader = BufReader::new(stream.lock().unwrap().try_clone().unwrap());
        let mut client_id = String::new();

        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    println!("Client {} disconnected", peer_addr);
                    // Remove client from clients list
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
                        );
                    } else {
                        eprintln!("Failed to parse message: {}", line);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from client {}: {}", peer_addr, e);
                    // Remove client from clients list
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
    ) {
        match message {
            Message::Join { client_id } => {
                let response = Message::Welcome { 
                    client_id: client_id.clone() 
                };
                Self::send_message(stream, &response);
                println!("Client {} joined", client_id);
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
                    
                    println!("Client requested media: {} ({} bytes)", filename, media_file.data.len());
                    
                    // Send media data to the requesting client
                    let response = Message::MediaData {
                        filename: media_file.filename.clone(),
                        data: media_file.data.clone(),
                        media_type: media_file.media_type.clone(),
                        timestamp,
                    };
                    
                    println!("Sending media data to CLIENT for: {} ({} bytes)", filename, media_file.data.len());
                    Self::send_message(stream, &response);
                    
                    // Set as current media and start playing
                    *current_media.lock().unwrap() = Some(filename.clone());
                    *is_playing.lock().unwrap() = true;
                    
                    // Play media ONLY on the server/host side
                    if let Err(e) = Self::play_media_on_host(&media_file.filename, &media_file.data, &media_file.media_type) {
                        eprintln!("Error playing media on host: {}", e);
                    } else {
                        println!("Started playing {} on HOST", filename);
                    }
                    
                    // Send play command to all OTHER clients (not the requesting one)
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

    fn send_message(stream: &Arc<Mutex<TcpStream>>, message: &Message) {
        if let Ok(data) = serde_json::to_string(message) {
            if let Ok(mut stream) = stream.lock() {
                let line = format!("{}\n", data);
                if let Err(e) = stream.write_all(line.as_bytes()) {
                    eprintln!("Error sending message: {}", e);
                    return;
                }
                if let Err(e) = stream.flush() {
                    eprintln!("Error flushing stream: {}", e);
                }
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

    pub fn play_media(&self, filename: &str) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        let command = Message::PlayCommand {
            filename: filename.to_string(),
            timestamp,
        };
        
        // Broadcast to all connected clients
        let clients = self.clients.lock().unwrap();
        for (_client_id, client_stream) in clients.iter() {
            Self::send_message(client_stream, &command);
        }
        
        *self.current_media.lock().unwrap() = Some(filename.to_string());
        *self.is_playing.lock().unwrap() = true;
        
        println!("Playing: {}", filename);
    }

    pub fn pause_media(&self) {
        let command = Message::PauseCommand;
        
        // Broadcast to all connected clients
        let clients = self.clients.lock().unwrap();
        for (_client_id, client_stream) in clients.iter() {
            Self::send_message(client_stream, &command);
        }
        
        *self.is_playing.lock().unwrap() = false;
        
        println!("Media paused");
    }
}

#[derive(Clone)]
pub struct MediaClient {
    server_addr: String,
    client_id: String,
}

impl MediaClient {
    pub fn new(server_addr: String, client_id: String) -> Self {
        Self {
            server_addr,
            client_id,
        }
    }

    pub fn connect(&self) -> Result<(), Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(&self.server_addr)?;
        println!("Connected to media server at {}", self.server_addr);

        // Send join message
        let join_msg = Message::Join {
            client_id: self.client_id.clone(),
        };
        self.send_message(&stream, &join_msg)?;

        // Handle server messages
        self.handle_server_messages(stream)?;
        
        Ok(())
    }

    fn handle_server_messages(&self, stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let stream = Arc::new(Mutex::new(stream));
        
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    println!("Server disconnected");
                    break;
                }
                Ok(_) => {
                    line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    
                    if let Ok(message) = serde_json::from_str::<Message>(&line) {
                        self.process_server_message(message, &stream)?;
                    } else {
                        eprintln!("Failed to parse server message: {}", line);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from server: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }

    fn process_server_message(&self, message: Message, stream: &Arc<Mutex<TcpStream>>) -> Result<(), Box<dyn std::error::Error>> {
        match message {
            Message::Welcome { client_id } => {
                println!("Welcome! Client ID: {}", client_id);
                
                // Request media list
                let request = Message::RequestMediaList;
                self.send_message_arc(stream, &request)?;
            }
            
            Message::MediaList { files } => {
                println!("Available media files:");
                for (i, file) in files.iter().enumerate() {
                    println!("  {}. {}", i + 1, file);
                }
                
                // Auto-request first media file for demo
                if !files.is_empty() {
                    println!("Requesting: {}", files[0]);
                    let request = Message::RequestMedia {
                        filename: files[0].clone(),
                    };
                    self.send_message_arc(stream, &request)?;
                }
            }
            
            Message::MediaData { filename, data, media_type, timestamp } => {
                println!("Received media: {} ({} bytes, type: {}, timestamp: {})", 
                         filename, data.len(), media_type, timestamp);
                
                // Play media on CLIENT device
                self.handle_media_playback(&filename, &data, &media_type, timestamp)?;
            }
            
            Message::PlayCommand { filename, timestamp } => {
                println!("Play command received for: {} at timestamp {}", filename, timestamp);
                // This client should start synchronized playback
                // (Implementation depends on your sync requirements)
            }
            
            Message::PauseCommand => {
                println!("Pause command received");
                // Implement pause functionality
            }
            
            Message::Error { message } => {
                eprintln!("Server error: {}", message);
            }
            
            _ => {}
        }
        
        Ok(())
    }

    // This function now only plays on the CLIENT machine
    fn handle_media_playback(&self, filename: &str, data: &[u8], media_type: &str, _timestamp: u64) -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary file for the media on CLIENT
        let temp_file = format!("client_temp_{}_{}", self.client_id, filename);
        fs::write(&temp_file, data)?;
        
        println!("Playing media on CLIENT {}: {} ({} bytes)", self.client_id, filename, data.len());
        
        match media_type {
            "video" => {
                println!("Playing video on CLIENT: {} ({} bytes)", filename, data.len());
                
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
            
            "audio" => {
                println!("Playing audio on CLIENT: {} ({} bytes)", filename, data.len());
                
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
            
            "image" => {
                println!("Displaying image on CLIENT: {} ({} bytes)", filename, data.len());
                
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
            
            _ => {
                println!("Unknown media type: {}", media_type);
            }
        }
        
        Ok(())
    }

    fn send_message(&self, stream: &TcpStream, message: &Message) -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = stream.try_clone()?;
        let data = serde_json::to_string(message)?;
        let line = format!("{}\n", data);
        stream.write_all(line.as_bytes())?;
        stream.flush()?;
        Ok(())
    }

    fn send_message_arc(&self, stream: &Arc<Mutex<TcpStream>>, message: &Message) -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::to_string(message)?;
        let line = format!("{}\n", data);
        let mut stream = stream.lock().unwrap();
        stream.write_all(line.as_bytes())?;
        stream.flush()?;
        Ok(())
    }
}

// Main application
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        println!("Usage:");
        println!("  {} server <port> <media_directory>", args[0]);
        println!("  {} client <server_ip:port> <client_id>", args[0]);
        println!("  {} web [port]", args[0]);
        return Ok(());
    }

    match args[1].as_str() {
        "server" => {
            if args.len() < 4 {
                println!("Usage: {} server <port> <media_directory>", args[0]);
                return Ok(());
            }
            
            let port: u16 = args[2].parse()?;
            let media_dir = &args[3];
            
            println!("Starting MEDIA SERVER - Media will play on HOST device");
            let server = MediaServer::new();
            server.load_media_path(media_dir)?;
            server.start_server(port)?;
        }
        
        "client" => {
            if args.len() < 4 {
                println!("Usage: {} client <server_ip:port> <client_id>", args[0]);
                return Ok(());
            }
            
            let server_addr = args[2].clone();
            let client_id = args[3].clone();
            
            println!("Starting MEDIA CLIENT - Media will play on CLIENT device");
            let client = MediaClient::new(server_addr, client_id);
            client.connect()?;
        }
        
        "web" => {
            println!("Starting web interface on port 3000");
            let web_server = web_server::WebServer::new();
            web_server.start_web_server().await?;
        }
        
        _ => {
            println!("Invalid command. Use 'server', 'client', or 'web'");
        }
    }
    
    Ok(())
}