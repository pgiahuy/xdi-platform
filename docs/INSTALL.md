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
Máy ít RAM hơn thì giới hạn số tiến trình biên dịch bằng `CARGO_BUILD_JOBS` trong `.env`, đặt bằng khoảng số GiB RAM
còn trống, nếu không máy có thể đứng giữa chừng (cách đặt ở mục 6). Đo cùng cách trên máy đó với `CARGO_BUILD_JOBS=4`
(`docker compose build --no-cache` qua một builder buildx riêng không giới hạn nhân): build mất 18 phút, dùng tới
4,2 GiB RAM; đếm tiến trình trong container builder mỗi 2 giây, không lần nào thấy quá 4 tiến trình biên dịch Rust
và C++ chạy cùng lúc.

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
một lần nạp tối đa `MAX_UPLOAD_MB` (mặc định 200). Định dạng, giới hạn, phiên bản và tên cột: [Dữ liệu](DATA.md).

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
| `QUERY_TIMEOUT_SECS` | `30` | Thời gian tối đa một câu SQL của câu hỏi chạy trên DuckDB; quá thì câu bị dừng |
| `RUST_LOG` | `info` | Mức log |
| `CARGO_BUILD_JOBS` | trống (mọi nhân) | Số tiến trình biên dịch khi build image; máy ít RAM thì đặt nhỏ hơn (mục 6) |

Mô hình ngôn ngữ:

- `gemini`: khóa lấy ở Google AI Studio, đặt vào `GEMINI_API_KEY`. Thiếu khóa thì backend vẫn chạy, ghi cảnh báo vào
  log lúc khởi động, và câu hỏi nhận lỗi nhắc đặt khóa. Khóa chỉ gửi tới Google trong header, không ghi vào log.
- `ollama`: cần Ollama đang chạy và mô hình đã tải (`ollama pull qwen2.5-coder:7b`, đổi mô hình bằng `OLLAMA_MODEL`).
  Chạy compose mà để trống `OLLAMA_URL` thì backend gọi `http://host.docker.internal:11434`, tức Ollama trên máy host.
  Trên Linux, Ollama mặc định chỉ nghe `127.0.0.1`, container không gọi tới được: chạy Ollama với
  `OLLAMA_HOST=0.0.0.0`, và chặn cổng 11434 từ bên ngoài bằng tường lửa vì Ollama không có đăng nhập.
- Thời gian chờ tối đa mỗi lần gọi: `GEMINI_TIMEOUT_SECS` (mặc định 60 giây), `OLLAMA_TIMEOUT_SECS` (mặc định 300 giây,
  vì mô hình chạy bằng CPU chậm hơn nhiều).

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
mặc định `./data`, tính từ thư mục `backend`); git bỏ qua thư mục này nên dữ liệu nạp thử không lẫn vào commit.

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

`make install` build bản release rồi chép binary từ `backend/target`, hoặc từ `CARGO_TARGET_DIR` nếu biến này được đặt.

Binary cần một Postgres (`DATABASE_URL`), thư mục dữ liệu ghi được (`DATA_DIR`) và, nếu muốn có giao diện, thư mục
giao diện đã build (`STATIC_DIR`). Gói binary trong mỗi bản phát hành trên GitHub chỉ có backend.

## 3. Đưa lên Internet

Cần:

- Một máy chủ Linux có Docker như mục 1.
- Một tên miền hoặc subdomain riêng (ví dụ `du-lieu.example.com`) đã trỏ về địa chỉ IP của máy chủ (bản ghi A hoặc
  AAAA). Chạy dưới đường dẫn con của một trang khác (`example.com/xdi/`) chưa hỗ trợ: giao diện và API dùng đường dẫn
  tính từ gốc tên miền (`/api`, `/assets`).
- Cổng 80 và 443 mở ra Internet, cho reverse proxy (Caddy hoặc nginx, bên dưới) nhận HTTPS.

### Cấu hình bản chạy thật

1. Clone và tạo `.env` như mục 1, nhưng **trước khi chạy lần đầu**, thêm ba dòng vào `.env` (thay tên miền; mật khẩu
   Postgres sinh ngẫu nhiên):

   ```bash
   cat >> .env <<EOF
   COMPOSE_FILE=docker-compose.yml:docker-compose.prod.yml
   POSTGRES_PASSWORD=$(openssl rand -hex 24)
   PUBLIC_URL=https://du-lieu.example.com
   EOF
   ```

   Dòng `COMPOSE_FILE` làm mọi lệnh `docker compose` (và `make backup`, `make restore`) dùng thêm
   `docker-compose.prod.yml`. File đó chỉ ghi phần khác với bản chạy trên máy:

   - Postgres không mở cổng ra máy host, chỉ backend nối tới được. Docker tự mở cổng của container qua iptables, tường
     lửa như ufw không chặn được, nên cách chắc chắn là không mở.
   - Backend chỉ nghe trên `127.0.0.1:8000`; reverse proxy trên cùng máy nhận HTTPS rồi chuyển vào.
   - Thiếu `POSTGRES_PASSWORD` hoặc `PUBLIC_URL` thì `docker compose` dừng và báo thiếu biến nào, không chạy với mật
     khẩu mẫu.
   - `PUBLIC_URL` bắt đầu bằng `https://` thì cookie đăng nhập có cờ `Secure`: trình duyệt không gửi nó qua HTTP thường.
   - Hai container tự chạy lại khi máy chủ khởi động lại.

   Chọn cách đè thêm một file thay vì một file compose đầy đủ thứ hai, vì người cài chỉ thêm một dòng vào `.env` mà
   không phải nhớ `-f` ở mỗi lệnh, và hai file không lệch nhau khi file gốc đổi. Cần Docker Compose 2.24.4 trở lên
   (`docker compose version`), bản đầu tiên có thẻ `!override` mà file này dùng (theo
   https://docs.docker.com/reference/compose-file/merge/).

   Tự đặt mật khẩu thì chỉ dùng chữ và số (lệnh `openssl` trên cho ra đúng như vậy): mật khẩu nằm trong chuỗi kết nối
   của backend, ký tự như `@`, `:`, `/` làm hỏng chuỗi đó.

2. Chạy và tạo tài khoản quản trị như mục 1 (lệnh `curl` gọi `http://localhost:8000` vẫn chạy trên máy chủ).

   Kiểm Postgres không mở cổng ra máy host, nhất là khi không chắc phiên bản Compose:

   ```bash
   docker compose ps                         # cột PORTS của postgres chỉ có 5432/tcp, không có 0.0.0.0:...->5432
   ss -ltnp | grep -E ':8000|:5433'          # chỉ thấy 127.0.0.1:8000, không thấy 5433
   ```

   Log của backend không có dòng `sample Postgres password` (`docker compose logs backend | grep -i password`).

3. Cài một reverse proxy theo một trong hai mẫu dưới.

**Đã chạy bằng mật khẩu mẫu rồi mới chuyển sang cấu hình này.** `POSTGRES_PASSWORD` chỉ có tác dụng khi Postgres tạo
volume lần đầu, nên phải đổi mật khẩu trong database trước, khi chưa sửa `.env`:

```bash
docker compose exec postgres psql -U xdi -d xdi -c "ALTER USER xdi PASSWORD 'mat-khau-moi'"
```

Rồi thêm ba dòng trên vào `.env` (cùng mật khẩu đó) và chạy `docker compose up -d`. Backend còn dùng mật khẩu mẫu thì
mỗi lần khởi động in cảnh báo `DATABASE_URL uses the sample Postgres password` ra log.

### Caddy

Caddy tự xin và gia hạn chứng chỉ HTTPS, không giới hạn dung lượng file gửi lên hay thời gian chờ backend. Cài theo
https://caddyserver.com/docs/install, rồi thay nội dung `/etc/caddy/Caddyfile` (gói cài sẵn đã tạo file này) bằng:

```
du-lieu.example.com {
    reverse_proxy localhost:8000
    header Strict-Transport-Security "max-age=31536000"
}
```

Rồi `sudo systemctl reload caddy`. Mở https://du-lieu.example.com.

### nginx

Máy chủ đã có nginx thì dùng nó thay Caddy. Chứng chỉ lấy bằng certbot (https://certbot.eff.org), mẫu dưới giả sử
certbot đã ghi chứng chỉ vào `/etc/letsencrypt/live/du-lieu.example.com/`. Ghi vào
`/etc/nginx/conf.d/xdi.conf`, thay tên miền:

```nginx
server {
    listen 443 ssl;
    server_name du-lieu.example.com;
    ssl_certificate     /etc/letsencrypt/live/du-lieu.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/du-lieu.example.com/privkey.pem;

    # MAX_UPLOAD_MB cộng 1 (mặc định 200 + 1). Mặc định nginx chỉ nhận 1m, file lớn hơn bị trả 413.
    client_max_body_size 201m;
    # Backend chỉ trả lời khi đã đọc và chuyển xong file. Một file Excel được đọc tới 360 giây và mỗi lúc chỉ đọc
    # một file, file thứ hai phải chờ file trước. Mặc định nginx chờ 60 giây rồi trả 504.
    proxy_read_timeout 900s;
    # Chuyển file sang backend ngay khi nhận, không ghi tạm ra đĩa của nginx trước.
    proxy_request_buffering off;

    # HSTS: proxy gửi, vì chỉ proxy biết chắc trình duyệt đang vào bằng HTTPS. Backend tự gửi các header bảo mật khác.
    add_header Strict-Transport-Security "max-age=31536000" always;

    location / {
        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name du-lieu.example.com;
    return 301 https://$host$request_uri;
}
```

Rồi `sudo nginx -t && sudo systemctl reload nginx`. Đổi `MAX_UPLOAD_MB` trong `.env` thì đổi `client_max_body_size`
theo.

### Header bảo mật

Backend tự gửi `Content-Security-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff` và
`Referrer-Policy: same-origin` cho cả giao diện và API, chạy qua proxy nào cũng vậy. `Strict-Transport-Security`
(HSTS) để proxy gửi như hai mẫu trên. HSTS làm trình duyệt chỉ vào tên miền này bằng HTTPS trong một năm: chỉ bật khi
tên miền đó sẽ luôn có HTTPS.

## 4. Sao lưu và khôi phục

Bản sao lưu gồm hai phần: database (`db.dump`) và thư mục file Parquet (`data.tgz`). Chạy trong thư mục chứa
`docker-compose.yml`:

```bash
make backup                                     # ghi vào ~/xdi-backups/<ngày-giờ>/
make backup BACKUP_DIR=/mnt/backup/xdi          # chỗ khác
```

Bản sao lưu có dữ liệu và chuỗi băm mật khẩu: lệnh tạo thư mục chỉ chủ sở hữu đọc được, mặc định nằm ngoài thư mục
repo. Chép nó sang một máy hoặc ổ đĩa khác, vì hỏng máy chủ thì mất cả bản sao lưu nằm trên đó.

Sao lưu chạy được khi backend vẫn chạy, nhưng database và file được chụp ở hai thời điểm liền nhau. Ai đó nạp file, xóa
dataset hay xóa workspace đúng lúc đó thì hai phần có thể không khớp, ví dụ database còn một workspace mà file của nó đã
bị xóa. Nên sao lưu lúc ít người dùng; cần chắc chắn thì dừng backend trong lúc sao lưu:

```bash
docker compose stop backend && make backup && docker compose up -d
```

Khôi phục, trên bản cài đang chạy hoặc một bản cài mới (đã clone và tạo `.env` như mục 1 hoặc mục 3, chưa cần chạy):

```bash
make restore FROM=~/xdi-backups/20261022-101500
```

Lệnh này **thay toàn bộ** dữ liệu hiện có bằng bản sao lưu. Trước hết nó đọc hết `db.dump` và `data.tgz`; file nào
thiếu, hỏng hay bị cắt cụt (chép dở, ổ đầy) thì dừng và báo, chưa dừng hay xóa gì. Hai file đọc được thì nó dừng
backend, xóa và tạo lại database từ `db.dump`, xóa và giải nén lại thư mục dữ liệu từ `data.tgz`, rồi chạy lại cả hệ
thống. Tài khoản, mật khẩu đăng nhập, workspace và dataset trở về như lúc sao lưu; mật khẩu Postgres thì giữ của bản cài
hiện tại.

Khôi phục vào bản cài cùng phiên bản với lúc sao lưu, hoặc mới hơn. Bản cài mới hơn thì backend tự áp thêm các migration
còn thiếu khi khởi động. Bản cài cũ hơn thì backend không khởi động được, vì database có migration mà code cũ không
biết: cập nhật bản cài (`git pull`) trước khi khôi phục.

Không dùng Makefile thì chạy tay đúng các lệnh trong target `backup` và `restore` của `Makefile`.

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
song trên mọi nhân CPU, mỗi tiến trình cần khoảng 1 GiB RAM khi biên dịch DuckDB. Đặt số tiến trình bằng khoảng số
GiB RAM còn trống (`free -g`, cột `available`) vào dòng `CARGO_BUILD_JOBS=` của `.env`, ví dụ `CARGO_BUILD_JOBS=4`
(`.env` chép từ bản cũ của `.env.example` chưa có dòng này thì thêm vào), rồi build như thường:

```bash
docker compose up -d --build
```

Biến này giới hạn cả phần C++ của DuckDB. Kết quả đo ở mục 1. Giữ dòng đó trong `.env` thì các lần cập nhật sau
(`git pull && docker compose up -d --build`) cũng theo giới hạn.

Phương án phụ, khi đã đặt `CARGO_BUILD_JOBS` mà máy vẫn đứng: `CARGO_BUILD_JOBS` chỉ giới hạn cargo, còn các bước
khác của build (cài gói npm, build giao diện) chạy cùng lúc vẫn dùng mọi nhân. Builder buildx riêng giới hạn mọi bước
trong `N` nhân (`N` không quá `nproc`), rồi chạy không build lại:

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
