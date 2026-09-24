import { expect, test } from '@playwright/test';
import { openApp, unknownCommands } from './fixtures';

// F09-05: o diagnóstico mostra cada arquivo, já redigido, antes de salvar o `.zip`.

test('exportar diagnóstico mostra o pacote antes de salvar', async ({ page }) => {
  await openApp(page, { onboardingDone: true });
  await page.getByRole('button', { name: 'Configurações' }).click();
  await page
    .getByRole('navigation', { name: 'Seções das configurações' })
    .getByRole('button', { name: 'Avançado' })
    .click();
  await page.getByRole('button', { name: 'Exportar diagnóstico…' }).click();

  const dialog = page.getByRole('dialog', { name: 'Exportar diagnóstico' });
  await expect(dialog.getByRole('tab', { name: 'relatorio.json' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(dialog.getByLabel('Conteúdo de relatorio.json')).toContainText('~/.aisense');
  await dialog.getByRole('tab', { name: 'aisense-app.log' }).click();
  await expect(dialog.getByLabel('Conteúdo de aisense-app.log')).toContainText(
    'AISENSE_TOKEN=‹redigido›',
  );
  await expect(dialog.getByText('Só o fim do arquivo entra no pacote')).toBeVisible();
  await expect(dialog.getByText('Fica de fora:')).toBeVisible();

  await dialog.getByRole('button', { name: 'Salvar .zip…' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByRole('status').filter({ hasText: 'Diagnóstico salvo em' })).toContainText(
    '/home/voce/aisense-diagnostico-2026-09-24.zip (2 KB)',
  );
  expect(await unknownCommands(page)).toEqual([]);
});
