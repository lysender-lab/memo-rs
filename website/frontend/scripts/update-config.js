const fs = require('node:fs');
const path = require('node:path');
const uuid = require('uuid');

function updateConfig() {
  const suffix = uuid.v4().split('-').shift();
  const target = path.resolve(__dirname, '../bundles.json');
  const data = { suffix };
  fs.writeFile(target, JSON.stringify(data), function (err) {
    if (err) {
      console.log(err);
    }
  });
}

updateConfig();
