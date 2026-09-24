#!/usr/bin/env python3
"""Teste de fumaça do app instalado (F09-01) — Linux, macOS e Windows.

Prova, no pacote de verdade, o que a Fase 09 lista como risco: os sidecars `aisense` e
`aisense-mcp` foram empacotados, têm permissão de execução, estão no `PATH` do agente e
falam com o barramento. Sobe o app instalado com um diretório de dados novo, semeia uma
equipe com um agente `custom` que roda `aisense whoami` e o `initialize` do `aisense-mcp`,
liga "religar os agentes" (F08-06) para o próprio app iniciá-lo sem clique, e espera a
saída dos dois num arquivo.

Uso: scripts/smoke-install.py <executável do app instalado> [--timeout 90]
  Linux:   /usr/bin/aisense-app            (depois de `dpkg -i`), ou o AppImage
  macOS:   /Applications/AISENSE.app/Contents/MacOS/aisense-app
  Windows: "%LOCALAPPDATA%\\AISENSE\\aisense-app.exe"  (depois do instalador NSIS)
No Linux sem tela, roda sob `xvfb-run`.
"""
import argparse
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

WINDOWS = sys.platform == "win32"
TEAM = "team_SMOKE"
AGENT = "agent_SMOKE"
HANDLE = "fumaca"
MCP_INIT = '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'


def launch(binary: str, home: Path, log: Path) -> subprocess.Popen:
    env = dict(os.environ, AISENSE_HOME=str(home), AISENSE_LOG="info")
    cmd = [binary]
    if sys.platform.startswith("linux") and not os.environ.get("DISPLAY"):
        cmd = ["xvfb-run", "-a", "-s", "-screen 0 1440x900x24", binary]
    extra = (
        {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
        if WINDOWS
        else {"start_new_session": True}
    )
    return subprocess.Popen(cmd, env=env, stdout=log.open("w"), stderr=subprocess.STDOUT, **extra)


def stop(proc: subprocess.Popen) -> None:
    if WINDOWS:
        subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)], capture_output=True)
    else:
        import signal

        try:
            os.killpg(proc.pid, signal.SIGTERM)
            proc.wait(timeout=10)
        except Exception:
            os.killpg(proc.pid, signal.SIGKILL)
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        pass


def migrated(db: Path) -> bool:
    if not db.exists():
        return False
    try:
        con = sqlite3.connect(db, timeout=1)
        try:
            return con.execute("SELECT COUNT(*) FROM agents").fetchone() is not None
        finally:
            con.close()
    except sqlite3.Error:
        return False


def agent_command(out: Path) -> list[str]:
    """O que o agente roda: grava a saída dos dois sidecars e fica vivo."""
    if WINDOWS:
        return [
            "cmd",
            "/c",
            f'aisense whoami > "{out}" 2>&1 & echo {MCP_INIT}| aisense-mcp >> "{out}" 2>&1'
            " & ping -n 600 127.0.0.1 > nul",
        ]
    return [
        "sh",
        "-c",
        f"aisense whoami > '{out}' 2>&1; printf '%s\\n' '{MCP_INIT}' | aisense-mcp >> '{out}' 2>&1;"
        " sleep 600",
    ]


def seed(home: Path, out: Path) -> None:
    work = home / "work"
    work.mkdir(exist_ok=True)
    now = int(time.time() * 1000)
    db = sqlite3.connect(home / "aisense.db")
    db.execute(
        "INSERT INTO teams (id, name, workdir, created_at, updated_at) VALUES (?, 'Fumaça', ?, ?, ?)",
        (TEAM, str(work), now, now),
    )
    db.execute(
        "INSERT INTO agents (id, team_id, handle, name, adapter_id, args, color, restart_policy,"
        " created_at, updated_at) VALUES (?, ?, ?, 'Fumaça', 'custom', ?, 'cyan', 'never', ?, ?)",
        (AGENT, TEAM, HANDLE, json.dumps(agent_command(out)), now, now),
    )
    db.commit()
    db.close()
    settings = home / "settings.json"
    current = json.loads(settings.read_text()) if settings.exists() else {}
    current.update(
        {
            "onboardingDone": True,
            "session": {
                **current.get("session", {}),
                "restoreLastTeam": True,
                "relaunchAgents": True,
                "lastTeam": TEAM,
                "runningAgents": [AGENT],
            },
        }
    )
    settings.write_text(json.dumps(current))


def wait_for(check, timeout: float) -> bool:
    end = time.time() + timeout
    while time.time() < end:
        if check():
            return True
        time.sleep(0.2)
    return False


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary")
    parser.add_argument("--timeout", type=float, default=90)
    args = parser.parse_args()
    if not Path(args.binary).exists() and not shutil.which(args.binary):
        print(f"não achei o app em {args.binary}", file=sys.stderr)
        return 2

    home = Path(tempfile.mkdtemp(prefix="aisense-smoke-"))
    out = home / "sidecars.txt"
    ok = False
    try:
        # 1ª subida: o app cria o banco e migra.
        first = launch(args.binary, home, home / "first.log")
        if not wait_for(lambda: migrated(home / "aisense.db"), args.timeout):
            print("o app não criou o banco", file=sys.stderr)
            return report(home, False)
        time.sleep(2)
        stop(first)

        # 2ª subida: religa o agente, que chama os sidecars.
        seed(home, out)
        app = launch(args.binary, home, home / "run.log")

        def done() -> bool:
            text = out.read_text(errors="ignore") if out.exists() else ""
            return f"@{HANDLE}" in text and '"serverInfo"' in text

        ok = wait_for(done, args.timeout)
        stop(app)
        return report(home, ok)
    finally:
        if ok:
            shutil.rmtree(home, ignore_errors=True)


def report(home: Path, ok: bool) -> int:
    out = home / "sidecars.txt"
    print("saída dos sidecars:")
    print(out.read_text(errors="ignore") if out.exists() else "(nenhuma)")
    if ok:
        print("OK: aisense whoami e aisense-mcp responderam de dentro de um agente do app instalado")
        return 0
    for name in ("first.log", "run.log", "logs/aisense-app.log"):
        path = home / name
        if path.exists():
            print(f"--- {name} ---")
            print(path.read_text(errors="ignore")[-6000:])
    print(f"FALHOU (dados em {home})", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
