$ErrorActionPreference = 'Stop'
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
New-Item -ItemType Directory -Force -Path dist | Out-Null
Copy-Item -Force target\wasm32-unknown-unknown\release\cart.wasm dist\cart.wasm
Write-Host "Built dist/cart.wasm"
