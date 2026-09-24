import { expect, test } from '@playwright/test';
import { openApp, unknownCommands } from './fixtures';

// F08-06: fechar com 6 agentes e reabrir restaura equipe, vista, painel em foco, tamanhos e
// a rolagem da linha do tempo. O reload da página faz o papel de fechar e reabrir o app.
test('reabrir o app volta para a equipe como estava', async ({ page }) => {
  const handles = ['arquiteto', 'backend', 'frontend', 'revisor', 'qa', 'ops'];
  await openApp(page, {
    onboardingDone: true,
    persist: true,
    messages: 400,
    team: { name: 'Squad Produto', handles, running: true },
  });

  await page.getByText('Squad Produto').first().click();
  await expect(page.getByRole('log', { name: 'Linha do tempo da equipe' })).toHaveCount(0);

  // Foco no @qa e vista Linha do tempo, rolada para longe do fim.
  await page.getByRole('button', { name: /@qa/ }).first().click();
  await page.getByRole('radio', { name: /Mensagens/ }).click();
  const log = page.getByRole('log', { name: 'Linha do tempo da equipe' });
  await expect(log.getByText('Mensagem 400:')).toBeVisible();
  await log.hover();
  await page.mouse.wheel(0, -4000);
  await page.waitForTimeout(700);
  const firstVisibleBefore = await topMessage(page);
  expect(firstVisibleBefore).not.toBeNull();

  // Largura da sidebar.
  const sidebar = page.getByRole('complementary', { name: 'Agentes da equipe' });
  const handle = page.getByRole('separator', { name: 'Largura da lista de agentes' });
  const box = await handle.boundingBox();
  if (!box) throw new Error('sem alça de redimensionar');
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 60, box.y + box.height / 2, { steps: 5 });
  await page.mouse.up();
  const widthBefore = (await sidebar.boundingBox())?.width;
  await page.waitForTimeout(600);

  await page.reload();

  await expect(page.getByRole('radio', { name: /Mensagens/ })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(log).toBeVisible();
  await page.waitForTimeout(1600);
  expect(await topMessage(page)).toBe(firstVisibleBefore);
  expect((await sidebar.boundingBox())?.width).toBe(widthBefore);
  await expect(page.getByRole('button', { name: /@qa/ }).first()).toHaveAttribute(
    'aria-current',
    'true',
  );
  expect(await unknownCommands(page)).toEqual([]);
});

/** Texto da primeira mensagem inteira visível no topo da linha do tempo. */
function topMessage(page: import('@playwright/test').Page): Promise<string | null> {
  return page.evaluate(() => {
    const log = document.querySelector('[role="log"]');
    if (!log) return null;
    const top = log.getBoundingClientRect().top;
    for (const li of log.querySelectorAll('li[data-key]')) {
      if (li.getBoundingClientRect().top >= top - 1) return li.getAttribute('data-key');
    }
    return null;
  });
}
