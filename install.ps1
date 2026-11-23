# Instalador do GameFlow para Windows (PowerShell).
#   irm https://raw.githubusercontent.com/KMC1525/GameFlow/main/install.ps1 | iex
#
# Variáveis opcionais:  $env:GAMEFLOW_VERSION = "v0.1.0"   $env:GAMEFLOW_INSTALL_DIR = "C:\Ferramentas\GameFlow"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$repo = 'KMC1525/GameFlow'

function Say($m)  { Write-Host "==> $m" -ForegroundColor Blue }
function Warn($m) { Write-Host "aviso: $m" -ForegroundColor Yellow }

$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') { 'aarch64' } else { 'x86_64' }
$target = "$arch-pc-windows-msvc"
$asset = "gameflow-$target.zip"
$base = if ($env:GAMEFLOW_VERSION) { "https://github.com/$repo/releases/download/$($env:GAMEFLOW_VERSION)" } else { "https://github.com/$repo/releases/latest/download" }
$installDir = if ($env:GAMEFLOW_INSTALL_DIR) { $env:GAMEFLOW_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'GameFlow\bin' }

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("gameflow-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Say "Baixando GameFlow para $target…"
    try { Invoke-WebRequest "$base/$asset" -OutFile (Join-Path $tmp $asset) }
    catch {
        Warn "download indisponível ($base/$asset)"
        if (Get-Command cargo -ErrorAction SilentlyContinue) {
            Say 'Compilando a partir do código-fonte com cargo (pode levar alguns minutos)…'
            cargo install --git "https://github.com/$repo" --locked --force
            Say 'Pronto! Rode:  gameflow'; return
        }
        throw "Não encontrei uma versão publicada. Instale o Rust (https://rustup.rs) e rode: cargo install --git https://github.com/$repo --locked"
    }

    try {
        Invoke-WebRequest "$base/$asset.sha256" -OutFile (Join-Path $tmp "$asset.sha256")
        $want = ((Get-Content (Join-Path $tmp "$asset.sha256") -Raw).Trim() -split '\s+')[0].ToLower()
        $got = (Get-FileHash (Join-Path $tmp $asset) -Algorithm SHA256).Hash.ToLower()
        if ($want -ne $got) { throw "verificação de integridade falhou (esperado $want, obtido $got)" }
        Say 'Integridade verificada (SHA-256).'
    } catch [System.Net.WebException] { Warn 'esta versão não publicou checksum; pulando a verificação' }

    Expand-Archive (Join-Path $tmp $asset) -DestinationPath $tmp -Force
    $exe = Get-ChildItem $tmp -Recurse -Filter 'gameflow.exe' | Select-Object -First 1
    if (-not $exe) { throw 'o pacote baixado não contém gameflow.exe' }
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item $exe.FullName (Join-Path $installDir 'gameflow.exe') -Force
    Say "Instalado em $installDir\gameflow.exe"

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($userPath -split ';') -notcontains $installDir) {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$installDir", 'User')
        Say 'Adicionei a pasta ao seu PATH. Abra um novo terminal para usar o comando.'
    }
    Say 'Tudo pronto! Para começar, rode:  gameflow'
    Say 'Para atualizar no futuro:        gameflow update'
} finally { Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue }
