const swc = require('@swc/core');
const fs = require('fs/promises');
const fsSync = require('node:fs');
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

// Organize js files in bundles
const bundles = [
  {
    bundle: `assets/bundles/js/gallery-${suffix}.js`,
    files: [
      'assets/vendors/photoswipe/5.4.4/umd/photoswipe.umd.min.js',
      'assets/vendors/photoswipe/5.4.4/umd/photoswipe-lightbox.umd.min.js',
      'assets/js/photo-gallery.js',
    ],
  },
  {
    bundle: `assets/bundles/js/upload-${suffix}.js`,
    files: ['assets/vendors/axios/1.7.2/axios.min.js', 'assets/js/upload.js'],
  },
  {
    bundle: `assets/bundles/js/main-${suffix}.js`,
    files: [
      'assets/js/site.js',
      'assets/js/nav.js',
      'assets/js/login.js',
      'assets/js/create-album.js',
    ],
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

  let { code, map } = await swc.transform(contents, {
    filename: filename,
    sourceMaps: true,
    isModule: false,
    jsc: {
      parser: {
        syntax: 'ecmascript',
      },
      transform: {},
    },
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
