# Ollama Proxy Service Installer
# Installs the proxy as a Windows service for the current user

# Get the current directory
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$exePath = Join-Path $scriptDir "ollama-proxy.exe"

# Check if the executable exists
if (!(Test-Path $exePath)) {
    Write-Host "Error: ollama-proxy.exe not found at $exePath" -ForegroundColor Red
    Write-Host "Please build the project first with: cargo build --release" -ForegroundColor Yellow
    exit 1
}

# Create the service
$serviceName = "ollama-proxy"
$username = $env:USERNAME

# Using NSSM (Non-Sucking Service Manager) to install as user service
# Check if nssm is available
try {
    $nssmPath = Get-Command "nssm" -ErrorAction Stop
    Write-Host "Found NSSM at: $($nssmPath.Source)" -ForegroundColor Green
} catch {
    Write-Host "Error: NSSM not found. Please install NSSM first." -ForegroundColor Red
    Write-Host "You can download it from: https://nssm.cc/download" -ForegroundColor Yellow
    exit 1
}

# Stop service if it's already running
try {
    Stop-Service $serviceName -Force -ErrorAction SilentlyContinue
    Write-Host "Stopped existing service (if running)" -ForegroundColor Green
} catch {
    # Service probably doesn't exist yet, which is fine
}

# Install the service
try {
    # Remove existing service if it exists
    nssm remove $serviceName confirm 2>$null
    
    # Install new service
    nssm install $serviceName $exePath
    nssm set $serviceName Description "Ollama Proxy Service for user $username"
    nssm set $serviceName Start SERVICE_AUTO_START
    nssm set $serviceName AppDirectory $scriptDir
    nssm set $serviceName AppEnvironmentExtra "USERPROFILE=$env:USERPROFILE"
    # Set service recovery options to restart unless manually stopped
    nssm set $serviceName AppExit Default Restart
    nssm set $serviceName AppRestartDelay 10000
    nssm set $serviceName AppThrottle 15000
    nssm set $serviceName AppExitPostScript 1
    
    # Start the service
    Start-Service $serviceName
    
    Write-Host "Successfully installed and started $serviceName" -ForegroundColor Green
    Write-Host "Service is now running as user $username" -ForegroundColor Green
    Write-Host "Service will restart automatically unless manually stopped" -ForegroundColor Green
} catch {
    Write-Host "Error installing service: $_" -ForegroundColor Red
    exit 1
}

Write-Host "Installation complete!" -ForegroundColor Green