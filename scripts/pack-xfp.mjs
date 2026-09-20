import { mkdir, cp, writeFile, readFile, chmod } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
const { version } = JSON.parse(await readFile('package.json', 'utf8'));
await mkdir('xfp-package/bin', { recursive: true });
for (const name of ['agent-browser-darwin-arm64', 'agent-browser-darwin-x64', 'agent-browser-win32-x64.exe']) {
  await cp(`native-artifacts/${name}`, `xfp-package/bin/${name}`);
  await chmod(`xfp-package/bin/${name}`, 0o755);
}
await cp('bin/agent-browser.js', 'xfp-package/bin/agent-browser.js');
await chmod('xfp-package/bin/agent-browser.js', 0o755);
for (const name of ['LICENSE', 'XFP.md']) await cp(name, `xfp-package/${name}`);
for (const name of ['LICENSE-axe-core.txt', 'LICENSE-axe-core-THIRD-PARTY.txt'])
  await cp(`cli/src/native/a11y/${name}`, `xfp-package/${name}`);
await writeFile('xfp-package/package.json', JSON.stringify({
  name: 'agent-browser', version, type: 'module', license: 'Apache-2.0',
  description: 'XFP native Agent Browser build; background tabs by default',
  repository: 'https://github.com/gfreezy/agent-browser',
  bin: { 'agent-browser': './bin/agent-browser.js' },
  files: ['bin', 'LICENSE*', 'XFP.md'],
}, null, 2));
execFileSync('npm', ['pack'], { cwd: 'xfp-package', stdio: 'inherit' });
