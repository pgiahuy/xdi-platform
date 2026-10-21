// licenses.mjs - sinh docs/licenses-dependencies.md: giấy phép của mọi crate Rust và gói npm mà dự án kéo về.
// Chạy bằng make licenses sau khi đổi Cargo.lock hoặc package-lock.json. Nguồn: cargo metadata và package-lock.json,
// tức đúng các phiên bản đã khóa, không phải bản mới nhất trên registry.
// Bảng tóm tắt và phần đánh giá viết tay nằm ở docs/LICENSES.md.

import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'

// Giấy phép cho phép, dùng chung với Apache-2.0 không cần điều kiện gì thêm ngoài giữ thông báo bản quyền.
const PERMISSIVE = ['MIT', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'ISC', '0BSD', 'Zlib', 'Unicode-3.0',
  'Unlicense', 'CC0-1.0', 'BSL-1.0', 'BlueOak-1.0.0', 'Python-2.0', 'CC-BY-4.0']

function isPermissive(license) {
  // "MIT OR Apache-2.0": một lựa chọn cho phép là đủ. "A AND B": cả hai đều phải cho phép.
  const tokens = license.replace(/[()]/g, ' ').split(/\s+/).filter((t) => t && t !== 'OR' && t !== 'AND')
  if (license.includes(' AND ')) return tokens.every((t) => PERMISSIVE.includes(t))
  return tokens.some((t) => PERMISSIVE.includes(t))
}

function rustPackages() {
  const out = execFileSync('cargo', ['metadata', '--format-version', '1', '--manifest-path', 'backend/Cargo.toml'], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  })
  return JSON.parse(out)
    .packages.filter((p) => p.source) // bỏ chính crate của dự án (source null)
    .map((p) => ({ name: p.name, version: p.version, license: (p.license || 'NOASSERTION').replace(/\//g, ' OR ') }))
}

function npmPackages() {
  const lock = JSON.parse(readFileSync('frontend/package-lock.json', 'utf8'))
  return Object.entries(lock.packages)
    .filter(([path]) => path !== '')
    .map(([path, p]) => ({
      name: path.split('node_modules/').pop(),
      version: p.version,
      license: p.license || 'NOASSERTION',
      dev: Boolean(p.dev),
    }))
}

// withDev: thêm cột cho biết gói npm chỉ dùng lúc build (devDependencies), không vào giao diện chạy thật.
function table(rows, withDev) {
  const head = withDev ? ['Gói', 'Phiên bản', 'Giấy phép', 'Chỉ lúc build'] : ['Crate', 'Phiên bản', 'Giấy phép']
  const line = (cells) => `| ${cells.join(' | ')} |`
  const body = [...rows]
    .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))
    .map((r) => line(withDev ? [r.name, r.version, r.license, r.dev ? 'có' : ''] : [r.name, r.version, r.license]))
  return [line(head), line(head.map(() => '---')), ...body].join('\n')
}

const rust = rustPackages()
const npm = npmPackages()
const flagged = [...rust, ...npm].filter((r) => !isPermissive(r.license))

const doc = `# Giấy phép các phụ thuộc

File này sinh bằng \`make licenses\` (scripts/licenses.mjs), không sửa tay. Đánh giá và bảng tóm tắt ở
[LICENSES.md](LICENSES.md).

${rust.length} crate Rust (từ \`backend/Cargo.lock\`, gồm cả crate chỉ dùng trên nền tảng khác Linux), ${npm.length} gói npm
(từ \`frontend/package-lock.json\`). Giấy phép không thuộc nhóm cho phép thông dụng: ${flagged.length}.

## Không thuộc nhóm cho phép thông dụng

${flagged.length ? table(flagged, true) : 'Không có.'}

## Rust

${table(rust, false)}

## npm

${table(npm, true)}
`

writeFileSync('docs/licenses-dependencies.md', doc)
console.log(`docs/licenses-dependencies.md: ${rust.length} crate, ${npm.length} npm, ${flagged.length} cần xem tay`)
