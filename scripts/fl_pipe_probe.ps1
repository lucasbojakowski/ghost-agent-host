$pipe = [System.IO.Pipes.NamedPipeServerStream]::new(
    "GhostBridgeProbe2",
    [System.IO.Pipes.PipeDirection]::InOut,
    1,
    [System.IO.Pipes.PipeTransmissionMode]::Byte,
    [System.IO.Pipes.PipeOptions]::Asynchronous
)

Write-Host "waiting for FL..."
$pipe.WaitForConnection()
Write-Host "FL CONNECTED"
Start-Sleep -Seconds 30
$pipe.Dispose()