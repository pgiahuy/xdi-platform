# Giấy phép

xdi-platform phát hành theo [Apache License 2.0](https://github.com/pgiahuy/xdi-platform/blob/main/LICENSE). Mỗi file mã nguồn ghi giấy phép ở dòng đầu
(`SPDX-License-Identifier`). Trang này liệt kê giấy phép của các thành phần bên thứ ba và đánh giá khả năng dùng chung
với Apache 2.0.

Danh sách đầy đủ từng crate và gói npm, đúng phiên bản đã khóa: [licenses-dependencies.md](licenses-dependencies.md),
sinh bằng `make licenses`. Chạy lại lệnh này mỗi khi đổi `Cargo.lock` hoặc `package-lock.json`.

## Tóm tắt

| Nhóm | Số lượng | Giấy phép | Đánh giá |
|---|---|---|---|
| Crate Rust (trong `Cargo.lock`, gồm cả crate chỉ dùng trên nền tảng khác Linux) | 224 | MIT, Apache-2.0, Unicode-3.0, Unlicense, Zlib, BSL-1.0, BSD | Cho phép, dùng chung được |
| Gói npm | 485 | Phần lớn MIT; ISC, BSD, Apache-2.0, BlueOak, 0BSD, CC0-1.0, CC-BY-4.0 (dữ liệu `caniuse-lite`) | Cho phép, dùng chung được |
| Gói npm `lightningcss` | 2 phiên bản, mỗi phiên bản kèm bản build cho từng nền tảng | MPL-2.0 | Xem bên dưới |
| Font Geist (`@fontsource-variable/geist`) | 1 | OFL-1.1 | Xem bên dưới |

Không có thành phần nào chỉ có giấy phép GPL, LGPL hay AGPL. Crate `r-efi` cho chọn `MIT OR Apache-2.0 OR
LGPL-2.1-or-later`; dự án dùng nó theo MIT, và nó chỉ được biên dịch cho nền tảng UEFI, không có trong binary Linux.

Công cụ chỉ chạy lúc build, không có trong image hay gói phát hành: image `rust:1.96-bookworm` và `node:24-bookworm-slim`
(giai đoạn build của Dockerfile), `cargo-chef` (MIT OR Apache-2.0), Vite, TypeScript, oxlint, Tailwind. Giấy phép của
các gói npm trong số này có trong danh sách đầy đủ.

## Trường hợp cần ghi rõ

**lightningcss, MPL-2.0.** Công cụ biên dịch CSS mà Tailwind dùng lúc build. Nó chạy trên máy build, không nằm trong
file giao diện đã build, không nằm trong image Docker (chỉ có ở giai đoạn build). Dự án không sửa mã của nó. MPL-2.0
chỉ ràng buộc với chính các file của lightningcss, nên dùng chung với Apache 2.0 được.

**Font Geist, SIL Open Font License 1.1.** File font có trong giao diện đã build. OFL cho phép đóng gói font kèm phần
mềm, với điều kiện giữ thông báo bản quyền và văn bản giấy phép, và không bán riêng font. Văn bản giấy phép đi kèm ở
`frontend/public/licenses/geist-font-OFL.txt`, có trong bản build và được phục vụ ở `/licenses/geist-font-OFL.txt`.

**Mã sinh từ shadcn/ui, MIT.** `frontend/src/components/ui/` và `frontend/src/lib/utils.ts` lấy từ
[shadcn/ui](https://github.com/shadcn-ui/ui), ghi `SPDX-License-Identifier: MIT` ở đầu file.
Thông báo bản quyền: Copyright (c) 2023 shadcn. Văn bản MIT ở cuối trang này.

## Phân phối lại

| Bản phân phối | Kèm theo |
|---|---|
| Giao diện đã build (`frontend/dist`, trong image) | `licenses.md`: giấy phép của mọi gói JavaScript đã đóng vào bundle, do Vite sinh lúc build, phục vụ ở `/licenses.md`; văn bản OFL của font |
| Image Docker | Như trên, cộng binary backend. Image nền `debian:bookworm-slim` mang giấy phép của từng gói Debian; phần mềm chỉ chạy trên đó, không sửa hay liên kết tĩnh |
| Gói binary trong Release | `LICENSE`, `LICENSES.md`, `licenses-dependencies.md` |
| Tarball mã nguồn | Toàn bộ repo, gồm các file trên |

## Dịch vụ và phần mềm chạy bên cạnh

Không được đóng gói vào phần mềm; người cài chọn và chịu điều khoản của từng thứ.

| Thành phần | Giấy phép | Ghi chú |
|---|---|---|
| PostgreSQL, image `pgvector/pgvector:pg17` | PostgreSQL License; pgvector cũng PostgreSQL License | Chạy thành container riêng trong compose |
| Ollama | MIT | Tùy chọn, `LLM_PROVIDER=ollama` |
| Mô hình mặc định cho Ollama, `qwen2.5-coder:7b` | Apache-2.0 | Tải về máy người dùng qua Ollama, không đi kèm repo |
| Gemini API | Dịch vụ của Google, theo điều khoản của Google | Tùy chọn, `LLM_PROVIDER=gemini`, cần khóa API. Mặc định không dùng |

Mặc định (`LLM_PROVIDER=mock`) phần mềm chạy hoàn toàn không cần dịch vụ hay khóa API đóng.

Dữ liệu mẫu: nguồn và giấy phép từng dataset ghi ở [data/README.md](https://github.com/pgiahuy/xdi-platform/blob/main/data/README.md).

## Văn bản MIT cho mã shadcn/ui

```
MIT License

Copyright (c) 2023 shadcn

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
