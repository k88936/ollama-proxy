# Ollama Proxy Service Uninstaller
# Uninstalls the Windows service

$serviceName = "OllamaProxy"

# Check if NSSM is available
try {
    $nssmPath = Get-Command "nssm" -ErrorAction Stop
    Write-Host "Found NSSM at: $($nssmPath.Source)" -ForegroundColor Green
} catch {
    Write-Host "Warning: NSSM not found. You may need to manually remove the service." -ForegroundColor Yellow
}

# Stop service if it's running
try {
    Stop-Service $serviceName -Force -ErrorAction SilentlyContinue
    Write-Host "Stopped service $serviceName" -ForegroundColor Green
} catch {
    Write-Host "Service $serviceName was not running or doesn't exist" -ForegroundColor Yellow
}

# Remove the service
try {
    # Try using NSSM first
    nssm remove $serviceName confirm 2>$null
    Write-Host "Removed service $serviceName using NSSM" -ForegroundColor Green
} catch {
    # If NSSM fails, try using sc command
    try {
        sc.exe delete $serviceName 2>$null
        Write-Host "Removed service $serviceName using sc command" -ForegroundColor Green
    } catch {
        Write-Host "Failed to remove service $serviceName" -ForegroundColor Red
        Write-Host "You may need to manually remove it" -ForegroundColor Yellow
    }
}

Write-Host "Uninstallation complete!" -ForegroundColor Green