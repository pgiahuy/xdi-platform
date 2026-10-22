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

Máy build image cần RAM trống khoảng 1 GiB cho mỗi nhân CPU, vì cargo biên dịch song song trên mọi nhân, và khoảng
6 GB đĩa trống (cache build 4,8 GB theo `docker buildx du`, image backend 215 MB và image Postgres 640 MB theo
`docker images`). Đo trên máy 16 nhân, 30 GiB RAM, build không dùng cache trong một builder buildx riêng, đọc bộ nhớ
(không tính page cache) của container builder: build lần đầu mất 8 phút, các tiến trình build dùng tới 12,6 GiB RAM.
Máy ít RAM hơn thì build theo cách giới hạn số nhân ở mục 6, nếu không máy có thể đứng giữa chừng.

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up -d --build
```

Lần đầu build lâu vì phải biên dịch backend (Rust, kèm DuckDB) và giao diện. Các lần sau dùng lại cache; đổi phụ thuộc
(`Cargo.lock`) thì biên dịch lại cả phần phụ thuộc.

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
khoảng trắng vẫn nhận), email, mật khẩu từ 8 đến 128 ký tự (nhập hai lần); tên hiển thị và tên workspace
đầu tiên không bắt buộc. Xong thì giao diện vào thẳng workspace vừa tạo.

Máy chủ không có trình duyệt thì tạo bằng lệnh (thay mã, email, mật khẩu, tên workspace):

```bash
curl http://localhost:8000/api/auth/setup \
  -H 'content-type: application/json' \
  -d '{"setup_code":"ABCD-EFGH-JKMN","email":"ban@example.com","password":"mat khau it nhat 8 ky tu","workspace_name":"Cửa hàng"}'
```

Tài khoản này là quản trị hệ thống (quản lý tài khoản của cả bản cài) và là `admin` của workspace đầu tiên. Mã thiết
lập chỉ dùng được một lần. Khởi động lại backend mà vẫn chưa có tài khoản nào thì backend in mã mới.

### Nạp dữ liệu

Trên giao diện: mục **Dữ liệu** ở thanh bên, chọn một hoặc nhiều file rồi bấm **Nạp**. Chỉ `admin` của workspace thấy
phần nạp file. Tên dataset để trống thì lấy theo tên file.

Qua API, khi cần nạp bằng script: đăng nhập bằng lệnh để lấy cookie phiên vào `cookie.txt` (file này là chìa khóa
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

Dữ liệu nằm trong hai volume Docker: `xdi-platform_pgdata` (Postgres) và `xdi-platform_xdidata` (file Parquet). Tiền
tố là tên project của compose: mặc định là tên thư mục chứa `docker-compose.yml`, đổi được bằng `COMPOSE_PROJECT_NAME`
trong `.env`. Image backend cũng mang tên đó, ví dụ `xdi-platform-backend`.

### Dừng, cập nhật, gỡ

```bash
docker compose down                       # dừng, giữ dữ liệu
git pull && docker compose up -d --build  # cập nhật; backend tự áp migration database khi khởi động
docker compose down -v                    # gỡ hẳn, XÓA cả hai volume dữ liệu
```

Gỡ hẳn và xóa luôn image backend: `docker compose down -v --rmi local`. `docker builder prune` xóa cache build (của
mọi project trên máy, không riêng project này).

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

Backend chạy bằng `make dev` không đọc `.env` (chỉ bước bật Postgres bằng `docker compose` đọc); biến cần đổi thì đặt
trên dòng lệnh. Ví dụ đã đổi `POSTGRES_PORT=5434` trong `.env` và muốn backend ở cổng 8001:

```bash
DATABASE_URL=postgres://xdi:xdi@localhost:5434/xdi BACKEND_PORT=8001 make dev
BACKEND_URL=http://localhost:8001 make dev-frontend   # giao diện dev gọi /api sang đúng cổng đó
```

Lần chạy đầu mã thiết lập in ra terminal, tạo tài khoản như mục 1. File dữ liệu nằm ở `backend/data` (`DATA_DIR`
mặc định `./data`, tính từ thư mục `backend`).

Đã chạy bản Docker ở mục 1 trong cùng thư mục thì `make dev` dùng lại Postgres của bản đó: tài khoản đã có nên không
in mã thiết lập, còn file dữ liệu thì không chung (bản Docker giữ trong volume, bản dev ở `backend/data`). Backend
Docker đang giữ cổng 8000 thì dừng nó trước bằng `docker compose stop backend`, hoặc đặt `BACKEND_PORT` khác.

Lần build đầu biên dịch DuckDB từ mã nguồn C++, mất vài phút; thư mục `backend/target` lớn dần qua mỗi lần build, có
thể lên hàng chục GB (`cargo clean` để xóa). Như build Docker, cargo biên dịch song song trên mọi nhân CPU; máy ít RAM
thì giới hạn số tiến trình, ví dụ `CARGO_BUILD_JOBS=4 make dev`. `make help`
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
của tài khoản đó. Đăng nhập bằng mật khẩu tạm rồi đổi ở **Tài khoản và mật khẩu** (menu
tài khoản góc trên bên phải).

```bash
docker compose exec backend xdi-backend reset-password --email ban@example.com
docker compose exec backend xdi-backend list-admins   # các quản trị hệ thống: email, tên, trạng thái
```

Build từ mã nguồn (mục 2) thì chạy cùng lệnh với cùng `DATABASE_URL` mà backend dùng, ví dụ
`cd backend && cargo run -- list-admins`, hoặc `xdi-backend list-admins` nếu đã `make install`.

## 6. Lỗi hay gặp

**Build Docker làm máy đứng, chậm hẳn, hoặc tiến trình biên dịch bị dừng vì hết RAM.** Mặc định cargo biên dịch song
song trên mọi nhân CPU, mỗi nhân cần khoảng 1 GiB RAM khi biên dịch DuckDB. Build bằng một builder chỉ dùng `N` nhân,
`N` bằng khoảng số GiB RAM còn trống và không quá số nhân của máy (`nproc`), rồi chạy không build lại:

```bash
N=4   # số nhân dùng để build
docker buildx create --name xdi-build --driver docker-container --driver-opt cpuset-cpus=0-$((N - 1))
docker buildx build --builder xdi-build --load -f backend/Dockerfile \
  -t "$(docker compose config --images | grep backend)" .
docker compose up -d --no-build
```

Đo trên máy 16 nhân với cách đo ở mục 1: `N=4` thì build dùng tới 4,0 GiB RAM và mất 19 phút. Lần cập nhật sau:
`git pull` rồi chạy lại hai lệnh `docker buildx build` và `docker compose up -d --no-build`, thay cho
`docker compose up -d --build`. Giữ builder `xdi-build` giữa các lần cập nhật, vì cache build nằm trong nó: xóa
builder (`docker buildx rm xdi-build`) thì lần sau phải build lại từ đầu.

**Build Docker lỗi `spurious network error` hoặc `transfer too slow` khi tải crate.** Mạng của Docker trên một số máy
Linux chậm hoặc chập chờn. Build bằng mạng của máy host rồi chạy không build lại:

```bash
docker build --network host -f backend/Dockerfile -t "$(docker compose config --images | grep backend)" .
docker compose up -d --no-build
```

Tên image phải khớp tên compose dùng (`<tên project>-backend`, mục 1), nên lệnh trên lấy tên từ `docker compose config`.

**Cổng đã có chương trình khác dùng.** `docker compose up` báo `Bind for 0.0.0.0:5433 failed: port is already
allocated` (một container khác giữ cổng) hoặc `failed to bind host port ... address already in use` (một chương trình
ngoài Docker giữ cổng); `make dev` báo `cannot bind port, check BACKEND_PORT`.

- Chạy bằng Docker: đổi `BACKEND_PORT` hoặc `POSTGRES_PORT` trong `.env` rồi chạy lại.
- `make dev`: backend không đọc `.env`, nên đặt `BACKEND_PORT` trên dòng lệnh (mục 2). Đã đổi `POSTGRES_PORT` trong
  `.env` thì đặt cả `DATABASE_URL`, không thì backend báo `cannot connect to postgres, check DATABASE_URL`.

Postgres cài sẵn trên máy thường ở cổng 5432 nên không đụng cổng mặc định 5433. Chạy hai bản cài trên cùng máy thì bản
thứ hai đổi cả `BACKEND_PORT` và `POSTGRES_PORT`. Hai thư mục cùng tên (ví dụ cùng là `xdi-platform`) thì đổi cổng chưa
đủ: hai bản có cùng tên project compose, bản thứ hai tạo lại container và dùng chung volume dữ liệu của bản thứ nhất.
Bản thứ hai đặt thêm `COMPOSE_PROJECT_NAME` khác trong `.env` (mục 1).

**Không thấy mã thiết lập trong log.** Backend chỉ in mã khi database chưa có tài khoản nào. Đã tạo tài khoản ở lần
chạy trước (`docker compose down` giữ dữ liệu; `make dev` dùng chung Postgres với bản Docker trong cùng thư mục) thì
không in nữa; quên mật khẩu thì dùng lệnh `reset-password` ở mục 5. Mã in ở mức `warn`, nên đặt `RUST_LOG=error` cũng
làm mất dòng này.

**Hết dung lượng đĩa.** Bản build DuckDB nặng. `cargo clean` trong `backend/` xóa thư mục build; `docker image prune`
xóa image cũ, `docker builder prune` xóa cache build của Docker, `docker buildx rm xdi-build` xóa builder riêng (nếu đã
tạo theo mục trên) cùng cache của nó.

**Caddy không khởi động, báo cổng 80 hoặc 443 đang bận.** Có web server khác (nginx, apache) đang dùng cổng đó. Tắt nó,
hoặc cấu hình chính web server đó làm reverse proxy tới `localhost:8000`.
