# Throws an error if arguments are passed, since none are expected.
[CmdletBinding()]
Param()

$ProjectRoot = (Get-Item $PSScriptRoot).Parent.FullName

# The image name is defined once, in nix/flake.nix.
$Match = Select-String -Path (Join-Path $ProjectRoot "nix/flake.nix") `
  -Pattern '^\s*builderImage = "([^"]*)";' | Select-Object -First 1
if (-not $Match) {
  throw "couldn't find builderImage in nix/flake.nix"
}
$Image = $Match.Matches[0].Groups[1].Value

# Docker picks the image matching the host's architecture. Set
# $env:KIDNEYOS_PLATFORM (e.g. linux/amd64) to force a specific one.
$PlatformFlags = @()
if ($env:KIDNEYOS_PLATFORM) {
  $PlatformFlags = @("--platform", $env:KIDNEYOS_PLATFORM)
}

docker run @PlatformFlags --rm -it `
  -v "${ProjectRoot}:/KidneyOS" `
  -w /KidneyOS `
  "${Image}:latest"
