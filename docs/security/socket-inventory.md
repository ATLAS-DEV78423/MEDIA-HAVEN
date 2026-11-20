# Socket and privilege inventory — F1/F4/F6 evidence (Task C4, Lane C)

> Evidence pass. Read-only host inspection only. No service was started or
> stopped, no backend or dev server was launched, no firewall/host/network
> configuration was changed, nothing was installed, and no build or test was run.
> Repo content was treated strictly as data.

## Capture context (verbatim)

Command:

```powershell
Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz'; Write-Output "PSVersion=$($PSVersionTable.PSVersion.ToString())"; Write-Output "User=$([Security.Principal.WindowsIdentity]::GetCurrent().Name)"; Write-Output "Machine=$env:COMPUTERNAME"
```

Output:

```text
2026-11-19 20:45:24 +04:00
PSVersion=5.1.26100.9818
User=DESKTOP-VIPFNKO\Stanley
Machine=DESKTOP-VIPFNKO
```

Reference machine profile: Windows 11 (build 26100). This is a "before app
session" snapshot: the MEDIA_HAVEN application is **not running** in this pass.

---

## 1. Listening sockets — full list

Command:

```powershell
Get-NetTCPConnection -State Listen | Select-Object LocalAddress, LocalPort, OwningProcess
```

Output (verbatim):

```text
LocalAddress  LocalPort OwningProcess
------------  --------- -------------
::                49678          1736
::1               49671          5748
::                49670          3048
::                49669          3468
::                49668          2748
::                49665          1620
::                49664          1792
::                  445             4
::                  135           900
127.0.0.1         56652         16840
0.0.0.0           49678          1736
0.0.0.0           49670          3048
0.0.0.0           49669          3468
0.0.0.0           49668          2748
0.0.0.0           49665          1620
0.0.0.0           49664          1792
127.0.0.1         45287         16840
127.0.0.1         28894          5948
0.0.0.0            8082         17372
0.0.0.0            5040          7028
192.168.0.149       139             4
172.27.192.1        139             4
0.0.0.0             135           900
```

Enrichment command (read-only, resolves owning process names):

```powershell
Get-NetTCPConnection -State Listen | Select-Object LocalAddress, LocalPort, OwningProcess, @{N='ProcessName';E={(Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).ProcessName}} | Sort-Object LocalAddress, LocalPort
```

Output (verbatim):

```text
LocalAddress  LocalPort OwningProcess ProcessName
------------  --------- ------------- -----------
::                  135           900 svchost
::                  445             4 System
::                49664          1792 lsass
::                49665          1620 wininit
::                49668          2748 svchost
::                49669          3468 svchost
::                49670          3048 spoolsv
::                49678          1736 services
::1               49671          5748 jhi_service
0.0.0.0             135           900 svchost
0.0.0.0            5040          7028 svchost
0.0.0.0            8082         17372 pythonw
0.0.0.0           49664          1792 lsass
0.0.0.0           49665          1620 wininit
0.0.0.0           49668          2748 svchost
0.0.0.0           49669          3468 svchost
0.0.0.0           49670          3048 spoolsv
0.0.0.0           49678          1736 services
127.0.0.1         28894          5948 opencode
127.0.0.1         45287         16840 Code
127.0.0.1         56652         16840 Code
172.27.192.1        139             4 System
192.168.0.149       139             4 System
```

### Interpretation

- **Loopback-only listeners** (127.0.0.1 / ::1): ports 56652 and 45287 owned by
  `Code` (the editor), 28894 owned by `opencode` (this agent harness), and 49671
  owned by `jhi_service` (a pre-existing system service on ::1). None belongs to
  MEDIA_HAVEN.
- **Wildcard listeners** (0.0.0.0 / ::): all are pre-existing Windows OS/services —
  `System` (SMB 445, NetBIOS 139), `svchost` (RPC 135, service 5040), `lsass`,
  `wininit`, `spoolsv`, `services`, and high ephemeral RPC dynamic ports
  (49664–49678). One non-OS wildcard listener exists: `0.0.0.0:8082` owned by
  `pythonw` (PID 17372) — an unrelated external process, **not** the app.
- **LAN-address listeners**: `192.168.0.149:139` and `172.27.192.1:139` owned by
  `System` are NetBIOS over the physical and WSL virtual (vEthernet) adapters.
  `172.27.192.1` is a WSL-related address; its port-139 listener is a Windows
  `System` service, not the app and not the Docker daemon.
- **Expected vs observed:** the app is expected to bind loopback only. Observed:
  there is **no MEDIA_HAVEN listener at all** because the application is not
  running in this pass. This is a "before app session" baseline, not an app PASS.

> **Caveat — Docker/WSL and other external listeners are not the app.** Another
> agent is starting the Docker workspace on port 18081 concurrently. Any listener
> on 18081, any WSL2 adapter listener (e.g. 172.27.192.x), any `dockerd`/`com.docker.*`
> process, or any other process observed during later passes is EXTERNAL to
> MEDIA_HAVEN and must not be attributed to it. Only a socket whose owning process
> is the MEDIA_HAVEN core/host binary would be an app listener. Docker Engine on
> Windows/WSL2 normally uses a named pipe (`//./pipe/docker_engine`), not a TCP
> port, so absence of a Docker TCP listener here is expected.

---

## 2. Docker control ports 2375 / 2376

Command:

```powershell
Get-NetTCPConnection -State Listen -LocalPort 2375,2376 -ErrorAction SilentlyContinue | Select-Object LocalAddress, LocalPort, OwningProcess; Write-Output "EXIT_MARKER_2375_2376_CHECK_DONE"
```

Output (verbatim):

```text
EXIT_MARKER_2375_2376_CHECK_DONE
```

**Interpretation:** no row was returned — there is no listener on 2375 or 2376.
**Expected: none. Observed: none — PASS for the observed state.** The marker line
confirms the command completed; the empty result set is the evidence.

---

## 3. Windows Firewall — before snapshot (counts only, no change)

Command:

```powershell
$all = Get-NetFirewallRule; $enabled = $all | Where-Object { $_.Enabled -eq $true }; Write-Output "TotalRules=$($all.Count)"; Write-Output "EnabledRules=$($enabled.Count)"
```

Output (verbatim):

```text
TotalRules=560
EnabledRules=306
```

**Interpretation:** read-only baseline captured: 560 total rules, 306 enabled.
No rule was added, removed, enabled or disabled by this task. Because the app was
not run in this pass, this cannot be used as evidence that "the app does not
change firewall rules" — that claim remains unverified until an app-session
before/after comparison is performed. `Development.md` §8.3 requires that the app
not silently alter firewall rules; nothing here contradicts it, and nothing here
proves it.

---

## 4. Process elevation

Command:

```powershell
([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
```

Output (verbatim):

```text
False
```

**Interpretation:** the agent shell is **not elevated**. This is consistent with
the contract that the app must not require administrator privileges for normal
operation (`Development.md` §8.2). This is a statement about the inspection shell,
not a test of the app: the app itself was not launched in this pass, so "app runs
un-elevated" remains **NOT RUN**.

---

## 5. Session phases

| Phase | Status | Reason |
|---|---|---|
| Before app session (baseline) | RECORDED — this document | Read-only snapshot above, 2026-11-19 20:45:24 +04:00. |
| During app session | **NOT RUN** | The application is not running in this pass; no listener/privilege observation was possible. |
| After app session | **NOT RUN** | No app session occurred, so no teardown/residual-socket or firewall-delta check was possible. |

A complete F4 claim (loopback-only across the full lifecycle, no 2375/2376, no
firewall delta, no elevation requirement) requires the "during" and "after"
snapshots with the app actually running; those are gated on a separate pass.

## Security implication review

- Ports/addresses recorded; no listener found on Docker control ports.
- No secrets, tokens or personal data recorded beyond the local machine/user name
  already present in the capture context (host evidence, not renderer-facing).
- No host mutation of any kind. No elevation was used (`IsInRole = False`).
