# Build the Windows text service and engine (run on Windows, in a Visual Studio Build Tools developer prompt).
#   .\windows\build.ps1 [-Release]
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."
$dist = Join-Path $PSScriptRoot "dist"
New-Item -ItemType Directory -Force "$dist\x64", "$dist\x86", "$dist\arm64" | Out-Null
Push-Location $root
try {
    foreach ($t in "x86_64-pc-windows-msvc", "i686-pc-windows-msvc", "aarch64-pc-windows-msvc") { rustup target add $t }
    # the text service, once per architecture: a 32-bit app can only load a 32-bit DLL
    cargo build --release -p khtsf --target x86_64-pc-windows-msvc
    cargo build --release -p khtsf --target i686-pc-windows-msvc
    cargo build --release -p khtsf --target aarch64-pc-windows-msvc
    Copy-Item target\x86_64-pc-windows-msvc\release\khtsf.dll  "$dist\x64\khtsf.dll"
    Copy-Item target\i686-pc-windows-msvc\release\khtsf.dll    "$dist\x86\khtsf.dll"
    Copy-Item target\aarch64-pc-windows-msvc\release\khtsf.dll "$dist\arm64\khtsf.dll"
    # one engine per user session; x64 also runs on ARM64 Windows
    cargo build --release -p khengine --target x86_64-pc-windows-msvc
    Copy-Item target\x86_64-pc-windows-msvc\release\khengine.exe "$dist\x64\khengine.exe"
    # compiled language data (Latin, Khmer, and the English glossary)
    cargo run --release -p khcore --bin khbuild -- data\data.json data\lexicon.json "$dist\x64\khdata.bin" data\english.json
    Write-Host "built into $dist"
} finally { Pop-Location }
