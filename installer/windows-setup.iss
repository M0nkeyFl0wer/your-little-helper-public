; Little Helper Windows Installer Script (Inno Setup)
; Compile with Inno Setup: https://jrsoftware.org/isinfo.php
; Run from repo root: ISCC.exe installer\windows-setup.iss

#define SourceDir ".."

[Setup]
AppName=Little Helper
AppVersion=1.0.0-beta
AppPublisher=Ben
DefaultDirName={autopf}\Little Helper
DefaultGroupName=Little Helper
OutputBaseFilename=LittleHelper-Setup
OutputDir={#SourceDir}\Output
Compression=lzma
SolidCompression=yes
WizardStyle=modern
DisableWelcomePage=no
LicenseFile=
InfoBeforeFile={#SourceDir}\installer\welcome-message.txt

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#SourceDir}\target\release\app.exe"; DestDir: "{app}"; DestName: "Little Helper.exe"; Flags: ignoreversion
Source: "{#SourceDir}\target\release\ollama.exe"; DestDir: "{app}"; DestName: "ollama.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\Little Helper"; Filename: "{app}\Little Helper.exe"
Name: "{autodesktop}\Little Helper"; Filename: "{app}\Little Helper.exe"

[Run]
Filename: "{app}\Little Helper.exe"; Description: "Launch Little Helper"; Flags: postinstall nowait skipifsilent
