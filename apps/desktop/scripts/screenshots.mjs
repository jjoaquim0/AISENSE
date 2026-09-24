// Screenshots de todas as telas nos dois temas (F08-09), com o core falso.
// Uso: pnpm --filter @aisense/desktop screenshots [pasta]  (servidor e2e em :5181)
import { mkdirSync } from 'node:fs';
import { chromium } from '@playwright/test';

const out = process.argv[2] ?? '../../docs/screenshots/fase-08';
const base = process.env.E2E_URL ?? 'http://localhost:5181/';
mkdirSync(out, { recursive: true });
const browser = await chromium.launch(
  process.env.PW_CHROMIUM_PATH ? { executablePath: process.env.PW_CHROMIUM_PATH } : {},
);

const SQUAD = {
  name: 'Squad Produto',
  handles: ['arquiteto', 'backend', 'frontend', 'revisor'],
  running: true,
};

async function open(scheme, seed) {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    colorScheme: scheme,
    deviceScaleFactor: 1,
  });
  const page = await context.newPage();
  await page.addInitScript((s) => {
    window.__fakeSeed = s;
  }, seed);
  await page.goto(base);
  return page;
}

async function shot(page, name, scheme) {
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${out}/${name}-${scheme}.png` });
}

for (const scheme of ['light', 'dark']) {
  // T1 — onboarding
  let page = await open(scheme, {});
  await page.getByText('Claude Code').waitFor();
  await shot(page, 't1-onboarding-1', scheme);
  await page.getByRole('button', { name: 'Continuar →' }).click();
  await shot(page, 't1-onboarding-2', scheme);
  await page.getByRole('button', { name: 'Continuar →' }).click();
  await page.getByText('Squad completo').waitFor();
  await shot(page, 't1-onboarding-3', scheme);
  await page.close();

  // T2 — equipes
  page = await open(scheme, { onboardingDone: true, team: SQUAD, messages: 40 });
  await page.getByText('Squad Produto').first().waitFor();
  await shot(page, 't2-equipes', scheme);

  // T3 — assistente
  await page.getByRole('button', { name: 'Nova equipe' }).first().click();
  await shot(page, 't3-nova-equipe', scheme);
  await page.keyboard.press('Escape');

  // T4 — Sala da Equipe, cada vista
  await page.getByText('Squad Produto').first().click();
  for (const [view, name] of [
    ['Grade', 't4-1-grade'],
    ['Foco', 't4-2-foco'],
    ['Fluxo', 't4-3-fluxo'],
    ['Mensagens', 't4-4-mensagens'],
    ['Quadro', 't8-quadro'],
  ]) {
    await page.getByRole('radio', { name: new RegExp(view) }).click();
    await page.waitForTimeout(600);
    await shot(page, name, scheme);
  }
  await page.getByRole('radio', { name: /Grade/ }).click();

  // T5 — novo agente
  await page.getByRole('button', { name: 'Novo agente' }).first().click();
  await shot(page, 't5-novo-agente', scheme);
  await page.keyboard.press('Escape');

  // T10 — paleta
  await page.keyboard.press('Control+k');
  await shot(page, 't10-paleta', scheme);
  await page.keyboard.press('Escape');

  // T9 — configurações
  await page.keyboard.press('Control+,');
  const nav = page.getByRole('navigation', { name: 'Seções das configurações' });
  for (const section of ['Aparência', 'Runtimes', 'Barramento', 'Notificações', 'Atalhos', 'Segredos', 'Avançado']) {
    await nav.getByRole('button', { name: section }).click();
    await shot(page, `t9-${section.toLowerCase().normalize('NFD').replace(/[^a-z]/g, '')}`, scheme);
  }

  // T7 — skills
  await page.getByRole('button', { name: 'Biblioteca de skills' }).click();
  await shot(page, 't7-skills', scheme);
  await page.close();
}
await browser.close();
console.log(`screenshots em ${out}`);
