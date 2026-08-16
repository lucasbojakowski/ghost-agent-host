$debugArg = "--remote-debugging-port=9222"
$existing = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
if ([string]::IsNullOrWhiteSpace($existing)) {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $debugArg
} elseif ($existing -notmatch "--remote-debugging-port=") {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "$existing $debugArg"
}

& "D:\Image-Line\FL Studio 2026\FL64.exe"

Start-Sleep -Seconds 5
$tries = 5

for ($i = 0; $i -lt $tries; $i++) {
    try {
        $check = curl http://localhost:9222/json
        
        if ($check -match "Gopher") {
            Write-Host "FL Studio is running with remote debugging enabled."
            cargo run -p ghost-fl-agent -- --i-accept-live-fl-writes
            break
        }
    } catch {
        Write-Host "Attempt $($i + 1) failed. Retrying..."
    }
    Start-Sleep -Seconds 2
}

