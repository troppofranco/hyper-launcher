; Installer Windows di Hyper Launcher (Inno Setup 6).
; Compilato dalla GitHub Action `release`:
;   iscc /DAppVersion=0.1.12 /DBuild=12 installer\hyper-launcher.iss
; Installa per l'utente corrente (nessun permesso di amministratore) l'app
; grafica `hyper-launcher.exe` (menu Start e, a scelta, desktop) e la CLI
; `hyper.exe`, la cui cartella va nel PATH dell'utente. Rieseguito sopra
; un'installazione esistente (anche in modo silenzioso, dall'aggiornamento
; automatico) la aggiorna sul posto; con `/relaunch=1` riapre l'app alla
; fine (aggiornamento avviato dall'app stessa).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef Build
  #define Build "0"
#endif

[Setup]
AppId={{064185E1-75EB-4CFD-B5D8-E5F9741FCCC4}
AppName=Hyper Launcher
AppVersion={#AppVersion}
AppVerName=Hyper Launcher {#AppVersion}
VersionInfoVersion={#AppVersion}
; Se l'app o la CLI sono ancora in uso durante l'aggiornamento, vengono chiuse.
CloseApplications=force
RestartApplications=no
AppPublisher=Hyper Launcher
DefaultDirName={localappdata}\Programs\HyperLauncher
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
OutputDir=..\dist
OutputBaseFilename=HyperLauncher-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\crates\app\icons\icon.ico
UninstallDisplayName=Hyper Launcher
UninstallDisplayIcon={app}\hyper-launcher.exe

[Languages]
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\target\release\hyper-launcher.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\hyper.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Hyper Launcher"; Filename: "{app}\hyper-launcher.exe"
Name: "{autodesktop}\Hyper Launcher"; Filename: "{app}\hyper-launcher.exe"; Tasks: desktopicon

[Run]
; Installazione normale: casella "Avvia Hyper Launcher" nell'ultima pagina.
Filename: "{app}\hyper-launcher.exe"; Description: "{cm:LaunchProgram,Hyper Launcher}"; \
  Flags: nowait postinstall skipifsilent
; Aggiornamento silenzioso avviato dall'app: la riapre.
Filename: "{app}\hyper-launcher.exe"; Flags: nowait; Check: ShouldRelaunch

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; \
  ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Code]
function ShouldRelaunch(): Boolean;
begin
  Result := WizardSilent() and (ExpandConstant('{param:relaunch|0}') = '1');
end;

function PathContains(Paths, Dir: string): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';') > 0;
end;

function NeedsAddPath(Dir: string): Boolean;
var
  Paths: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Paths) then
    Paths := '';
  Result := not PathContains(Paths, Dir);
end;

{ Alla disinstallazione rimuove la cartella dal PATH dell'utente. I dati
  del launcher in %APPDATA%\HyperLauncher (istanze, mondi) restano. }
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Paths, Dir: string;
  P: Integer;
begin
  if CurUninstallStep <> usPostUninstall then
    exit;
  { Voce che avvia i server con il computer. }
  RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run',
    'HyperLauncherServers');
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Paths) then
    exit;
  Dir := ExpandConstant('{app}');
  P := Pos(';' + Uppercase(Dir), Uppercase(Paths));
  if P > 0 then
  begin
    Delete(Paths, P, Length(Dir) + 1);
    RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Paths);
  end;
end;
