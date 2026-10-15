import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import MarkdownIt from 'markdown-it';
const root = process.cwd();
const files = ['README.md', 'docs/images/README.md'];
const md = new MarkdownIt({ html: true });
let checked = 0;
for (const file of files) {
  const source = fs.readFileSync(file, 'utf8');
  assert(!source.includes('\uFFFD'), 'Invalid encoding: ' + file);
  const urls = [...source.matchAll(/(?:src|href)="([^"]+)"|\]\(([^)]+)\)/g)].map(m => m[1] || m[2]);
  for (const url of urls) {
    if (/^https?:/.test(url)) continue;
    if (url.startsWith('#')) { assert(source.includes(`id="${url.slice(1)}"`), 'Missing anchor: ' + url); continue; }
    assert(fs.existsSync(path.resolve(path.dirname(file), url)), 'Broken link: ' + url);
    checked++;
  }
  const tokens = md.parse(source, {});
  const headings = tokens.filter(t => t.type === 'heading_open');
  assert(headings.length >= (file === 'README.md' ? 7 : 2), 'Headings were swallowed by HTML');
  const images = tokens.flatMap(t => t.children || []).filter(t => t.type === 'image');
  for (const image of images) assert(image.content, 'Image missing alt text');
  if (file === 'README.md') {
    fs.writeFileSync('.scratch/readme-capture/readme-preview.html', '<!doctype html><html lang="ko"><meta charset="utf-8"><base href="../../"><title>Rhyme Terminal README</title><style>body{margin:40px auto;padding:0 32px;max-width:960px;font:16px/1.65 system-ui;color:#24292f}img{max-width:100%;height:auto}h1,h2{border-bottom:1px solid #d0d7de;padding-bottom:.3em}table{border-collapse:collapse;width:100%;margin:16px 0}td,th{padding:8px 12px;border:1px solid #d0d7de}tr:nth-child(2n){background:#f6f8fa}a{color:#0969da}pre{padding:16px;overflow:auto;background:#f6f8fa;border-radius:6px}code{font:13px/1.5 Consolas,monospace}summary{cursor:pointer}details{margin:16px 0}sub{font-size:12px}</style><body>' + md.render(source) + '</body></html>');
    console.log('README headings:', headings.length, '/ screenshots:', images.length);
  }
}
for (const name of ['terminal-workspace','rhyme-flow','ai-providers']) {
  const data = fs.readFileSync(`docs/images/${name}.png`);
  assert.equal(data.subarray(1,4).toString(), 'PNG');
  assert.equal(data.readUInt32BE(16), 1600);
  assert.equal(data.readUInt32BE(20), 960);
  console.log(`${name}.png: ${Math.round(data.length/1024)} KiB, 1600x960`);
}
console.log(`PASS: ${checked} local references, Markdown rendering, encoding and all screenshot dimensions.`);
