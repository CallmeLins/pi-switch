// Auto-generated loader for pi-switch-native
const { existsSync } = require('fs');
const { join, resolve } = require('path');

const { platform, arch } = process;

function getBinaryName() {
  const parts = [platform];
  parts.push(arch);
  if (platform === 'linux') parts.push('gnu');
  else if (platform === 'win32') parts.push('msvc');
  return `pi-switch-native.${parts.join('-')}.node`;
}

const binaryName = getBinaryName();
const localPath = resolve(__dirname, binaryName);

if (!existsSync(localPath)) {
  throw new Error(`Native binding not found: ${localPath}. Run: npm run build:native`);
}

module.exports = require(localPath);
