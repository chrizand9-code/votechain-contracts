/**
 * Bundle size checker for VoteChain frontend.
 *
 * Budget thresholds:
 *   - Main JS bundle:  500 KB gzipped  (hard limit — CI fails if exceeded)
 *   - Total assets:    no hard limit    (informational)
 *
 * A PR comment is posted automatically by the frontend-ci workflow when
 * the main bundle changes by more than 5% relative to the base branch.
 *
 * Usage:
 *   node scripts/bundle-size.js
 *
 * Environment variables:
 *   BUNDLE_SIZE_LIMIT_KB  Override the gzip limit in KB (default: 500)
 */

import fs from 'fs';
import path from 'path';
import zlib from 'zlib';

const GZIP_LIMIT_KB = Number(process.env['BUNDLE_SIZE_LIMIT_KB'] ?? 500);
const assetsDir = path.resolve(process.cwd(), 'dist', 'assets');

if (!fs.existsSync(assetsDir)) {
  console.error('dist/assets directory not found; run npm run build first.');
  process.exit(1);
}

const bundleFiles = fs.readdirSync(assetsDir).filter((file) => file.endsWith('.js'));

if (bundleFiles.length === 0) {
  console.error('No JavaScript bundle files found in dist/assets.');
  process.exit(1);
}

/**
 * Compute the gzipped size of a file in bytes.
 * @param {string} filePath
 * @returns {number}
 */
function gzipSize(filePath) {
  const content = fs.readFileSync(filePath);
  return zlib.gzipSync(content).length;
}

let mainBundleFile = null;
let mainBundleGzipKb = 0;
let failed = false;

const results = [];

for (const file of bundleFiles) {
  const filePath = path.join(assetsDir, file);
  const rawBytes = fs.statSync(filePath).size;
  const gzipBytes = gzipSize(filePath);
  const rawKb = (rawBytes / 1024).toFixed(2);
  const gzipKb = (gzipBytes / 1024).toFixed(2);

  results.push({ file, rawKb, gzipKb, gzipBytes });

  // Identify the main entry bundle (typically the largest JS file)
  if (gzipBytes > mainBundleGzipKb) {
    mainBundleGzipKb = gzipBytes;
    mainBundleFile = file;
  }

  console.log(`${file}: ${rawKb} KB raw | ${gzipKb} KB gzip`);
}

const totalRaw = bundleFiles.reduce((sum, file) => {
  return sum + fs.statSync(path.join(assetsDir, file)).size;
}, 0);

const totalGzip = bundleFiles.reduce((sum, file) => {
  return sum + gzipSize(path.join(assetsDir, file));
}, 0);

console.log(`\nTotal bundle: ${(totalRaw / 1024).toFixed(2)} KB raw | ${(totalGzip / 1024).toFixed(2)} KB gzip`);
console.log(`\nBudget limit: ${GZIP_LIMIT_KB} KB gzip (main bundle)`);

// Enforce gzip limit on the main bundle
const mainGzipKb = mainBundleGzipKb / 1024;
if (mainGzipKb > GZIP_LIMIT_KB) {
  console.error(
    `\n❌ BUDGET EXCEEDED: Main bundle "${mainBundleFile}" is ${mainGzipKb.toFixed(2)} KB gzipped` +
    ` (limit: ${GZIP_LIMIT_KB} KB). Reduce bundle size before merging.`,
  );
  failed = true;
} else {
  console.log(
    `\n✅ Budget OK: Main bundle "${mainBundleFile}" is ${mainGzipKb.toFixed(2)} KB gzipped` +
    ` (limit: ${GZIP_LIMIT_KB} KB).`,
  );
}

// Write a machine-readable size report for the CI PR-comment step
const report = {
  mainBundle: mainBundleFile,
  mainBundleGzipKb: parseFloat(mainGzipKb.toFixed(2)),
  totalGzipKb: parseFloat((totalGzip / 1024).toFixed(2)),
  limitKb: GZIP_LIMIT_KB,
  passed: !failed,
  bundles: results.map(({ file, rawKb, gzipKb }) => ({ file, rawKb, gzipKb })),
};

fs.writeFileSync(
  path.resolve(process.cwd(), 'dist', 'bundle-report.json'),
  JSON.stringify(report, null, 2),
);

if (failed) {
  process.exit(1);
}
