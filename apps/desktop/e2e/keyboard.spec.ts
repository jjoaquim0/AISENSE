import { expect, type Page, test } from '@playwright/test';
import { openApp, unknownCommands } from './fixtures';

// F08-02: os fluxos críticos só com o teclado, sem becos sem saída. `Control` é o `⌘`
// fora do macOS (docs/08); o Chromium dos testes roda em Linux.

/** Nome acessível de quem tem o foco. */
function focused(page: Page): Promise<string> {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el) return '';
    return el.getAttribute('aria-label') ?? el.textContent?.trim().slice(0, 40) ?? el.tagName;
  });
}

/** Aperta Tab até o foco chegar em `name` (ou falha, com o caminho percorrido). */
async function tabTo(page: Page, name: RegExp, max = 40): Promise<void> {
  const path: string[] = [];
  for (let i = 0; i < max; i++) {
    const now = await focused(page);
    if (name.test(now)) return;
    path.push(now);
    await page.keyboard.press('Tab');
  }
  throw new Error(`Tab não chegou em ${name}: ${path.join(' → ')}`);
}

/**
 * `Esc Esc` sai do terminal. As duas teclas precisam cair em 400 ms (`DOUBLE_ESCAPE_MS`):
 * numa máquina de CI carregada o intervalo entre dois `press` pode passar disso, e aí o
 * gesto é refeito — como uma pessoa faria. O que se exige é sair, não o tempo do robô.
 */
async function leaveTerminal(page: Page): Promise<void> {
  const inTerminal = () => page.evaluate(() => Boolean(document.activeElement?.closest('.xterm')));
  await expect.poll(inTerminal).toBe(true);
  for (let i = 0; i < 3 && (await inTerminal()); i++) {
    await page.keyboard.press('Escape');
    await page.keyboard.press('Escape');
  }
  expect(await inTerminal()).toBe(false);
}

test('F1 pelo teclado: onboarding até a equipe rodando', async ({ page }) => {
  await openApp(page);
  await expect(page.getByText('Bem-vindo ao AISENSE')).toBeVisible();
  await expect(page.getByText('Claude Code')).toBeVisible();
  // "Continuar" já nasce com o foco.
  await page.keyboard.press('Enter');
  await expect(page.getByText('Escolha o tema')).toBeVisible();
  await tabTo(page, /Continuar/);
  await page.keyboard.press('Enter');
  // A pasta nasce com o foco; Enter cria.
  await expect(page.getByText('Sua primeira equipe')).toBeVisible();
  await page.keyboard.type('/home/voce/projetos/api');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Minha equipe' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Painel de @dev' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Painel de @revisor' })).toBeVisible();
  await expect(
    page.getByRole('region', { name: 'Painel de @dev' }).getByText('Ocioso'),
  ).toBeVisible();
  expect(await unknownCommands(page)).toEqual([]);
});

test('paleta leva a qualquer tela e Esc fecha', async ({ page }) => {
  await openApp(page, { onboardingDone: true, team: { name: 'Squad', handles: ['backend'] } });
  await expect(page.getByText('Squad')).toBeVisible();
  await page.keyboard.press('Control+k');
  await page.keyboard.type('configura');
  await page.keyboard.press('Enter');
  const nav = page.getByRole('navigation', { name: 'Seções das configurações' });
  await expect(nav).toBeVisible();
  await tabTo(page, /^Atalhos$/);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Atalhos' })).toBeVisible();
  await page.keyboard.press('Control+k');
  await page.keyboard.type('skills');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: /skills/i }).first()).toBeVisible();
});

test('F5 pelo teclado: ⌘⇧D troca o tema sem recarregar', async ({ page }) => {
  await openApp(page, {
    onboardingDone: true,
    team: { name: 'Squad', handles: ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i'], running: true },
  });
  await page.getByText('Squad').first().click();
  await expect(page.getByRole('region', { name: 'Painel de @i' })).toBeVisible();
  await page.evaluate(() => {
    (window as unknown as { __marker: number }).__marker = 42;
  });
  const before = await page.evaluate(() => document.documentElement.dataset.theme);
  await page.keyboard.press('Control+Shift+D');
  const after = await page.evaluate(() => document.documentElement.dataset.theme);
  expect(after).not.toBe(before);
  // Sem reload: o marcador sobrevive; os terminais continuam montados.
  expect(await page.evaluate(() => (window as unknown as { __marker: number }).__marker)).toBe(42);
  await expect(page.locator('.xterm')).toHaveCount(9);
});

test('F4 pelo teclado: agente que caiu volta pelo menu do painel', async ({ page }) => {
  await openApp(page, {
    onboardingDone: true,
    team: { name: 'Squad', handles: ['backend', 'frontend'], running: true },
  });
  await page.getByText('Squad').first().click();
  const pane = page.getByRole('region', { name: 'Painel de @backend' });
  await expect(pane).toBeVisible();
  await page.evaluate(() =>
    (
      window as unknown as { __fake: { setState: (h: string, s: string) => Promise<void> } }
    ).__fake.setState('backend', 'failed'),
  );
  await expect(pane.getByText('Erro')).toBeVisible();
  // ⌘1 leva ao painel; Esc Esc devolve o teclado à interface.
  await page.keyboard.press('Control+1');
  await leaveTerminal(page);
  await tabTo(page, /Ações de @backend/);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('menu')).toBeVisible();
  // Caído, "Reiniciar" fica desabilitado e o caminho é "Iniciar".
  for (let i = 0; i < 6 && (await focused(page)) !== 'Iniciar'; i++) {
    await page.keyboard.press('ArrowDown');
  }
  expect(await focused(page)).toBe('Iniciar');
  await page.keyboard.press('Enter');
  await expect(pane.getByText('Ocioso')).toBeVisible();
});

for (const screen of ['equipes', 'configurações'] as const) {
  test(`Tab percorre ${screen} e volta ao começo, sem prender o foco`, async ({ page }) => {
    await openApp(page, { onboardingDone: true, team: { name: 'Squad', handles: ['backend'] } });
    await expect(page.getByText('Squad')).toBeVisible();
    if (screen === 'configurações') await page.keyboard.press('Control+,');
    const seen: string[] = [];
    for (let i = 0; i < 80; i++) {
      await page.keyboard.press('Tab');
      seen.push(
        await page.evaluate(() => {
          const el = document.activeElement;
          return el
            ? `${el.tagName}:${el.getAttribute('aria-label') ?? el.textContent?.slice(0, 20)}`
            : '';
        }),
      );
    }
    // Andou por vários controles e voltou a um já visto: o ciclo fecha, nada prende.
    expect(new Set(seen).size).toBeGreaterThan(6);
    const first = seen[0];
    expect(seen.slice(1).includes(first ?? '')).toBe(true);
  });
}
