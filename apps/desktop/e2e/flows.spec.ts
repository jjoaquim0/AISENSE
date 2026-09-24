import { expect, type Page, test } from '@playwright/test';
import { openApp, unknownCommands } from './fixtures';

// Fluxos críticos da Fase 8 (docs/09) na interface. O core é o falso: aqui se prova o que
// a interface faz com o que o core diz. O que o core faz de verdade (PTY, barramento,
// BOOT.md, política de reinício) tem testes de processo real em Rust — o mapa está em
// docs/fases/FASE-08-acabamento.md (F08-08).

type Fake = {
  crash: (handle: string) => Promise<void>;
  route: (
    from: string,
    to: string,
    body: string,
    kind?: string,
    replyTo?: string,
  ) => Promise<string>;
};

/** `window.__fake.route(...)` na página. */
function route(page: Page, ...args: Parameters<Fake['route']>): Promise<string> {
  return page.evaluate(
    (a) =>
      (window as unknown as { __fake: Fake }).__fake.route(...(a as Parameters<Fake['route']>)),
    args,
  );
}

function crash(page: Page, handle: string): Promise<void> {
  return page.evaluate((h) => (window as unknown as { __fake: Fake }).__fake.crash(h), handle);
}

test('F1 — do zero à equipe rodando: onboarding, modelo, ▶, 4 terminais vivos', async ({
  page,
}) => {
  await openApp(page);
  await page.getByRole('button', { name: 'Continuar →' }).click();
  await page.getByRole('button', { name: 'Continuar →' }).click();
  await page.getByLabel('Nome').fill('Squad Produto');
  await page.getByLabel('Pasta do projeto').fill('/home/voce/projetos/api');
  await page.getByLabel(/Squad completo/).check();
  await expect(page.getByText(/@revisor usará Shell/)).toBeVisible();
  await page.getByRole('button', { name: 'Criar equipe →' }).click();

  await expect(page.getByRole('heading', { name: 'Squad Produto' })).toBeVisible();
  for (const handle of ['arquiteto', 'backend', 'frontend', 'revisor']) {
    const pane = page.getByRole('region', { name: `Painel de @${handle}` });
    await expect(pane.getByText('Ocioso')).toBeVisible();
    await expect(pane.locator('.xterm')).toHaveCount(1);
  }
  expect(await unknownCommands(page)).toEqual([]);
});

test('F2 — agentes conversando: ask e reply aparecem ao vivo na linha do tempo', async ({
  page,
}) => {
  await openApp(page, {
    onboardingDone: true,
    team: { name: 'Squad', handles: ['a', 'b'], running: true },
  });
  await page.getByText('Squad').first().click();
  await page.getByRole('radio', { name: /Mensagens/ }).click();
  const log = page.getByRole('log', { name: 'Linha do tempo da equipe' });
  await expect(log.getByText('Nenhuma mensagem ainda')).toBeVisible();

  const ask = await route(page, '@a', '@b', 'qual a porta da API?', 'request');
  await expect(log.getByText('qual a porta da API?')).toBeVisible();
  await route(page, '@b', '@a', 'a API sobe na 8080', 'response', ask);
  await expect(log.getByText('a API sobe na 8080')).toBeVisible();
});

test('F4 — recuperação de falha: o agente cai, mostra o erro e a política o traz de volta', async ({
  page,
}) => {
  await openApp(page, {
    onboardingDone: true,
    team: { name: 'Squad', handles: ['backend', 'frontend'], running: true },
  });
  await page.getByText('Squad').first().click();
  const pane = page.getByRole('region', { name: 'Painel de @backend' });
  await expect(pane.getByText('Ocioso')).toBeVisible();

  await crash(page, 'backend');
  await expect(pane.getByText('Erro')).toBeVisible();
  // O estado passa por "Erro" e volta sozinho, sem ninguém clicar.
  await expect(pane.getByText('Ocioso')).toBeVisible();
  const states = await page.evaluate(() =>
    (window as unknown as { __fake: { calls: { cmd: string }[] } }).__fake.calls.map((c) => c.cmd),
  );
  expect(states).not.toContain('agent_start');
  // O vizinho não foi afetado.
  await expect(
    page.getByRole('region', { name: 'Painel de @frontend' }).getByText('Ocioso'),
  ).toBeVisible();
});

test('F5 — troca de tema com 9 terminais: sem flash, sem reload, xterm re-tematizado', async ({
  page,
}) => {
  await openApp(page, {
    onboardingDone: true,
    settings: {
      appearance: {
        theme: 'light',
        density: 'comfortable',
        terminalFontSize: 13,
        terminalFontFamily: '',
      },
    },
    team: { name: 'Squad', handles: ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i'], running: true },
  });
  await page.getByText('Squad').first().click();
  await expect(page.locator('.xterm')).toHaveCount(9);
  const background = () =>
    page.evaluate(() => {
      const el = document.querySelector('.xterm .xterm-scrollable-element') as HTMLElement | null;
      return el ? getComputedStyle(el).backgroundColor : null;
    });
  const before = await background();
  await page.evaluate(() => {
    (window as unknown as { __marker: boolean }).__marker = true;
  });
  await page.getByRole('button', { name: 'Usar tema escuro' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect.poll(background).not.toBe(before);
  expect(await page.evaluate(() => (window as unknown as { __marker: boolean }).__marker)).toBe(
    true,
  );
  await expect(page.locator('.xterm')).toHaveCount(9);
});
