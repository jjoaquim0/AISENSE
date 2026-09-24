import { describe, expect, it } from 'vitest';
import type { SessionSettings } from '@/types/generated/SessionSettings';
import type { TeamSummary } from '@/types/generated/TeamSummary';
import { teamToRestore } from '../restore';

const session = (over: Partial<SessionSettings> = {}): SessionSettings => ({
  restoreLastTeam: true,
  relaunchAgents: false,
  lastTeam: 'team_a',
  runningAgents: [],
  ...over,
});
const team = (id: string, archivedAt: number | null = null) =>
  ({ team: { id, archivedAt }, agents: [] }) as unknown as TeamSummary;

describe('teamToRestore', () => {
  it('reabre a última equipe', () => {
    expect(teamToRestore(session(), [team('team_a'), team('team_b')], null)).toBe('team_a');
  });
  it('respeita a opção desligada, a equipe apagada ou arquivada e a escolha já feita', () => {
    expect(teamToRestore(session({ restoreLastTeam: false }), [team('team_a')], null)).toBeNull();
    expect(teamToRestore(session(), [team('team_b')], null)).toBeNull();
    expect(teamToRestore(session(), [team('team_a', 1)], null)).toBeNull();
    expect(teamToRestore(session(), [team('team_a')], 'team_b')).toBeNull();
    expect(teamToRestore(session({ lastTeam: null }), [team('team_a')], null)).toBeNull();
  });
});
