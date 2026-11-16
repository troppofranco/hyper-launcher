; Installer Windows di Hyper Launcher (Inno Setup 6).
; Compilato dalla GitHub Action `release`:
;   iscc /DAppVersion=0.1.0 installer\hyper-launcher.iss
; Installa per l'utente corrente (nessun permesso di amministratore) e
; aggiunge la cartella di installazione al PATH dell'utente, così `hyper`
; è disponibile in ogni terminale.

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif

[Setup]
AppId={{064185E1-75EB-4CFD-B5D8-E5F9741FCCC4}
AppName=Hyper Launcher
AppVersion={#AppVersion}
AppVerName=Hyper Launcher {#AppVersion}
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
UninstallDisplayName=Hyper Launcher

[Languages]
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\target\release\hyper.exe"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; \
  ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Code]
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
