# Cài đặt

Ba cách chạy, chọn một:

1. Docker (mục 1): nhanh nhất, chỉ cần Docker.
2. Build từ mã nguồn (mục 2): để phát triển, hoặc chạy backend không dùng Docker.
3. Đưa lên Internet (mục 3): một máy chủ có tên miền, HTTPS tự động bằng Caddy.

Sao lưu và khôi phục ở mục 4, lệnh quản trị ở mục 5, lỗi hay gặp ở mục 6.

Phần mềm đang phát triển. Phần nào đã có, xem mục Trạng thái trong [ARCHITECTURE.md](ARCHITECTURE.md).

## 1. Chạy bằng Docker

Cần: Docker Engine kèm Docker Compose v2 (`docker compose version` chạy được), Git.

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up -d --build
```

Lần đầu build lâu vì phải biên dịch backend (Rust, kèm DuckDB) và giao diện. Các lần sau dùng lại cache.

Kiểm tra:

```bash
docker compose ps                         # backend và postgres ở trạng thái healthy
curl http://localhost:8000/api/health     # {"status":"ok"}
```

Mở http://localhost:8000 để xem giao diện.

### Tạo tài khoản quản trị

Lần chạy đầu, khi chưa có tài khoản nào, backend in một mã thiết lập ra log. Chỉ ai đọc được log của máy chủ mới tạo
được tài khoản quản trị, nên người ngoài không chiếm được bản cài vừa dựng.

```bash
docker compose logs backend | grep setup_code
```

Màn hình thiết lập trên giao diện đang làm. Trong lúc chờ, tạo tài khoản bằng lệnh (thay mã, email, mật khẩu, tên
workspace):

```bash
curl -c cookie.txt http://localhost:8000/api/auth/setup \
  -H 'content-type: application/json' \
  -d '{"setup_code":"ABCD-EFGH-JKMN","email":"ban@example.com","password":"mat khau it nhat 8 ky tu","workspace_name":"Cửa hàng"}'
```

Tài khoản này là quản trị hệ thống (quản lý tài khoản của cả bản cài) và là `admin` của workspace đầu tiên. Mã thiết
lập chỉ dùng được một lần. Khởi động lại backend mà vẫn chưa có tài khoản nào thì backend in mã mới.

### Nạp dữ liệu

Giao diện nạp file đang làm. Qua API, dùng cookie vừa nhận (`<slug>` là slug của workspace, có trong kết quả lệnh trên,
ví dụ `cua-hang`):

```bash
curl -b cookie.txt http://localhost:8000/api/workspaces/<slug>/datasets \
  -F name='Bán hàng' -F file=@thang1.csv -F file=@thang2.csv
```

Nhận CSV (UTF-8 hoặc latin-1) và Parquet, một hoặc nhiều file. Nhiều file cùng cột gộp thành một bảng. Tổng dung lượng
một lần nạp tối đa `MAX_UPLOAD_MB` (mặc định 200).

### Cấu hình

Sửa `.env` rồi chạy lại `docker compose up -d`. Các biến chính (đầy đủ ở [.env.example](https://github.com/pgiahuy/xdi-platform/blob/main/.env.example)):

| Biến | Mặc định | Ý nghĩa |
|---|---|---|
| `BACKEND_PORT` | `8000` | Cổng của giao diện và API |
| `POSTGRES_PORT` | `5433` | Cổng Postgres mở ra máy host (để công cụ quản trị database kết nối) |
| `LLM_PROVIDER` | `mock` | `mock` không gọi ra ngoài; `gemini` cần `GEMINI_API_KEY`; `ollama` cần máy chạy Ollama ở `OLLAMA_URL` |
| `MAX_UPLOAD_MB` | `200` | Tổng dung lượng tối đa một lần nạp file |
| `RUST_LOG` | `info` | Mức log |

Dữ liệu nằm trong hai volume Docker: `xdi-platform_pgdata` (Postgres) và `xdi-platform_xdidata` (file Parquet). Tên
có tiền tố là tên thư mục chứa `docker-compose.yml`.

### Dừng, cập nhật, gỡ

```bash
docker compose down                       # dừng, giữ dữ liệu
git pull && docker compose up -d --build  # cập nhật; backend tự áp migration database khi khởi động
docker compose down -v                    # gỡ hẳn, XÓA cả hai volume dữ liệu
```

## 2. Build từ mã nguồn

Cần:

- Rust stable từ 1.88 trở lên (`rustup`), kèm trình biên dịch C++ (`g++` hoặc `clang`) để biên dịch DuckDB.
- Node.js 24 (hoặc 22.12 trở lên) để build giao diện.
- GNU Make, Docker (chỉ để chạy Postgres).

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
make hooks          # một lần: hook pre-push chạy kiểm tra trước khi push (dành cho người đóng góp)
make dev            # Postgres trong Docker, backend chạy bằng cargo run ở cổng 8000
```

Giao diện, ở terminal khác:

```bash
make dev-frontend   # http://localhost:5173, gọi /api sang backend ở cổng 8000
```

Hoặc build giao diện một lần rồi để backend phục vụ, giống bản chạy thật:

```bash
cd frontend && npm ci && npm run build && cd ..
STATIC_DIR=$PWD/frontend/dist make dev   # mở http://localhost:8000
```

Lần build đầu biên dịch DuckDB từ mã nguồn C++, mất vài phút; thư mục `backend/target` có thể lên vài GB. `make help`
liệt kê mọi lệnh, `make check` chạy toàn bộ kiểm tra.

Cài binary vào hệ thống (mặc định `/usr/local/bin`, đổi bằng `PREFIX`):

```bash
make install PREFIX=$HOME/.local
make uninstall PREFIX=$HOME/.local
```

Binary cần một Postgres (`DATABASE_URL`), thư mục dữ liệu ghi được (`DATA_DIR`) và, nếu muốn có giao diện, thư mục
giao diện đã build (`STATIC_DIR`). Gói binary trong mỗi bản phát hành trên GitHub chỉ có backend.

## 3. Đưa lên Internet

Cách dưới chạy Caddy trên cùng máy chủ, tự xin và gia hạn chứng chỉ HTTPS. Cần: một máy chủ Linux có Docker, một tên
miền đã trỏ (bản ghi A hoặc AAAA) về địa chỉ IP của máy chủ, cổng 80 và 443 mở ra Internet.

1. Chạy phần mềm như mục 1.
2. Không để Postgres và cổng 8000 lộ ra ngoài. Trong `.env`, cho Postgres chỉ nghe trên máy:

   ```bash
   POSTGRES_PORT=127.0.0.1:5433
   ```

   Chặn cổng 8000 từ bên ngoài bằng tường lửa, chỉ mở 80, 443 (và SSH):

   ```bash
   sudo ufw allow OpenSSH && sudo ufw allow 80,443/tcp && sudo ufw enable
   ```

   Lưu ý: Docker tự mở cổng của container qua iptables, không đi qua ufw. Nếu máy chủ không có tường lửa của nhà cung
   cấp chặn cổng 8000, hãy đặt Caddy chung mạng Docker với backend thay vì mở cổng 8000 ra host.

3. Cài Caddy (theo hướng dẫn ở https://caddyserver.com/docs/install), tạo `/etc/caddy/Caddyfile`:

   ```
   du-lieu.example.com {
       reverse_proxy localhost:8000
   }
   ```

   Rồi `sudo systemctl reload caddy`. Mở https://du-lieu.example.com.

4. Tạo tài khoản quản trị ngay (mục 1), trước khi chia sẻ địa chỉ.

Đang làm cho bản chạy thật ([#55](https://github.com/pgiahuy/xdi-platform/issues/55)): cookie đăng nhập có cờ `Secure`
khi chạy sau HTTPS, compose riêng cho bản chạy thật với mật khẩu Postgres không mặc định, `make backup`.

## 4. Sao lưu và khôi phục

Sao lưu cả hai phần: database và thư mục file Parquet. Chạy trong thư mục chứa `docker-compose.yml`:

```bash
docker compose exec -T postgres pg_dump -U xdi -Fc xdi > xdi-db.dump
docker run --rm -v xdi-platform_xdidata:/data:ro -v "$PWD":/backup debian:bookworm-slim \
  tar czf /backup/xdi-data.tgz -C /data .
```

Khôi phục vào một bản cài mới (sau `docker compose up -d` lần đầu):

```bash
docker compose stop backend
docker compose exec -T postgres pg_restore -U xdi -d xdi --clean --if-exists < xdi-db.dump
docker run --rm -v xdi-platform_xdidata:/data -v "$PWD":/backup debian:bookworm-slim \
  sh -c 'tar xzf /backup/xdi-data.tgz -C /data && chown -R 10001 /data'
docker compose start backend
```

`10001` là user không phải root mà backend chạy trong container.

## 5. Quản trị

Quên mật khẩu quản trị, không còn ai đăng nhập được: chạy trên máy chủ, lệnh in một mật khẩu tạm và đăng xuất mọi phiên
của tài khoản đó.

```bash
docker compose exec backend xdi-backend reset-password --email ban@example.com
docker compose exec backend xdi-backend list-admins
```

## 6. Lỗi hay gặp

**Build Docker lỗi `spurious network error` hoặc `transfer too slow` khi tải crate.** Mạng của Docker trên một số máy
Linux chậm hoặc chập chờn. Build bằng mạng của máy host rồi chạy không build lại:

```bash
docker build --network host -f backend/Dockerfile -t xdi-platform-backend .
docker compose up -d --no-build
```

**Cổng đã có chương trình khác dùng.** Đổi `BACKEND_PORT` hoặc `POSTGRES_PORT` trong `.env`. Đổi `POSTGRES_PORT` thì
`DATABASE_URL` trong `.env` (dùng khi chạy backend ngoài Docker) cũng đổi theo.

**Không thấy mã thiết lập trong log.** Đã có tài khoản thì backend không in mã nữa. Quên mật khẩu thì dùng lệnh
`reset-password` ở mục 5.

**Hết dung lượng đĩa khi build từ mã nguồn.** Bản build DuckDB nặng. `cargo clean` trong `backend/` xóa thư mục build;
`docker image prune` xóa image cũ.
