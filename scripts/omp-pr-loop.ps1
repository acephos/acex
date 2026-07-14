param(
    [string]$BaseBranch = "master",
    [string]$Remote = "origin",
    [int]$MaxIters = 40,
    [string]$Prompt = "continue from the last checkpoint",
    [string]$Profile = "omp-pr-loop",
    [switch]$AutoMerge,
    [switch]$NoWaitForMerge
)

$ErrorActionPreference = "Stop"

function Invoke-Checked {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Command)
    & $Command[0] @($Command[1..($Command.Count - 1)])
    if ($LASTEXITCODE -ne 0) {
        throw "command failed ($LASTEXITCODE): $($Command -join ' ')"
    }
}

function Invoke-OmpContinuation {
    param(
        [string]$ProfileName,
        [string]$PromptText
    )
    & omp --profile $ProfileName --no-session -p $PromptText
    if ($LASTEXITCODE -ne 0) {
        throw "omp continuation failed ($LASTEXITCODE)"
    }
}

function Assert-Command {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "missing required command: $Name"
    }
}

function Get-BashCommand {
    $candidates = @(
        "$env:ProgramFiles\Git\usr\bin\bash.exe",
        "$env:ProgramFiles\Git\bin\bash.exe",
        "bash"
    )
    foreach ($candidate in $candidates) {
        $cmd = Get-Command $candidate -ErrorAction SilentlyContinue
        if ($cmd) {
            return $cmd.Source
        }
    }
    throw "missing required command: bash (Git Bash recommended on Windows)"
}

function Assert-CleanTree {
    $dirty = git status --porcelain
    if ($dirty) {
        throw "working tree is dirty; start the PR loop from a clean checkout"
    }
}

function Get-Checkpoint {
    $raw = cargo run --quiet -p acex -- --checkpoint-status | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "checkpoint-status failed"
    }
    return $raw | ConvertFrom-Json
}

Assert-Command cargo
Assert-Command gh
Assert-Command git
$Bash = Get-BashCommand
Assert-Command omp

gh auth status -h github.com | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "gh is not authenticated for github.com"
}

Assert-CleanTree

for ($iter = 1; $iter -le $MaxIters; $iter++) {
    Write-Host "=== omp PR iteration $iter ==="
    Invoke-Checked git checkout $BaseBranch
    Invoke-Checked git pull --ff-only $Remote $BaseBranch

    $before = Get-Checkpoint
    if (-not $before.tracker.next_ready -or $before.tracker.next_ready.Count -eq 0) {
        Write-Host "tracker.next_ready is empty; loop complete"
        exit 0
    }

    $feature = [string]$before.tracker.next_ready[0]
    $prevHash = [string]$before.ledger.latest_hash
    $branchFeature = $feature.ToLowerInvariant() -replace '[^a-z0-9._-]', '-'
    $branch = "omp/$branchFeature-$(Get-Date -AsUTC -Format 'yyyyMMddHHmmss')"

    Invoke-Checked git checkout -b $branch
    Invoke-OmpContinuation -ProfileName $Profile -PromptText $Prompt

    Invoke-Checked $Bash scripts/verify-pr.sh --base-ref "$Remote/$BaseBranch"

    $dirty = git status --porcelain
    if (-not $dirty) {
        throw "omp iteration produced no file changes; refusing to open an empty PR"
    }

    $after = Get-Checkpoint
    $newHash = [string]$after.ledger.latest_hash
    if ($newHash -eq $prevHash) {
        throw "ledger hash did not advance; refusing to open PR"
    }
    if (-not $after.tracker.valid -or -not $after.ledger.valid) {
        throw "checkpoint-status reports invalid tracker or ledger"
    }

    Invoke-Checked git add -A
    Invoke-Checked git commit -m "omp: $feature checkpoint $($newHash.Substring(0, 12))"
    Invoke-Checked git push -u $Remote $branch

    $bodyPath = [System.IO.Path]::GetTempFileName()
    @"
Automated continuation PR for $feature.

Prompt injected into a fresh foreground omp session:

``````
$Prompt
``````

Local validation before opening this PR:

``````
scripts/verify-pr.sh --base-ref $Remote/$BaseBranch
``````

Checkpoint ledger advanced from $prevHash to $newHash.

This PR intentionally does not bypass CODEOWNERS; the foreground loop resumes only after required review, CI, and merge.
"@ | Set-Content -NoNewline -Encoding UTF8 $bodyPath

    $prUrl = gh pr create --base $BaseBranch --head $branch --title "omp: $feature continuation" --body-file $bodyPath
    if ($LASTEXITCODE -ne 0) {
        throw "gh pr create failed"
    }
    Remove-Item $bodyPath -Force
    Write-Host "opened $prUrl"

    Invoke-Checked gh pr checks $prUrl --watch --fail-fast

    if ($AutoMerge) {
        Invoke-Checked gh pr merge $prUrl --auto --squash --delete-branch
    }

    if ($NoWaitForMerge) {
        Write-Host "not waiting for merge (-NoWaitForMerge); stop here and rerun after merge"
        exit 0
    }

    Write-Host "waiting in foreground for CODEOWNERS review and merge: $prUrl"
    while ($true) {
        $mergedAt = gh pr view $prUrl --json mergedAt --jq '.mergedAt // ""'
        if ($LASTEXITCODE -ne 0) {
            throw "gh pr view failed"
        }
        if ($mergedAt) {
            Write-Host "merged at $mergedAt"
            break
        }
        $state = gh pr view $prUrl --json state --jq '.state'
        if ($state -eq "CLOSED") {
            throw "PR closed without merge: $prUrl"
        }
        Start-Sleep -Seconds 60
    }
}

throw "max iteration cap reached: $MaxIters"
