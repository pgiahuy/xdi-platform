# Phát hành

Quy trình ra một phiên bản mới. Số phiên bản theo Semantic Versioning: `MAJOR.MINOR.PATCH`.

## Trước khi tag

1. `main` xanh: `make check` qua trên máy và CI qua trên GitHub.
2. Đổi số phiên bản ở ba chỗ, cùng một số: `backend/Cargo.toml`, `frontend/package.json`, `docker-compose.yml`
   (tag image).
3. `CHANGELOG.md`: đổi mục `Unreleased` thành `[x.y.z] - YYYY-MM-DD`, tạo mục `Unreleased` trống mới ở trên.
4. Tài liệu trong `docs/` khớp với code: INSTALL.md chạy theo được, ARCHITECTURE.md mục Trạng thái cập nhật.
5. Mỗi file mã nguồn chương trình mới có dòng giấy phép ở đầu (`SPDX-License-Identifier: Apache-2.0`).
6. Một PR tên `chore(release): vx.y.z` gồm các thay đổi trên. Merge xong mới tag.

## Tag và Release

```bash
git checkout main && git pull
git tag -a vx.y.z -m "vx.y.z"
git push origin vx.y.z
```

Workflow release chạy khi có tag `v*`:

- Build image backend và frontend, đẩy lên GitHub Container Registry với tag `x.y.z` và `latest`.
- Tạo GitHub Release với ghi chú lấy từ mục tương ứng trong CHANGELOG.md.
- Đính kèm `xdi-platform-x.y.z.tar.gz` (giải nén ra thư mục `xdi-platform-x.y.z/`) và binary backend cho Linux.

## Sau khi phát hành

1. Kiểm tra trên máy sạch: `docker compose up` với image vừa đẩy, không build local.
2. Kiểm tra `make install` từ tarball: giải nén, `make`, `make install PREFIX=/tmp/xdi`, binary chạy được.
3. Đăng một bài ở Discussions mục Announcements: phiên bản, ba thay đổi đáng chú ý, link Release.
4. Trang web (`docs/`) tự cập nhật theo `main`.

## Phiên bản vá

Lỗi nghiêm trọng trên bản đã phát hành: nhánh từ tag, sửa, PR vào `main`, rồi tag `x.y.(z+1)` theo đúng quy trình
trên. Không sửa thẳng trên tag cũ.
