; Little Helper Windows Installer Script (Inno Setup)
; Compile with Inno Setup: https://jrsoftware.org/isinfo.php

[Setup]
AppName=Little Helper
AppVersion=1.0.0-beta
AppPublisher=Ben
DefaultDirName={autopf}\Little Helper
DefaultGroupName=Little Helper
OutputBaseFilename=LittleHelper-Setup
Compression=lzma
SolidCompression=yes
WizardStyle=modern
DisableWelcomePage=no
LicenseFile=
InfoBeforeFile=installer\welcome-message.txt

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "target\release\app.exe"; DestDir: "{app}"; DestName: "Little Helper.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\Little Helper"; Filename: "{app}\Little Helper.exe"
Name: "{autodesktop}\Little Helper"; Filename: "{app}\Little Helper.exe"

[Run]
Filename: "{app}\Little Helper.exe"; Description: "Launch Little Helper"; Flags: postinstall nowait skipifsilent
