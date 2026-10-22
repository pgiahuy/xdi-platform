# frontend

Giao diện của xdi-platform: React, Vite, TypeScript, Tailwind, shadcn/ui, React Router, TanStack Query. Build ra file
tĩnh trong `dist/`; backend phục vụ các file này cùng địa chỉ với API.

```bash
npm install
npm run dev     # mở http://localhost:5173; /api chuyển sang backend ở BACKEND_URL (mặc định http://localhost:8000)
npm run build   # kiểm kiểu rồi build ra dist/
npm run lint
```

Xem bản build qua backend, giống khi chạy thật: `npm run build`, rồi ở gốc repo chạy
`STATIC_DIR=$PWD/frontend/dist make dev` và mở http://localhost:8000.

Cấu trúc và quy tắc:

```
src/api/          gọi backend: client.ts và các hook TanStack Query, một file cho một nhóm endpoint; schema.ts là kiểu
                  API sinh từ ../openapi.json (make openapi), không sửa tay
src/components/   component hiển thị, chỉ nhận dữ liệu qua props; ui/ là component sinh từ shadcn/ui,
                  shell/ là bố cục chung (thanh bên, thanh trên, đổi workspace, tài khoản)
src/pages/        trang: nơi duy nhất nối hook với component và dùng router; WorkspaceLayout dựng bố cục chung
src/router.tsx    bảng đường dẫn; trang trong workspace nằm dưới /w/:ws, mục điều hướng ở pages/workspaceNav.ts
                  đăng nhập (/login) và thiết lập lần đầu (/setup) nằm ngoài khung, đường dẫn ở pages/authNav.ts
```

Quy tắc chung của dự án ở [AGENTS.md](../AGENTS.md).
