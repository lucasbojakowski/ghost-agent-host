param(
    [int]$Port = 9222,
    [int]$TimeoutSeconds = 30,
    [string]$Tool = 'get_session_context',
    [string]$ArgsJson = '{}'
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-call] $Message"
}

function Get-GopherTarget {
    $targets = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json" -TimeoutSec 5
    return $targets | Where-Object {
        ($_.title -match '(?i)gopher') -or ($_.url -match '(?i)gopher')
    } | Select-Object -First 1
}

function Send-WebSocketText {
    param(
        [System.Net.WebSockets.ClientWebSocket]$Socket,
        [string]$Text,
        [System.Threading.CancellationToken]$Token
    )

    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Text)
    $segment = [System.ArraySegment[byte]]::new($bytes)
    [void]$Socket.SendAsync(
        $segment,
        [System.Net.WebSockets.WebSocketMessageType]::Text,
        $true,
        $Token
    ).GetAwaiter().GetResult()
}

function Receive-WebSocketText {
    param(
        [System.Net.WebSockets.ClientWebSocket]$Socket,
        [System.Threading.CancellationToken]$Token
    )

    $buffer = New-Object byte[] 65536
    $stream = [System.IO.MemoryStream]::new()
    try {
        do {
            $segment = [System.ArraySegment[byte]]::new($buffer)
            $result = $Socket.ReceiveAsync($segment, $Token).GetAwaiter().GetResult()
            if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Close) {
                throw 'CDP WebSocket closed unexpectedly.'
            }
            if ($result.Count -gt 0) {
                $stream.Write($buffer, 0, $result.Count)
            }
        } while (-not $result.EndOfMessage)

        return [System.Text.Encoding]::UTF8.GetString($stream.ToArray())
    }
    finally {
        $stream.Dispose()
    }
}

$script:NextCdpId = 1
function Invoke-Cdp {
    param(
        [System.Net.WebSockets.ClientWebSocket]$Socket,
        [string]$Method,
        [hashtable]$Params,
        [System.Threading.CancellationToken]$Token
    )

    $id = $script:NextCdpId
    $script:NextCdpId += 1

    $request = @{
        id = $id
        method = $Method
        params = $Params
    } | ConvertTo-Json -Compress -Depth 50

    Send-WebSocketText -Socket $Socket -Text $request -Token $Token

    while ($true) {
        $raw = Receive-WebSocketText -Socket $Socket -Token $Token
        $message = $raw | ConvertFrom-Json
        if ($null -ne $message.id -and [int]$message.id -eq $id) {
            if ($null -ne $message.error) {
                throw "CDP $Method failed: $($message.error | ConvertTo-Json -Compress -Depth 20)"
            }
            return $message.result
        }
    }
}

function Convert-MaybeJson([object]$Payload) {
    if ($Payload -is [string]) {
        try {
            return $Payload | ConvertFrom-Json
        }
        catch {
            return $Payload
        }
    }
    return $Payload
}

function Invoke-FlTool {
    param(
        [System.Net.WebSockets.ClientWebSocket]$Socket,
        [string]$Name,
        [object]$Arguments,
        [System.Threading.CancellationToken]$Token
    )

    $envelope = @{
        jsonrpc = '2.0'
        id = 1
        method = 'tools/call'
        params = @{
            name = $Name
            arguments = $Arguments
        }
    }
    $requestJson = $envelope | ConvertTo-Json -Compress -Depth 50

    $expression = @"
(() => {
  function ghostGetScriptHandler() {
    try {
      if (typeof script_handler === 'object' && script_handler) return script_handler;
    } catch (_) {}
    try {
      if (window.chrome && window.chrome.webview && window.chrome.webview.hostObjects)
        return window.chrome.webview.hostObjects.script_handler || null;
    } catch (_) {}
    return null;
  }

  const request = $requestJson;

  return new Promise((resolve, reject) => {
    const sh = ghostGetScriptHandler();
    if (!sh) return reject(new Error('script_handler unavailable'));

    const helper = window.flHelper = window.flHelper || {};
    const previous = helper.onRunJson;
    let timer = null;

    const restore = () => {
      if (timer) clearTimeout(timer);
      if (typeof previous === 'function') helper.onRunJson = previous;
      else delete helper.onRunJson;
    };

    helper.onRunJson = payload => {
      restore();
      resolve(payload);
    };

    timer = setTimeout(() => {
      restore();
      reject(new Error('runJson timeout'));
    }, 20000);

    try {
      sh.runJson = JSON.stringify(request);
    } catch (error) {
      restore();
      reject(error);
    }
  });
})()
"@

    $result = Invoke-Cdp -Socket $Socket -Method 'Runtime.evaluate' -Params @{
        expression = $expression
        returnByValue = $true
        awaitPromise = $true
    } -Token $Token

    if ($null -ne $result.exceptionDetails) {
        throw "FL tool call threw: $($result.exceptionDetails | ConvertTo-Json -Compress -Depth 30)"
    }

    return Convert-MaybeJson $result.result.value
}

try {
    $arguments = $ArgsJson | ConvertFrom-Json
}
catch {
    throw "ArgsJson must be valid JSON. Received: $ArgsJson"
}

Write-Step "Looking for Gopher at http://127.0.0.1:$Port/json ..."
$target = Get-GopherTarget
if ($null -eq $target) {
    throw 'Gopher CDP target not found. Launch FL with WebView2 debugging enabled and open Gopher first.'
}

Write-Step "Found target '$($target.title)'"
$cts = [System.Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds($TimeoutSeconds))
$ws = [System.Net.WebSockets.ClientWebSocket]::new()

try {
    $ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl, $cts.Token).GetAwaiter().GetResult() | Out-Null
    Write-Step 'CDP WebSocket connected.'

    [void](Invoke-Cdp -Socket $ws -Method 'Runtime.enable' -Params @{} -Token $cts.Token)

    Write-Step "Calling FL native tool '$Tool' ..."
    $result = Invoke-FlTool -Socket $ws -Name $Tool -Arguments $arguments -Token $cts.Token
    Write-Step "Tool '$Tool' returned successfully."
    $result | ConvertTo-Json -Depth 50
}
finally {
    if ($ws.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        try {
            $ws.CloseAsync(
                [System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure,
                'done',
                [System.Threading.CancellationToken]::None
            ).GetAwaiter().GetResult() | Out-Null
        }
        catch {}
    }
    $ws.Dispose()
    $cts.Dispose()
}
