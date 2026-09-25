// Imagens do README e da divulgação, na identidade do manual de marca (F09-10).
// Uso: pnpm --filter @aisense/desktop readme-images   (servidor e2e em :5181)
// Gera em assets/readme/: logo claro/escuro, banner de divulgação (1280×640, o tamanho
// da prévia social do GitHub) e capturas da interface com o core falso.
import { mkdirSync, readFileSync } from 'node:fs';
import { chromium } from '@playwright/test';

const out = process.argv[2] ?? '../../assets/readme';
const base = process.env.E2E_URL ?? 'http://localhost:5181/';
mkdirSync(out, { recursive: true });
const browser = await chromium.launch(
  process.env.PW_CHROMIUM_PATH ? { executablePath: process.env.PW_CHROMIUM_PATH } : {},
);

const font = (pkg, file) =>
  readFileSync(`node_modules/@fontsource-variable/${pkg}/files/${file}`).toString('base64');
const FONTS = `
  @font-face { font-family: 'Bricolage'; font-weight: 200 800;
    src: url(data:font/woff2;base64,${font('bricolage-grotesque', 'bricolage-grotesque-latin-wght-normal.woff2')}) format('woff2'); }
  @font-face { font-family: 'Jakarta'; font-weight: 200 800;
    src: url(data:font/woff2;base64,${font('plus-jakarta-sans', 'plus-jakarta-sans-latin-wght-normal.woff2')}) format('woff2'); }`;

// Símbolo Elo (manual, seção 01): traços na cor do texto, nó do topo verde.
const elo = (size, node = '#2EE68A') => `
  <svg width="${size}" height="${size}" viewBox="0 0 64 64">
    <path d="M32 15.5L13 49.5M32 15.5L51 49.5" fill="none" stroke="currentColor" stroke-width="7" stroke-linecap="round"/>
    <circle cx="13" cy="49.5" r="9" fill="currentColor"/><circle cx="51" cy="49.5" r="9" fill="currentColor"/>
    <circle cx="32" cy="15.5" r="10" fill="${node}"/></svg>`;
// Escrita: minúsculas, Bricolage 650, −4%, ponto do "i" verde.
const wordmark = (size) => `
  <span style="font-family:Bricolage;font-weight:650;font-size:${size}px;letter-spacing:-.04em;line-height:1;padding-bottom:${size * 0.08}px">a<span style="position:relative;display:inline-block">ı<span style="position:absolute;left:50%;top:.035em;width:.2em;height:.2em;border-radius:50%;background:#2EE68A;transform:translateX(-50%)"></span></span>sense</span>`;

async function render(html, path, { width, height, transparent = false }) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 2 });
  await page.setContent(
    `<html><head><style>${FONTS} *{margin:0;box-sizing:border-box} body{width:${width}px;height:${height}px;${transparent ? 'background:transparent' : ''}}</style></head><body>${html}</body></html>`,
  );
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path, omitBackground: transparent });
  await page.close();
}

// Logo horizontal nas duas versões do manual: principal (fundo claro) e negativa (escuro).
for (const [name, color] of [
  ['logo-claro', '#0C0F0D'],
  ['logo-escuro', '#FFFFFF'],
]) {
  await render(
    `<div style="height:120px;display:flex;align-items:center;justify-content:center;gap:18px;color:${color}">${elo(68)}${wordmark(68)}</div>`,
    `${out}/${name}.png`,
    { width: 440, height: 120, transparent: true },
  );
}

// Banner de divulgação / prévia social: fundo Tinta, verde só no nó e num detalhe.
await render(
  `<div style="width:1280px;height:640px;background:#0C0F0D;color:#F5F6F4;padding:72px 88px;display:flex;flex-direction:column;justify-content:space-between;font-family:Jakarta">
     <div style="display:flex;align-items:center;gap:14px">${elo(44)}${wordmark(44)}</div>
     <div style="display:flex;flex-direction:column;gap:26px">
       <div style="font-family:Bricolage;font-weight:700;font-size:104px;line-height:.95;letter-spacing:-.045em">Uma equipe.<br>Muitos agentes.</div>
       <div style="font-size:26px;line-height:1.5;color:#C9CFCB;max-width:900px">Monte equipes de agentes de IA em terminais reais que conversam entre si, dividem um quadro de tarefas e sobem já sabendo o seu papel.</div>
     </div>
     <div style="font-family:ui-monospace,monospace;font-size:17px;letter-spacing:.08em;text-transform:uppercase;color:#9BA39E">
       <span style="color:#2EE68A">●</span>&nbsp; Claude Code · Codex · OpenCode · Gemini CLI · qualquer terminal &nbsp;—&nbsp; macOS · Windows · Linux
     </div>
   </div>`,
  `${out}/banner.png`,
  { width: 1280, height: 640 },
);

// Capturas da interface com o core falso, contando uma história de uso real.
const SQUAD = {
  name: 'Squad Produto',
  handles: ['arquiteto', 'backend', 'frontend', 'revisor'],
  running: true,
};
const CARDS = [
  { title: 'Definir o contrato OAuth do /auth', column: 'done', assignee: 'arquiteto', labels: ['api'] },
  { title: 'Endpoint de troca de token', column: 'review', assignee: 'backend', labels: ['api'], checklist: [['testes', true], ['migração', true]] },
  { title: 'Tela de login com o provedor', column: 'doing', assignee: 'frontend', labels: ['ui'], priority: 'high' },
  { title: 'Refresh token com rotação', column: 'doing', assignee: 'backend', labels: ['api'], checklist: [['rotação', true], ['revogação', false]] },
  { title: 'Revisar o fluxo de logout', column: 'todo', assignee: 'revisor' },
  { title: 'Documentar a migração para OAuth', column: 'todo', labels: ['docs'] },
  { title: 'Remover as sessões antigas', column: 'backlog', priority: 'low' },
  { title: 'Rate limit no /auth/token', column: 'blocked', assignee: 'backend', labels: ['api'] },
];
const TALK = [
  ['@arquiteto', '@backend', 'Contrato do /auth/token está no quadro. Pode começar pela troca de token.'],
  ['@backend', '@arquiteto', 'Fechado. Subo o endpoint e aviso o @revisor.'],
  ['@frontend', '@backend', 'O /auth/token devolve expires_in em segundos?'],
  ['@backend', '@frontend', 'Segundos, sim. E o refresh vem em cookie httpOnly.'],
  ['@backend', '@revisor', 'Troca de token pronta para revisão: 4 arquivos, testes passando.'],
  ['@revisor', '@backend', 'Revisado. Um ajuste: validar o state antes de trocar o code.'],
];

async function app(scheme, seed) {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    colorScheme: scheme,
    deviceScaleFactor: 2,
  });
  const page = await context.newPage();
  await page.addInitScript((s) => {
    window.__fakeSeed = s;
  }, seed);
  await page.goto(base);
  return page;
}

const route = (page, from, to, body) =>
  page.evaluate(([f, t, b]) => window.__fake.route(f, t, b), [from, to, body]);

for (const scheme of ['dark', 'light']) {
  const page = await app(scheme, { onboardingDone: true, team: SQUAD, cards: CARDS });
  await page.getByText('Squad Produto').first().click();
  await page.waitForTimeout(800);
  for (const [from, to, body] of TALK) await route(page, from, to, body);
  for (const [view, name] of [
    ['Grade', 'sala-grade'],
    ['Mensagens', 'sala-mensagens'],
  ]) {
    await page.getByRole('radio', { name: new RegExp(view) }).click();
    await page.waitForTimeout(900);
    await page.screenshot({ path: `${out}/${name}-${scheme}.png` });
  }
  // O quadro pede a largura toda: sem o inspetor e sem a lista de agentes.
  await page.keyboard.press('Control+i');
  await page.keyboard.press('Control+b');
  await page.getByRole('radio', { name: /Quadro/ }).click();
  await page.waitForTimeout(900);
  await page.screenshot({ path: `${out}/sala-quadro-${scheme}.png` });
  await page.context().close();
}

await browser.close();
console.log(`imagens em ${out}`);
