#!/usr/bin/env python3
"""Auditoria de performance do app real (F08-01) — Linux, sob Xvfb.

Sobe o binário do AISENSE com um diretório de dados novo, semeia uma equipe com N agentes
`custom` e liga "religar os agentes" (F08-06): na subida o próprio app inicia todos e reabre
a Sala da Equipe, sem precisar de clique. Mede:

- cold start até a Sala da Equipe na tela (log "tela da equipe");
- RAM (RSS) do app + processos do WebKit, sem contar os processos dos agentes;
- CPU do app + WebKit com os agentes ociosos e com todos escrevendo sem parar.

Uso: scripts/perf-audit.py target/release/aisense-app [--agents 12]
Precisa de `xvfb-run`. Os números são desta máquina (sem GPU): registre junto com ela.
"""
import argparse
import json
import os
import re
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

CLK = os.sysconf("SC_CLK_TCK")
PAGE = os.sysconf("SC_PAGE_SIZE")


def children(pid: int) -> list[int]:
    out = []
    for task in Path(f"/proc/{pid}/task").glob("*"):
        try:
            out += [int(c) for c in (task / "children").read_text().split()]
        except OSError:
            pass
    return out


def tree(pid: int) -> list[int]:
    seen, stack = [], [pid]
    while stack:
        p = stack.pop()
        seen.append(p)
        stack += children(p)
    return seen


def comm(pid: int) -> str:
    try:
        return Path(f"/proc/{pid}/comm").read_text().strip()
    except OSError:
        return ""


def app_processes(root: int) -> list[int]:
    """O app e o WebKit; os agentes (shells, sleep...) ficam de fora."""
    return [p for p in tree(root) if p == root or comm(p).startswith("WebKit")]


def rss_mb(pids: list[int]) -> float:
    total = 0
    for p in pids:
        try:
            total += int(Path(f"/proc/{p}/statm").read_text().split()[1]) * PAGE
        except OSError:
            pass
    return total / 1024 / 1024


def pss_mb(pids: list[int]) -> float:
    """PSS: memória compartilhada dividida entre quem a usa. Sem GPU, o WebKit carrega o
    renderizador por software (LLVM), e o RSS conta essas bibliotecas inteiras."""
    total = 0
    for p in pids:
        try:
            for line in Path(f"/proc/{p}/smaps_rollup").read_text().splitlines():
                if line.startswith("Pss:"):
                    total += int(line.split()[1]) * 1024
        except OSError:
            pass
    return total / 1024 / 1024


def cpu_ticks(pids: list[int]) -> int:
    total = 0
    for p in pids:
        try:
            fields = Path(f"/proc/{p}/stat").read_text().rsplit(")", 1)[1].split()
            total += int(fields[11]) + int(fields[12])
        except OSError:
            pass
    return total


def cpu_percent(root: int, seconds: float) -> dict[str, float]:
    """CPU de cada processo (100 = um núcleo inteiro)."""
    pids = app_processes(root)
    before = {p: cpu_ticks([p]) for p in pids}
    time.sleep(seconds)
    out: dict[str, float] = {}
    for p in pids:
        name = comm(p)
        out[name] = round(out.get(name, 0) + (cpu_ticks([p]) - before[p]) / CLK / seconds * 100, 2)
    return out


def wait_log(log: Path, pattern: str, timeout: float, start: float) -> float | None:
    rx = re.compile(pattern)
    while time.time() - start < timeout:
        if log.exists() and rx.search(log.read_text(errors="ignore")):
            return time.time() - start
        time.sleep(0.02)
    return None


def seed(home: Path, agents: int, busy: bool) -> list[str]:
    db = sqlite3.connect(home / "aisense.db")
    now = int(time.time() * 1000)
    team = "team_PERF"
    work = home / "work"
    work.mkdir(exist_ok=True)
    db.execute(
        "INSERT INTO teams (id, name, mission, workdir, color, layout, created_at, updated_at)"
        " VALUES (?, 'Perf', '', ?, 'violet', '{}', ?, ?)",
        (team, str(work), now, now),
    )
    # Ocioso de verdade: o `shell` tem `idle_regex` e o detector marca "ocioso" no prompt.
    # O `custom` sem regex ficaria 60 s em "iniciando" (com o ponto animado).
    adapter, args = (
        ("custom", ["sh", "-c", "while true; do date +%T.%N; sleep 0.05; done"])
        if busy
        # PERF_IDLE_STARTING=1 reproduz agentes presos em "iniciando" (ponto animado).
        else ("custom", ["sh", "-c", "printf 'pronto> '; sleep 100000"])
        if os.environ.get("PERF_IDLE_STARTING")
        else ("shell", [])
    )
    ids = []
    for i in range(agents):
        aid = f"agent_PERF{i:02d}"
        ids.append(aid)
        db.execute(
            "INSERT INTO agents (id, team_id, handle, name, adapter_id, args, color, autostart,"
            " restart_policy, position, created_at, updated_at)"
            " VALUES (?, ?, ?, ?, ?, ?, 'cyan', 1, 'never', ?, ?, ?)",
            (aid, team, f"a{i}", f"A{i}", adapter, json.dumps(args), i, now, now),
        )
    db.commit()
    db.close()
    (home / "settings.json").write_text(
        json.dumps(
            {
                "onboardingDone": True,
                "session": {
                    "restoreLastTeam": True,
                    "relaunchAgents": True,
                    "lastTeam": team,
                    "runningAgents": ids,
                },
            }
        )
    )
    return ids


def run(binary: str, home: Path, log: Path) -> subprocess.Popen:
    env = dict(os.environ, AISENSE_HOME=str(home), AISENSE_LOG="info", SHELL="/bin/sh")
    return subprocess.Popen(
        ["xvfb-run", "-a", "-s", "-screen 0 1440x900x24", binary],
        env=env,
        stdout=log.open("w"),
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )


def app_pid(launcher: subprocess.Popen, binary: str, timeout: float = 20) -> int:
    name = Path(binary).name[:15]
    end = time.time() + timeout
    while time.time() < end:
        for p in tree(launcher.pid):
            if comm(p) == name:
                return p
        time.sleep(0.05)
    raise RuntimeError("o app não subiu")


def stop(launcher: subprocess.Popen) -> None:
    try:
        os.killpg(launcher.pid, 15)
        launcher.wait(timeout=10)
    except Exception:
        os.killpg(launcher.pid, 9)


def scenario(binary: str, agents: int, busy: bool) -> dict:
    with tempfile.TemporaryDirectory(prefix="aisense-perf-") as tmp:
        home = Path(tmp)
        # Primeira subida: cria o banco e migra.
        log = home / "first.log"
        first = run(binary, home, log)
        wait_log(log, "AISENSE iniciando", 30, time.time())
        time.sleep(4)
        stop(first)
        seed(home, agents, busy)

        log = home / "run.log"
        start = time.time()
        launcher = run(binary, home, log)
        pid = app_pid(launcher, binary)
        shown = wait_log(log, r"tela da equipe.*team_PERF", 60, start)
        relaunched = None
        end = time.time() + 60
        while time.time() < end:
            text = log.read_text(errors="ignore")
            if text.count("agente religado") >= agents:
                relaunched = time.time() - start
                break
            time.sleep(0.05)
        time.sleep(5)  # deixa o detector e a UI assentarem
        result = {
            "agents": agents,
            "busy": busy,
            "cold_start_to_team_room_s": round(shown, 2) if shown else None,
            "all_agents_up_s": round(relaunched, 2) if relaunched else None,
            "rss_app_webkit_mb": round(rss_mb(app_processes(pid)), 1),
            "pss_app_webkit_mb": round(pss_mb(app_processes(pid)), 1),
            "pss_by_process_mb": {comm(p): round(pss_mb([p]), 1) for p in app_processes(pid)},
            "cpu_percent_by_process": cpu_percent(pid, 10),
        }
        stop(launcher)
        return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary")
    parser.add_argument("--agents", type=int, default=12)
    parser.add_argument(
        "--only", choices=["idle0", "idle6", "busy"], help="roda um cenário só (investigação)"
    )
    args = parser.parse_args()
    scenarios = {
        "idle0": (0, False),
        "idle6": (6, False),
        "busy": (args.agents, True),
    }
    chosen = [scenarios[args.only]] if args.only else list(scenarios.values())
    report = [scenario(args.binary, n, busy) for n, busy in chosen]
    print(json.dumps(report, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
