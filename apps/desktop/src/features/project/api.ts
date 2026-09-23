import { invoke } from '@tauri-apps/api/core';
import type { ProjectLookup } from '@/types/generated/ProjectLookup';

/** Único ponto do front que chama os comandos do `aisense.toml` (docs/10, docs/17). */
export const projectApi = {
  lookup: (workdir: string): Promise<ProjectLookup> => invoke('project_lookup', { workdir }),
  /** Grava o arquivo revisado. O core valida o conteúdo e nunca sobrescreve um existente. */
  accept: (workdir: string, content: string): Promise<ProjectLookup> =>
    invoke('project_accept', { workdir, content }),
};
