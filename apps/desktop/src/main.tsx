import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from '@/App';
import './styles/globals.css';

async function boot(): Promise<void> {
  // Só com `vite --mode e2e`: um core falso no lugar do Rust (F08-08). Em qualquer outro
  // modo o bloco some do bundle.
  if (import.meta.env.MODE === 'e2e') {
    const { installFakeCore } = await import('./e2e/fakeCore');
    installFakeCore();
  }

  const container = document.getElementById('root');
  if (!container) throw new Error('Elemento #root não encontrado em index.html');

  createRoot(container).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}

void boot();
