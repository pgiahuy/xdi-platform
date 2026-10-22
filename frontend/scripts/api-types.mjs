// SPDX-License-Identifier: Apache-2.0
// scripts/api-types.mjs - sinh src/api/schema.ts (kiểu TypeScript của API) từ openapi.json ở gốc repo, bằng
// openapi-typescript. make openapi gọi script này sau khi backend in openapi.json.
//   node scripts/api-types.mjs          ghi src/api/schema.ts
//   node scripts/api-types.mjs --check  không ghi; thoát 1 nếu schema.ts lệch openapi.json (make check-frontend, CI)
// Kiểu API sinh ra, không viết tay (AGENTS.md mục 2): sửa backend rồi chạy make openapi, không sửa schema.ts.

import { readFile, writeFile } from 'node:fs/promises'
import openapiTS, { astToString } from 'openapi-typescript'

const specUrl = new URL('../../openapi.json', import.meta.url)
const outUrl = new URL('../src/api/schema.ts', import.meta.url)

const header = `// SPDX-License-Identifier: Apache-2.0
/* api/schema.ts - FILE SINH TỰ ĐỘNG, không sửa tay. Kiểu TypeScript của mọi endpoint và schema trong openapi.json ở
gốc repo, sinh bằng openapi-typescript (frontend/scripts/api-types.mjs). Đổi API: sửa backend, chạy make openapi, commit
cả openapi.json và file này. Các hook trong src/api lấy kiểu từ đây, không tự khai lại. */

`

const generated = header + astToString(await openapiTS(specUrl))

if (process.argv.includes('--check')) {
  const current = await readFile(outUrl, 'utf8').catch(() => '')
  if (current !== generated) {
    console.error('src/api/schema.ts is out of date with openapi.json: run `make openapi` and commit both files')
    process.exit(1)
  }
  console.log('src/api/schema.ts matches openapi.json')
} else {
  await writeFile(outUrl, generated)
  console.log('wrote src/api/schema.ts')
}
