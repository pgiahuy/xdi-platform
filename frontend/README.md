# frontend

Giao diện của xdi-platform: React, Vite, TypeScript, Tailwind, shadcn/ui, React Router, TanStack Query. Build ra file
tĩnh trong `dist/`; backend phục vụ các file này cùng địa chỉ với API.

```bash
npm install
npm run dev     # mở http://localhost:5173; /api chuyển sang backend ở BACKEND_URL (mặc định http://localhost:8000)
npm run build   # kiểm kiểu rồi build ra dist/
npm run lint
```

Cấu trúc và quy tắc:

```
src/api/          gọi backend: client.ts và các hook TanStack Query, một file cho một nhóm endpoint
src/components/   component hiển thị, chỉ nhận dữ liệu qua props; ui/ là component sinh từ shadcn/ui
src/pages/        trang: nơi duy nhất nối hook với component và dùng router
src/router.tsx    bảng đường dẫn
```

Quy tắc chung của dự án ở [AGENTS.md](../AGENTS.md).
