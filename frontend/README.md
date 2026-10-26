# frontend

Giao diện của xdi-platform: React, Vite, TypeScript, Tailwind, shadcn/ui, React Router, TanStack Query, Recharts.
Build ra file tĩnh trong `dist/`; backend phục vụ các file này cùng địa chỉ với API.

Cần Node.js 24 (hoặc 22.22 trở lên).

```bash
npm install
npm run dev        # mở http://localhost:5173; /api chuyển sang backend ở BACKEND_URL (mặc định http://localhost:8000)
npm run build      # kiểm kiểu (tsc -b) rồi build ra dist/
npm run lint       # oxlint, cảnh báo cũng là lỗi
npm run api:check  # schema.ts còn khớp ../openapi.json không
```

Ở gốc repo, `make dev-frontend` chạy `npm install` và `npm run dev`; `make check-frontend` chạy `npm ci`, lint,
`api:check` và build, như CI. Backend chạy ở cổng khác thì đặt `BACKEND_URL`, ví dụ
`BACKEND_URL=http://localhost:8001 npm run dev`.

Xem bản build qua backend, giống khi chạy thật: `npm run build`, rồi ở gốc repo chạy
`STATIC_DIR=$PWD/frontend/dist make dev` và mở http://localhost:8000.

Cấu trúc và quy tắc:

```
src/api/          nơi duy nhất gọi backend: client.ts và các hook TanStack Query, một file cho một nhóm endpoint;
                  schema.ts là kiểu API sinh từ ../openapi.json, examples/ask.json là response mẫu sinh từ backend
                  (cả hai bằng make openapi, không sửa tay)
src/components/   component hiển thị, chỉ nhận dữ liệu qua props, không gọi API, không dùng router; ui/ là component
                  sinh từ shadcn/ui, shell/ là bố cục chung (thanh bên, thanh trên, đổi workspace, tài khoản)
src/pages/        trang: nơi duy nhất nối hook với component và dùng router; AppLayout dựng bố cục chung
src/router.tsx    bảng đường dẫn; trang trong workspace nằm dưới /w/:ws, mục điều hướng ở pages/workspaceNav.ts;
                  trang quản trị hệ thống (/admin/...) và tài khoản (/account) nằm ngoài workspace, cùng khung
                  AppLayout, đường dẫn ở pages/globalNav.ts;
                  đăng nhập (/login) và thiết lập lần đầu (/setup) nằm ngoài khung, đường dẫn ở pages/authNav.ts
```

Đổi API: sửa backend rồi chạy `make openapi` ở gốc repo; lệnh này ghi lại `../openapi.json`, `src/api/examples/ask.json`
và `src/api/schema.ts` (qua `npm run api:types`). Hook trong `src/api` lấy kiểu từ `schema.ts`, không khai lại.

Trang hỏi dữ liệu khi chạy `npm run dev` có công tắc "Dữ liệu mẫu": trả lời bằng các response mẫu trong
`src/api/examples/ask.json` và dòng nguồn mẫu (`src/api/drillSamples.ts`) thay vì gọi backend, để làm giao diện khi
không có mô hình ngôn ngữ hay dataset DataCo. Bản build không có công tắc này và
không chứa file mẫu.

Quy tắc chung của dự án ở [AGENTS.md](../AGENTS.md).
