# 讀出 Windows 執行檔內嵌的應用程式資訊清單（RT_MANIFEST #1），確認 Common Controls 6.0.0.0 恰好一份、
# 權限宣告是 asInvoker 且 uiAccess=false（連結器預設）。
# 清單改由 build.rs 經連結器嵌入（tauri-build 那份關掉）；少了它對話框起不來，多一份連結會撞。
param([Parameter(Mandatory = $true)][string]$Exe)
$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Exe)) { throw "找不到執行檔：$Exe" }
$mt = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\mt.exe" |
  Sort-Object FullName -Descending | Select-Object -First 1
if (-not $mt) { throw "找不到 Windows SDK 的 mt.exe" }

$out = Join-Path ([System.IO.Path]::GetTempPath()) "tt-manifest-$PID.xml"
& $mt.FullName -nologo "-inputresource:$((Resolve-Path -LiteralPath $Exe).Path);#1" "-out:$out"
if ($LASTEXITCODE -ne 0) { throw "mt.exe 讀不出 $Exe 的資訊清單（exit $LASTEXITCODE）" }

$text = Get-Content -LiteralPath $out -Raw
Write-Host $text
$doc = [xml]$text
$ids = $doc.SelectNodes("//*[local-name()='assemblyIdentity']") | Where-Object {
  $_.GetAttribute("name") -eq "Microsoft.Windows.Common-Controls" -and $_.GetAttribute("version") -eq "6.0.0.0"
}
$count = @($ids).Count
if ($count -ne 1) { throw "Common-Controls 6.0.0.0 應恰好一份，實際 $count 份" }
$levels = @($doc.SelectNodes("//*[local-name()='requestedExecutionLevel']"))
if ($levels.Count -ne 1) { throw "requestedExecutionLevel 應恰好一份，實際 $($levels.Count) 份" }
$level = $levels[0].GetAttribute("level")
$uiAccess = $levels[0].GetAttribute("uiAccess")
if ($level -ne "asInvoker" -or $uiAccess -ne "false") {
  throw "權限宣告應為 asInvoker／uiAccess=false，實際 $level／$uiAccess"
}
Write-Host "資訊清單正確：Common-Controls 6.0.0.0 一份、asInvoker、uiAccess=false"
