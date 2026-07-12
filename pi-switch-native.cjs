// Auto-generated loader for pi-switch-native
const { existsSync } = require('fs');
const { join, resolve } = require('path');
const { execSync } = require('child_process');

const { platform, arch } = process;

function detectLinuxLibc() {
  if (platform !== 'linux') return null;

  try {
    // Try to detect musl by checking ldd version
    const lddVersion = execSync('ldd --version 2>&1', { encoding: 'utf8' });
    if (lddVersion.includes('musl')) {
      return 'musl';
    }
  } catch (e) {
    // ldd might not exist or fail
  }

  // Check for musl libc file
  if (existsSync('/lib/ld-musl-x86_64.so.1') || existsSync('/lib/libc.musl-x86_64.so.1')) {
    return 'musl';
  }

  // Default to glibc on Linux
  return 'gnu';
}

function getBinaryName(libc) {
  const parts = [platform];
  parts.push(arch);
  if (platform === 'linux') {
    parts.push(libc || 'gnu');
  } else if (platform === 'win32') {
    parts.push('msvc');
  }
  return `pi-switch-native.${parts.join('-')}.node`;
}

// Try to load the appropriate binary
function loadBinary() {
  if (platform === 'linux') {
    const libc = detectLinuxLibc();
    const primaryBinary = getBinaryName(libc);
    const primaryPath = resolve(__dirname, primaryBinary);

    // Try primary binary (glibc or musl based on detection)
    if (existsSync(primaryPath)) {
      try {
        return require(primaryPath);
      } catch (e) {
        // If glibc fails (e.g., version mismatch), try musl as fallback
        if (libc === 'gnu') {
          const muslBinary = getBinaryName('musl');
          const muslPath = resolve(__dirname, muslBinary);
          if (existsSync(muslPath)) {
            try {
              return require(muslPath);
            } catch (muslError) {
              // Both failed, throw original error
              throw e;
            }
          }
        }
        throw e;
      }
    }

    throw new Error(`Native binding not found: ${primaryPath}. Run: npm run build:native`);
  }

  // Non-Linux platforms: simple load
  const binaryName = getBinaryName();
  const localPath = resolve(__dirname, binaryName);

  if (!existsSync(localPath)) {
    throw new Error(`Native binding not found: ${localPath}. Run: npm run build:native`);
  }

  return require(localPath);
}

module.exports = loadBinary();
