//! Transcrições sintéticas por runtime, rodadas com as regras **reais** dos
//! adaptadores embutidos (`adapters/*.toml`). As telas imitam o formato que cada
//! regex foi escrito para reconhecer — são modelos, não gravações de sessões reais.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use super::*;
use crate::adapter::{AdapterCatalog, StateRules, BUILTIN_ADAPTERS};
use crate::agent::AgentState::{self, *};

fn rules(adapter: &str) -> StateRules {
    AdapterCatalog::load_from(BUILTIN_ADAPTERS, None)
        .get(adapter)
        .unwrap_or_else(|| panic!("adaptador {adapter} embutido"))
        .state
        .clone()
}

/// Um passo: esperar `after_ms` e então o processo escreve `output`.
struct Step(u64, &'static str);

/// Roda a transcrição como o supervisor faria: dispara `tick` em cada prazo que o
/// detector pede e alimenta a saída no instante certo. Termina com 2 s de silêncio.
fn run(rules: &StateRules, steps: &[Step]) -> Vec<(AgentState, StateConfidence)> {
    let t0 = Instant::now();
    let mut detector = StateDetector::new(rules, 24, 80, t0);
    let mut seen = Vec::new();
    let mut now = t0;
    let advance = |detector: &mut StateDetector, until: Instant, seen: &mut Vec<_>| {
        while let Some(deadline) = detector.next_deadline().filter(|d| *d <= until) {
            if let Some(d) = detector.tick(deadline) {
                seen.push((d.state, d.confidence));
            }
        }
    };
    for Step(after_ms, output) in steps {
        now += Duration::from_millis(*after_ms);
        advance(&mut detector, now, &mut seen);
        if let Some(d) = detector.feed(output.as_bytes(), now) {
            seen.push((d.state, d.confidence));
        }
    }
    advance(&mut detector, now + Duration::from_secs(2), &mut seen);
    seen
}

fn states(seen: &[(AgentState, StateConfidence)]) -> Vec<AgentState> {
    seen.iter().map(|(s, _)| *s).collect()
}

// ─────────────────────────── por runtime ───────────────────────────

#[test]
fn claude_code_full_turn_with_a_permission_prompt() {
    let prompt = "\x1b[2m────────────────────────────────\x1b[0m\r\n\x1b[1m>\x1b[0m \r\n\x1b[2m────────────────────────────────\x1b[0m\r\n  \x1b[2m? for shortcuts\x1b[0m";
    let steps = [
        Step(0, "\x1b[38;5;174m✻ Welcome to Claude Code!\x1b[0m\r\n\r\n"),
        Step(50, prompt),
        // O humano digita e envia; a tela vira spinner + caixa de entrada.
        Step(3_000, "oi\r"),
        Step(50, "\x1b[2J\x1b[H> oi\r\n\r\n✻ Thinking… (esc to interrupt)\r\n"),
        Step(150, "\x1b[2J\x1b[H> oi\r\n\r\n✢ Thinking… (esc to interrupt)\r\n"),
        Step(150, "\x1b[2J\x1b[H> oi\r\n\r\n✳ Thinking… (esc to interrupt)\r\n"),
        // O diálogo de permissão toma o lugar da caixa de entrada.
        Step(150, "\x1b[2J\x1b[H> oi\r\n\r\n╭──────────────────────╮\r\n│ Edit file src/a.rs   │\r\n│ Do you want to make this edit? │\r\n│ ❯ 1. Yes             │\r\n│   2. No              │\r\n╰──────────────────────╯\r\n"),
        // O humano responde "1": a ferramenta roda e a resposta chega.
        Step(5_000, "\x1b[2J\x1b[H⏺ Update(src/a.rs)\r\n"),
        Step(100, "\x1b[2J\x1b[H⏺ Pronto, editei o arquivo.\r\n\r\n"),
        Step(20, prompt),
    ];
    let seen = run(&rules("claude"), &steps);
    assert_eq!(
        states(&seen),
        [Idle, Busy, AwaitingInput, Busy, Idle],
        "{seen:?}"
    );
    assert!(seen.iter().all(|(_, c)| *c == StateConfidence::High));
}

#[test]
fn codex_idles_on_its_chevron_and_waits_on_approval() {
    let prompt = "\r\n\x1b[1m›\x1b[0m \r\n\x1b[2m  ⏎ send   ⌃J newline\x1b[0m";
    let steps = [
        Step(0, "\x1b[1m>_ OpenAI Codex\x1b[0m\r\n"),
        Step(30, prompt),
        Step(2_000, "rode os testes\r"),
        Step(40, "\x1b[2J\x1b[H• Working (3s • esc to interrupt)\r\n"),
        Step(200, "\x1b[2J\x1b[H• Working (4s • esc to interrupt)\r\n"),
        Step(
            200,
            "\x1b[2J\x1b[HAllow command?\r\n  $ cargo test\r\n  y) yes   n) no\r\n",
        ),
        Step(4_000, "\x1b[2J\x1b[H• Ran cargo test — 12 passed\r\n"),
        Step(50, prompt),
    ];
    let seen = run(&rules("codex"), &steps);
    assert_eq!(
        states(&seen),
        [Idle, Busy, AwaitingInput, Busy, Idle],
        "{seen:?}"
    );
}

#[test]
fn opencode_idles_on_its_bar_prompt() {
    let prompt = "\x1b[2J\x1b[H  opencode\r\n\r\n\x1b[38;5;99m┃\x1b[0m \r\n  enter send";
    let steps = [
        Step(0, prompt),
        Step(1_500, "explique o main.rs\r"),
        Step(40, "\x1b[2J\x1b[H  Generating…\r\n"),
        Step(300, "\x1b[2J\x1b[H  Generating…  ▰▰▱\r\n"),
        Step(300, "\x1b[2J\x1b[H  O main.rs sobe o servidor.\r\n"),
        Step(20, prompt),
    ];
    let seen = run(&rules("opencode"), &steps);
    assert_eq!(states(&seen), [Idle, Busy, Idle], "{seen:?}");
}

#[test]
fn gemini_idles_on_its_prompt_and_waits_on_approval() {
    let prompt = "\r\n\x1b[35m>\x1b[0m \r\n";
    let steps = [
        Step(0, "Gemini CLI\r\n"),
        Step(20, prompt),
        Step(2_000, "liste os arquivos\r"),
        Step(40, "\x1b[2J\x1b[H⠋ Thinking… (esc to cancel)\r\n"),
        Step(300, "\x1b[2J\x1b[HAllow execution of: ls? (y/n)\r\n"),
        Step(3_000, "\x1b[2J\x1b[Ha.rs  b.rs\r\n"),
        Step(20, prompt),
    ];
    let seen = run(&rules("gemini"), &steps);
    assert_eq!(
        states(&seen),
        [Idle, Busy, AwaitingInput, Busy, Idle],
        "{seen:?}"
    );
}

#[test]
fn shell_is_idle_at_the_prompt_and_busy_while_a_command_prints() {
    let steps = [
        Step(0, "\x1b[32mdev@box\x1b[0m:\x1b[34m~/app\x1b[0m$ "),
        Step(1_000, "cargo build\r\n"),
        Step(100, "   Compiling app v0.1.0\r\n"),
        Step(300, "   Compiling dep v1.0.0\r\n"),
        Step(
            300,
            "    Finished dev in 3.2s\r\n\x1b[32mdev@box\x1b[0m:\x1b[34m~/app\x1b[0m$ ",
        ),
    ];
    let seen = run(&rules("shell"), &steps);
    assert_eq!(states(&seen), [Idle, Busy, Idle], "{seen:?}");
}

// ─────────────────────────── regras ───────────────────────────

#[test]
fn never_idle_before_the_quiet_period() {
    let r = rules("shell");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    d.feed(b"$ ", t0);
    let quiet = Duration::from_millis(u64::from(r.quiet_ms));
    assert_eq!(d.tick(t0 + quiet - Duration::from_millis(1)), None);
    assert_eq!(d.state(), Starting);
    assert_eq!(d.tick(t0 + quiet).map(|x| x.state), Some(Idle));
}

#[test]
fn awaiting_wins_over_idle_on_the_same_screen() {
    let r = rules("claude");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    // O prompt ocioso acima da pergunta (histórico) não a anula.
    d.feed("> \r\nDo you want to proceed? (y/n)".as_bytes(), t0);
    let got = d.tick(t0 + Duration::from_secs(1)).unwrap();
    assert_eq!(got.state, AwaitingInput);
}

#[test]
fn an_answered_question_left_on_screen_does_not_hold_the_agent() {
    // Programa de linha: a pergunta respondida fica na tela, com o prompt embaixo.
    let r = rules("claude");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    d.feed("Continue? (y/n) y\r\nok\r\n> ".as_bytes(), t0);
    assert_eq!(d.tick(t0 + Duration::from_secs(1)).unwrap().state, Idle);
}

#[test]
fn only_the_bottom_of_the_screen_counts() {
    // "permission" lá no alto, numa resposta antiga, não é um diálogo aberto.
    let r = rules("claude");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 40, 80, t0);
    let mut screen = String::from("A permission error happened earlier.\r\n");
    for i in 0..TAIL_LINES {
        screen.push_str(&format!("linha {i}\r\n"));
    }
    screen.push_str("> \r\n");
    d.feed(screen.as_bytes(), t0);
    assert_eq!(d.tick(t0 + Duration::from_secs(1)).unwrap().state, Idle);
}

#[test]
fn ansi_is_stripped_before_matching() {
    let r = rules("shell");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    d.feed(b"\x1b[1;32muser\x1b[0m \x1b[38;2;10;20;30m$\x1b[0m ", t0);
    assert_eq!(d.screen_tail(), "user $");
    assert_eq!(d.tick(t0 + Duration::from_secs(1)).unwrap().state, Idle);
}

#[test]
fn a_stalled_spinner_stays_busy_until_the_long_silence() {
    let r = rules("claude");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    d.feed(b"> \r\n", t0);
    d.tick(t0 + Duration::from_secs(1));
    let t1 = t0 + Duration::from_secs(2);
    d.feed("✻ Thinking… (esc to interrupt)".as_bytes(), t1);
    assert_eq!(d.state(), Busy);
    assert_eq!(
        d.tick(t1 + Duration::from_secs(5)),
        None,
        "busy_regex keeps it busy"
    );
    let late = d.tick(t1 + LOW_CONFIDENCE_SILENCE).unwrap();
    assert_eq!((late.state, late.confidence), (Idle, StateConfidence::Low));
    assert_eq!(
        d.next_deadline(),
        None,
        "nothing more to decide until new output"
    );
}

#[test]
fn a_runtime_without_regex_is_idle_only_with_low_confidence() {
    let r = rules("custom");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    d.feed(b"servidor ouvindo na porta 8080\r\n", t0);
    assert_eq!(d.tick(t0 + Duration::from_secs(5)), None);
    assert_eq!(d.state(), Starting);
    let got = d.tick(t0 + LOW_CONFIDENCE_SILENCE).unwrap();
    assert_eq!((got.state, got.confidence), (Idle, StateConfidence::Low));
    // Nova saída: ocupado de novo, com certeza.
    let got = d.feed(b"GET /\r\n", t0 + Duration::from_secs(70)).unwrap();
    assert_eq!((got.state, got.confidence), (Busy, StateConfidence::High));
}

#[test]
fn output_while_starting_does_not_flip_to_busy() {
    let r = rules("claude");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    assert_eq!(d.feed(b"carregando...\r\n", t0), None);
    assert_eq!(d.state(), Starting);
}

#[test]
fn prompt_redrawn_in_place_is_read_as_the_final_screen() {
    // Carriage return e apagar linha: o texto final da linha é o que conta.
    let r = rules("shell");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    d.feed(b"baixando 10%\rbaixando 90%\r\x1b[2K$ ", t0);
    assert_eq!(d.screen_tail(), "$");
    assert_eq!(d.tick(t0 + Duration::from_secs(1)).unwrap().state, Idle);
}

#[test]
fn last_lines_are_what_a_thumbnail_shows() {
    let r = rules("shell");
    let t0 = Instant::now();
    let mut d = StateDetector::new(&r, 24, 80, t0);
    d.feed(b"\x1b[31mum\x1b[0m\r\n\r\ndois\r\ntr\xc3\xaas\r\n$ ", t0);
    assert_eq!(d.last_lines(2), ["três", "$"]);
    assert_eq!(d.last_lines(10), ["um", "dois", "três", "$"]);
}
