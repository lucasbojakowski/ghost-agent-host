param(
    [int]$Port = 9222,
    [int]$TimeoutSeconds = 30
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-probe] $Message"
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
    $Socket.SendAsync(
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
    } | ConvertTo-Json -Compress -Depth 40

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

Write-Step "Looking for Gopher at http://127.0.0.1:$Port/json ..."
$target = Get-GopherTarget
if ($null -eq $target) {
    throw 'Gopher CDP target not found. Open Gopher in FL Studio and rerun this probe.'
}

Write-Step "Found target '$($target.title)'"
Write-Step "Connecting to $($target.webSocketDebuggerUrl)"

$cts = [System.Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds($TimeoutSeconds))
$ws = [System.Net.WebSockets.ClientWebSocket]::new()

try {
    $ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl, $cts.Token).GetAwaiter().GetResult()
    Write-Step 'CDP WebSocket connected.'

    [void](Invoke-Cdp -Socket $ws -Method 'Runtime.enable' -Params @{} -Token $cts.Token)

    $probeExpression = @'
(() => {
  let direct = false;
  let projected = false;
  try { direct = typeof script_handler === 'object' && !!script_handler; } catch (_) {}
  try {
    projected = !!(window.chrome && window.chrome.webview && window.chrome.webview.hostObjects && window.chrome.webview.hostObjects.script_handler);
  } catch (_) {}
  return {
    present: direct || projected,
    direct,
    projected,
    title: document.title,
    href: location.href
  };
})()
'@

    Write-Step 'Probing script_handler ...'
    $probeResult = Invoke-Cdp -Socket $ws -Method 'Runtime.evaluate' -Params @{
        expression = $probeExpression
        returnByValue = $true
        awaitPromise = $false
    } -Token $cts.Token

    if ($null -ne $probeResult.exceptionDetails) {
        throw "script_handler probe threw: $($probeResult.exceptionDetails | ConvertTo-Json -Compress -Depth 20)"
    }

    $probe = $probeResult.result.value
    Write-Host ($probe | ConvertTo-Json -Depth 20)
    if (-not $probe.present) {
        throw 'Gopher target is reachable, but script_handler is not visible in this JavaScript context.'
    }

    $catalogExpression = @'
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

  return new Promise((resolve, reject) => {
    const sh = ghostGetScriptHandler();
    if (!sh) return reject(new Error('script_handler unavailable'));

    const helper = window.flHelper = window.flHelper || {};
    const previous = helper.onMCPTools;
    let timer = null;

    const restore = () => {
      if (timer) clearTimeout(timer);
      if (typeof previous === 'function') helper.onMCPTools = previous;
      else delete helper.onMCPTools;
    };

    helper.onMCPTools = payload => {
      restore();
      resolve(payload);
    };

    timer = setTimeout(() => {
      restore();
      reject(new Error('MCPTools timeout'));
    }, 20000);

    try {
      sh.MCPTools = '1';
    } catch (error) {
      restore();
      reject(error);
    }
  });
})()
'@

    Write-Step 'Requesting FL native MCP tool catalog ...'
    $catalogResult = Invoke-Cdp -Socket $ws -Method 'Runtime.evaluate' -Params @{
        expression = $catalogExpression
        returnByValue = $true
        awaitPromise = $true
    } -Token $cts.Token

    if ($null -ne $catalogResult.exceptionDetails) {
        throw "MCPTools request threw: $($catalogResult.exceptionDetails | ConvertTo-Json -Compress -Depth 20)"
    }

    $catalogPayload = $catalogResult.result.value
    if ($catalogPayload -is [string]) {
        try {
            $catalog = $catalogPayload | ConvertFrom-Json
        }
        catch {
            $catalog = $catalogPayload
        }
    }
    else {
        $catalog = $catalogPayload
    }

    Write-Step 'MCP tool catalog received.'
    $catalog | ConvertTo-Json -Depth 50
}
finally {
    if ($ws.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        try {
            $ws.CloseAsync(
                [System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure,
                'done',
                [System.Threading.CancellationToken]::None
            ).GetAwaiter().GetResult()
        }
        catch {}
    }
    $ws.Dispose()
    $cts.Dispose()
}
