<#
.SYNOPSIS
    NS8-SoftwareManager Windows Endpoint Bootstrap & Service Installer
.PARAMETER ServerUrl
    Base HTTPS URL of the NS8-SoftwareManager server (e.g. https://swm.example.org)
.PARAMETER Token
    Enrollment token generated in the NS8-SoftwareManager Admin Console
.PARAMETER Headless
    Set to 1 to install only the background swm-agent Windows Service without the desktop Software Center & Tray app (default: 0 = workstation mode)
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ServerUrl,

    [Parameter(Mandatory = $true)]
    [string]$Token,

    [Parameter(Mandatory = $false)]
    [ValidateRange(0, 1)]
    [int]$Headless = 0
)

$ErrorActionPreference = 'Stop'

$InstallDir = 'C:\Program Files\NS8-SoftwareManager'
$DataDir    = 'C:\ProgramData\NS8-SoftwareManager'
$BaseUrl    = $ServerUrl.TrimEnd('/')

Write-Host "[NS8-SWM] Preparing directories ($InstallDir, $DataDir)..."
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir    | Out-Null

# Restrict ProgramData\NS8-SoftwareManager ACL to SYSTEM and Administrators (protects device_token)
$Acl = New-Object System.Security.AccessControl.DirectorySecurity
$Acl.SetAccessRuleProtection($true, $false)
$SysRule   = New-Object System.Security.AccessControl.FileSystemAccessRule('NT AUTHORITY\SYSTEM', 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
$AdminRule = New-Object System.Security.AccessControl.FileSystemAccessRule('BUILTIN\Administrators', 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
$Acl.AddAccessRule($SysRule)
$Acl.AddAccessRule($AdminRule)
Set-Acl -Path $DataDir -AclObject $Acl

$AgentExe  = Join-Path $InstallDir 'swm-agent.exe'
$CenterExe = Join-Path $InstallDir 'swm-center.exe'

Write-Host "[NS8-SWM] Downloading swm-agent.exe from $BaseUrl/dl/agents/windows/swm-agent.exe ..."
Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/dl/agents/windows/swm-agent.exe" -OutFile $AgentExe

if ($Headless -eq 0) {
    Write-Host "[NS8-SWM] Workstation mode: downloading single-file swm-center.exe ..."
    Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/dl/agents/windows/swm-center.exe" -OutFile $CenterExe
}

Write-Host "[NS8-SWM] Enrolling endpoint with NS8-SoftwareManager..."
& $AgentExe --data-dir $DataDir enroll --server $BaseUrl --token $Token
if ($LASTEXITCODE -ne 0) {
    throw "Enrollment failed with exit code $LASTEXITCODE"
}

$ExistingSvc = Get-Service -Name 'NS8SoftwareManagerAgent' -ErrorAction SilentlyContinue
if ($null -eq $ExistingSvc) {
    Write-Host "[NS8-SWM] Registering NS8SoftwareManagerAgent Windows Service..."
    & $AgentExe --data-dir $DataDir service-install
}

$RunKey = 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run'
if ($Headless -eq 0) {
    Write-Host "[NS8-SWM] Configuring Software Center System Tray autostart for interactive desktop users..."
    Set-ItemProperty -Path $RunKey -Name 'NS8SoftwareCenter' -Value '"C:\Program Files\NS8-SoftwareManager\swm-center.exe" gui --tray-only'

    $CommonStartMenu = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs'
    $WshShell = New-Object -ComObject WScript.Shell
    $Shortcut = $WshShell.CreateShortcut((Join-Path $CommonStartMenu 'NS8 Software Center.lnk'))
    $Shortcut.TargetPath = $CenterExe
    $Shortcut.Arguments = 'gui'
    $Shortcut.WorkingDirectory = $InstallDir
    $Shortcut.Description = 'NS8 Software Center Self-Service App Store'
    $Shortcut.Save()
} else {
    Remove-ItemProperty -Path $RunKey -Name 'NS8SoftwareCenter' -ErrorAction SilentlyContinue
}

Write-Host "[NS8-SWM] Starting NS8SoftwareManagerAgent service..."
Start-Service -Name 'NS8SoftwareManagerAgent'
Write-Host "[NS8-SWM] Installation complete."
