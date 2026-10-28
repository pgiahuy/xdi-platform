# Cài đặt

Chạy nhanh trên một máy có Docker:

```bash
git clone https://github.com/pgiahuy/xdi-platform.git && cd xdi-platform
cp .env.example .env
docker compose up -d --build                      # build lần đầu mất vài phút
docker compose logs backend | grep setup_code     # mã để tạo tài khoản quản trị
```

Mở http://localhost:8000, nhập mã thiết lập. Workspace đầu tiên có sẵn dataset mẫu DataCo, hỏi được ngay với mô hình
`mock`. Tiếp theo: mục 7 (Dùng lần đầu).

| Mục | Khi nào cần |
|---|---|
| [1. Docker](#1-chạy-bằng-docker) | Cài, cấu hình `.env`, nạp dữ liệu qua API, cập nhật, gỡ |
| [2. Build từ mã nguồn](#2-build-từ-mã-nguồn) | Phát triển, hoặc chạy backend không dùng Docker |
| [3. Đưa lên Internet](#3-đưa-lên-internet) | Máy chủ có tên miền, HTTPS bằng Caddy hoặc nginx |
| [4. Sao lưu và khôi phục](#4-sao-lưu-và-khôi-phục) | `make backup`, `make restore` |
| [5. Quản trị](#5-quản-trị) | Quên mật khẩu quản trị |
| [6. Lỗi hay gặp](#6-lỗi-hay-gặp) | Build hết RAM, trùng cổng, lỗi Gemini |
| [7. Dùng lần đầu](#7-dùng-lần-đầu) | Chọn mô hình, nạp dataset, hỏi, đọc minh chứng, mời thành viên |

Phần mềm đang phát triển; phần nào đã có xem bảng Trạng thái trong [ARCHITECTURE.md](ARCHITECTURE.md#trạng-thái).

## 1. Chạy bằng Docker

Cần: Docker Engine kèm Docker Compose v2 (`docker compose version` chạy được), Git. Chưa có Docker: xem
https://docs.docker.com/engine/install/. Trên Linux, để chạy `docker` không cần `sudo`, thêm user vào nhóm `docker`
theo https://docs.docker.com/engine/install/linux-postinstall/ (đăng xuất rồi đăng nhập lại). Trên Windows, chạy các
lệnh dưới đây trong WSL2.

Máy build image cần khoảng 1 GiB RAM trống cho mỗi nhân CPU (cargo biên dịch song song trên mọi nhân) và khoảng
6 GB đĩa trống. Máy ít RAM thì đặt `CARGO_BUILD_JOBS` trong `.env` bằng khoảng số GiB RAM còn trống, nếu không máy có
thể đứng giữa chừng (mục 6).

| Build không dùng cache, máy 16 nhân, 30 GiB RAM | Thời gian | RAM tối đa |
|---|---|---|
| Mặc định (mọi nhân) | 8 phút | 12,6 GiB |
| `CARGO_BUILD_JOBS=4` | 18 phút | 4,2 GiB |

Cách đo: build trong một builder buildx riêng, đọc bộ nhớ của container builder (không tính page cache). Đĩa: cache
build 4,8 GB (`docker buildx du`), image backend 215 MB, image Postgres 640 MB (`docker images`).

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

Trên giao diện: mục **Dữ liệu** ở thanh bên, chọn một hoặc nhiều file rồi bấm **Nạp** (từng bước ở mục 7). Chỉ
`admin` của workspace thấy phần nạp file. Tên dataset để trống thì lấy theo tên file.

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
cùng cột thành một bảng; file chỉ có một sheet đang hiện thì không cần chọn):

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
| `BACKEND_BIND` | trống (mọi địa chỉ) | Địa chỉ backend nghe trên máy host khi chạy compose; đặt `127.0.0.1` khi có Caddy trên cùng máy để cổng HTTP thường không mở ra Internet (mục 3) |
| `POSTGRES_PORT` | `5433` | Cổng Postgres mở ra máy host (để công cụ quản trị database kết nối) |
| `DB_MAX_CONNECTIONS` | `10` | Số kết nối Postgres tối đa của backend |
| `DB_ACQUIRE_TIMEOUT_SECS` | `5` | Số giây một request chờ lấy kết nối Postgres khi mọi kết nối đang bận; quá thì request nhận lỗi thay vì treo |
| `LLM_PROVIDER` | `mock` | `mock` không gọi ra ngoài; `gemini` cần `GEMINI_API_KEY`; `ollama` cần máy chạy Ollama ở `OLLAMA_URL` |
| `GEMINI_MODEL` | `gemini-3.5-flash-lite` | Mô hình Gemini khi `LLM_PROVIDER=gemini` |
| `MAX_UPLOAD_MB` | `200` | Tổng dung lượng tối đa một lần nạp file |
| `QUERY_TIMEOUT_SECS` | `30` | Thời gian tối đa một câu SQL của câu hỏi chạy trên DuckDB; quá thì câu bị dừng, trừ phần DuckDB tính trước khi chạy ([Guard SQL](GUARD.md), mục Hạn chế đã biết) |
| `ASK_MAX_ROWS` | `100000` | Trần số dòng guard đặt cho SQL của câu hỏi; chạm trần thì giao diện ghi "ít nhất N dòng". Giao diện chỉ nhận 1000 dòng đầu |
| `RUST_LOG` | `info` | Mức log |
| `ANSWER_TEXT` | `true` | Câu hỏi thường có đoạn trả lời do mô hình viết từ tối đa 10 dòng đầu của kết quả ([PRIVACY.md](PRIVACY.md)); `false` để không gửi dòng kết quả nào cho mô hình, đoạn trả lời là câu khuôn |
| `SEED_SAMPLE` | `true` | Thiết lập lần đầu nạp dataset mẫu DataCo kèm chỉ số mẫu vào workspace đầu tiên (mục 7, bước 1); `false` để workspace trống |
| `CARGO_BUILD_JOBS` | trống (mọi nhân) | Số tiến trình biên dịch khi build image; máy ít RAM thì đặt nhỏ hơn (mục 6) |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | trống | Để dành cho đăng nhập bằng Google, backend hiện chưa đọc; để trống |

Mô hình ngôn ngữ:

- `gemini`: khóa lấy ở Google AI Studio, đặt vào `GEMINI_API_KEY`. Thiếu khóa thì backend vẫn chạy, ghi cảnh báo vào
  log lúc khởi động, và câu hỏi nhận lỗi nhắc đặt khóa. Khóa chỉ gửi tới Google trong header, không ghi vào log.
  Mô hình mặc định là `gemini-3.5-flash-lite` vì hạn mức miễn phí theo ngày của bản lite cao hơn nhiều so với bản
  flash; đổi mô hình bằng `GEMINI_MODEL`. Hạn mức hiện tại của từng mô hình xem trong Google AI Studio. Lỗi hay gặp
  với Gemini (khóa bị từ chối, không có mô hình, quá tải, hết hạn mức) và cách xử lý ở mục 6.
- `ollama`: cần Ollama đang chạy và mô hình đã tải (`ollama pull qwen2.5-coder:7b`, đổi mô hình bằng `OLLAMA_MODEL`).
  Chạy compose mà để trống `OLLAMA_URL` thì backend gọi `http://host.docker.internal:11434`, tức Ollama trên máy host.
  Trên Linux, Ollama mặc định chỉ nghe `127.0.0.1`, container không gọi tới được: chạy Ollama với
  `OLLAMA_HOST=0.0.0.0`, và chặn cổng 11434 từ bên ngoài bằng tường lửa vì Ollama không có đăng nhập.
- Cửa sổ ngữ cảnh của Ollama: `OLLAMA_NUM_CTX` (mặc định 12288 token), gửi trong `options.num_ctx` của mỗi request.
  Không gửi thì Ollama dùng mặc định của nó (4096 token trên máy dưới 24 GiB VRAM, theo [tài liệu
  Ollama](https://docs.ollama.com/context-length)) và lặng lẽ bỏ phần đầu của prompt dài hơn, tức phần luật. Đếm bằng
  tokenizer của Qwen2.5-Coder trên DataCo: prompt SQL khoảng 3 050 token, prompt dài nhất backend gửi (trần 24 000 ký
  tự) khoảng 7 180 token, prompt ở trần mà toàn danh sách giá trị khoảng 8 000 token. Prompt SQL nay có thêm mục luật
  của guard (khoảng 3 000 ký tự), chưa đếm lại. 12288 đủ cho prompt đó cộng phần sửa SQL và câu trả lời. Mỗi token cần
  thêm bộ nhớ K/V: với `qwen2.5-coder:7b` và bộ đệm `f16` là 56 KiB, tức khoảng 672 MiB ở 12288 token (tính từ
  `config.json` của mô hình, chưa đo; số thật xem bằng `ollama ps`). Đặt nhỏ hơn mặc định thì backend ghi cảnh báo lúc
  khởi động.
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

Mô hình ngôn ngữ cũng đặt trên dòng lệnh như vậy, ví dụ `LLM_PROVIDER=gemini GEMINI_API_KEY=<khóa> make dev`.

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
giao diện đã build (`STATIC_DIR`). Gói binary trong mỗi bản phát hành trên GitHub chỉ có backend. Muốn có dataset mẫu
lúc thiết lập lần đầu thì đặt `SAMPLES_DIR` trỏ tới thư mục `data/samples` của repo; không thấy file mẫu thì backend bỏ
qua bước này.

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
tài khoản: tên người dùng ở chân thanh bên, hoặc góc phải thanh trên khi màn hẹp).

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

**Hỏi thì nhận `Chế độ mock chỉ trả lời các câu hỏi mẫu của dataset mẫu`, hoặc trang hỏi ghi "Mô hình hiện tại không lập
được câu gợi ý cho dataset này".** Backend đang chạy với `LLM_PROVIDER=mock` (mặc định), mà mock chỉ có câu trả lời
viết sẵn cho dataset DataCo (mục 7, bước 1). Đổi sang `gemini` hoặc `ollama` rồi khởi động lại backend; lúc khởi động,
backend lập hồ sơ cho các dataset còn thiếu, không phải nạp lại file.

**Lỗi khi dùng Gemini.** Lời báo hiện dưới ô câu hỏi trên trang **Hỏi dữ liệu**. Log của backend có thêm dòng
`gemini returned an error` kèm mã HTTP (`status`), mã của Google (`gemini_status`) và lời gốc của Google. Lỗi lúc lập
hồ sơ dataset sau khi nạp thì không hiện trên giao diện, chỉ có trong log; sửa xong và khởi động lại backend thì backend
lập lại hồ sơ. Đổi `.env` rồi chạy `docker compose up -d` là đủ để backend nhận giá trị mới.

- `Thiếu khóa Gemini: đặt GEMINI_API_KEY trong .env rồi khởi động lại backend`: chưa đặt khóa. Đặt khóa rồi khởi động
  lại.
- `Gemini từ chối khóa API (mã 403): kiểm tra GEMINI_API_KEY` (có khi là mã 400 hoặc 401): khóa sai, chép thiếu ký tự,
  hoặc không đúng loại (không phải khóa Gemini API tạo trong Google AI Studio). Tạo khóa trong Google AI Studio rồi
  đặt lại `GEMINI_API_KEY`.
- `Gemini không có mô hình gemini-2.5-flash: kiểm tra GEMINI_MODEL` (mã 404): khóa này không gọi được mô hình đó; đã
  gặp với khóa mới tạo và mô hình cũ như `gemini-2.5-flash`. Bỏ dòng `GEMINI_MODEL` để dùng mặc định
  `gemini-3.5-flash-lite`, hoặc chọn một mô hình có trong danh sách của Google AI Studio.
- `Gemini đang lỗi hoặc quá tải (mã 503). Thử lại sau`: phía Google đang quá tải. Chờ một lúc rồi hỏi lại; lặp lại
  nhiều lần thì thử một mô hình khác bằng `GEMINI_MODEL`.
- `Đã hết hạn mức gọi Gemini của khóa này. Chờ một lúc rồi thử lại, hoặc xem hạn mức trong Google AI Studio` (mã 429):
  đã dùng hết số lần gọi được phép của khóa. Hạn mức theo phút thì chờ một phút; hạn mức theo ngày thì chờ sang ngày,
  hoặc xem hạn mức và gói trả phí của khóa trong Google AI Studio.
- `Không kết nối được tới Gemini` hoặc `Gemini không trả lời trong 60 giây`: máy chủ không ra được Internet (tường lửa,
  proxy) hoặc mô hình trả lời chậm; lời báo ghi cách xử lý, số giây theo `GEMINI_TIMEOUT_SECS`.

**Hết dung lượng đĩa.** Bản build DuckDB nặng. `cargo clean` trong `backend/` xóa thư mục build; `docker image prune`
xóa image cũ, `docker builder prune` xóa cache build của Docker, `docker buildx rm xdi-build` xóa builder riêng (nếu đã
tạo theo mục trên) cùng cache của nó.

**Caddy không khởi động, báo cổng 80 hoặc 443 đang bận.** Có web server khác (nginx, apache) đang dùng cổng đó. Tắt nó,
hoặc cấu hình chính web server đó làm reverse proxy tới `localhost:8000`.

## 7. Dùng lần đầu

Từ màn thiết lập tới câu trả lời đầu tiên, theo giao diện hiện tại. Địa chỉ là http://localhost:8000 (cổng theo
`BACKEND_PORT`); chạy giao diện bằng `make dev-frontend` thì là http://localhost:5173.

### Bước 1. Chọn mô hình ngôn ngữ

Chọn trước khi nạp dữ liệu. Sau mỗi lần nạp, backend gọi mô hình đang cấu hình để lập hồ sơ dataset (mô tả cột, bí
danh, câu gợi ý), nên mô hình lúc nạp quyết định dataset có câu gợi ý ngay sau khi nạp hay không; đổi mô hình rồi khởi động lại
backend thì backend lập lại (bước 4).

| `LLM_PROVIDER` | Cần | Hỏi được |
|---|---|---|
| `mock` (mặc định) | Không cần gì, không gọi ra ngoài | Chỉ các câu hỏi mẫu viết sẵn cho dataset DataCo (bên dưới); câu khác nhận lời báo `Chế độ mock chỉ trả lời các câu hỏi mẫu của dataset mẫu`; trên dataset khác, kể cả câu mẫu cũng lỗi (SQL viết cho bảng `dataco`) kèm lời báo đó |
| `gemini` | `GEMINI_API_KEY`, khóa tạo trong Google AI Studio | Câu tự do trên dataset bất kỳ |
| `ollama` | Ollama đang chạy và mô hình đã tải (mục 1, Cấu hình) | Câu tự do trên dataset bất kỳ |

Chạy bằng Docker: sửa `.env`, ví dụ `LLM_PROVIDER=gemini` và `GEMINI_API_KEY=<khóa>`, rồi `docker compose up -d`. Build
từ nguồn: đặt trên dòng lệnh, ví dụ `LLM_PROVIDER=gemini GEMINI_API_KEY=<khóa> make dev` (mục 2). Mô hình đang dùng ghi ở
dòng **Mô hình**, góc dưới thanh bên và dưới màn thiết lập.

Thử với `mock`: không cần tải gì. Khi bước 2 tạo xong workspace đầu tiên, backend nạp sẵn dataset `DataCo` (bảng
`dataco`, 1500 dòng lấy từ DataCo Smart Supply Chain, giấy phép CC BY 4.0) kèm vài chỉ số mẫu như "Doanh thu", "Lợi
nhuận", "Tỷ lệ giao trễ"; nguồn, chỉ số và cách tắt (`SEED_SAMPLE=false`) ở
[data/README.md](https://github.com/pgiahuy/xdi-platform/blob/main/data/README.md). Danh sách câu mẫu nằm trong
`backend/src/engine/llm/mock_answers.json`, ví dụ "Doanh thu theo thị trường", "Top 10 danh mục có doanh thu cao
nhất", "Vì sao doanh thu tháng 5 năm 2017 tăng".

Bản cài đã thiết lập trước khi có dataset mẫu, hay đặt `SEED_SAMPLE=false`, thì nạp tay ở bước 3: file
`data/samples/dataco.csv` trong repo, hoặc bản đầy đủ tải bằng `sh data/download.sh dataco`. Đặt tên dataset là
`DataCo` để tên bảng là `dataco`, đúng bảng mà các câu mẫu viết cho; để trống tên thì bảng mang tên file và câu mẫu chạy
lỗi. Workspace đã có bảng `dataco` thì dataset thứ hai thành `dataco_2`, nên nạp DataCo vào workspace chưa có nó.

### Bước 2. Tạo tài khoản quản trị

Lần đầu mở giao diện, màn **Thiết lập lần đầu** hiện ra. Lấy mã thiết lập trong log: chạy bằng Docker thì
`docker compose logs backend | grep setup_code`, build từ nguồn thì tìm dòng có `setup_code=` trong terminal đang chạy
`make dev`. Điền **Mã thiết lập**, **Email quản trị**, **Mật khẩu** và **Nhập lại mật khẩu** (8 đến 128 ký tự); **Tên
hiển thị** và **Tên workspace đầu tiên** không bắt buộc (để trống thì workspace tên là "Workspace đầu tiên"). Bấm **Tạo
tài khoản quản trị**: giao diện đăng nhập luôn và mở trang **Hỏi dữ liệu** của workspace vừa tạo, đã có dataset mẫu
`DataCo` (bước 1) trừ khi `SEED_SAMPLE=false`. Chi tiết về mã thiết lập ở mục 1, Tạo tài khoản quản trị.

Workspace thứ hai trở đi: mở ô chọn workspace ở đầu thanh bên, chọn **Tạo workspace** ở cuối danh sách, nhập tên.
Người tạo thành `admin` của workspace mới và giao diện mở trang đầu của nó. Tài khoản nào cũng tạo được workspace.

### Bước 3. Nạp dataset

Mục **Dữ liệu** ở thanh bên, khung **Nạp dữ liệu** (chỉ `admin` của workspace thấy):

1. **File**: chọn một hoặc nhiều file CSV hoặc Parquet cùng cột (gộp thành một bảng), hoặc một file Excel `.xlsx`. CSV
   đọc được UTF-8, windows-1252 và latin-1. Giao diện không cho chọn file `.xls`: mở bằng Excel rồi lưu lại dạng `.xlsx`.
2. File Excel: giao diện đọc danh sách sheet rồi hiện **Chọn sheet cần nạp**, mỗi sheet kèm số cột và số dòng ước lượng.
   Các sheet chọn mà khác cột nhau thì thành nhiều dataset riêng, giao diện báo trước số dataset.
3. **Tên dataset (không bắt buộc)**: để trống thì lấy tên file. Tên bảng dùng trong SQL đặt theo tên dataset: chữ
   thường, bỏ dấu, khoảng trắng thành `_`, ví dụ "Bán hàng 2024" thành `ban_hang_2024`.
4. Bấm **Nạp**. Xong thì hiện `Đã nạp vào <tên>, phiên bản 1, <số> dòng.` và số cột (bấm để xem danh sách cột).

Tổng dung lượng một lần nạp tối đa `MAX_UPLOAD_MB` (mặc định 200 MB); file Excel còn vài giới hạn riêng. Định dạng, giới
hạn, phiên bản và tên cột: [Dữ liệu](DATA.md). Nạp lại đúng file đã có thì nhận `Đã có: dữ liệu này giống hệt phiên bản
...`, không tạo bản mới. Dữ liệu tháng sau của cùng bảng thì bấm **Nạp phiên bản mới** ở dòng của dataset đó: phiên bản
mới thành dữ liệu hiện hành, phiên bản cũ vẫn giữ.

Bỏ một dataset: bấm **Xóa** ở dòng của nó rồi xác nhận **Xóa dataset** (chỉ `admin`). Mọi phiên bản, hồ sơ cột, chỉ
số, cuộc trò chuyện và file Parquet của dataset bị xóa hẳn, không khôi phục được. Lịch sử câu hỏi (câu hỏi, SQL, bảng
phân rã của câu "tại sao") vẫn ở lại trong database, chỉ mất khi xóa cả workspace ([PRIVACY.md](PRIVACY.md)).

### Bước 4. Chờ hồ sơ dữ liệu

Phần cấu trúc của hồ sơ (kiểu và vai trò từng cột) xong ngay trong lần nạp. Phần do mô hình viết (mô tả cột, bí danh,
câu gợi ý) chạy nền sau đó, mỗi lúc một dataset. Trong lúc chờ, trang **Hỏi dữ liệu** ghi dưới ô **Dataset**:
"Đang lập hồ sơ dataset: câu gợi ý và mô tả cột sẽ có sau ít phút." Trang tự kiểm lại trong vài phút đầu; xong thì
hiện thẻ **Câu gợi ý**, chờ lâu hơn thì tải lại trang. Hỏi được ngay cả khi hồ sơ chưa xong.

- Với `mock`, chỉ dataset `dataco` có hồ sơ viết sẵn. Dataset khác không có câu gợi ý; sau lượt thử, dòng "Đang lập hồ
  sơ" đổi thành "Mô hình hiện tại không lập được câu gợi ý cho dataset này; vẫn hỏi được bình thường." Dòng này cũng hiện
  khi mô hình thật trả lời mà không dùng được (ví dụ JSON hỏng, hay chặn prompt vì nội dung). Đổi `LLM_PROVIDER` (hoặc
  model) rồi khởi động lại backend thì trang lại ghi "Đang lập hồ sơ" trong lúc backend thử với mô hình mới (mục 6).
- Mô hình lỗi lúc lập hồ sơ (khóa sai, quá tải, hết hạn mức) thì giao diện không báo, chỉ log ghi lỗi; backend thử lại
  ở lần khởi động sau.
- "Lập hồ sơ dataset lỗi nên chưa hỏi được phiên bản này" nghĩa là phần cấu trúc lỗi: nạp lại file ở trang **Dữ liệu**.

### Chỉ số và cách gộp (không bắt buộc)

Trang **Dữ liệu** có nút **Chỉ số** ở mỗi dataset. Màn này liệt kê chỉ số của dataset (tên, mô tả, công thức, đơn vị)
và cách gộp của từng cột số đo; mọi thành viên xem được, chỉ `admin` sửa được.

- **Thêm chỉ số**: đặt **Tên** (tên người dùng gõ khi hỏi, ví dụ "Doanh thu"), **Mô tả**, rồi chọn **Phép tính**
  (tổng, trung bình, đếm dòng, đếm giá trị khác nhau) và **Cột**, hoặc bấm **Viết bằng SQL** để gõ công thức như
  `SUM("Order Item Total") / COUNT(DISTINCT "Order Id")`. **Điều kiện**, **Cột thời gian**, **Đơn vị** không bắt buộc.
  Backend chạy thử công thức trên phiên bản hiện hành trước khi lưu; sai cột, không phải hàm gộp hay bị guard từ chối
  thì lời báo hiện ngay dưới ô sai. Câu hỏi kế tiếp nhắc tên chỉ số thì mô hình dùng đúng công thức này (xem ở tab SQL).
- **Sửa** (biểu tượng bút) lưu một phiên bản mới cùng tên, bản cũ giữ lại; không đổi tên được. **Xóa** (biểu tượng
  thùng rác) xóa chỉ số cùng mọi phiên bản sau khi hỏi lại.
- **Cách gộp của cột số đo**: **Tổng** cho tiền, số lượng hàng; **Trung bình** cho giá trị mô tả một dòng (tuổi, điểm,
  số tín chỉ, số ngày). Mô hình đề xuất lúc lập hồ sơ; đổi ở đây có hiệu lực từ câu hỏi kế tiếp, cho phiên bản hiện
  hành. Câu hỏi trên cột trung bình dùng AVG, câu "vì sao" so trung bình mỗi dòng.

### Bước 5. Hỏi

Trang **Hỏi dữ liệu** là một khung trò chuyện: ô **Dataset** ở đầu khung (dính khi cuộn) chọn sẵn dataset tạo gần nhất,
đổi được (đổi dataset là bắt đầu cuộc mới); ô hỏi dính đáy màn. Gõ câu hỏi rồi bấm **Hỏi** (Enter để hỏi, Shift+Enter để
xuống dòng), hoặc bấm một câu gợi ý khi cuộc còn trống để hỏi ngay. Câu gợi ý đặt câu khó lên trước, gồm hai câu "vì
sao". Với `mock` trên DataCo, gõ "Doanh thu theo thị trường", rồi hỏi tiếp "5 danh mục có doanh thu cao nhất ở châu Âu"
trong cùng cuộc.

Mỗi lượt là câu hỏi (căn phải) và câu trả lời (căn trái). Cột trái (màn hẹp: nút **Các cuộc trò chuyện**) liệt kê các cuộc
đã hỏi trên dataset, mới trước, kèm nút **Cuộc trò chuyện mới**; bấm một cuộc để mở lại và hỏi tiếp vào cuộc đó. Nút
ở đầu cột ẩn cột này cho khung trò chuyện rộng ra, nút **Hiện danh sách cuộc trò chuyện** hiện lại. Nút ở góc trên
phải thanh bên thu thanh bên thành dải icon. Lượt cũ
chỉ hiện câu hỏi và tóm tắt phần đã hiểu, vì bảng kết quả không được lưu; bấm **Hỏi lại** để chạy lại câu đó. Đường dẫn
của trang ghi cuộc đang mở, nên tải lại trang vẫn mở đúng cuộc.

Câu có cụm khớp nhiều cột thì backend hỏi lại trước khi viết SQL: chọn cách hiểu rồi bấm **Hỏi lại**. Lỗi (mô hình lỗi,
SQL bị từ chối hay chạy lỗi) hiện ngay trên ô hỏi; lỗi của Gemini và cách xử lý ở mục 6.

### Bước 6. Đọc kết quả và minh chứng

Câu trả lời nằm trong thẻ **Trả lời** ngay dưới câu đã hỏi. Đầu thẻ là lời trả lời ngắn (câu "tại sao" thì hàng ba số và
lời giải thích, xem dưới). Tiếp theo là biểu đồ khi kết quả hợp: biểu đồ đường khi cột nhóm duy nhất là thời gian, biểu
đồ cột khi cột nhóm duy nhất là một chiều và có đúng một độ đo ("Doanh thu theo thị trường" trên DataCo ra biểu đồ cột);
các dạng khác chỉ có bảng. Bảng **Kết quả** ghi số dòng (`5 dòng.`; kết quả dài thì `Hiện 1.000 trên <tổng> dòng.`),
phân trang, số hiện theo kiểu Việt Nam (dấu chấm ngăn nghìn, dấu phẩy thập phân). **Tải CSV** tải các dòng giao diện đã
nhận, tối đa 1000 dòng, giữ giá trị gốc. Cuối thẻ là khung **Đã hiểu là**: cách hệ thống hiểu câu hỏi, đọc từ câu SQL đã
chạy, gồm các dòng **Đo**, **Lọc**, **Nhóm theo**, **Sắp xếp**, **Giới hạn**, **Khoảng thời gian** (dòng nào không có
thì không hiện); câu có giả định thì kèm **Giả định, bấm để đổi rồi hỏi lại**. Bấm tiêu đề **Kết quả** hay **Đã hiểu
là** để thu gọn hoặc mở phần đó; khung **Đã hiểu là** mở sẵn ở lượt mới nhất, thu gọn ở các lượt trước.

Pane **Minh chứng** nằm bên phải trên màn rộng, dưới khung trò chuyện trên màn hẹp, và hiện minh chứng của lượt đang
chọn (mặc định lượt mới nhất); nút **Minh chứng** ở cuối mỗi câu trả lời chọn lượt đó. Nút **Thu gọn** ẩn pane, chỉ
còn một nút nhỏ dính mép phải; bấm nút đó (**Mở minh chứng**) để hiện lại. Bốn tab:

- **SQL**: **Mô hình viết** là câu mô hình viết (ghi "bản đã sửa" khi câu đầu lỗi và mô hình sửa lại); **Sau guard
  (bản đã chạy)** là câu đã chạy trên dữ liệu, kèm những gì guard thêm, ví dụ `Thêm LIMIT 100000`. Mỗi bản có nút chép
  nguyên văn. Guard là gì: [GUARD.md](GUARD.md).
- **Phân rã**: chỉ có với câu hỏi "tại sao" (xem dưới), mở sẵn khi câu trả lời có phân rã: mỗi chiều một bảng giá trị
  hai kỳ (hay hai nhóm) và phần chênh lệch của từng nhóm kèm câu SQL đã chạy, và bảng **Lượng và giá** khi tách được.
  Chiều nhiều nhóm chỉ hiện 8 nhóm đóng góp lớn nhất và một dòng **Còn lại**, bấm **Xem tất cả** để mở hết. Câu thường
  thì tab này ghi `Câu này không phải câu hỏi "tại sao", nên không có phân rã.`
- **Dòng nguồn**: các dòng dữ liệu đứng sau một con số, mở bằng cách bấm một ô số đo của bảng **Kết quả**, hay số của
  một kỳ hay một nhóm trong bảng phân rã; kèm câu SQL đã chạy và cột `_source_file` ghi file gốc của từng dòng.
- **Các bước**: từng bước kèm thời gian, `đạt` hay `lỗi` và chi tiết: Đọc hồ sơ dữ liệu, Mô hình viết SQL, Kiểm SQL
  (guard), Chạy SQL, Mô hình sửa SQL (nếu có), Tính phân rã và Mô hình viết lời giải thích (câu "tại sao"), Lưu vào
  lịch sử. Bước có gọi mô hình ghi `có mô hình`, để thấy mô hình chỉ viết SQL, điền form và viết lời, còn con số do
  DuckDB và backend tính.

Câu hỏi "tại sao" (backend nhận ra bằng các cụm "tại sao", "vì sao", "sao lại", "vì đâu", "lý do", "nguyên nhân",
"do đâu", gõ không dấu cũng được): mô hình không viết SQL mà điền một form (đo
gì, so hai kỳ tháng, quý, năm hay hai nhóm), backend kiểm form rồi tự dựng mọi câu SQL. Đầu thẻ trả lời là một hàng
ba số (hai kỳ hay hai nhóm và chênh lệch), khung **Giải thích**, rồi thanh chênh lệch của từng nhóm theo chiều giải
thích tốt nhất. Lời giải thích nói so gì với gì, chênh bao nhiêu, rồi nhóm nào tạo ra phần lớn chênh lệch. So hai kỳ
thì nói "tăng", "giảm"; so hai nhóm thì nói "cao hơn", "thấp hơn" và luôn là câu do backend viết. Lời của mô hình
được kiểm: số nào không có trong bảng phân rã, hay chữ "tăng", "giảm" ngược dấu của số đứng sau, thì backend dùng câu
của mình và ghi "Câu viết theo khuôn từ số đã tính". Không phân rã được (ví dụ kỳ so sánh không có dòng nào) thì vẫn
có bảng tổng kèm lời báo lý do. Hỏi "vì sao giảm" mà số thật ra tăng (hay không đổi) thì kết quả có thêm một cảnh báo
nói chênh lệch thực tế. Với `mock` trên DataCo, thử "Vì sao doanh thu tháng 5 năm 2017 tăng", và "Tại sao doanh thu
quý 3 năm 2017 giảm" để thấy cảnh báo đó.

Câu "tại sao" không nêu kỳ ("Tại sao doanh thu giảm") vẫn có phân rã: backend chọn kỳ gần nhất có dữ liệu (theo tháng
nếu câu không nói tháng, quý hay năm) và so với kỳ liền trước. Kỳ cuối có chưa tới một nửa số dòng của kỳ trước nó
(dữ liệu dừng giữa kỳ) thì lùi một kỳ. Thẻ **Đã hiểu là** ghi giả định này, ví dụ "Kỳ gần nhất có dữ liệu: Quý 4/2017
so với Quý 3/2017", kèm nút cho các kỳ gần nhất có dữ liệu; bấm một nút để hỏi lại với kỳ đó.

Câu hỏi cần thông tin dataset không có (ví dụ "Tại sao khách hàng không hài lòng?" khi không có cột đánh giá, hay hỏi
dự báo) thì không có bảng số: kết quả báo dữ liệu không có thông tin này, nói lý do và gợi ý các cột gần nhất để hỏi.
Với `mock`, thử đúng câu "Tại sao khách hàng không hài lòng?".

### Bước 7. Mời thành viên

Chưa có thư mời qua email hay tự đăng ký. Thành viên cần có tài khoản trước, rồi được thêm vào workspace:

1. Quản trị hệ thống mở **Quản lý tài khoản** ở thanh bên, khung **Tạo tài khoản**: **Email**, **Tên hiển thị**,
   **Mật khẩu ban đầu** (8 đến 128 ký tự), bấm **Tạo tài khoản**. Chuyển mật khẩu cho người đó; họ đổi lại ở **Tài khoản
   và mật khẩu** trong menu tài khoản (tên người dùng ở chân thanh bên, hoặc góc phải thanh trên khi màn hẹp).
2. `admin` của workspace mở **Thành viên**, khung **Thêm thành viên**: **Email** của tài khoản vừa tạo, **Vai** là
   **Admin** hoặc **Thành viên**, bấm **Thêm**.

**Thành viên** chọn dataset, hỏi và xem chỉ số, không nạp được file. **Admin** thêm nạp và xóa dataset, sửa chỉ số
và cách gộp, thêm, xóa, đổi vai thành viên, đổi tên và xóa workspace. Người được thêm thấy workspace trong ô chọn workspace sau khi đăng nhập.
