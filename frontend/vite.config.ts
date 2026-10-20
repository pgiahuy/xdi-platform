// SPDX-License-Identifier: Apache-2.0
/* vite.config.ts - build giao diện thành file tĩnh trong dist/; backend phục vụ các file này.
Khi chạy dev, mọi lời gọi /api chuyển sang backend Rust, nên trình duyệt chỉ thấy một địa chỉ giống bản chạy thật. */

import path from 'node:path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

const backend = process.env.BACKEND_URL ?? 'http://localhost:8000'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': path.resolve(__dirname, './src') } },
  server: { proxy: { '/api': backend } },
})
