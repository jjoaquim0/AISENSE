import { expect, type Page, test } from '@playwright/test';
import { openApp, unknownCommands } from './fixtures';

// F09-03: a faixa de atualização, as novidades, o progresso e o "desligar".

function calls(page: Page, cmd: string): Promise<number> {
  return page.evaluate(
    (c) =>
      (window as unknown as { __fake: { calls: { cmd: string }[] } }).__fake.calls.filter(
        (call) => call.cmd === c,
      ).length,
    cmd,
  );
}

test('versão nova aparece numa faixa com novidades e progresso da instalação', async ({ page }) => {
  await openApp(page, {
    onboardingDone: true,
    update: { version: '0.2.0', notes: '## Novidades\n\n- Quadro mais rápido' },
  });
  const banner = page.getByRole('status').filter({ hasText: 'aisense 0.2.0 disponível' });
  await expect(banner).toContainText('você tem a 0.1.0', { timeout: 10_000 });

  await banner.getByRole('button', { name: 'Novidades' }).click();
  const notes = page.getByRole('dialog', { name: 'Novidades do aisense 0.2.0' });
  await expect(notes).toContainText('Quadro mais rápido');
  await page.keyboard.press('Escape');

  await banner.getByRole('button', { name: 'Instalar e reiniciar' }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Instalando o aisense 0.2.0 — 50%' }),
  ).toBeVisible();
  expect(await unknownCommands(page)).toEqual([]);
});

test('"Depois" esconde a faixa sem perder a atualização nas Configurações', async ({ page }) => {
  await openApp(page, { onboardingDone: true, update: { version: '0.2.0' } });
  const banner = page.getByRole('status').filter({ hasText: 'aisense 0.2.0 disponível' });
  await banner.getByRole('button', { name: 'Depois' }).click({ timeout: 10_000 });
  await expect(banner).toBeHidden();
  await page.getByRole('button', { name: 'Configurações' }).click();
  await page
    .getByRole('navigation', { name: 'Seções das configurações' })
    .getByRole('button', { name: 'Avançado' })
    .click();
  await expect(page.getByText('A versão 0.2.0 está disponível.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Instalar e reiniciar' })).toBeVisible();
});

test('desligado, o app não procura ao abrir; "Procurar agora" ainda funciona', async ({ page }) => {
  await openApp(page, {
    onboardingDone: true,
    settings: { advanced: { logLevel: 'info', checkUpdates: false } },
  });
  await page.getByRole('button', { name: 'Configurações' }).click();
  await page
    .getByRole('navigation', { name: 'Seções das configurações' })
    .getByRole('button', { name: 'Avançado' })
    .click();
  const toggle = page.getByRole('checkbox', { name: /Procurar atualizações ao abrir/ });
  await expect(toggle).not.toBeChecked();
  // O app procuraria 4 s depois de abrir.
  await page.waitForTimeout(5_000);
  expect(await calls(page, 'update_check')).toBe(0);

  await page.getByRole('button', { name: 'Procurar agora' }).click();
  await expect(page.getByText('Você está na versão mais nova.')).toBeVisible();
  expect(await calls(page, 'update_check')).toBe(1);
});
