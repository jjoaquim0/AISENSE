import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { WebglAddon } from '@xterm/addon-webgl';
import { Terminal as Xterm } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import { useEffect, useRef, useState } from 'react';
import { cn } from '@/lib/cn';
import { onPtyData, onPtyExit } from '@/lib/events';
import { useTheme } from '@/lib/theme';
import { terminalApi } from './api';
import { decodeChunk } from './decode';
import { registerTerminalFocus } from './focus';
import { HydrationGate } from './hydration';
import { TerminalSearch } from './TerminalSearch';
import { readTerminalTheme } from './theme';
import { useOnScreen } from './useOnScreen';

/** Debounce do redimensionamento: `fit()` a cada pixel arrastado é caro. */
const RESIZE_DEBOUNCE_MS = 50;

interface TerminalProps {
  agentId: string;
  /**
   * `false` força o painel a não receber eventos mesmo montado. Fora da tela ou com
   * a janela em segundo plano ele já não recebe; o histórico fica no core.
   */
  visible?: boolean;
  onExit?: (code: number) => void;
  /** Cada mudança limpa a tela e o histórico retido no core ("Limpar" do painel). */
  clearSignal?: number;
  /** Nome acessível, ex.: "Terminal de @backend". */
  label?: string;
  className?: string;
}

export function Terminal({
  agentId,
  visible = true,
  onExit,
  clearSignal,
  label = 'Terminal',
  className,
}: TerminalProps) {
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<Xterm | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const search = useRef<SearchAddon | null>(null);
  const gate = useRef<HydrationGate | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const { theme } = useTheme();
  // O efeito de montagem lê o tema uma vez; colocá-lo nas dependências recriaria o
  // terminal (e apagaria o conteúdo) a cada troca de tema.
  const themeAtMount = useRef(theme);
  themeAtMount.current = theme;

  // ── Ciclo de vida: uma instância por painel visível, destruída no unmount ──
  useEffect(() => {
    const container = host.current;
    if (!container) return;

    const xterm = new Xterm({
      allowProposedApi: true,
      cursorBlink: true,
      fontFamily: 'var(--font-mono)',
      fontSize: 13,
      lineHeight: 1.4,
      scrollback: 10_000,
      theme: readTerminalTheme(themeAtMount.current),
      // O histórico vive no core; o xterm é só a tela.
      convertEol: false,
    });

    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    xterm.loadAddon(fitAddon);
    xterm.loadAddon(searchAddon);
    xterm.loadAddon(new WebLinksAddon());
    xterm.open(container);

    // ⌘F/Ctrl+F precisa ser interceptado aqui: o xterm move o foco para um textarea
    // escondido, então um onKeyDown no elemento de fora praticamente nunca dispara.
    xterm.attachCustomKeyEventHandler((event) => {
      if (event.type !== 'keydown') return true;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f') {
        setSearchOpen(true);
        return false;
      }
      if (event.key === 'Escape') setSearchOpen(false);
      return true;
    });

    // WebGL é obrigatório para aguentar vários terminais; se a webview não tiver,
    // o canvas serve — mas nunca o renderer DOM, que trava com saída volumosa.
    try {
      xterm.loadAddon(new WebglAddon());
    } catch (error) {
      console.warn(`WebGL indisponível para @${agentId}, usando o renderer padrão`, error);
    }

    term.current = xterm;
    const unregisterFocus = registerTerminalFocus(agentId, () => xterm.focus());
    fit.current = fitAddon;
    search.current = searchAddon;

    fitAddon.fit();
    void terminalApi.resize(agentId, xterm.rows, xterm.cols).catch(reportError);

    // A saída ao vivo passa pelo portão: durante uma reidratação ela espera o
    // histórico ser escrito, para não aparecer acima dele.
    const hydration = new HydrationGate((data) => xterm.write(data));
    gate.current = hydration;
    const stopData = onPtyData(agentId, ({ dataBase64 }) => {
      hydration.push(decodeChunk(dataBase64));
    });
    const stopExit = onPtyExit(agentId, ({ code }) => {
      xterm.write(`\r\n\u001b[2m— processo encerrado (código ${code}) —\u001b[0m\r\n`);
      onExit?.(code);
    });

    const typed = xterm.onData((data) => {
      void terminalApi.write(agentId, data).catch(reportError);
    });

    return () => {
      unregisterFocus();
      stopData();
      stopExit();
      typed.dispose();
      hydration.cancel();
      gate.current = null;
      xterm.dispose();
      term.current = null;
      fit.current = null;
      search.current = null;
    };
  }, [agentId, onExit]);

  // ── Redimensionamento: o processo precisa saber o tamanho, senão TUIs quebram ──
  useEffect(() => {
    const container = host.current;
    if (!container) return;

    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        const xterm = term.current;
        if (!xterm || !fit.current) return;
        fit.current.fit();
        void terminalApi.resize(agentId, xterm.rows, xterm.cols).catch(reportError);
      }, RESIZE_DEBOUNCE_MS);
    });

    observer.observe(container);
    return () => {
      clearTimeout(timer);
      observer.disconnect();
    };
  }, [agentId]);

  // ── Tema: trocar cor sem recriar o terminal, para não perder o conteúdo ──
  useEffect(() => {
    if (term.current) term.current.options.theme = readTerminalTheme(theme);
  }, [theme]);

  // ── Limpar: tela do xterm e histórico do core juntos, senão a próxima reidratação
  //    traria de volta o que o usuário acabou de apagar ──
  useEffect(() => {
    if (!clearSignal) return;
    term.current?.clear();
    void terminalApi.clear(agentId).catch(reportError);
  }, [agentId, clearSignal]);

  // ── Visibilidade (F03-05): só o painel que está de fato na tela recebe eventos.
  //    Ao aparecer, pede ao core para ligar a emissão e reidrata com o histórico de
  //    uma vez só (escrever linha a linha 10.000 linhas congela a interface). Ao
  //    sumir — ou desmontar, ao trocar de vista —, desliga. ──
  const onScreen = useOnScreen(host);
  const shown = visible && onScreen;
  useEffect(() => {
    const xterm = term.current;
    const hydration = gate.current;
    if (!shown || !xterm || !hydration) return;
    xterm.reset();
    const generation = hydration.begin();
    terminalApi
      .show(agentId)
      .then((base64) => hydration.finish(generation, base64 ? decodeChunk(base64) : null))
      .catch((error: unknown) => {
        hydration.finish(generation, null);
        reportError(error);
      });
    return () => {
      hydration.cancel();
      void terminalApi.setVisible(agentId, false).catch(reportError);
    };
  }, [agentId, shown]);

  return (
    <div className={cn('relative size-full bg-terminal', className)}>
      {searchOpen && search.current && (
        <TerminalSearch addon={search.current} onClose={() => setSearchOpen(false)} />
      )}
      <div
        ref={host}
        role="application"
        aria-label={`${label}. Esc duas vezes volta para a interface.`}
        className="size-full"
      />
    </div>
  );
}

function reportError(error: unknown): void {
  console.error('Erro no terminal:', error);
}
