# Media Sync Software

A Rust-based media synchronization application that allows streaming and syncing media files across multiple devices. Now includes both CLI and web-based GUI interfaces.

## Features

- **Media Server**: Host media files and stream to multiple clients
- **Media Client**: Connect to server and receive synchronized media
- **Web Interface**: Modern HTML/CSS/JS GUI for easy interaction
- **CLI Interface**: Command-line interface for programmatic usage
- **Multiple Media Types**: Support for video (MP4, AVI, MKV, MOV, WebM), audio (MP3, WAV, FLAC, OGG, AAC), and images (JPG, PNG, GIF, BMP, WebP)
- **Synchronized Playback**: All connected clients play media in sync
- **Real-time Communication**: TCP-based protocol for low-latency streaming

## Installation

1. Make sure you have Rust installed: https://rustup.rs/
2. Clone or download this project
3. Navigate to the project directory
4. Build the project:
   ```powershell
   cargo build --release
   ```

## Usage

### Web Interface (Recommended)

1. Start the web interface:
   ```powershell
   cargo run web
   ```
   Or specify a custom port:
   ```powershell
   cargo run web 3000
   ```

2. Open your browser and go to: `http://localhost:3000`

3. Use the web interface to:
   - **Server Tab**: Start a media server, select media directory, view connected clients
   - **Client Tab**: Connect to a server, browse available files, request and play media

### Command Line Interface

#### Starting a Server
```powershell
cargo run server <port> <media_directory>
```

Example:
```powershell
cargo run server 8080 "C:\Users\YourName\Videos"
```

#### Connecting as a Client
```powershell
cargo run client <server_ip:port> <client_id>
```

Example:
```powershell
cargo run client 127.0.0.1:8080 client1
```

## File Structure

```
media-sync/
├── src/
│   ├── main.rs          # Main application entry point
│   ├── gui.rs           # Native GUI implementation (egui)
│   └── web_server.rs    # Web server for HTML interface
├── index.html           # Web interface HTML
├── style.css            # Web interface styling
├── script.js            # Web interface JavaScript
├── Cargo.toml           # Rust dependencies
└── README.md            # This file
```

## Web Interface Usage

### Server Mode
1. Enter a port number (default: 8080)
2. Select or type a media directory path
3. Click "Start Server"
4. View loaded media files and connected clients
5. Stream files to all connected clients

### Client Mode
1. Enter server address (format: ip:port)
2. Enter a unique client ID
3. Click "Connect"
4. Browse available media files
5. Request files to play locally
6. Use media player controls

## Protocol

The application uses a JSON-based TCP protocol for communication:

- `Join`: Client joins server
- `RequestMediaList`: Get list of available media
- `RequestMedia`: Request specific media file
- `MediaData`: Server sends media file data
- `PlayCommand`: Synchronized play command
- `PauseCommand`: Synchronized pause command

## Supported Media Formats

- **Video**: MP4, AVI, MKV, MOV, WebM
- **Audio**: MP3, WAV, FLAC, OGG, AAC  
- **Images**: JPG/JPEG, PNG, GIF, BMP, WebP

## Dependencies

- `tokio`: Async runtime
- `serde`/`serde_json`: Serialization
- `warp`: Web server framework
- `eframe`/`egui`: Native GUI (optional)
- `base64`: Binary data encoding
- `bytes`: Byte manipulation

## Troubleshooting

### Common Issues

1. **Port already in use**: Choose a different port number
2. **Media files not loading**: Check directory permissions and file formats
3. **Client can't connect**: Verify server is running and firewall settings
4. **Web interface not loading**: Ensure HTML/CSS/JS files are in the same directory

### Firewall Configuration

Make sure to allow the application through Windows Firewall:
1. Windows Security → Firewall & network protection
2. Allow an app through firewall
3. Add the media-sync executable

## Development

To modify the web interface:
1. Edit `index.html`, `style.css`, or `script.js`
2. Restart the web server to see changes
3. The web server serves files from the current directory

To add new features:
1. Modify the Rust source code in `src/`
2. Update the web API in `web_server.rs` if needed
3. Update the JavaScript in `script.js` to use new API endpoints
4. Rebuild with `cargo build`

## License

This project is open source. Feel free to modify and distribute as needed.
