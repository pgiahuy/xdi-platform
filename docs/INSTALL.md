# Cài đặt

Ba cách chạy, chọn một:

1. Docker (mục 1): nhanh nhất, chỉ cần Docker.
2. Build từ mã nguồn (mục 2): để phát triển, hoặc chạy backend không dùng Docker.
3. Đưa lên Internet (mục 3): một máy chủ có tên miền, HTTPS tự động bằng Caddy.

Sao lưu và khôi phục ở mục 4, lệnh quản trị ở mục 5, lỗi hay gặp ở mục 6.

Phần mềm đang phát triển. Phần nào đã có, xem mục Trạng thái trong [ARCHITECTURE.md](ARCHITECTURE.md).

## 1. Chạy bằng Docker

Cần: Docker Engine kèm Docker Compose v2 (`docker compose version` chạy được), Git. Chưa có Docker: xem
https://docs.docker.com/engine/install/. Trên Linux, để chạy `docker` không cần `sudo`, thêm user vào nhóm `docker`
theo https://docs.docker.com/engine/install/linux-postinstall/ (đăng xuất rồi đăng nhập lại). Trên Windows, chạy các
lệnh dưới đây trong WSL2.

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

Mở http://localhost:8000: lần đầu giao diện hiện màn **Thiết lập lần đầu**. Nhập mã thiết lập (gõ chữ thường hay thừa
khoảng trắng vẫn nhận), email, mật khẩu từ 8 đến 128 ký tự và tên workspace đầu tiên. Xong thì giao diện vào thẳng
workspace vừa tạo.

Máy chủ không có trình duyệt thì tạo bằng lệnh (thay mã, email, mật khẩu, tên workspace):

```bash
curl http://localhost:8000/api/auth/setup \
  -H 'content-type: application/json' \
  -d '{"setup_code":"ABCD-EFGH-JKMN","email":"ban@example.com","password":"mat khau it nhat 8 ky tu","workspace_name":"Cửa hàng"}'
```

Tài khoản này là quản trị hệ thống (quản lý tài khoản của cả bản cài) và là `admin` của workspace đầu tiên. Mã thiết
lập chỉ dùng được một lần. Khởi động lại backend mà vẫn chưa có tài khoản nào thì backend in mã mới.

### Nạp dữ liệu

Giao diện nạp file đang làm. Qua API: đăng nhập bằng lệnh để lấy cookie phiên vào `cookie.txt` (file này là chìa khóa
phiên của bạn, xóa sau khi dùng xong), rồi nạp file. `<slug>` là slug của workspace, có trong kết quả lệnh đăng nhập,
ví dụ `cua-hang`:

```bash
curl -c cookie.txt http://localhost:8000/api/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"ban@example.com","password":"mat khau it nhat 8 ky tu"}'
curl -b cookie.txt http://localhost:8000/api/workspaces/<slug>/datasets \
  -F name='Bán hàng' -F file=@thang1.csv -F file=@thang2.csv
```

Nhận CSV (UTF-8, hoặc windows-1252 và latin-1 như file xuất từ phần mềm cũ) và Parquet, một hoặc nhiều file. File UTF-8
có byte hỏng bị từ chối kèm số dòng: lưu lại dạng UTF-8 rồi nạp lại. Nhiều file cùng cột gộp thành một bảng. Tổng dung lượng
một lần nạp tối đa `MAX_UPLOAD_MB` (mặc định 200).

File Excel (.xlsx) nạp từng file một. Xem tên sheet và cột, rồi chọn sheet cần nạp (lặp `-F sheet=` để gộp nhiều sheet
cùng cột thành một bảng; file chỉ có một sheet thì không cần chọn):

```bash
curl -b cookie.txt http://localhost:8000/api/workspaces/<slug>/datasets/sheets -F file=@ban_hang.xlsx
curl -b cookie.txt http://localhost:8000/api/workspaces/<slug>/datasets \
  -F name='Bán hàng' -F sheet='Năm 2023' -F sheet='Năm 2024' -F file=@ban_hang.xlsx
```

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

- Rust stable từ 1.89 trở lên (`rustup`), kèm trình biên dịch C++ (`g++` hoặc `clang`) để biên dịch DuckDB.
- Node.js 24 (hoặc 22.22 trở lên) để build giao diện.
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

`make dev` không đọc `.env`; biến cần đổi thì đặt trên dòng lệnh, ví dụ `BACKEND_PORT=8001 make dev`. Lần chạy đầu
mã thiết lập in ra terminal, tạo tài khoản như mục 1.

Lần build đầu biên dịch DuckDB từ mã nguồn C++, mất vài phút; thư mục `backend/target` lớn dần qua mỗi lần build, có
thể lên hàng chục GB (`cargo clean` để xóa). `make help`
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
miền đã trỏ (bản ghi A hoặc AAAA) về địa chỉ IP của máy chủ, cổng 80 và 443 mở ra Internet và chưa có chương trình
khác dùng (nginx, apache).

1. Clone và tạo `.env` như mục 1, nhưng **trước khi chạy lần đầu**, cho backend và Postgres chỉ nghe trên chính máy
   chủ. Thêm vào `.env`:

   ```bash
   BACKEND_BIND=127.0.0.1
   POSTGRES_PORT=127.0.0.1:5433
   ```

   Không có hai dòng này, cổng 8000 (HTTP thường, chưa mã hóa) và Postgres (mật khẩu mặc định) mở ra Internet. Tường lửa
   như ufw không chặn được, vì Docker tự mở cổng của container qua iptables. Đã chạy rồi mới thêm thì chạy lại
   `docker compose up -d` để áp cấu hình. Kiểm: `ss -ltnp | grep -E ':8000|:5433'` chỉ thấy `127.0.0.1`.

2. Chạy và tạo tài khoản quản trị như mục 1 (lệnh `curl` gọi `http://localhost:8000` vẫn chạy trên máy chủ).

3. Cài Caddy theo https://caddyserver.com/docs/install. Thay nội dung `/etc/caddy/Caddyfile` (gói cài sẵn đã tạo
   file này) bằng:

   ```
   du-lieu.example.com {
       reverse_proxy localhost:8000
   }
   ```

   Rồi `sudo systemctl reload caddy`. Mở https://du-lieu.example.com.

Đang làm cho bản chạy thật ([#55](https://github.com/pgiahuy/xdi-platform/issues/55)): cookie đăng nhập có cờ `Secure`
khi chạy sau HTTPS, compose riêng cho bản chạy thật với mật khẩu Postgres không mặc định, `make backup`.

## 4. Sao lưu và khôi phục

Sao lưu cả hai phần: database và thư mục file Parquet. Chạy trong thư mục chứa `docker-compose.yml`. Hai file sao lưu
có dữ liệu và chuỗi băm mật khẩu: cất ở chỗ an toàn, ngoài thư mục repo.

```bash
docker compose exec -T postgres pg_dump -U xdi -Fc xdi > ~/xdi-db.dump
docker compose exec -T backend tar czf - -C /data . > ~/xdi-data.tgz
```

Khôi phục vào một bản cài mới: chỉ bật Postgres, khôi phục, rồi mới bật backend. Nếu bật backend trước, nó tạo schema
trống và in một mã thiết lập thừa.

```bash
docker compose up -d postgres
docker compose exec -T postgres pg_restore -U xdi -d xdi < ~/xdi-db.dump
docker compose run --rm --no-deps -T --user root backend \
  sh -c 'tar xzf - -C /data && chown -R 10001 /data' < ~/xdi-data.tgz
docker compose up -d
```

`10001` là user không phải root mà backend chạy trong container. Khi khởi động, backend áp thêm các migration mới hơn
bản sao lưu (nếu có).

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
docker build --network host -f backend/Dockerfile -t "$(docker compose config --images | grep backend)" .
docker compose up -d --no-build
```

Tên image phải khớp tên compose dùng (`<tên thư mục>-backend`), nên lệnh trên lấy tên từ `docker compose config`.

**`make dev` không in mã thiết lập.** Đã có tài khoản trong database dev, hoặc `RUST_LOG` đang đặt mức cao hơn
`warn`. Không đặt `RUST_LOG` thì backend in log mức `info`.

**Cổng đã có chương trình khác dùng.** Đổi `BACKEND_PORT` hoặc `POSTGRES_PORT` trong `.env`. Đổi `POSTGRES_PORT` thì
`DATABASE_URL` trong `.env` (dùng khi chạy backend ngoài Docker) cũng đổi theo.

**Không thấy mã thiết lập trong log.** Đã có tài khoản thì backend không in mã nữa. Quên mật khẩu thì dùng lệnh
`reset-password` ở mục 5.

**Hết dung lượng đĩa.** Bản build DuckDB nặng. `cargo clean` trong `backend/` xóa thư mục build; `docker image prune`
xóa image cũ, `docker builder prune` xóa cache build của Docker.

**Caddy không khởi động, báo cổng 80 hoặc 443 đang bận.** Có web server khác (nginx, apache) đang dùng cổng đó. Tắt nó,
hoặc cấu hình chính web server đó làm reverse proxy tới `localhost:8000`.
