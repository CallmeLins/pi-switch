// ESM wrapper for pi-switch native addon
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const native = require('./pi-switch-native.cjs');

// Re-export all native functions
export const {
  initConfig,
  listPresets,
  showPreset,
  addProvider,
  listProfiles,
  showProfile,
  useProfile,
  removeProfile,
  listBackups,
  doctor,
  daemonStartNative,
  daemonStopNative,
  daemonStatusNative,
  getUsageStats,
  exportConfig,
  importConfig,
  exportDir,
  runNativeTui,
  validateConfig,
  testProvider,
  restoreBackup,
  duplicateProvider,
  exportLogsJson,
  exportLogsCsv,
  fetchModels,
  runProxyServer,
  runWebServer,
  upsertProfileRaw,
  updateExposedModels,
  updateProviderModels,
  setProxyTarget,
  setProxyFailover,
} = native;

export default native;
