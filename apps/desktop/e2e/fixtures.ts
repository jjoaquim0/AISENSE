import type { Page } from '@playwright/test';
import type { FakeSeed } from '../src/e2e/fakeCore';

/** Abre o app com o core falso semeado (antes de qualquer script da página rodar). */
export async function openApp(page: Page, seed: FakeSeed = {}): Promise<void> {
  await page.addInitScript((s) => {
    (window as unknown as { __fakeSeed: FakeSeed }).__fakeSeed = s;
  }, seed);
  page.on('pageerror', (error) => {
    throw error;
  });
  await page.goto('/');
}

/** Comandos que o core falso recebeu e não conhece — um teste nunca deve deixar nenhum. */
export function unknownCommands(page: Page): Promise<string[]> {
  return page.evaluate(
    () => (window as unknown as { __fake: { unknown: string[] } }).__fake.unknown,
  );
}
