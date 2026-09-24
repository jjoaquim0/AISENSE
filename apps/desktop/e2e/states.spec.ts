import { expect, test } from '@playwright/test';
import { heal, openApp, unknownCommands } from './fixtures';

// F08-03: nenhum erro sem próximo passo; nenhum vazio sem ação.

test('erro ao carregar as equipes diz o que houve e deixa tentar de novo', async ({ page }) => {
  await openApp(page, {
    onboardingDone: true,
    failing: ['teams_list'],
    team: { name: 'Squad Produto', handles: ['backend'] },
  });
  const alert = page.getByRole('alert').filter({ hasText: 'Não foi possível carregar as equipes' });
  await expect(alert).toBeVisible();
  await expect(alert).toContainText('Tente de novo.');
  await heal(page, 'teams_list');
  await alert.getByRole('button', { name: 'Tentar de novo' }).click();
  await expect(page.getByText('Squad Produto')).toBeVisible();
  await expect(alert).toHaveCount(0);
});

test('lista vazia de equipes convida a criar a primeira', async ({ page }) => {
  await openApp(page, { onboardingDone: true });
  await expect(page.getByRole('heading', { name: /equipe/i }).first()).toBeVisible();
  await expect(page.getByRole('button', { name: /Nova equipe|Criar/ }).first()).toBeVisible();
  expect(await unknownCommands(page)).toEqual([]);
});

test('equipe sem agentes oferece criar o primeiro', async ({ page }) => {
  await openApp(page, { onboardingDone: true, team: { name: 'Vazia', handles: [] } });
  await page.getByText('Vazia').first().click();
  await expect(page.getByText('Nenhum agente ainda').first()).toBeVisible();
  await page.getByRole('button', { name: 'Novo agente' }).last().click();
  await expect(page.getByRole('dialog')).toBeVisible();
});

test('runtimes que falham na verificação podem ser verificados de novo', async ({ page }) => {
  await openApp(page, { failing: ['runtimes_overview'] });
  const alert = page
    .getByRole('alert')
    .filter({ hasText: 'Não foi possível verificar os runtimes' });
  await expect(alert).toBeVisible();
  await heal(page, 'runtimes_overview');
  await alert.getByRole('button', { name: 'Tentar de novo' }).click();
  await expect(page.getByText('Claude Code')).toBeVisible();
});
