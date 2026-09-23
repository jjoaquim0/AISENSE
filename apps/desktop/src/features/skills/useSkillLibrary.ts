import { useCallback, useEffect, useState } from 'react';
import { errorMessage } from '@/features/teams/api';
import type { SkillLibraryView } from '@/types/generated/SkillLibraryView';
import { onSkillsChanged, skillsApi } from './api';

/**
 * A biblioteca de skills, sempre atual: recarrega quando um `SKILL.md` muda no disco
 * (evento `skills:changed`, F04-02) — sem reiniciar o app.
 */
export function useSkillLibrary() {
  const [library, setLibrary] = useState<SkillLibraryView | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  const reload = useCallback(() => {
    skillsApi
      .library()
      .then((view) => {
        setLibrary(view);
        setProblem(null);
      })
      .catch((e: unknown) => setProblem(errorMessage(e)));
  }, []);

  useEffect(() => {
    reload();
    const unlisten = onSkillsChanged(reload);
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [reload]);

  return { library, problem, reload };
}
