import AxeBuilder from '@axe-core/playwright';
import { expect, type Page, test } from '@playwright/test';
import { openApp } from './fixtures';

// F08-02: WCAG 2.1 AA nas telas principais, nos dois temas. O axe acha o que é mecânico
// (contraste, rótulos, papéis ARIA); o teclado tem o próprio teste em `keyboard.spec.ts`.

const SQUAD = { name: 'Squad Produto', handles: ['backend', 'frontend'], running: true };

async function audit(page: Page) {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    // O xterm desenha num canvas e mantém um textarea invisível próprio; o conteúdo do
    // terminal é o do processo, fora do controle do app.
    .exclude('.xterm')
    .analyze();
  return result.violations.map((v) => ({
    id: v.id,
    impact: v.impact,
    help: v.help,
    nodes: v.nodes.slice(0, 5).map((n) => n.target.join(' ')),
  }));
}

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`tema ${scheme}`, () => {
    test.use({ colorScheme: scheme });

    test('onboarding', async ({ page }) => {
      await openApp(page);
      await expect(page.getByText('Bem-vindo ao AISENSE')).toBeVisible();
      await expect(page.getByText('Claude Code')).toBeVisible();
      expect(await audit(page)).toEqual([]);
      await page.getByRole('button', { name: 'Continuar →' }).click();
      expect(await audit(page)).toEqual([]);
      await page.getByRole('button', { name: 'Continuar →' }).click();
      await expect(page.getByText('Squad completo')).toBeVisible();
      expect(await audit(page)).toEqual([]);
    });

    test('lista de equipes', async ({ page }) => {
      await openApp(page, { onboardingDone: true, team: SQUAD });
      await expect(page.getByText('Squad Produto')).toBeVisible();
      expect(await audit(page)).toEqual([]);
    });

    test('sala da equipe, em cada vista', async ({ page }) => {
      await openApp(page, { onboardingDone: true, team: SQUAD, messages: 30 });
      await page.getByText('Squad Produto').first().click();
      await expect(page.getByRole('radio', { name: /Grade/ })).toBeVisible();
      for (const view of ['Grade', 'Foco', 'Fluxo', 'Mensagens', 'Quadro']) {
        await page.getByRole('radio', { name: new RegExp(view) }).click();
        await page.waitForTimeout(300);
        expect(await audit(page), view).toEqual([]);
      }
    });

    test('configurações', async ({ page }) => {
      await openApp(page, { onboardingDone: true, team: SQUAD });
      await page.getByRole('button', { name: 'Configurações' }).click();
      const nav = page.getByRole('navigation', { name: 'Seções das configurações' });
      await expect(nav).toBeVisible();
      for (const section of [
        'Aparência',
        'Runtimes',
        'Barramento',
        'Notificações',
        'Atalhos',
        'Segredos',
        'Avançado',
      ]) {
        await nav.getByRole('button', { name: section }).click();
        await page.waitForTimeout(200);
        expect(await audit(page), section).toEqual([]);
      }
    });

    test('biblioteca de skills', async ({ page }) => {
      await openApp(page, { onboardingDone: true });
      await page.getByRole('button', { name: 'Biblioteca de skills' }).click();
      await page.waitForTimeout(300);
      expect(await audit(page)).toEqual([]);
    });
  });
}
