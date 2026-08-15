$debugArg = "--remote-debugging-port=9222"
$existing = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
if ([string]::IsNullOrWhiteSpace($existing)) {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $debugArg
} elseif ($existing -notmatch "--remote-debugging-port=") {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "$existing $debugArg"
}

& "D:\Image-Line\FL Studio 2026\FL64.exe"