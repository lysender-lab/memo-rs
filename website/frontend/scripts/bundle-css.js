const fs = require('fs/promises');
const fsSync = require('node:fs');
const lightningcss = require('lightningcss');
const path = require('node:path');

const ROOT_DIR = path.join(__dirname, '..');
const SOURCE_DIR = path.join(ROOT_DIR, 'public');
const DEST_DIR = path.join(ROOT_DIR, 'public');

const configContents = fsSync.readFileSync(
  path.join(__dirname, '..', 'bundles.json'),
  {
    encoding: 'utf-8',
  },
);
let suffix = null;
const config = JSON.parse(configContents);
if (config && config['suffix']) {
  suffix = config['suffix'];
}

if (!suffix) {
  throw Error('Unable to indentify current bundle suffix.');
}

// Organize css files in bundles
const bundles = [
  {
    bundle: `assets/bundles/css/gallery-${suffix}.css`,
    files: ['assets/vendors/photoswipe/5.4.4/photoswipe.css'],
  },
  {
    bundle: `/assets/bundles/css/main-${suffix}.css`,
    files: ['assets/css/style.css'],
  },
];

async function minifyBundle(destFile, files) {
  // Compile all file contents
  let contents = '';
  for (const file of files) {
    const content = await fs.readFile(file);
    contents = contents.concat(content.toString(), '\n');
  }

  const pathChunks = destFile.split('/');
  const filename = pathChunks.pop();

  let { code, map } = lightningcss.transform({
    filename: filename,
    code: Buffer.from(contents),
    minify: true,
    sourceMap: true,
  });

  // Save code and map
  const destMap = `${destFile}.map`;
  await fs.writeFile(destFile, code);
  await fs.writeFile(destMap, map);
}

async function run() {
  for (const bundle of bundles) {
    const destPath = path.join(DEST_DIR, ...bundle.bundle.split('/'));
    const files = bundle.files.map((file) => {
      return path.join(SOURCE_DIR, ...file.split('/'));
    });

    await minifyBundle(destPath, files);
  }
}

run();
