# Phát hành

Quy trình ra một phiên bản mới. Số phiên bản theo Semantic Versioning: `MAJOR.MINOR.PATCH`.

## Trước khi tag

1. `main` xanh: `make check` qua trên máy và CI qua trên GitHub.
2. Đổi số phiên bản ở hai chỗ, cùng một số: `backend/Cargo.toml`, `frontend/package.json`.
3. `CHANGELOG.md`: đổi mục `Unreleased` thành `[x.y.z] - YYYY-MM-DD`, tạo mục `Unreleased` trống mới ở trên.
4. Tài liệu trong `docs/` khớp với code: hướng dẫn cài đặt làm theo được trên máy sạch, ARCHITECTURE.md mục Trạng thái cập nhật.
5. Mỗi file mã nguồn chương trình mới có dòng giấy phép ở đầu (`SPDX-License-Identifier: Apache-2.0`).
6. Một PR tên `chore(release): vx.y.z` gồm các thay đổi trên. Merge xong mới tag.

## Tag và Release

```bash
git checkout main && git pull
git tag -a vx.y.z -m "vx.y.z"
git push origin vx.y.z
```

Không xóa rồi tag lại cùng một số: image sẽ bị ghi đè còn bước tạo Release lỗi vì Release đã có. Phát hành sai thì
ra bản vá `x.y.(z+1)`. Số phiên bản chỉ nhận dạng `x.y.z`, chưa có quy trình cho bản thử như `-rc1`.

Workflow `release.yml` chạy khi có tag `v*`:

- Kiểm số phiên bản ở tag khớp `backend/Cargo.toml` và `frontend/package.json` (`scripts/check-version.sh`).
- `make release-artifacts VERSION=x.y.z`: binary Linux `xdi-backend-x.y.z-linux-x86_64.tar.gz` (kèm LICENSE, README),
  mã nguồn `xdi-platform-x.y.z.tar.gz` giải nén ra thư mục `xdi-platform-x.y.z/`, ghi chú lấy từ mục `[x.y.z]` của
  CHANGELOG.md (`scripts/release-notes.sh`). Thiếu mục đó thì workflow dừng.
- Build image `ghcr.io/pgiahuy/xdi-platform`, đẩy với tag `x.y.z` và `latest`.
- Tạo GitHub Release đính kèm hai file trên.

Chạy thử trên máy trước khi tag: `make release-artifacts VERSION=x.y.z`, kết quả ở thư mục `release/`. PR đổi quy
trình phát hành (workflow, `scripts/`, Makefile) tự chạy workflow ở chế độ thử: build đủ, không phát hành.

## Sau khi phát hành

1. Kiểm tra trên máy sạch: `docker compose up` với image vừa đẩy, không build local.
2. Tải `xdi-backend-x.y.z-linux-x86_64.tar.gz` từ Release, giải nén, chạy binary với `DATABASE_URL` trỏ tới một
   Postgres, `/api/health` trả ok. Không cần Rust. Tarball mã nguồn thì cần Rust: `make install PREFIX=/tmp/xdi`.
3. Đăng một bài ở Discussions mục Announcements: phiên bản, ba thay đổi đáng chú ý, link Release.
4. Trang web (`docs/`) tự cập nhật theo `main`.

## Phiên bản vá

Lỗi nghiêm trọng trên bản đã phát hành: nhánh từ tag, sửa, PR vào `main`, rồi tag `x.y.(z+1)` theo đúng quy trình
trên. Không sửa thẳng trên tag cũ.
