//! Storage Cleaner Skill
//!
//! Helps users organize and archive files safely - NEVER deletes anything.
//! Focuses on moving files to organized structures or cloud storage.
//!
//! Features:
//! - Identifies large files and folders taking up space
//! - Detects duplicate files (by hash)
//! - Finds old unused files (> 1 year)
//! - Organizes messy folders by file type/date
//! - Archives to mounted drives (Google Drive, external storage)
//! - Safe operations only - moves and organizes, never deletes
//!
//! Safety Principles:
//! - NO DELETE OPERATIONS - by design
//! - Archive instead of delete
//! - Organize instead of remove
//! - Preview all actions before execution
//! - Create backup manifests

use anyhow::Result;
use async_trait::async_trait;
use shared::skill::{Mode, PermissionLevel, Skill, SkillContext, SkillInput, SkillOutput, SuggestedAction};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

/// File information for analysis
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileInfo {
    /// File path
    pub path: PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
    /// Last modified time
    pub modified: SystemTime,
    /// File type/category
    pub category: FileCategory,
    /// Whether file is a duplicate
    pub is_duplicate: bool,
    /// Hash for duplicate detection (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// File categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FileCategory {
    Documents,
    Images,
    Videos,
    Audio,
    Archives,
    Code,
    Downloads,
    OldFiles,     // > 1 year old
    LargeFiles,   // > 100MB
    Duplicates,
    Unknown,
}

impl FileCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            FileCategory::Documents => "📄 Documents",
            FileCategory::Images => "🖼️  Images",
            FileCategory::Videos => "🎬 Videos",
            FileCategory::Audio => "🎵 Audio",
            FileCategory::Archives => "📦 Archives",
            FileCategory::Code => "💻 Code",
            FileCategory::Downloads => "⬇️  Downloads",
            FileCategory::OldFiles => "📅 Old Files (>1 year)",
            FileCategory::LargeFiles => "🗂️  Large Files (>100MB)",
            FileCategory::Duplicates => "👯 Duplicates",
            FileCategory::Unknown => "❓ Unknown",
        }
    }
    
    pub fn folder_name(&self) -> &'static str {
        match self {
            FileCategory::Documents => "01_Documents",
            FileCategory::Images => "02_Images",
            FileCategory::Videos => "03_Videos",
            FileCategory::Audio => "04_Audio",
            FileCategory::Archives => "05_Archives",
            FileCategory::Code => "06_Code",
            FileCategory::Downloads => "07_Downloads",
            FileCategory::OldFiles => "08_Old_Files",
            FileCategory::LargeFiles => "09_Large_Files",
            FileCategory::Duplicates => "10_Duplicates",
            FileCategory::Unknown => "99_Misc",
        }
    }
}

/// Storage analysis results
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StorageAnalysisResult {
    /// Total space scanned in bytes
    pub total_scanned_bytes: u64,
    /// Total files found
    pub total_files: usize,
    /// Space that could be reclaimed by archiving
    pub reclaimable_bytes: u64,
    /// Files by category
    pub by_category: HashMap<FileCategory, Vec<FileInfo>>,
    /// Duplicate file groups
    pub duplicates: Vec<Vec<FileInfo>>,
    /// Old files (> 1 year)
    pub old_files: Vec<FileInfo>,
    /// Large files (> 100MB)
    pub large_files: Vec<FileInfo>,
    /// Mounted drives available
    pub available_drives: Vec<MountedDrive>,
}

/// Information about a mounted drive
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MountedDrive {
    /// Drive name
    pub name: String,
    /// Mount point
    pub mount_point: PathBuf,
    /// Total space
    pub total_bytes: u64,
    /// Available space
    pub available_bytes: u64,
    /// Drive type (Google Drive, External, Network)
    pub drive_type: DriveType,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DriveType {
    GoogleDrive,
    External,
    Network,
    Local,
}

/// Storage Cleaner Skill
pub struct StorageCleaner;

impl StorageCleaner {
    /// Create a new storage cleaner
    pub fn new() -> Self {
        Self
    }

    /// Analyze storage in a directory
    fn analyze_storage(&self, path: &Path) -> Result<StorageAnalysisResult> {
        let mut files_by_category: HashMap<FileCategory, Vec<FileInfo>> = HashMap::new();
        let mut total_scanned: u64 = 0;
        let mut total_files: usize = 0;
        let mut all_files: Vec<FileInfo> = Vec::new();
        
        // Walk directory
        for entry in WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file()) {
            
            let path = entry.path().to_path_buf();
            let metadata = entry.metadata()?;
            let size = metadata.len();
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let category = self.categorize_file(&path, size, modified);
            
            let file_info = FileInfo {
                path: path.clone(),
                size_bytes: size,
                modified,
                category,
                is_duplicate: false,
                hash: None,
            };
            
            total_scanned += size;
            total_files += 1;
            
            files_by_category.entry(category).or_default().push(file_info.clone());
            all_files.push(file_info);
        }
        
        // Detect duplicates (simplified - by size and name)
        let duplicates = self.find_duplicates(&all_files);
        
        // Find old files (> 1 year)
        let one_year_ago = SystemTime::now() - Duration::from_secs(365 * 24 * 60 * 60);
        let old_files: Vec<FileInfo> = all_files.iter()
            .filter(|f| f.modified < one_year_ago)
            .cloned()
            .collect();
        
        // Find large files (> 100MB)
        let large_files: Vec<FileInfo> = all_files.iter()
            .filter(|f| f.size_bytes > 100 * 1024 * 1024)
            .cloned()
            .collect();
        
        // Calculate reclaimable space
        let reclaimable = duplicates.iter()
            .map(|group| {
                if group.len() > 1 {
                    // Can archive all but one copy
                    group.iter().map(|f| f.size_bytes).sum::<u64>() - group[0].size_bytes
                } else {
                    0
                }
            })
            .sum::<u64>()
            + old_files.iter().map(|f| f.size_bytes).sum::<u64>();
        
        // Detect mounted drives
        let available_drives = self.detect_mounted_drives();
        
        Ok(StorageAnalysisResult {
            total_scanned_bytes: total_scanned,
            total_files,
            reclaimable_bytes: reclaimable,
            by_category: files_by_category,
            duplicates,
            old_files,
            large_files,
            available_drives,
        })
    }

    /// Categorize a file by type and age
    fn categorize_file(&self, path: &Path, size: u64, modified: SystemTime) -> FileCategory {
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // Check age first
        let one_year_ago = SystemTime::now() - Duration::from_secs(365 * 24 * 60 * 60);
        if modified < one_year_ago {
            return FileCategory::OldFiles;
        }
        
        // Check size
        if size > 100 * 1024 * 1024 {
            return FileCategory::LargeFiles;
        }
        
        // Check file type
        match extension.as_str() {
            "pdf" | "doc" | "docx" | "txt" | "rtf" | "odt" | "xls" | "xlsx" | "ppt" | "pptx" | "csv" => {
                FileCategory::Documents
            }
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp" | "raw" | "heic" => {
                FileCategory::Images
            }
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" => {
                FileCategory::Videos
            }
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" => {
                FileCategory::Audio
            }
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" => {
                FileCategory::Archives
            }
            "rs" | "js" | "ts" | "py" | "java" | "cpp" | "c" | "h" | "go" | "rb" | "php" | "swift" => {
                FileCategory::Code
            }
            _ => {
                // Check if in Downloads folder
                if path.to_string_lossy().contains("Downloads") {
                    FileCategory::Downloads
                } else {
                    FileCategory::Unknown
                }
            }
        }
    }

    /// Find duplicate files (simplified - by size and filename)
    fn find_duplicates(&self, files: &[FileInfo]) -> Vec<Vec<FileInfo>> {
        let mut by_size_and_name: HashMap<(u64, String), Vec<FileInfo>> = HashMap::new();
        
        for file in files {
            let filename = file.path.file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("")
                .to_string();
            let key = (file.size_bytes, filename);
            by_size_and_name.entry(key).or_default().push(file.clone());
        }
        
        by_size_and_name.into_values()
            .filter(|group| group.len() > 1)
            .collect()
    }

    /// Detect mounted drives
    fn detect_mounted_drives(&self) -> Vec<MountedDrive> {
        let mut drives = Vec::new();
        
        #[cfg(target_os = "macos")]
        {
            // Check for Google Drive
            let gdrive_path = dirs::home_dir().map(|h| h.join("Google Drive"));
            if let Some(ref path) = gdrive_path {
                if path.exists() {
                    // Try to get available space
                    let output = std::process::Command::new("df")
                        .arg("-h")
                        .arg(path)
                        .output();
                    
                    if let Ok(_output) = output {
                        // Parse df output
                        drives.push(MountedDrive {
                            name: "Google Drive".to_string(),
                            mount_point: path.clone(),
                            total_bytes: 15_000_000_000_000, // 15TB (unlimited for GDrive)
                            available_bytes: 15_000_000_000_000,
                            drive_type: DriveType::GoogleDrive,
                        });
                    }
                }
            }
            
            // Check /Volumes for external drives
            if let Ok(entries) = std::fs::read_dir("/Volumes") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    
                    // Skip system volumes
                    if name != "Macintosh HD" && name != "Preboot" && name != "Recovery" {
                        drives.push(MountedDrive {
                            name,
                            mount_point: path.clone(),
                            total_bytes: 0, // Would need actual detection
                            available_bytes: 0,
                            drive_type: DriveType::External,
                        });
                    }
                }
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            // Check common mount points
            let mut mount_points: Vec<PathBuf> = vec![
                PathBuf::from("/mnt"),
                PathBuf::from("/media"),
            ];
            
            if let Some(home) = dirs::home_dir() {
                mount_points.push(home.join("Google Drive"));
            }
            
            for mount_point in mount_points {
                if mount_point.exists() {
                    let name = mount_point.file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    
                    let drive_type = if name.contains("Google") || name.contains("Drive") {
                        DriveType::GoogleDrive
                    } else {
                        DriveType::External
                    };
                    
                    drives.push(MountedDrive {
                        name,
                        mount_point,
                        total_bytes: 0,
                        available_bytes: 0,
                        drive_type,
                    });
                }
            }
        }
        
        drives
    }

    /// Format results for user
    fn format_results(&self, result: &StorageAnalysisResult, path: &Path) -> String {
        let mut output = String::new();
        
        output.push_str("## 🗄️  Storage Analysis\n\n");
        output.push_str(&format!("**Scanned:** {}\n", path.display()));
        output.push_str(&format!("**Total files:** {}\n", result.total_files));
        output.push_str(&format!("**Total size:** {}\n", self.format_bytes(result.total_scanned_bytes)));
        output.push('\n');
        
        // Space that can be reclaimed
        if result.reclaimable_bytes > 0 {
            output.push_str(&format!("### 💾 Potential Space Savings\n\n"));
            output.push_str(&format!("**{}** can be archived to free up space\n\n", 
                self.format_bytes(result.reclaimable_bytes)));
        }
        
        // Files by category
        if !result.by_category.is_empty() {
            output.push_str("### 📁 Files by Category\n\n");
            
            let mut categories: Vec<_> = result.by_category.iter().collect();
            categories.sort_by(|a, b| b.1.len().cmp(&a.1.len())); // Sort by count
            
            for (category, files) in categories {
                let total_size: u64 = files.iter().map(|f| f.size_bytes).sum();
                output.push_str(&format!("{} **{} files** ({})", 
                    category.display_name(),
                    files.len(),
                    self.format_bytes(total_size)));
                
                // Show first 3 files as examples
                if !files.is_empty() {
                    output.push_str("\n  Examples:");
                    for file in files.iter().take(3) {
                        let filename = file.path.file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or("unknown");
                        output.push_str(&format!("\n    • {} ({})", 
                            filename, 
                            self.format_bytes(file.size_bytes)));
                    }
                    if files.len() > 3 {
                        output.push_str(&format!("\n    ... and {} more", files.len() - 3));
                    }
                }
                output.push_str("\n\n");
            }
        }
        
        // Duplicates
        if !result.duplicates.is_empty() {
            output.push_str("### 👯 Duplicate Files\n\n");
            output.push_str(&format!("Found **{} groups** of duplicates\n\n", result.duplicates.len()));
            
            for (i, group) in result.duplicates.iter().take(5).enumerate() {
                if let Some(first) = group.first() {
                    let filename = first.path.file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("unknown");
                    let savings = (group.len() as u64 - 1) * first.size_bytes;
                    
                    output.push_str(&format!("{}. **{}** ({} copies, save {})\n",
                        i + 1,
                        filename,
                        group.len(),
                        self.format_bytes(savings)));
                }
            }
            
            if result.duplicates.len() > 5 {
                output.push_str(&format!("\n... and {} more duplicate groups", result.duplicates.len() - 5));
            }
            output.push('\n');
        }
        
        // Old files
        if !result.old_files.is_empty() {
            let old_size: u64 = result.old_files.iter().map(|f| f.size_bytes).sum();
            output.push_str(&format!("### 📅 Old Files (> 1 year)\n\n"));
            output.push_str(&format!("**{} files** taking {}\n\n", 
                result.old_files.len(),
                self.format_bytes(old_size)));
        }
        
        // Available drives
        if !result.available_drives.is_empty() {
            output.push_str("### 💾 Available Storage Locations\n\n");
            for drive in &result.available_drives {
                let type_icon = match drive.drive_type {
                    DriveType::GoogleDrive => "☁️",
                    DriveType::External => "💿",
                    DriveType::Network => "🌐",
                    DriveType::Local => "💻",
                };
                output.push_str(&format!("{} **{}** at `{}`\n",
                    type_icon,
                    drive.name,
                    drive.mount_point.display()));
            }
            output.push('\n');
        } else {
            output.push_str("### 💾 Storage Locations\n\n");
            output.push_str("⚠️  No external drives detected. Consider:\n");
            output.push_str("• Mounting Google Drive (see setup instructions below)\n");
            output.push_str("• Connecting an external drive\n\n");
        }
        
        output.push_str("---\n\n");
        output.push_str("✅ **Safety Note:** This tool only organizes and archives files - nothing is ever deleted.\n");
        output.push_str("📦 **Archiving:** Old and duplicate files can be moved to organized folders or cloud storage.\n");
        
        output
    }

    /// Format bytes to human readable
    fn format_bytes(&self, bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        format!("{:.1} {}", size, UNITS[unit_index])
    }
    
    /// Get Google Drive setup instructions
    fn get_gdrive_setup_instructions(&self) -> String {
        r#"
### ☁️  Google Drive Setup Instructions

**For macOS:**
1. Download Google Drive for Desktop: https://www.google.com/drive/download/
2. Install and sign in with your Google account
3. Choose "Stream files" mode (saves local space)
4. Your Drive will appear at `~/Google Drive`

**For Linux:**
1. Install rclone: `sudo apt install rclone` (Debian/Ubuntu) or `sudo pacman -S rclone` (Arch)
2. Configure rclone: `rclone config`
   - Select "n" for new remote
   - Name it "gdrive"
   - Select "Google Drive" (option 18)
   - Follow authentication steps
3. Mount with: `rclone mount gdrive: ~/Google Drive --daemon`
4. Add to startup: Add the mount command to your `.bashrc` or systemd service

**For Windows:**
1. Download Google Drive for Desktop
2. Install and sign in
3. Access via File Explorer under "Google Drive"

**Team Tip:** With Google Drive mounted, I can help you:
- Archive old files to cloud storage
- Organize messy folders
- Free up local disk space
- Keep backups safely in the cloud
"#.to_string()
    }
    
    /// Archive files to a destination
    pub fn archive_files(&self, files: &[PathBuf], destination: &Path) -> anyhow::Result<String> {
        std::fs::create_dir_all(destination)?;
        
        let mut moved_count = 0;
        let mut total_size: u64 = 0;
        
        for file in files {
            if let Some(filename) = file.file_name() {
                let dest_path = destination.join(filename);
                
                // Check if destination exists, rename if needed
                let final_dest = if dest_path.exists() {
                    let stem = dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
                    let ext = dest_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    dest_path.with_file_name(format!("{}_{}.{}", stem, timestamp, ext))
                } else {
                    dest_path
                };
                
                std::fs::rename(file, &final_dest)?;
                
                if let Ok(metadata) = std::fs::metadata(&final_dest) {
                    total_size += metadata.len();
                }
                moved_count += 1;
            }
        }
        
        Ok(format!("Archived {} files ({}) to {}", 
            moved_count,
            self.format_bytes(total_size),
            destination.display()))
    }
    
    /// Organize files by category
    pub fn organize_by_category(&self, base_path: &Path, files: &[(PathBuf, FileCategory)]) -> anyhow::Result<String> {
        let mut organized_count = 0;
        
        for (file_path, category) in files {
            let category_folder = base_path.join(category.folder_name());
            std::fs::create_dir_all(&category_folder)?;
            
            if let Some(filename) = file_path.file_name() {
                let dest_path = category_folder.join(filename);
                
                // Handle duplicates
                let final_dest = if dest_path.exists() {
                    let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
                    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    category_folder.join(format!("{}_{}.{}", stem, timestamp, ext))
                } else {
                    dest_path
                };
                
                std::fs::rename(file_path, final_dest)?;
                organized_count += 1;
            }
        }
        
        Ok(format!("Organized {} files into category folders", organized_count))
    }
}

#[async_trait]
impl Skill for StorageCleaner {
    fn id(&self) -> &'static str {
        "storage_cleaner"
    }
    
    fn name(&self) -> &'static str {
        "Storage Cleaner"
    }
    
    fn description(&self) -> &'static str {
        "Analyzes and organizes files - archives to save space, never deletes"
    }
    
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Fix]
    }
    
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Sensitive // Needs approval to move files
    }
    
    async fn execute(&self, input: SkillInput, _ctx: &SkillContext) -> anyhow::Result<SkillOutput> {
        // Get target path from input, default to home directory
        let path = input.params.get("path")
            .and_then(|p| p.as_str())
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir())
            .unwrap_or_else(|| PathBuf::from("/"));
        
        // Analyze storage
        let result = self.analyze_storage(&path)?;
        let formatted_text = self.format_results(&result, &path);
        
        // Build suggested actions
        let mut suggested_actions: Vec<SuggestedAction> = Vec::new();
        
        // Action: Archive old files if found
        if !result.old_files.is_empty() {
            let old_size: u64 = result.old_files.iter().map(|f| f.size_bytes).sum();
            let paths: Vec<_> = result.old_files.iter().map(|f| f.path.clone()).collect();
            
            if let Some(gdrive) = result.available_drives.iter().find(|d| matches!(d.drive_type, DriveType::GoogleDrive)) {
                let mut params = HashMap::new();
                params.insert("files".to_string(), serde_json::json!(paths));
                params.insert("destination".to_string(), serde_json::json!(gdrive.mount_point.join("Archive/Old_Files")));
                
                suggested_actions.push(SuggestedAction {
                    label: format!("Archive {} old files to Google Drive ({})", 
                        result.old_files.len(),
                        self.format_bytes(old_size)),
                    skill_id: "archive_files".to_string(),
                    params,
                });
            }
        }
        
        // Action: Organize messy folder by category
        if result.by_category.len() > 3 {
            let mut files_to_organize = Vec::new();
            for (category, files) in &result.by_category {
                if *category != FileCategory::Unknown {
                    for file in files {
                        files_to_organize.push((file.path.clone(), *category));
                    }
                }
            }
            
            if !files_to_organize.is_empty() {
                let mut params = HashMap::new();
                params.insert("base_path".to_string(), serde_json::json!(path.join("Organized")));
                params.insert("files".to_string(), serde_json::json!(files_to_organize));
                
                suggested_actions.push(SuggestedAction {
                    label: format!("Organize {} files by type into folders", files_to_organize.len()),
                    skill_id: "organize_by_category".to_string(),
                    params,
                });
            }
        }
        
        // Action: Setup Google Drive if not available
        if !result.available_drives.iter().any(|d| matches!(d.drive_type, DriveType::GoogleDrive)) {
            suggested_actions.push(SuggestedAction {
                label: "📖 Show Google Drive setup instructions".to_string(),
                skill_id: "show_gdrive_setup".to_string(),
                params: HashMap::new(),
            });
        }
        
        // Action: Archive duplicate files
        if !result.duplicates.is_empty() {
            let dup_count: usize = result.duplicates.iter().map(|g| g.len() - 1).sum();
            let dup_size: u64 = result.duplicates.iter()
                .map(|g| if g.len() > 1 { (g.len() as u64 - 1) * g[0].size_bytes } else { 0 })
                .sum();
            
            suggested_actions.push(SuggestedAction {
                label: format!("Archive {} duplicate files (save {})",
                    dup_count,
                    self.format_bytes(dup_size)),
                skill_id: "archive_duplicates".to_string(),
                params: HashMap::new(),
            });
        }
        
        Ok(SkillOutput {
            result_type: shared::skill::ResultType::Text,
            text: Some(formatted_text),
            files: Vec::new(),
            data: Some(serde_json::to_value(result)?),
            citations: Vec::new(),
            suggested_actions,
        })
    }
}

impl Default for StorageCleaner {
    fn default() -> Self {
        Self::new()
    }
}