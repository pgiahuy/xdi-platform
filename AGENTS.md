# Quy tắc dự án: dành cho trợ lý AI và cả đội

xdi-platform: phân tích dữ liệu bằng ngôn ngữ tự nhiên, có bằng chứng cho từng con số. Backend Rust (Axum, DuckDB
nhúng, Postgres), frontend React với Vite. Giấy phép Apache 2.0. Trả lời người dùng bằng tiếng Việt.

Kiến trúc và lý do ở `docs/ARCHITECTURE.md`. Cách đóng góp ở `CONTRIBUTING.md`. File này là nguồn duy nhất cho quy
tắc làm việc; tài liệu khác trỏ về đây, không chép lại.

## 1. Ngôn ngữ

- Văn xuôi (README, docs, nội dung PR, issue, chú thích trong code): tiếng Việt có dấu.
- Mọi thứ là mã: tiếng Anh. Tên biến, hàm, module, trường JSON, mã lỗi, cờ CLI, biến môi trường, thông điệp log,
  thông điệp `panic!`, `expect`, `assert`, và dữ liệu trong test.
- Ngoại lệ: trường `message` trong lỗi trả cho người dùng qua API (`AppError`) và chữ hiện trên giao diện là tiếng
  Việt, vì người đọc là người dùng cuối.
  Ranh giới: cái gì máy đọc hoặc người dùng API đọc thì tiếng Anh; cái gì chỉ người trong đội đọc thì tiếng Việt.
- Tiêu đề commit và tiêu đề PR: tiếng Anh, theo Conventional Commits.
- Mặt ngoài trên GitHub (description, Discussions, label): tiếng Anh.

## 2. Cấu trúc và phân công

```
backend/     một crate Rust: api/ -> service/ -> engine/ và auth/. Phụ thuộc một chiều, không gọi ngược.
frontend/    React với Vite, build ra file tĩnh, backend phục vụ. Kiểu API sinh từ openapi.json, không viết tay.
docs/        tài liệu, cũng là nguồn GitHub Pages.
data/        README nguồn và giấy phép, download.sh, samples/ dưới 1 MB (chưa có file), domains/ gói khái niệm ngành.
scripts/     hook pre-push, kiểm số phiên bản, ghi chú phát hành, danh sách giấy phép.
```

| Phần | Thư mục |
|---|---|
| Engine phân tích, guard SQL, giải thích | `backend/src/engine/`, `backend/src/service/explain/` |
| Service, auth, Postgres, CI, compose | `backend/src/service/`, `backend/src/auth/`, `backend/migrations/`, `.github/`, `Makefile`, `scripts/`, `docker-compose*.yml`, `backend/Dockerfile` |
| Frontend, docs | `frontend/`, `docs/` |
| Dữ liệu mẫu, gói ngành | `data/` |

- Một PR chỉ đụng một phần: lõi, giao diện hoặc dữ liệu, như mục 4. Ai làm việc gì: xem người được gán issue.
- Hợp đồng chung: `openapi.json` (API backend và frontend), schema Postgres trong `backend/migrations/`, kiểu dữ liệu
  trong `backend/src/api/`, kiểu request và response của hỏi dữ liệu và truy dòng nguồn trong
  `backend/src/service/ask/contract.rs` và `backend/src/service/drill/contract.rs`. Đụng hợp đồng chung thì báo cả đội
  trong PR.

## 3. Code

**Chú thích**

- Mỗi file mở đầu bằng một khối chú thích: tên file, vai trò, cách nó nối với phần khác.
- Chú thích giải thích vì sao, không kể lại mã làm gì. Đặt ngay trên dòng cần giải thích, nêu hệ quả nếu làm khác.
- Mỗi file mã nguồn chương trình (`.rs`, `.ts`, `.tsx`) có dòng `// SPDX-License-Identifier: Apache-2.0` ở đầu. File
  lấy từ dự án khác ghi giấy phép gốc của file đó (xem mục shadcn bên dưới). File cấu hình và script không bắt buộc.

**Đặt tên**

- Tên theo chức năng, không theo khách hàng hay tên riêng.
- Rust: `build_*` trả giá trị mới, `apply_*` sửa tại chỗ, `*_el` không dùng. Struct request và response kết thúc bằng
  `Request`, `Response`.

**Phong cách**

- Hàm ngắn, mỗi hàm một việc. Ưu tiên cách đơn giản, dễ giải thích hơn cách thông minh.
- Không biến toàn cục; state đi qua `AppState`. Mọi lời gọi ra ngoài (HTTP, LLM, file, DB) phải bắt lỗi và báo
  người dùng cách sửa.
- Rust: `cargo fmt`, `cargo clippy` không cảnh báo trước khi commit. Crate phổ biến, module thẳng, không trait generic
  phức tạp, không macro tự viết. Trait duy nhất là `LlmProvider`.
- Frontend: linter của dự án (`npm run lint`); không `any`.
- Frontend chia ba lớp, để giao diện dễ đọc và để sau này chuyển sang framework khác (ví dụ Next.js) mà không viết lại
  component:
  - `src/api/`: nơi duy nhất gọi backend, dạng hook TanStack Query.
  - `src/components/`: chỉ nhận dữ liệu qua props; không gọi API, không dùng router.
  - `src/pages/`: nối hook với component; chỉ lớp này dùng router.
- File trong `frontend/src/components/ui/` và `frontend/src/lib/utils.ts` sinh từ shadcn/ui (giấy phép MIT): giữ nguyên
  phần mã, chỉ thêm hai dòng đầu file: `// SPDX-License-Identifier: MIT` và dòng bản quyền kèm nguồn (chép từ file có
  sẵn). Sinh thêm component bằng `npx shadcn add` thì thêm hai dòng này ngay.
- Không ghi cứng chữ, màu, đường dẫn, tên cột trong code: lấy từ dữ liệu hoặc cấu hình.
- Tách file khi một phần vượt khoảng 200 dòng hoặc có logic riêng đáng kể.

**Icon và emoji**

- Trong mã, chú thích, commit, PR, issue, tài liệu và câu trả lời: không emoji, không ký hiệu trang trí (dấu tích, tên
  lửa, mũi tên, dấu chấm giữa, gạch dài). Ngăn ý bằng dấu phẩy hoặc " - ".
- Trong giao diện: icon chỉ ở chỗ giúp đọc nhanh hơn chữ (nút chức năng, trạng thái). Một bộ icon duy nhất (Lucide).
  Không emoji làm icon. Không icon trước mỗi tiêu đề hay mỗi thẻ. Icon đứng một mình phải có `aria-label`.
- Chữ hiện ra cho người dùng không dùng `·` hay `—` làm dấu ngăn.

**Số liệu**

- Không ghi số liệu hiệu năng chưa đo vào tài liệu, README hay giao diện. Đo thật rồi ghi kèm cách đo.

## 4. Git, nhánh, PR

- Không trailer AI trong commit và PR: không `Co-Authored-By: Claude`, không "Generated with". Khi Squash and merge,
  người merge xóa dòng `Co-authored-by` của AI nếu GitHub tự chép vào; dòng của thành viên thì giữ.
- Nhánh `<type>/<mô-tả-ngắn>` hoặc `<type>/<số-issue>-<mô-tả>`. `git pull --rebase` trước khi push. Không
  `push --force` lên `main`.
- Mọi PR merge bằng Squash and merge. Tiêu đề PR là tiêu đề commit trên `main` (tiếng Anh, Conventional Commits); nội
  dung PR là body commit (tiếng Việt): gạch đầu dòng từng thay đổi, vì sao, đã kiểm thế nào.
- Một PR một việc, ứng với một issue. Nên dưới khoảng 500 dòng mã thay đổi, không tính file sinh tự động, test, dữ
  liệu. Lớn hơn thì tách theo lớp: lõi, giao diện, dữ liệu.
- Thay đổi nhìn thấy được với người dùng: thêm một dòng vào `Unreleased` của `CHANGELOG.md` trong cùng PR.
- Đổi API: chạy `make openapi`, commit `openapi.json`, kiểu TypeScript sinh ra (`frontend/src/api/schema.ts`) và
  response mẫu của hỏi dữ liệu (`frontend/src/api/examples/ask.json`) trong cùng PR.
- Không commit khóa hay bí mật. `.env` trong `.gitignore`, mẫu ở `.env.example`. Ngoại lệ (nếu có) liệt kê đích danh
  ở đây kèm số issue gỡ.

## 5. Cách trợ lý AI làm việc trong dự án này

- Làm từng bước nhỏ, đúng phần được nhờ. Không viết cả tính năng lớn một lần, không sửa lan sang tệp khác.
- Chỉ commit, push, mở PR khi người dùng yêu cầu rõ. Không bao giờ tự merge.
- Không bỏ qua hook (`git push --no-verify`, `git commit --no-verify`). Hook báo lỗi thì sửa lỗi rồi push lại.
- Tự đánh giá kích thước trước khi đề xuất PR: `git diff --stat` so với `main`. Vượt giới hạn hoặc trộn nhiều lớp thì
  nói rõ và đề xuất cách tách.
- Viết xong thì giải thích ngắn gọn mã làm gì và vì sao, đủ để người dùng tự trình bày lại.
- Code sinh ra phải đọc được với người mới học Rust: crate phổ biến, module thẳng, chú thích ở chỗ ownership hay
  async không hiển nhiên.
- Không tự đặt ra quy tắc ngoài tài liệu. Không chắc thì nói không chắc. Không chép mã từ repo của người khác.
- Kết thúc mỗi phần việc: đề xuất tiêu đề PR tiếng Anh và nội dung PR tiếng Việt.

## 6. Trước khi push: tự kiểm

Sau khi clone, chạy `make hooks` một lần: mỗi lần `git push`, hook pre-push chạy mục đầu dưới đây, thêm mục thứ hai khi
nhánh đổi `frontend/`, `openapi.json` hoặc `Makefile` so với `main`, và chặn push nếu lỗi. Hook không nằm trong git
nên mỗi người tự cài; chạy lại khi `scripts/pre-push` thay đổi.

- [ ] `make check-fast` qua: fmt, clippy không cảnh báo, test.
- [ ] Đụng `frontend/`, `openapi.json` hoặc `Makefile` thì `make check-frontend` qua: lint, kiểu khớp `openapi.json`,
  kiểm kiểu, build.
- [ ] Chạy được bằng `make dev`, không lỗi trong log.
- [ ] Đụng compose, Dockerfile hoặc biến môi trường thì `docker compose up` vẫn chạy được.
- [ ] Các luồng chính ra đúng: nạp dataset, hỏi, xem bằng chứng, đổi role.
- [ ] Đổi cấu hình thì toàn hệ thống đổi theo, không chỗ nào ghi cứng.
- [ ] Người viết giải thích được từng dòng mình vừa thêm.
- [ ] Tiêu đề PR tiếng Anh theo Conventional Commits, nội dung PR tiếng Việt, dòng CHANGELOG nếu cần.
