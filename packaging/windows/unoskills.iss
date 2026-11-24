; Inno Setup script for UnoSkills (Windows x64).
; Compile with: ISCC.exe /DAppVersion=<x.y.z> packaging\windows\unoskills.iss
; Expects staged binaries in dist\windows\ (unoskills-app.exe, unoskills.exe).

#ifndef AppVersion
#define AppVersion "0.0.0"
#endif

#define AppName "UnoSkills"
#define AppExeName "unoskills-app.exe"
#define CliExeName "unoskills.exe"
#define AppId "dev.unoskills.UnoSkills"

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=codexlin
DefaultDirName={autopf}\UnoSkills
DefaultGroupName={#AppName}
OutputDir=..\..\dist
OutputBaseFilename=unoskills-setup-{#AppVersion}-windows-x64
SetupIconFile=..\icons\unoskills.ico
UninstallDisplayIcon={app}\{#AppExeName}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequiredOverridesAllowed=dialog

[Files]
Source: "..\..\dist\windows\unoskills-app.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dist\windows\unoskills.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\UnoSkills"; Filename: "{app}\{#AppExeName}"; AppUserModelID: "{#AppId}"
Name: "{autodesktop}\UnoSkills"; Filename: "{app}\{#AppExeName}"; AppUserModelID: "{#AppId}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional icons:"

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
