import { describe, expect, it } from 'vitest';
import { describeBoot } from '../components/inspector/OverviewTab';

describe('linha Boot da aba Visão', () => {
  it('diz o caminho e o que aconteceu', () => {
    expect(
      describeBoot({
        channel: { kind: 'systemPromptFlag', flag: '--append-system-prompt' },
        status: 'delivered',
        message: 'BOOT.md entregue pela flag --append-system-prompt',
      }),
    ).toBe('flag de system prompt — BOOT.md entregue pela flag --append-system-prompt');
    expect(
      describeBoot({ channel: { kind: 'stdin' }, status: 'failed', message: 'sem prompt' }),
    ).toBe('terminal — sem prompt');
  });
});
