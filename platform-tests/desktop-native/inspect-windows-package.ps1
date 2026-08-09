Set-StrictMode -Version Latest

function Read-U16([byte[]]$Bytes, [long]$Offset) {
  if ($Offset -lt 0 -or $Offset + 2 -gt $Bytes.LongLength) { throw "PE u16 read outside image at $Offset" }
  return [uint16]([uint16]$Bytes[[int]$Offset] -bor ([uint16]$Bytes[[int]($Offset + 1)] -shl 8))
}

function Read-U32([byte[]]$Bytes, [long]$Offset) {
  if ($Offset -lt 0 -or $Offset + 4 -gt $Bytes.LongLength) { throw "PE u32 read outside image at $Offset" }
  return [uint32](
    [uint32]$Bytes[[int]$Offset] -bor
    ([uint32]$Bytes[[int]($Offset + 1)] -shl 8) -bor
    ([uint32]$Bytes[[int]($Offset + 2)] -shl 16) -bor
    ([uint32]$Bytes[[int]($Offset + 3)] -shl 24)
  )
}

function Read-AsciiZ([byte[]]$Bytes, [long]$Offset, [int]$MaximumLength = 512) {
  if ($Offset -lt 0 -or $Offset -ge $Bytes.LongLength) { throw "PE string starts outside image at $Offset" }
  $end = $Offset
  while ($end -lt $Bytes.LongLength -and $end -lt $Offset + $MaximumLength -and $Bytes[[int]$end] -ne 0) {
    $value = $Bytes[[int]$end]
    if ($value -lt 0x20 -or $value -gt 0x7e) { throw "PE string contains non-ASCII byte at $end" }
    $end += 1
  }
  if ($end -ge $Bytes.LongLength -or $end -ge $Offset + $MaximumLength) { throw "PE string is not terminated within $MaximumLength bytes" }
  return [System.Text.Encoding]::ASCII.GetString($Bytes, [int]$Offset, [int]($end - $Offset))
}

function Read-PeImage([string]$Path) {
  $resolved = (Resolve-Path -LiteralPath $Path).Path
  $bytes = [System.IO.File]::ReadAllBytes($resolved)
  if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) { throw "$resolved is not an MZ image" }
  $peOffset = [long](Read-U32 $bytes 0x3c)
  if ($peOffset -lt 0x40 -or $peOffset + 24 -gt $bytes.LongLength) { throw "$resolved has an invalid PE header offset" }
  if ($bytes[[int]$peOffset] -ne 0x50 -or $bytes[[int]($peOffset + 1)] -ne 0x45 -or
      $bytes[[int]($peOffset + 2)] -ne 0 -or $bytes[[int]($peOffset + 3)] -ne 0) {
    throw "$resolved has no PE signature"
  }
  $coff = $peOffset + 4
  $signature = Read-U32 $bytes $peOffset
  if ($signature -ne 0x00004550) { throw "$resolved has the wrong PE signature" }
  $machine = Read-U16 $bytes $coff
  $sectionCount = Read-U16 $bytes ($coff + 2)
  $optionalSize = Read-U16 $bytes ($coff + 16)
  $characteristics = Read-U16 $bytes ($coff + 18)
  if ($sectionCount -lt 1 -or $sectionCount -gt 96) { throw "$resolved has an invalid section count $sectionCount" }
  $optional = $coff + 20
  if ($optionalSize -lt 240 -or $optional + $optionalSize -gt $bytes.LongLength) { throw "$resolved has a truncated PE32+ optional header" }
  $magic = Read-U16 $bytes $optional
  $entryPoint = Read-U32 $bytes ($optional + 16)
  $subsystem = Read-U16 $bytes ($optional + 68)
  $sizeOfHeaders = Read-U32 $bytes ($optional + 60)
  $directoryCount = Read-U32 $bytes ($optional + 108)
  if ($magic -ne 0x20b -or $directoryCount -lt 16) { throw "$resolved is not a complete PE32+ image" }
  $directories = @()
  for ($index = 0; $index -lt 16; $index += 1) {
    $directoryOffset = $optional + 112 + ($index * 8)
    $directories += [pscustomobject]@{
      Rva = Read-U32 $bytes $directoryOffset
      Size = Read-U32 $bytes ($directoryOffset + 4)
    }
  }
  $sections = @()
  $sectionOffset = $optional + $optionalSize
  for ($index = 0; $index -lt $sectionCount; $index += 1) {
    $offset = $sectionOffset + ($index * 40)
    if ($offset + 40 -gt $bytes.LongLength) { throw "$resolved has a truncated section table" }
    $nameLength = 0
    while ($nameLength -lt 8 -and $bytes[[int]($offset + $nameLength)] -ne 0) { $nameLength += 1 }
    $name = [System.Text.Encoding]::ASCII.GetString($bytes, [int]$offset, $nameLength)
    $section = [pscustomobject]@{
      Name = $name
      VirtualSize = Read-U32 $bytes ($offset + 8)
      VirtualAddress = Read-U32 $bytes ($offset + 12)
      RawSize = Read-U32 $bytes ($offset + 16)
      RawOffset = Read-U32 $bytes ($offset + 20)
    }
    if ([long]$section.RawOffset + [long]$section.RawSize -gt $bytes.LongLength) { throw "$resolved section $name exceeds the image" }
    $sections += $section
  }
  return [pscustomobject]@{
    Path = $resolved
    Bytes = $bytes
    Signature = $signature
    Machine = $machine
    Characteristics = $characteristics
    OptionalMagic = $magic
    EntryPoint = $entryPoint
    Subsystem = $subsystem
    SizeOfHeaders = $sizeOfHeaders
    Directories = $directories
    Sections = $sections
  }
}

function Convert-RvaToOffset($Image, [uint32]$Rva) {
  if ($Rva -lt $Image.SizeOfHeaders -and $Rva -lt $Image.Bytes.Length) { return [long]$Rva }
  foreach ($section in $Image.Sections) {
    $span = [Math]::Max([long]$section.VirtualSize, [long]$section.RawSize)
    if ([long]$Rva -ge [long]$section.VirtualAddress -and [long]$Rva -lt [long]$section.VirtualAddress + $span) {
      $delta = [long]$Rva - [long]$section.VirtualAddress
      if ($delta -ge [long]$section.RawSize) { throw "RVA 0x$($Rva.ToString('x')) has no raw backing in $($Image.Path)" }
      return [long]$section.RawOffset + $delta
    }
  }
  throw "RVA 0x$($Rva.ToString('x')) is outside every section in $($Image.Path)"
}

function Get-PeImports($Image) {
  $directory = $Image.Directories[1]
  if ($directory.Rva -eq 0 -and $directory.Size -eq 0) { return @() }
  if ($directory.Rva -eq 0 -or $directory.Size -lt 20) { throw "$($Image.Path) has an invalid import directory" }
  $offset = Convert-RvaToOffset $Image $directory.Rva
  $imports = @()
  $terminated = $false
  $descriptorCount = [Math]::Min([long]512, [Math]::Floor([double]$directory.Size / 20))
  for ($index = 0; $index -lt $descriptorCount; $index += 1) {
    $entry = $offset + ($index * 20)
    if ($entry + 20 -gt $Image.Bytes.LongLength) { throw "$($Image.Path) has a truncated import descriptor" }
    $originalThunk = Read-U32 $Image.Bytes $entry
    $timestamp = Read-U32 $Image.Bytes ($entry + 4)
    $forwarder = Read-U32 $Image.Bytes ($entry + 8)
    $nameRva = Read-U32 $Image.Bytes ($entry + 12)
    $firstThunk = Read-U32 $Image.Bytes ($entry + 16)
    if (($originalThunk -bor $timestamp -bor $forwarder -bor $nameRva -bor $firstThunk) -eq 0) { $terminated = $true; break }
    if ($nameRva -eq 0 -or $firstThunk -eq 0) { throw "$($Image.Path) has a malformed import descriptor" }
    $name = (Read-AsciiZ $Image.Bytes (Convert-RvaToOffset $Image $nameRva) 260).ToLowerInvariant()
    if ([System.IO.Path]::GetFileName($name) -ne $name -or $name.Contains('/') -or $name.Contains('\') -or $name.Contains(':')) {
      throw "$($Image.Path) imports a non-basename path: $name"
    }
    $imports += $name
  }
  if (-not $terminated) { throw "$($Image.Path) import descriptors have no bounded terminator" }
  if ($Image.Directories[13].Rva -ne 0 -or $Image.Directories[13].Size -ne 0) { throw "$($Image.Path) has unsupported delay-load imports" }
  return @($imports | Sort-Object -Unique)
}

function Get-PeExports($Image) {
  $directory = $Image.Directories[0]
  if ($directory.Rva -eq 0 -and $directory.Size -eq 0) {
    return [pscustomobject]@{ ModuleName = ''; FunctionCount = [uint32]0; Names = @() }
  }
  if ($directory.Rva -eq 0 -or $directory.Size -lt 40) { throw "$($Image.Path) has an invalid export directory" }
  $offset = Convert-RvaToOffset $Image $directory.Rva
  $moduleNameRva = Read-U32 $Image.Bytes ($offset + 12)
  $functionCount = Read-U32 $Image.Bytes ($offset + 20)
  $nameCount = Read-U32 $Image.Bytes ($offset + 24)
  $namesRva = Read-U32 $Image.Bytes ($offset + 32)
  $ordinalsRva = Read-U32 $Image.Bytes ($offset + 36)
  if ($functionCount -gt 4096 -or $nameCount -gt $functionCount) { throw "$($Image.Path) has invalid export counts" }
  $moduleName = Read-AsciiZ $Image.Bytes (Convert-RvaToOffset $Image $moduleNameRva) 260
  $names = @()
  if ($nameCount -gt 0) {
    $namesOffset = Convert-RvaToOffset $Image $namesRva
    $ordinalsOffset = Convert-RvaToOffset $Image $ordinalsRva
    for ($index = 0; $index -lt $nameCount; $index += 1) {
      $nameRva = Read-U32 $Image.Bytes ($namesOffset + ($index * 4))
      $ordinal = Read-U16 $Image.Bytes ($ordinalsOffset + ($index * 2))
      if ($ordinal -ge $functionCount) { throw "$($Image.Path) has an export ordinal outside the function table" }
      $names += Read-AsciiZ $Image.Bytes (Convert-RvaToOffset $Image $nameRva) 512
    }
  }
  return [pscustomobject]@{
    ModuleName = $moduleName
    FunctionCount = $functionCount
    Names = @($names | Sort-Object -Unique)
  }
}

function Test-PeHasManifestResource($Image) {
  $directory = $Image.Directories[2]
  if ($directory.Rva -eq 0 -and $directory.Size -eq 0) { return $false }
  if ($directory.Rva -eq 0 -or $directory.Size -lt 16) { throw "$($Image.Path) has an invalid resource directory" }
  $root = Convert-RvaToOffset $Image $directory.Rva
  $named = Read-U16 $Image.Bytes ($root + 12)
  $identified = Read-U16 $Image.Bytes ($root + 14)
  $entryCount = [int]$named + [int]$identified
  if ($entryCount -gt 4096 -or 16 + ($entryCount * 8) -gt $directory.Size) { throw "$($Image.Path) has an invalid resource root" }
  for ($index = $named; $index -lt $entryCount; $index += 1) {
    $identifier = Read-U32 $Image.Bytes ($root + 16 + ($index * 8))
    if (($identifier -band 0x80000000) -eq 0 -and ($identifier -band 0x7fffffff) -eq 24) { return $true }
  }
  return $false
}

function Assert-SequenceEqual([string]$Label, [string[]]$Actual, [string[]]$Expected) {
  $actualSorted = @($Actual | Sort-Object -Unique)
  $expectedSorted = @($Expected | Sort-Object -Unique)
  $equal = $actualSorted.Count -eq $expectedSorted.Count
  for ($index = 0; $equal -and $index -lt $actualSorted.Count; $index += 1) {
    $equal = $actualSorted[$index] -ceq $expectedSorted[$index]
  }
  if (-not $equal) {
    throw "$Label mismatch: actual=[$($actualSorted -join ',')] expected=[$($expectedSorted -join ',')]"
  }
}

function Assert-SystemImportAllowlist([string]$Label, [string[]]$Imports) {
  $fixed = @(
    'advapi32.dll', 'bcrypt.dll', 'bcryptprimitives.dll', 'kernel32.dll',
    'ntdll.dll', 'ole32.dll', 'shell32.dll', 'ucrtbase.dll', 'userenv.dll', 'vcruntime140.dll',
    'vcruntime140_1.dll', 'ws2_32.dll'
  )
  if ($Imports.Count -eq 0 -or $Imports -notcontains 'kernel32.dll') { throw "$Label must import kernel32.dll" }
  foreach ($name in $Imports) {
    $isApiSet = $name -match '^api-ms-win-(core|crt)-[a-z0-9-]+-l[0-9]+-[0-9]+-[0-9]+\.dll$'
    if ($fixed -notcontains $name -and -not $isApiSet) { throw "$Label has a non-allowlisted import: $name" }
  }
}

function Assert-PeContract([string]$Path, [bool]$ExpectDll, [string[]]$ExpectedExports, [string]$ExpectedModuleName) {
  $image = Read-PeImage $Path
  if ($image.Machine -ne 0x8664) { throw "$Path machine is not AMD64: 0x$($image.Machine.ToString('x'))" }
  if ($image.OptionalMagic -ne 0x20b) { throw "$Path is not PE32+" }
  if ($image.Subsystem -ne 3) { throw "$Path subsystem is not IMAGE_SUBSYSTEM_WINDOWS_CUI: $($image.Subsystem)" }
  if (($image.Characteristics -band 0x0002) -eq 0) { throw "$Path is not marked executable" }
  $isDll = ($image.Characteristics -band 0x2000) -ne 0
  if ($isDll -ne $ExpectDll) { throw "$Path DLL characteristic mismatch" }
  if ($image.EntryPoint -eq 0) { throw "$Path has no entry point" }
  $imports = @(Get-PeImports $image)
  Assert-SystemImportAllowlist $Path $imports
  $exports = Get-PeExports $image
  Assert-SequenceEqual "$Path exports" @($exports.Names) $ExpectedExports
  if ($ExpectedModuleName.Length -gt 0 -and $exports.ModuleName -ne $ExpectedModuleName) {
    throw "$Path export module name is '$($exports.ModuleName)', expected '$ExpectedModuleName'"
  }
  return [pscustomobject]@{
    Image = $image
    Imports = $imports
    Exports = @($exports.Names)
    HasManifest = Test-PeHasManifestResource $image
  }
}

function Assert-ExactInventory([string]$Directory, [string[]]$ExpectedNames) {
  $items = @(Get-ChildItem -LiteralPath $Directory -Force)
  foreach ($item in $items) {
    if ($item.PSIsContainer -or (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
      throw "unexpected directory or reparse point in package: $($item.FullName)"
    }
  }
  Assert-SequenceEqual "package inventory" @($items.Name) $ExpectedNames
}

function Assert-NoEmbeddedPath([string]$Artifact, [string[]]$ForbiddenPaths) {
  $bytes = [System.IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $Artifact).Path)
  $utf8 = [System.Text.Encoding]::UTF8.GetString($bytes)
  $utf16 = [System.Text.Encoding]::Unicode.GetString($bytes)
  $utf16Odd = if ($bytes.Length -gt 1) { [System.Text.Encoding]::Unicode.GetString($bytes, 1, $bytes.Length - 1) } else { '' }
  foreach ($path in $ForbiddenPaths) {
    foreach ($candidate in @($path, $path.Replace('\', '/'))) {
      if ($candidate.Length -gt 0 -and
          ($utf8.IndexOf($candidate, [System.StringComparison]::OrdinalIgnoreCase) -ge 0 -or
           $utf16.IndexOf($candidate, [System.StringComparison]::OrdinalIgnoreCase) -ge 0 -or
           $utf16Odd.IndexOf($candidate, [System.StringComparison]::OrdinalIgnoreCase) -ge 0)) {
        throw "$Artifact embeds forbidden build/package path $candidate"
      }
    }
  }
}

function Assert-ExternalManifestIsEffective([string]$Executable, [string]$Manifest) {
  if ((Resolve-Path -LiteralPath $Manifest).Path -ne ((Resolve-Path -LiteralPath $Executable).Path + '.manifest')) {
    throw 'external manifest is not named exactly after the executable'
  }
  $image = Read-PeImage $Executable
  if (Test-PeHasManifestResource $image) { throw 'packaged executable has an embedded manifest that overrides the external manifest' }
  [xml]$xml = Get-Content -LiteralPath $Manifest -Raw
  $namespaces = [System.Xml.XmlNamespaceManager]::new($xml.NameTable)
  $namespaces.AddNamespace('asm1', 'urn:schemas-microsoft-com:asm.v1')
  $namespaces.AddNamespace('asm3', 'urn:schemas-microsoft-com:asm.v3')
  $namespaces.AddNamespace('compat', 'urn:schemas-microsoft-com:compatibility.v1')
  $identity = $xml.SelectSingleNode('/asm1:assembly/asm1:assemblyIdentity', $namespaces)
  $execution = $xml.SelectSingleNode('/asm1:assembly/asm3:trustInfo/asm3:security/asm3:requestedPrivileges/asm3:requestedExecutionLevel', $namespaces)
  $supported = $xml.SelectSingleNode('/asm1:assembly/compat:compatibility/compat:application/compat:supportedOS', $namespaces)
  if ($null -eq $identity -or $identity.name -ne 'dev.semaprax.private.desktop-v3' -or $identity.type -ne 'win32' -or
      $null -eq $execution -or $execution.level -ne 'asInvoker' -or $execution.uiAccess -ne 'false' -or
      $null -eq $supported -or $supported.Id -ne '{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}') {
    throw 'external manifest contract changed'
  }
  if (-not ('SemapraxActivationContextProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class SemapraxActivationContextProbe {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  private struct ACTCTX {
    public uint cbSize; public uint dwFlags; public string lpSource;
    public ushort wProcessorArchitecture; public ushort wLangId;
    public string lpAssemblyDirectory; public IntPtr lpResourceName;
    public string lpApplicationName; public IntPtr hModule;
  }
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  private static extern IntPtr CreateActCtx(ref ACTCTX actctx);
  [DllImport("kernel32.dll", SetLastError = true)]
  private static extern bool ActivateActCtx(IntPtr handle, out UIntPtr cookie);
  [DllImport("kernel32.dll", SetLastError = true)]
  private static extern bool DeactivateActCtx(uint flags, UIntPtr cookie);
  [DllImport("kernel32.dll")] private static extern void ReleaseActCtx(IntPtr handle);
  public static void Validate(string manifest) {
    ACTCTX value = new ACTCTX(); value.cbSize = (uint)Marshal.SizeOf(typeof(ACTCTX)); value.lpSource = manifest;
    IntPtr handle = CreateActCtx(ref value);
    if (handle == new IntPtr(-1)) throw new Win32Exception(Marshal.GetLastWin32Error(), "CreateActCtx rejected packaged external manifest");
    try {
      UIntPtr cookie;
      if (!ActivateActCtx(handle, out cookie)) throw new Win32Exception(Marshal.GetLastWin32Error(), "ActivateActCtx rejected packaged external manifest");
      if (!DeactivateActCtx(0, cookie)) throw new Win32Exception(Marshal.GetLastWin32Error(), "DeactivateActCtx failed");
    } finally { ReleaseActCtx(handle); }
  }
}
'@
  }
  [SemapraxActivationContextProbe]::Validate((Resolve-Path -LiteralPath $Manifest).Path)
  return $xml
}

function Get-EffectiveManifest([string]$Executable, [string]$Manifest) {
  return Assert-ExternalManifestIsEffective $Executable $Manifest
}
