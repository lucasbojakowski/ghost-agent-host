$tries = 20

for ($i = 0; $i -lt $tries; $i++) {
    try {
        $check = curl http://localhost:9222/json
        
        if ($check -match "Gopher") {
            Write-Host "FL Studio is running with remote debugging enabled."
            # target\release\ghost-fl-runtime.exe --i-accept-live-fl-writes
            target\release\ghost-fl-workspace.exe
            break
        }
    } catch {
        Write-Host "Attempt $($i + 1) failed. Retrying..."
    }
    Start-Sleep -Seconds 2
}