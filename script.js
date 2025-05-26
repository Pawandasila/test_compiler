// Media Sync Application JavaScript

class MediaSyncApp {
    constructor() {
        this.serverRunning = false;
        this.clientConnected = false;
        this.ws = null;
        this.loadedFiles = [];
        this.availableFiles = [];
        this.connectedClients = [];
        
        this.initializeElements();
        this.setupEventListeners();
        this.setupTabSwitching();
        this.logMessage('Application initialized', 'info');
    }

    initializeElements() {
        // Server elements
        this.serverPortInput = document.getElementById('server-port');
        this.mediaDirectoryInput = document.getElementById('media-directory');
        this.startServerBtn = document.getElementById('start-server');
        this.stopServerBtn = document.getElementById('stop-server');
        this.serverStatusDot = document.getElementById('server-status-dot');
        this.serverStatusText = document.getElementById('server-status-text');
        this.loadedFilesContainer = document.getElementById('loaded-files');
        this.connectedClientsContainer = document.getElementById('connected-clients');
        this.browseDirectoryBtn = document.getElementById('browse-directory');

        // Client elements
        this.serverAddressInput = document.getElementById('server-address');
        this.clientIdInput = document.getElementById('client-id');
        this.connectClientBtn = document.getElementById('connect-client');
        this.disconnectClientBtn = document.getElementById('disconnect-client');
        this.clientStatusDot = document.getElementById('client-status-dot');
        this.clientStatusText = document.getElementById('client-status-text');
        this.availableFilesContainer = document.getElementById('available-files');
        this.mediaPlayerContainer = document.getElementById('media-player');

        // Media player controls
        this.playBtn = document.getElementById('play-btn');
        this.pauseBtn = document.getElementById('pause-btn');
        this.stopBtn = document.getElementById('stop-btn');

        // Status log
        this.statusLog = document.getElementById('status-log');
        this.clearLogBtn = document.getElementById('clear-log');        // Hidden file inputs
        this.fileInput = document.getElementById('file-input');
        this.fileInputDirectory = document.getElementById('file-input-directory');
        this.fileInputFiles = document.getElementById('file-input-files');
    }

    setupEventListeners() {        // Server events
        this.startServerBtn.addEventListener('click', () => this.startServer());
        this.stopServerBtn.addEventListener('click', () => this.stopServer());
        this.browseDirectoryBtn.addEventListener('click', () => this.browseDirectory());
        document.getElementById('browse-files').addEventListener('click', () => this.browseFiles());
        
        // Test buttons
        document.getElementById('use-test-media').addEventListener('click', () => this.useTestMedia());
        document.getElementById('test-browse').addEventListener('click', () => this.testBrowse());

        // Client events
        this.connectClientBtn.addEventListener('click', () => this.connectClient());
        this.disconnectClientBtn.addEventListener('click', () => this.disconnectClient());

        // Media player events
        this.playBtn.addEventListener('click', () => this.playMedia());
        this.pauseBtn.addEventListener('click', () => this.pauseMedia());
        this.stopBtn.addEventListener('click', () => this.stopMedia());        // Utility events
        this.clearLogBtn.addEventListener('click', () => this.clearLog());
        this.fileInput.addEventListener('change', (e) => this.handleFileSelection(e, 'general'));
        this.fileInputDirectory.addEventListener('change', (e) => this.handleFileSelection(e, 'directory'));
        this.fileInputFiles.addEventListener('change', (e) => this.handleFileSelection(e, 'files'));
    }

    setupTabSwitching() {
        const tabButtons = document.querySelectorAll('.tab-button');
        const tabContents = document.querySelectorAll('.tab-content');

        tabButtons.forEach(button => {
            button.addEventListener('click', () => {
                const tabName = button.getAttribute('data-tab');
                
                // Remove active class from all buttons and contents
                tabButtons.forEach(btn => btn.classList.remove('active'));
                tabContents.forEach(content => content.classList.remove('active'));
                
                // Add active class to clicked button and corresponding content
                button.classList.add('active');                document.getElementById(tabName).classList.add('active');
            });
        });    }

    // Server Methods
    async startServer() {
        const port = this.serverPortInput.value;
        let directory = this.mediaDirectoryInput.value.trim();

        // Remove any surrounding quotes that might have been added
        directory = directory.replace(/^["']|["']$/g, '');
        
        // Debug logging
        this.logMessage(`Raw directory input: "${this.mediaDirectoryInput.value}"`, 'info');
        this.logMessage(`Cleaned directory: "${directory}"`, 'info');
        this.logMessage(`Directory length: ${directory.length}`, 'info');

        if (!port) {
            this.showNotification('Please enter a port number', 'error');
            this.logMessage('Missing port number', 'error');
            return;
        }

        if (!directory) {
            this.showNotification('Please enter a directory path or select files', 'error');
            this.logMessage('Missing directory path', 'error');
            return;
        }

        // Validate that the directory looks like a real path
        if (!this.isValidDirectoryPath(directory)) {
            this.showNotification('Please enter a valid directory path (e.g., f:\\software\\test_media)', 'error');
            this.logMessage(`Invalid directory path: ${directory}`, 'error');
            this.logMessage('Use "Use Test Media" button or enter a full path manually', 'info');
            return;
        }

        try {
            this.logMessage(`Starting server on port ${port} with directory: ${directory}`, 'info');
            
            // Debug the exact payload being sent
            const payload = {
                port: parseInt(port),
                directory: directory
            };
            this.logMessage(`Sending payload: ${JSON.stringify(payload)}`, 'info');
            
            const response = await this.callRustCommand('start-server', payload);

            if (response.success) {
                this.serverRunning = true;
                this.updateServerStatus(true);
                this.loadedFiles = response.files || [];
                this.updateLoadedFiles();
                this.showNotification('Server started successfully', 'success');
                this.logMessage('Server started successfully', 'success');
            } else {
                throw new Error(response.error || 'Failed to start server');
            }
        } catch (error) {
            this.logMessage(`Failed to start server: ${error.message}`, 'error');
            this.showNotification(`Failed to start server: ${error.message}`, 'error');
        }
    }

    async stopServer() {
        try {
            this.logMessage('Stopping server...', 'info');
            
            const response = await this.callRustCommand('stop-server', {});
            
            if (response.success) {
                this.serverRunning = false;
                this.updateServerStatus(false);
                this.loadedFiles = [];
                this.connectedClients = [];
                this.updateLoadedFiles();
                this.updateConnectedClients();
                this.showNotification('Server stopped', 'info');
                this.logMessage('Server stopped', 'info');
            } else {
                throw new Error(response.error || 'Failed to stop server');
            }
        } catch (error) {
            this.logMessage(`Failed to stop server: ${error.message}`, 'error');        this.showNotification(`Failed to stop server: ${error.message}`, 'error');
        }
    }

    useTestMedia() {
        this.mediaDirectoryInput.value = 'f:\\software\\test_media';
        this.logMessage('Set directory to test_media folder', 'info');
        this.showNotification('Test media directory selected', 'success');
    }

    testBrowse() {
        this.logMessage('Testing browse functionality...', 'info');
        try {
            // Log file input elements details
            this.logMessage(`File input (general) exists: ${!!this.fileInput}`, 'info');
            this.logMessage(`File input (directory) exists: ${!!this.fileInputDirectory}`, 'info');
            this.logMessage(`File input (files) exists: ${!!this.fileInputFiles}`, 'info');
            
            if (this.fileInputDirectory) {
                this.logMessage(`Directory input - webkitdirectory: ${this.fileInputDirectory.webkitdirectory}`, 'info');
                this.logMessage(`Directory input - multiple: ${this.fileInputDirectory.multiple}`, 'info');
            }
            
            if (this.fileInputFiles) {
                this.logMessage(`Files input - accept: ${this.fileInputFiles.accept}`, 'info');
                this.logMessage(`Files input - multiple: ${this.fileInputFiles.multiple}`, 'info');
            }
            
            this.showNotification('Check the log for browse test results', 'info');
            
        } catch (error) {
            this.logMessage(`Browse test error: ${error.message}`, 'error');
        }
    }

    browseFiles() {
        try {
            this.logMessage('Opening file browser for individual files...', 'info');
            this.fileInputFiles.click();
        } catch (error) {
            this.logMessage(`Error opening file browser: ${error.message}`, 'error');
            this.showNotification('Error opening file browser', 'error');
        }
    }

    browseDirectory() {
        try {
            this.logMessage('Opening directory browser...', 'info');
            this.fileInputDirectory.click();
        } catch (error) {
            this.logMessage(`Error opening directory browser: ${error.message}`, 'error');
            this.showNotification('Error opening directory browser', 'error');
        }
    }

    handleFileSelection(event, selectionType) {
        try {
            const files = event.target.files;
            this.logMessage(`File selection (${selectionType}) triggered with ${files.length} files`, 'info');
            
            if (files.length === 0) {
                this.logMessage('No files selected', 'warning');
                return;
            }

            // Store selected files for processing
            this.selectedFiles = Array.from(files);
            
            if (selectionType === 'directory') {
                this.handleDirectorySelection(files);
            } else {
                this.handleIndividualFiles(files);
            }
            
        } catch (error) {
            this.logMessage(`Error handling file selection: ${error.message}`, 'error');
            this.showNotification('Error processing selected files', 'error');
        }
    }    handleDirectorySelection(files) {
        try {
            // Get the common path from all files
            const filePaths = Array.from(files).map(file => file.webkitRelativePath || file.name);
            
            if (filePaths.length > 0) {
                // Get the root directory name
                const firstPath = filePaths[0];
                const pathParts = firstPath.split('/');
                
                if (pathParts.length > 1) {
                    const rootDir = pathParts[0];
                    this.mediaDirectoryInput.value = rootDir;
                    this.logMessage(`Selected directory: ${rootDir} with ${files.length} files`, 'info');
                    this.showNotification(`Selected ${files.length} files from ${rootDir}`, 'success');
                    
                    // Filter media files
                    const mediaFiles = this.filterMediaFiles(files);
                    this.logMessage(`Found ${mediaFiles.length} media files out of ${files.length} total files`, 'info');
                    
                    // Show some of the selected files in the log
                    const fileNames = mediaFiles.slice(0, 5).map(f => f.name).join(', ');
                    this.logMessage(`Media files: ${fileNames}${mediaFiles.length > 5 ? '...' : ''}`, 'info');
                } else {
                    // Files selected without directory structure
                    this.logMessage('No directory structure detected in file selection', 'warning');
                    this.logMessage('Please type the directory path manually', 'info');
                    this.mediaDirectoryInput.value = '';
                    this.mediaDirectoryInput.placeholder = 'Enter directory path manually (e.g., f:\\software\\test_media)';
                    this.showNotification('Please enter directory path manually', 'warning');
                }
            }
        } catch (error) {
            this.logMessage(`Error handling directory selection: ${error.message}`, 'error');
        }    }

    handleIndividualFiles(files) {
        try {
            // Filter media files
            const mediaFiles = this.filterMediaFiles(files);
            
            if (mediaFiles.length === 0) {
                this.logMessage('No media files found in selection', 'warning');
                this.showNotification('No media files found in selection', 'warning');
                return;
            }

            // For individual file selection, try to extract directory from file path
            // Note: Browsers don't provide full paths for security, but we can try different approaches
            if (mediaFiles.length > 0) {
                const firstFile = mediaFiles[0];
                
                // Try webkitRelativePath first (works for folder selection)
                if (firstFile.webkitRelativePath) {
                    const pathParts = firstFile.webkitRelativePath.split('/');
                    if (pathParts.length > 1) {
                        const directoryName = pathParts[0];
                        this.mediaDirectoryInput.value = directoryName;
                        this.logMessage(`Extracted directory from path: ${directoryName}`, 'info');
                        this.showNotification(`Selected ${mediaFiles.length} files from ${directoryName}`, 'success');
                        return;
                    }
                }
                
                // Try to extract from file.path if available (some browsers)
                if (firstFile.path) {
                    const pathParts = firstFile.path.split(/[\/\\]/);
                    if (pathParts.length > 1) {
                        pathParts.pop(); // Remove filename
                        const directoryPath = pathParts.join('\\');
                        this.mediaDirectoryInput.value = directoryPath;
                        this.logMessage(`Extracted directory from file path: ${directoryPath}`, 'info');
                        this.showNotification(`Selected ${mediaFiles.length} files from directory`, 'success');
                        return;
                    }
                }
                
                // If we can't extract directory path, show helpful message
                this.showFileSelectionWarning(mediaFiles);
            }
            
        } catch (error) {
            this.logMessage(`Error handling individual files: ${error.message}`, 'error');
        }
    }    showFileSelectionWarning(mediaFiles) {
        const fileNames = mediaFiles.map(f => f.name).join(', ');
        this.logMessage(`Files selected: ${fileNames}`, 'info');
        this.logMessage('Cannot automatically determine directory path from individual file selection', 'warning');
        this.logMessage('Due to browser security restrictions, please manually enter the directory path', 'info');
        this.logMessage('Options:', 'info');
        this.logMessage('1. Use "Select Folder" button to choose an entire directory, or', 'info');
        this.logMessage('2. Manually type the directory path where your files are located', 'info');
        this.logMessage('   Example: C:\\Users\\YourName\\OneDrive\\Pictures', 'info');
        this.showNotification('Please enter the directory path manually', 'warning');
        
        // Clear the input and provide helpful placeholder
        this.mediaDirectoryInput.value = '';
        this.mediaDirectoryInput.placeholder = 'Enter full directory path (e.g., C:\\Users\\YourName\\Pictures)';
        this.mediaDirectoryInput.focus(); // Focus on the input to encourage manual entry
    }

    filterMediaFiles(files) {
        const mediaExtensions = [
            // Video
            'mp4', 'avi', 'mkv', 'mov', 'webm', 'wmv', 'flv', 'm4v',
            // Audio  
            'mp3', 'wav', 'flac', 'ogg', 'aac', 'm4a', 'wma',
            // Image
            'jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp', 'svg', 'tiff'
        ];
        
        return Array.from(files).filter(file => {
            const extension = file.name.split('.').pop().toLowerCase();
            return mediaExtensions.includes(extension);
        });
    }

    updateServerStatus(running) {
        if (running) {
            this.serverStatusDot.classList.add('running');
            this.serverStatusText.textContent = 'Server Running';
            this.startServerBtn.disabled = true;
            this.stopServerBtn.disabled = false;
        } else {
            this.serverStatusDot.classList.remove('running');
            this.serverStatusText.textContent = 'Server Stopped';
            this.startServerBtn.disabled = false;
            this.stopServerBtn.disabled = true;
        }
    }

    updateLoadedFiles() {
        if (this.loadedFiles.length === 0) {
            this.loadedFilesContainer.innerHTML = '<p class="empty-state">No media files loaded</p>';
        } else {
            this.loadedFilesContainer.innerHTML = this.loadedFiles.map(file => `
                <div class="file-item">
                    <div class="file-info">
                        <div class="file-name">${file.name}</div>
                        <div class="file-size">${this.formatFileSize(file.size)}</div>
                    </div>
                    <div class="file-actions">
                        <button class="btn btn-sm btn-primary" onclick="app.streamToClients('${file.name}')">
                            Stream to Clients
                        </button>
                    </div>
                </div>
            `).join('');
        }
    }

    updateConnectedClients() {
        if (this.connectedClients.length === 0) {
            this.connectedClientsContainer.innerHTML = '<p class="empty-state">No clients connected</p>';
        } else {
            this.connectedClientsContainer.innerHTML = this.connectedClients.map(client => `
                <div class="client-item">
                    <div class="file-info">
                        <div class="file-name">${client.id}</div>
                        <div class="file-size">Connected: ${client.connectedTime}</div>
                    </div>
                    <div class="file-actions">
                        <button class="btn btn-sm btn-danger" onclick="app.disconnectClient('${client.id}')">
                            Disconnect
                        </button>
                    </div>
                </div>
            `).join('');
        }
    }

    // Client Methods
    async connectClient() {
        const serverAddress = this.serverAddressInput.value;
        const clientId = this.clientIdInput.value;

        if (!serverAddress || !clientId) {
            this.showNotification('Please fill in server address and client ID', 'error');
            return;
        }

        try {
            this.logMessage(`Connecting to server at ${serverAddress} as ${clientId}`, 'info');
            
            const response = await this.callRustCommand('connect-client', {
                serverAddress: serverAddress,
                clientId: clientId
            });

            if (response.success) {
                this.clientConnected = true;
                this.updateClientStatus(true);
                this.availableFiles = response.files || [];
                this.updateAvailableFiles();
                this.showNotification('Connected to server', 'success');
                this.logMessage('Connected to server successfully', 'success');
            } else {
                throw new Error(response.error || 'Failed to connect to server');
            }
        } catch (error) {
            this.logMessage(`Failed to connect: ${error.message}`, 'error');
            this.showNotification(`Failed to connect: ${error.message}`, 'error');
        }
    }

    async disconnectClient() {
        try {
            this.logMessage('Disconnecting from server...', 'info');
            
            const response = await this.callRustCommand('disconnect-client', {});
            
            if (response.success) {
                this.clientConnected = false;
                this.updateClientStatus(false);
                this.availableFiles = [];
                this.updateAvailableFiles();
                this.clearMediaPlayer();
                this.showNotification('Disconnected from server', 'info');
                this.logMessage('Disconnected from server', 'info');
            } else {
                throw new Error(response.error || 'Failed to disconnect');
            }
        } catch (error) {
            this.logMessage(`Failed to disconnect: ${error.message}`, 'error');
            this.showNotification(`Failed to disconnect: ${error.message}`, 'error');
        }
    }

    updateClientStatus(connected) {
        if (connected) {
            this.clientStatusDot.classList.add('connected');
            this.clientStatusText.textContent = 'Connected';
            this.connectClientBtn.disabled = true;
            this.disconnectClientBtn.disabled = false;
        } else {
            this.clientStatusDot.classList.remove('connected');
            this.clientStatusText.textContent = 'Disconnected';
            this.connectClientBtn.disabled = false;
            this.disconnectClientBtn.disabled = true;
        }
    }

    updateAvailableFiles() {
        if (this.availableFiles.length === 0) {
            this.availableFilesContainer.innerHTML = '<p class="empty-state">Connect to server to see available files</p>';
        } else {
            this.availableFilesContainer.innerHTML = this.availableFiles.map(file => `
                <div class="file-item">
                    <div class="file-info">
                        <div class="file-name">${file}</div>
                    </div>
                    <div class="file-actions">
                        <button class="btn btn-sm btn-primary" onclick="app.requestMedia('${file}')">
                            Request
                        </button>
                    </div>
                </div>
            `).join('');
        }
    }

    // Media Methods
    async requestMedia(filename) {
        try {
            this.logMessage(`Requesting media: ${filename}`, 'info');
            
            const response = await this.callRustCommand('request-media', {
                filename: filename
            });

            if (response.success) {
                this.loadMediaInPlayer(response.mediaData);
                this.showNotification(`Media loaded: ${filename}`, 'success');
                this.logMessage(`Media received: ${filename}`, 'success');
            } else {
                throw new Error(response.error || 'Failed to request media');
            }
        } catch (error) {
            this.logMessage(`Failed to request media: ${error.message}`, 'error');
            this.showNotification(`Failed to request media: ${error.message}`, 'error');
        }
    }

    async streamToClients(filename) {
        try {
            this.logMessage(`Streaming ${filename} to all clients`, 'info');
            
            const response = await this.callRustCommand('stream-media', {
                filename: filename
            });

            if (response.success) {
                this.showNotification(`Streaming ${filename} to clients`, 'success');
                this.logMessage(`Started streaming ${filename}`, 'success');
            } else {
                throw new Error(response.error || 'Failed to stream media');
            }
        } catch (error) {
            this.logMessage(`Failed to stream media: ${error.message}`, 'error');
            this.showNotification(`Failed to stream media: ${error.message}`, 'error');
        }
    }    loadMediaInPlayer(mediaData) {
        const { filename, data, mediaType } = mediaData;
        
        // Convert base64 data to blob
        const binaryString = atob(data);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        
        const blob = new Blob([bytes], { type: this.getMimeType(mediaType, filename) });
        const url = URL.createObjectURL(blob);

        let playerHTML = '';
        
        if (mediaType === 'video') {
            playerHTML = `
                <video controls autoplay style="max-width: 100%; max-height: 100%;">
                    <source src="${url}" type="${this.getMimeType(mediaType, filename)}">
                    Your browser does not support the video tag.
                </video>
            `;
        } else if (mediaType === 'audio') {
            playerHTML = `
                <audio controls autoplay style="width: 100%;">
                    <source src="${url}" type="${this.getMimeType(mediaType, filename)}">
                    Your browser does not support the audio tag.
                </audio>
            `;
        } else if (mediaType === 'image') {
            playerHTML = `
                <img src="${url}" alt="${filename}" style="max-width: 100%; max-height: 100%; object-fit: contain;">
            `;
        }

        this.mediaPlayerContainer.innerHTML = playerHTML;
        this.updatePlayerControls(true);
    }

    clearMediaPlayer() {
        this.mediaPlayerContainer.innerHTML = `
            <div class="player-placeholder">
                <p>No media loaded</p>
            </div>
        `;
        this.updatePlayerControls(false);
    }

    updatePlayerControls(enabled) {
        this.playBtn.disabled = !enabled;
        this.pauseBtn.disabled = !enabled;
        this.stopBtn.disabled = !enabled;
    }

    playMedia() {
        const media = this.mediaPlayerContainer.querySelector('video, audio');
        if (media) {
            media.play();
            this.logMessage('Media playback started', 'info');
        }
    }

    pauseMedia() {
        const media = this.mediaPlayerContainer.querySelector('video, audio');
        if (media) {
            media.pause();
            this.logMessage('Media playback paused', 'info');
        }
    }

    stopMedia() {
        const media = this.mediaPlayerContainer.querySelector('video, audio');
        if (media) {
            media.pause();
            media.currentTime = 0;
            this.logMessage('Media playback stopped', 'info');
        }
    }

    // Utility Methods
    getMimeType(mediaType, filename) {
        const extension = filename.split('.').pop().toLowerCase();
        
        if (mediaType === 'video') {
            switch (extension) {
                case 'mp4': return 'video/mp4';
                case 'avi': return 'video/x-msvideo';
                case 'mkv': return 'video/x-matroska';
                case 'mov': return 'video/quicktime';
                case 'webm': return 'video/webm';
                default: return 'video/mp4';
            }
        } else if (mediaType === 'audio') {
            switch (extension) {
                case 'mp3': return 'audio/mpeg';
                case 'wav': return 'audio/wav';
                case 'flac': return 'audio/flac';
                case 'ogg': return 'audio/ogg';
                case 'aac': return 'audio/aac';
                default: return 'audio/mpeg';
            }
        } else if (mediaType === 'image') {
            switch (extension) {
                case 'jpg':
                case 'jpeg': return 'image/jpeg';
                case 'png': return 'image/png';
                case 'gif': return 'image/gif';
                case 'bmp': return 'image/bmp';
                case 'webp': return 'image/webp';
                default: return 'image/jpeg';
            }
        }
        
        return 'application/octet-stream';
    }

    formatFileSize(bytes) {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    }    // Communication with Rust Backend
    async callRustCommand(command, params) {
        try {
            const response = await fetch('/api', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    command: command,
                    params: params
                })
            });

            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }

            const result = await response.json();
            
            if (!result.success && result.error) {
                throw new Error(result.error);
            }

            return {
                success: result.success,
                ...result.data
            };
        } catch (error) {
            console.error('Error calling Rust command:', error);
            return {
                success: false,
                error: error.message
            };
        }
    }

    getMediaTypeFromFilename(filename) {
        const extension = filename.split('.').pop().toLowerCase();
        
        if (['mp4', 'avi', 'mkv', 'mov', 'webm'].includes(extension)) {
            return 'video';
        } else if (['mp3', 'wav', 'flac', 'ogg', 'aac'].includes(extension)) {
            return 'audio';
        } else if (['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp'].includes(extension)) {
            return 'image';
        }
          return 'unknown';
    }    isValidDirectoryPath(path) {
        // Check if the path looks like a valid directory path
        // This is a basic validation to prevent passing display text instead of paths
        if (!path || typeof path !== 'string') {
            return false;
        }
        
        // Trim the path
        path = path.trim();
        
        // Check if it contains common display text patterns that aren't paths
        const invalidPatterns = [
            /\d+\s+media\s+file/i,  // "1 media file(s) selected"
            /file.*selected/i,       // Any "file selected" text
            /selected/i,             // General "selected" text when short
            /^[^\\\/]*$/             // Text without any path separators (too short)
        ];
        
        for (const pattern of invalidPatterns) {
            if (pattern.test(path) && path.length < 10) {
                return false;
            }
        }
        
        // Check if it looks like a real path (has some path-like characteristics)
        const pathLikePatterns = [
            /^[a-zA-Z]:\\/,                    // Windows absolute path (C:\, f:\, etc.)
            /^[a-zA-Z]:[\/\\]/,                // Alternative Windows path format  
            /^\/[^\/]/,                        // Unix absolute path (/home, /var, etc.)
            /^\.{1,2}[\/\\]/,                  // Relative path (./folder, ../folder)
            /^[^\/\\]+[\/\\]/,                 // Folder with separator
            /test_media$/,                     // Our specific test directory
            /^.*[\/\\].*[\/\\]/,               // Path with multiple separators
            /^[a-zA-Z]:\\.*\\.*$/,             // Windows path with multiple levels
            /^C:\\Users\\.*\\.*$/,             // Common Windows user path
            /OneDrive/i,                       // OneDrive paths
            /Pictures/i,                       // Pictures folder
            /Documents/i,                      // Documents folder
            /Downloads/i                       // Downloads folder
        ];
        
        // Also accept longer paths that contain separators
        const hasPathSeparators = path.includes('\\') || path.includes('/');
        const isLongEnough = path.length > 3;
        
        return pathLikePatterns.some(pattern => pattern.test(path)) || 
               (hasPathSeparators && isLongEnough);
    }

    // UI Utility Methods
    logMessage(message, type = 'info') {
        const timestamp = new Date().toLocaleTimeString();
        const logEntry = document.createElement('div');
        logEntry.className = `log-entry ${type}`;
        logEntry.innerHTML = `
            <span class="timestamp">[${timestamp}]</span> ${message}
        `;
        
        this.statusLog.appendChild(logEntry);
        this.statusLog.scrollTop = this.statusLog.scrollHeight;
    }

    clearLog() {
        this.statusLog.innerHTML = '<p class="log-entry">Log cleared</p>';
    }

    showNotification(message, type = 'info') {
        const notification = document.createElement('div');
        notification.className = `notification ${type}`;
        notification.textContent = message;
        
        document.body.appendChild(notification);
        
        setTimeout(() => {
            notification.style.animation = 'slideIn 0.3s ease reverse';
            setTimeout(() => {
                document.body.removeChild(notification);
            }, 300);
        }, 3000);
    }
}

// Initialize the application when the page loads
let app;
document.addEventListener('DOMContentLoaded', () => {
    app = new MediaSyncApp();
});

// Make app globally accessible for onclick handlers
window.app = app;
